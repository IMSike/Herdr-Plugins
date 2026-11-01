//! Herdr sidebar plugin: a workspace → agents tree in the Agents panel, with
//! CPU, memory and shell counts, as sidebar tokens.
//!
//! - `install` (startup hook and action) copies this binary into the plugin
//!   state folder, so a `ui.tab_bar_right` command entry has a fixed path,
//!   and sorts the Agents panel by the tree order.
//! - `tick` (run by that entry every few seconds) measures every pane's
//!   process tree, reports the tokens and leaves the totals in `totals.json`
//!   for the `Tools` pane; it prints them for the tab bar only when the
//!   settings ask (an empty output hides the entry).
//! - `event` (agent and workspace hooks) places the agents in the tree
//!   right away; their figures wait for the next `tick`.
//! - `show` prints the tokens without reporting them.

mod machine;
mod seq;
mod settings;
mod snapshot;
mod tokens;
mod tree;
mod usage;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use sysinfo::{MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, System};

use herdr_api::sidebar::{GIB, Level, Totals, TotalsLevels};
use herdr_api::{request, request_field, unix_millis};
use settings::Settings;
use snapshot::{Pane, Snapshot};
use usage::{Proc, Sample, Shell, Table, Usage, cpu_share};

const SOURCE: &str = herdr_api::sidebar::ID;
const SAMPLE_FILE: &str = "sample.json";
/// The last report's `seq`: reports go on in order after the clock is set back.
const SEQ_FILE: &str = "seq";
/// Held by the install copying the binary: another one at the same time
/// leaves the copy to it.
const INSTALL_LOCK: &str = "install.lock";
/// A file of the state folder left this long by a run killed midway is removed.
const STALE_AFTER: Duration = Duration::from_secs(60);
/// The plugin's config folder, as `install` found it: the tab bar entry
/// runs without the plugin's variables.
const CONFIG_DIR_FILE: &str = "config-dir";
/// A run still asking Herdr after this long stops: a slow Herdr is not
/// sent more requests, and the tab bar entry (`timeout_seconds = 5`) is not
/// held up.
const RUN_DEADLINE: Duration = Duration::from_secs(4);

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        Some("install") => install(),
        Some("tick") => tick(Mode::Tick),
        Some("event") => tick(Mode::Event),
        Some("show") => tick(Mode::Show),
        _ => Err("usage : herdr-sidebar install|tick|event|show".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("herdr-sidebar : {e}");
            ExitCode::FAILURE
        }
    }
}

fn install() -> Result<(), String> {
    // The tab bar entry runs this copy: its path is the one to configure.
    println!("{}", copy_binary()?.display());
    if let Ok(config) = herdr_api::var(herdr_api::env::CONFIG_DIR) {
        let file = herdr_api::state_dir()?.join(CONFIG_DIR_FILE);
        herdr_api::write_atomic(&file, config.as_bytes()).map_err(|e| e.to_string())?;
    }
    // Agents follow the `order` token: each workspace's agents after its header.
    request(
        "agent.view.set",
        json!({
            "source": format!("plugin:{SOURCE}"),
            "label": "arbre",
            "sort": [{ "field": { "token": "order" }, "order": "asc" }],
        }),
    )?;
    Ok(())
}

/// Copies the running binary into the state folder, unless it is already
/// there, and returns the copy's path.
fn copy_binary() -> Result<PathBuf, String> {
    let dir = herdr_api::state_dir()?;
    let source = std::env::current_exe().map_err(|e| e.to_string())?;
    let target = dir.join(source.file_name().ok_or("binaire sans nom")?);
    // The startup hook and the action can run at once: the install that
    // takes the lock copies, the other one leaves the copy to it.
    if !take_lock(&dir).map_err(|e| format!("verrou impossible : {e}"))? {
        return Ok(target);
    }
    let replaced = replace(&dir, &source, &target);
    let _ = fs::remove_file(dir.join(INSTALL_LOCK));
    replaced.map(|()| target)
}

