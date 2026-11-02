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

use herdr_api::sidebar::{Settings, Totals};
use herdr_api::{Lock, request, request_field, unix_millis};
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
    let lock =
        Lock::take(&dir.join(INSTALL_LOCK)).map_err(|e| format!("verrou impossible : {e}"))?;
    if lock.is_none() {
        return Ok(target);
    }
    replace(&dir, &source, &target).map(|()| target)
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
    // Named after this process: what an install killed midway left (removed
    // above once stale) is not mixed with this copy.
    let partial = target.with_extension(format!("{}.partial", std::process::id()));
    fs::copy(source, &partial).map_err(|e| e.to_string())?;
    // A running binary cannot be overwritten on Windows, but can be renamed.
    // Elsewhere the rename below replaces it, the running one included.
    let old = target.with_extension(format!("{}.old", unix_millis()));
    let moved_aside = cfg!(windows) && fs::rename(target, &old).is_ok();
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
    if mode != Mode::Show {
        herdr_api::set_deadline(started + RUN_DEADLINE);
    }
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
    let snapshot: Snapshot = request_field("session.snapshot", json!({}), "snapshot")?;
    // Lets Herdr drop a report older than one already applied. Taken once
    // the session is read: a run that read it later reports with a later
    // `seq`, and its tree is not dropped for an earlier run's.
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

    let mut saved: Option<Sample> = herdr_api::read_json(&sample_path);
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

    let mut shells: HashMap<String, Shell> = HashMap::new();
    let measures = Measures::of(&snapshot.panes, |pane| {
        let known = known_shells.get(&pane.pane_id);
        let shell = shell_of(pane, known, &table, !late())?;
        let usage = table.tree(shell.pid);
        shells.insert(pane.pane_id.clone(), shell);
        Some(usage)
    });

    for block in blocks {
        let workspace = block.workspace_figures.then(|| {
            measures
                .workspace(&block.workspace_id)
                .map_or_else(String::new, |u| tokens::figures(cpu(u), u, false, &numbers))
        });
        // An agent whose processes could not be read keeps its place in the
        // tree, with the figures of the last run.
        let usage = measures.pane(&block.pane_id).map(|u| (cpu(u), u));
        let tokens = tokens::agent(&block, usage, workspace, &numbers);
        send(&block.pane_id, tokens);
    }

    let total = measures.total;
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
        machine_total: system.total_memory,
        colors: settings.colors,
        thresholds: settings.thresholds(),
    };
    if mode == Mode::Show || settings.tab_bar {
        println!("{}", totals.line(&numbers));
    }
    if mode == Mode::Tick {
        let mut sample = system.sample(now);
        sample.shells = shells;
        if let Err(e) = herdr_api::write_json(&sample_path, &sample) {
            eprintln!("herdr-sidebar : échantillon non enregistré : {e}");
        }
        if let Err(e) = totals.save(&state_dir) {
            eprintln!("herdr-sidebar : totaux non enregistrés : {e}");
        }
    }
    if late() {
        return too_slow();
    }
    Ok(())
}

/// What a run measured: each pane's usage, each workspace's, and the total
/// of all of them (Herdr's).
#[derive(Debug, Default)]
struct Measures<'a> {
    panes: Vec<(&'a str, Usage)>,
    workspaces: Vec<(&'a str, Usage)>,
    total: Usage,
}

impl<'a> Measures<'a> {
    /// The panes' usage, as `usage_of` reads it; a pane it cannot read is
    /// left out.
    fn of(panes: &'a [Pane], mut usage_of: impl FnMut(&Pane) -> Option<Usage>) -> Self {
        let mut measures = Self::default();
        for pane in panes {
            let Some(usage) = usage_of(pane) else {
                continue;
            };
            measures.total += usage;
            match measures
                .workspaces
                .iter_mut()
                .find(|(id, _)| *id == pane.workspace_id)
            {
                Some((_, sum)) => *sum += usage,
                None => measures.workspaces.push((&pane.workspace_id, usage)),
            }
            measures.panes.push((&pane.pane_id, usage));
        }
        measures
    }

    fn pane(&self, id: &str) -> Option<&Usage> {
        self.panes.iter().find(|(p, _)| *p == id).map(|(_, u)| u)
    }

    fn workspace(&self, id: &str) -> Option<&Usage> {
        self.workspaces
            .iter()
            .find(|(w, _)| *w == id)
            .map(|(_, u)| u)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(pane_id: &str, workspace_id: &str) -> Pane {
        serde_json::from_value(json!({
            "pane_id": pane_id,
            "workspace_id": workspace_id,
            "tab_id": "t",
        }))
        .unwrap()
    }

    #[test]
    fn sums_the_panes_by_workspace() {
        let panes = [
            pane("p1", "w1"),
            pane("p2", "w1"),
            pane("p3", "w2"),
            pane("p4", "w2"),
        ];
        let usage = |memory| Usage {
            memory,
            ..Usage::default()
        };
        // p4's processes cannot be read: it is left out.
        let measures = Measures::of(&panes, |p| match p.pane_id.as_str() {
            "p1" => Some(usage(1)),
            "p2" => Some(usage(2)),
            "p3" => Some(usage(4)),
            _ => None,
        });
        assert_eq!(measures.pane("p2"), Some(&usage(2)));
        assert_eq!(measures.pane("p4"), None);
        assert_eq!(measures.workspace("w1"), Some(&usage(3)));
        assert_eq!(measures.workspace("w2"), Some(&usage(4)));
        assert_eq!(measures.total, usage(7));
    }

    #[test]
    fn tells_a_copy_from_another_file() {
        let dir = std::env::temp_dir().join(format!("herdr-sidebar-copy-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a"), dir.join("b"));
        fs::write(&a, "binary").unwrap();
        assert!(!is_same_file_content(&a, &b).unwrap());
        fs::write(&b, "binary").unwrap();
        assert!(is_same_file_content(&a, &b).unwrap());
        fs::write(&b, "binarz").unwrap();
        assert!(!is_same_file_content(&a, &b).unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }
}