/// Takes the install lock; `false` when another install holds it. A lock
/// older than `STALE_AFTER` was left by an install killed midway: it is
/// taken over by renaming it, which only one install can do.
fn take_lock(dir: &Path) -> io::Result<bool> {
    let lock = dir.join(INSTALL_LOCK);
    for _ in 0..2 {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
        {
            Ok(_) => return Ok(true),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let stale =
                    fs::metadata(&lock).is_ok_and(|m| herdr_api::is_older_than(&m, STALE_AFTER));
                let taken = lock.with_extension(format!("{}.stale", std::process::id()));
                if !stale || fs::rename(&lock, &taken).is_err() {
                    return Ok(false);
                }
                let _ = fs::remove_file(&taken);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(false)
}

/// Puts a copy of `source` in place of `target`, unless it is the same
/// already; the caller holds the lock.
fn replace(dir: &Path, source: &Path, target: &Path) -> Result<(), String> {
    // What a run killed midway left, and the old copies no longer running
    // (one still running cannot be removed on Windows: a later install will).
    herdr_api::remove_stale(dir, STALE_AFTER, |name| name.ends_with(".partial"));
    herdr_api::remove_stale(dir, Duration::ZERO, |name| name.ends_with(".old"));
    if is_same_file_content(source, target).map_err(|e| e.to_string())? {
        return Ok(());
    }
    // Named after this process: an install that took over a stale lock
    // while another one still copied does not share its file.
    let partial = target.with_extension(format!("{}.partial", std::process::id()));
    fs::copy(source, &partial).map_err(|e| e.to_string())?;
    // A running binary cannot be overwritten on Windows, but can be renamed.
    let old = target.with_extension(format!("{}.old", unix_millis()));
    let moved_aside = fs::rename(target, &old).is_ok();
    if let Err(e) = fs::rename(&partial, target) {
        // An antivirus may still hold the new copy: put the old one back.
        let _ = fs::remove_file(&partial);
        if moved_aside {
            let _ = fs::rename(&old, target);
        }
        return Err(format!("copie impossible : {e}"));
    }
    Ok(())
}

/// Whether `target` holds what `source` does; the sizes first, which spare
/// reading both when they differ.
fn is_same_file_content(source: &Path, target: &Path) -> io::Result<bool> {
    let Ok(copy) = fs::metadata(target) else {
        return Ok(false);
    };
    if copy.len() != fs::metadata(source)?.len() {
        return Ok(false);
    }
    Ok(fs::read(target).ok() == Some(fs::read(source)?))
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Run by the tab bar entry: report, save the sample and the totals.
    Tick,
    /// Run by a hook (agent, workspace): place the agents only.
    Event,
    /// Print the tokens instead of reporting them, for a manual check.
    Show,
}

fn tick(mode: Mode) -> Result<(), String> {
    let started = Instant::now();
    let late = || mode != Mode::Show && started.elapsed() >= RUN_DEADLINE;
    let state_dir = match herdr_api::state_dir() {
        Ok(dir) => dir,
        // The tab bar entry gets no plugin context: it runs the copy that
        // `install` put in the state folder.
        Err(_) => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .map(Path::to_path_buf)
            .ok_or("dossier du binaire inconnu")?,
    };
    let sample_path = state_dir.join(SAMPLE_FILE);
    let settings = Settings::load(config_dir(&state_dir).as_deref()).unwrap_or_else(|e| {
        eprintln!("herdr-sidebar : {e}");
        Settings::default()
    });
    let numbers = settings.numbers();
    let ttl_ms = settings.ttl_seconds.saturating_mul(1000);
    // The processes first, the slow part: the session read after them, and
    // the reports, then describe the same moment. An event only moves the
    // tree: the figures stay as the last run measured them, which spares a
    // reading of every process on each agent change.
    let measured = match mode {
        Mode::Event => None,
        Mode::Tick => Some((None, read_system()?, unix_millis())),
        // No sample of a run 5 seconds earlier: take one a second earlier.
        Mode::Show => {
            let first = read_system()?.sample(unix_millis());
            std::thread::sleep(Duration::from_secs(1));
            Some((Some(first), read_system()?, unix_millis()))
        }
    };
    // Lets Herdr drop a report older than one already applied.
    let seq = match mode {
        Mode::Show => 0,
        _ => next_seq(&state_dir.join(SEQ_FILE)),
    };
    // Past the deadline, the reports left are not sent: the run fails at
    // the end, and the next one sends them all again.
    let send = |pane_id: &str, tokens: Map<String, Value>| match mode {
        Mode::Show => println!("{pane_id} : {}", Value::Object(tokens)),
        _ if late() => {}
        _ => report(pane_id, tokens, seq, ttl_ms),
    };
    let too_slow = || Err("Herdr trop lent : passage interrompu".to_string());

    let snapshot: Snapshot = request_field("session.snapshot", json!({}), "snapshot")?;
    let agents = snapshot.agents();
    let blocks = tree::blocks(&snapshot.spaces(), &agents);

    let Some((first, system, now)) = measured else {
        for block in blocks {
            let mut tokens = tokens::layout(&block);
            if !block.workspace_figures {
                tokens.insert("ws_stats".into(), Value::Null);
            }
            // Most events move one agent at most: the others are not sent again.
            let shown = snapshot.panes.iter().find(|p| p.pane_id == block.pane_id);
            if shown.is_none_or(|pane| !tokens::unchanged(&tokens, &pane.tokens)) {
                send(&block.pane_id, tokens);
            }
        }
        return if late() { too_slow() } else { Ok(()) };
    };

    let mut saved = read_sample(&sample_path);
    let known_shells = saved
        .as_mut()
        .map(|s| std::mem::take(&mut s.shells))
        .unwrap_or_default();
    let previous = first.or_else(|| saved.filter(|s| s.is_usable(now)));
    let empty = Sample::default();
    let table = Table::new(
        &system.processes,
        previous.as_ref().unwrap_or(&empty),
        &settings.shells,
    );
    // The time of all cores together since the previous run, from the
    // system's CPU times, which a change of the wall clock does not move;
    // the wall clock where the system gives none.
    let all_cores_ms = previous.as_ref().map(|p| {
        p.machine
            .zip(system.cpu_times)
            .and_then(|(before, after)| machine::all_cores_ms_between(before, after))
            .unwrap_or_else(|| {
                let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
                now.saturating_sub(p.at_ms) * cores as u64
            })
    });
    let cpu = |u: &Usage| all_cores_ms.map(|ms| cpu_share(u.cpu_ms, ms));
    let look = tokens::Look {
        numbers,
        levels: settings
            .colors
            .then_some((settings.cpu_levels, settings.memory_levels)),
    };

    let mut shells: HashMap<String, Shell> = HashMap::new();
    let mut sums: Vec<(String, Usage)> = Vec::new();
    let mut total = Usage::default();
    let mut usages: Vec<(&str, Usage)> = Vec::new();
    for pane in &snapshot.panes {
        let known = known_shells.get(&pane.pane_id);
        let Some(shell) = shell_of(pane, known, &table, !late()) else {
            continue;
        };
        let usage = table.tree(shell.pid);
        shells.insert(pane.pane_id.clone(), shell);
        total += usage;
        match sums.iter_mut().find(|(id, _)| *id == pane.workspace_id) {
            Some((_, sum)) => *sum += usage,
            None => sums.push((pane.workspace_id.clone(), usage)),
        }
        usages.push((&pane.pane_id, usage));
    }

    for block in blocks {
        let workspace = block.workspace_figures.then(|| {
            sums.iter()
                .find(|(id, _)| *id == block.workspace_id)
                .map_or_else(String::new, |(_, u)| {
                    tokens::figures(cpu(u), u, false, &look)
                })
        });
        // An agent whose processes could not be read keeps its place in the
        // tree, with the figures of the last run.
        let usage = usages
            .iter()
            .find(|(id, _)| *id == block.pane_id)
            .map(|(_, u)| (cpu(u), u));
        let tokens = tokens::agent(&block, usage, workspace, &look);
        send(&block.pane_id, tokens);
    }

    let total_cpu = cpu(&total);
    let machine_cpu = previous
        .as_ref()
        .and_then(|p| machine::cpu_percent(p.machine?, system.cpu_times?));
    let totals = Totals {
        at_ms: now,
        cpu: total_cpu,
        memory: total.memory,
        machine_cpu,
        machine_memory: system.used_memory,
        levels: settings.colors.then(|| TotalsLevels {
            cpu: total_cpu.map(|c| Level::of(c, &settings.cpu_levels)),
            memory: Level::of(total.memory as f64 / GIB, &settings.memory_levels),
            machine_cpu: machine_cpu.map(|c| Level::of(c, &settings.machine_cpu_levels)),
            machine_memory: Level::of(
                system.used_memory as f64 * 100.0 / system.total_memory.max(1) as f64,
                &settings.machine_memory_levels,
            ),
        }),
    };
    if mode == Mode::Show || settings.tab_bar {
        println!("{}", totals.line(&numbers));
    }
    if mode == Mode::Tick {
        let mut sample = system.sample(now);
        sample.shells = shells;
        write_sample(&sample_path, &sample);
        if let Err(e) = totals.save(&state_dir) {
            eprintln!("herdr-sidebar : totaux non enregistrés : {e}");
        }
    }
    if late() {
        return too_slow();
    }
    Ok(())
}

/// The pane's shell: the one the last run found, while its terminal and its
/// process are the same; otherwise Herdr is asked, when `may_ask`.
fn shell_of(pane: &Pane, known: Option<&Shell>, table: &Table, may_ask: bool) -> Option<Shell> {
    if let Some(known) = known
        && known.terminal_id == pane.terminal_id
        && let Some(start_time) = table.start_time(known.pid)
        && table.same_start(start_time, known.start_time)
    {
        // As read now: the next run compares with this one.
        return Some(Shell {
            start_time,
            ..known.clone()
        });
    }
    if !may_ask {
        return None;
    }
    let pid = herdr_api::process_info(&pane.pane_id).ok()?.shell_pid?;
    Some(Shell {
        terminal_id: pane.terminal_id.clone(),
        pid,
        start_time: table.start_time(pid)?,
    })
}

/// A failed report only loses one refresh: it is logged, never fatal.
fn report(pane_id: &str, tokens: Map<String, Value>, seq: u64, ttl_ms: u64) {
    let params = json!({
        "pane_id": pane_id,
        "source": SOURCE,
        "tokens": tokens,
        "ttl_ms": ttl_ms,
        "seq": seq,
    });
    if let Err(e) = request("pane.report_metadata", params) {
        eprintln!("herdr-sidebar : {e}");
    }
}

/// What one run reads from the system.
struct Reading {
    processes: Vec<Proc>,
    /// Memory in use on the whole machine.
    used_memory: u64,
    total_memory: u64,
    cpu_times: Option<machine::CpuTimes>,
}

impl Reading {
    fn sample(&self, at_ms: u64) -> Sample {
        let mut sample = Sample::of(at_ms, &self.processes);
        sample.machine = self.cpu_times;
        sample
    }
}

fn read_system() -> Result<Reading, String> {
    let mut system = System::new().map_err(|e| format!("processus illisibles : {e:?}"))?;
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        // On Linux, threads would otherwise be listed as processes of their own.
        ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .without_tasks(),
    );
    system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    let processes = system
        .processes()
        .values()
        .map(|p| Proc {
            pid: p.pid().as_u32(),
            parent: p.parent().map(|pid| pid.as_u32()),
            start_time: p.start_time(),
            cpu_ms: p.accumulated_cpu_time(),
            memory: p.memory(),
            name: p.name().to_string_lossy().into_owned(),
        })
        .collect();
    Ok(Reading {
        processes,
        used_memory: system.used_memory(),
        total_memory: system.total_memory(),
        cpu_times: machine::cpu_times(),
    })
}

/// The `seq` of this run's reports, kept for the next run (see `seq::next`).
fn next_seq(path: &Path) -> u64 {
    let last = fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse().ok());
    let seq = seq::next(unix_millis(), last, std::process::id());
    let _ = herdr_api::write_atomic(path, seq.to_string().as_bytes());
    seq
}

/// The plugin's config folder: from Herdr in a hook or an action, from
/// what `install` kept in the tab bar entry.
fn config_dir(state_dir: &Path) -> Option<PathBuf> {
    std::env::var_os(herdr_api::env::CONFIG_DIR)
        .map(PathBuf::from)
        .or_else(|| {
            let text = fs::read_to_string(state_dir.join(CONFIG_DIR_FILE)).ok()?;
            Some(PathBuf::from(text.trim()))
        })
}

fn read_sample(path: &Path) -> Option<Sample> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn write_sample(path: &Path, sample: &Sample) {
    let written = serde_json::to_vec(sample)
        .map_err(|e| e.to_string())
        .and_then(|bytes| herdr_api::write_atomic(path, &bytes).map_err(|e| e.to_string()));
    if let Err(e) = written {
        eprintln!("herdr-sidebar : échantillon non enregistré : {e}");
    }
}
