//! Herdr sidebar plugin: a workspace → agents tree in the Agents panel, with
//! CPU, memory and shell counts, as sidebar tokens.
//!
//! - `install` (startup hook and action) copies this binary into the plugin
//!   state folder, so a `ui.tab_bar_right` command entry has a fixed path,
//!   and sorts the Agents panel by the tree order.
//! - `tick` (run by that entry every few seconds) measures every pane's
//!   process tree, reports the tokens and prints the totals for the tab bar.
//! - `event` (agent and workspace hooks) places the agents in the tree
//!   right away; their figures wait for the next `tick`.
//! - `show` prints the tokens without reporting them.

mod machine;
mod snapshot;
mod tokens;
mod tree;
mod usage;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{Map, Value, json};
use sysinfo::{MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, System};

use herdr_api::{env, request, request_field, unix_millis};
use snapshot::{Pane, Snapshot};
use usage::{Proc, Sample, Shell, Table, Usage, cpu_share, format_cpu};

const SOURCE: &str = "imsike.sidebar";
const SAMPLE_FILE: &str = "sample.json";
/// Held by the install copying the binary: another one at the same time
/// leaves the copy to it.
const INSTALL_LOCK: &str = "install.lock";
/// A file of the state folder left this long by a run killed midway is removed.
const STALE_AFTER: Duration = Duration::from_secs(60);
/// Tokens vanish when no run refreshes them (Herdr closed the tab bar
/// entry, or the plugin was removed).
const TOKEN_TTL_MS: u64 = 20_000;

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
    // Agents follow the `order` token: each workspace's agents after its header.
    let plugin = std::env::var(env::PLUGIN_ID).map_err(|_| format!("{} absent", env::PLUGIN_ID))?;
    request(
        "agent.view.set",
        json!({
            "source": format!("plugin:{plugin}"),
            "label": "arbre",
            "sort": [{ "field": { "token": "order" }, "order": "asc" }],
        }),
    )?;
    Ok(())
}

/// Copies the running binary into the state folder, unless it is already
/// there, and returns the copy's path.
fn copy_binary() -> Result<PathBuf, String> {
    let dir = herdr_api::state_dir().ok_or(format!("{} absent", env::STATE_DIR))?;
    let source = std::env::current_exe().map_err(|e| e.to_string())?;
    let target = dir.join(source.file_name().ok_or("binaire sans nom")?);
    if fs::read(&target).ok() == Some(fs::read(&source).map_err(|e| e.to_string())?) {
        return Ok(target);
    }
    // The startup hook and the action can run at once: the install that
    // takes the lock copies, the other one leaves the copy to it.
    herdr_api::remove_stale(&dir, STALE_AFTER, |name| name == INSTALL_LOCK);
    let lock = dir.join(INSTALL_LOCK);
    if fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .is_err()
    {
        return Ok(target);
    }
    let replaced = replace(&dir, &source, &target);
    let _ = fs::remove_file(&lock);
    replaced.map(|()| target)
}

/// Puts a copy of `source` in place of `target`; the caller holds the lock.
fn replace(dir: &Path, source: &Path, target: &Path) -> Result<(), String> {
    // What a run killed midway left, and the old copies no longer running
    // (one still running cannot be removed on Windows: a later install will).
    herdr_api::remove_stale(dir, STALE_AFTER, |name| name.ends_with(".partial"));
    herdr_api::remove_stale(dir, Duration::ZERO, |name| name.ends_with(".old"));
    let partial = target.with_extension("partial");
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

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Run by the tab bar entry: report, save the sample, print the totals.
    Tick,
    /// Run by a hook (agent, workspace): place the agents only.
    Event,
    /// Print the tokens instead of reporting them, for a manual check.
    Show,
}

fn tick(mode: Mode) -> Result<(), String> {
    let state_dir = match herdr_api::state_dir() {
        Some(dir) => dir,
        // The tab bar entry gets no plugin context: it runs the copy that
        // `install` put in the state folder.
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .map(Path::to_path_buf)
            .ok_or("dossier du binaire inconnu")?,
    };
    let sample_path = state_dir.join(SAMPLE_FILE);
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
    let seq = unix_millis();
    let send = |pane_id: &str, tokens: Map<String, Value>| match mode {
        Mode::Show => println!("{pane_id} : {}", Value::Object(tokens)),
        _ => report(pane_id, tokens, seq),
    };

    let snapshot: Snapshot = request_field("session.snapshot", json!({}), "snapshot")?;
    let agents = snapshot.agents();
    let blocks = tree::blocks(&snapshot.spaces(), &agents);
    let workspace_of = |pane_id: &str| {
        agents
            .iter()
            .find(|a| a.pane_id == pane_id)
            .map_or("", |a| a.workspace_id.as_str())
    };
    // A lone agent's figures already tell its workspace's.
    let several = |workspace_id: &str| {
        agents
            .iter()
            .filter(|a| a.workspace_id == workspace_id)
            .nth(1)
            .is_some()
    };

    let Some((first, system, now)) = measured else {
        for block in blocks {
            let mut tokens = tokens::layout(&block);
            if block.header.is_none() || !several(workspace_of(&block.pane_id)) {
                tokens.insert("ws_stats".into(), Value::Null);
            }
            // Most events move one agent at most: the others are not sent again.
            let shown = snapshot.panes.iter().find(|p| p.pane_id == block.pane_id);
            if shown.is_none_or(|pane| !tokens::unchanged(&tokens, &pane.tokens)) {
                send(&block.pane_id, tokens);
            }
        }
        return Ok(());
    };

    let saved = read_sample(&sample_path);
    let known_shells = saved.as_ref().map(|s| s.shells.clone()).unwrap_or_default();
    let previous = first.or_else(|| saved.filter(|s| s.is_usable(now)));
    let elapsed = previous.as_ref().map(|s| now.saturating_sub(s.at_ms));
    let empty = Sample::default();
    let table = Table::new(&system.processes, previous.as_ref().unwrap_or(&empty));
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let cpu = |u: &Usage| elapsed.map(|e| format_cpu(cpu_share(u.cpu_ms, e, cores)));

    let mut shells: HashMap<String, Shell> = HashMap::new();
    let mut sums: Vec<(String, Usage)> = Vec::new();
    let mut total = Usage::default();
    let mut usages: Vec<(&str, Usage)> = Vec::new();
    for pane in &snapshot.panes {
        let Some(shell) = shell_of(pane, known_shells.get(&pane.pane_id), &table) else {
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
        let workspace_id = workspace_of(&block.pane_id);
        let workspace = (block.header.is_some() && several(workspace_id)).then(|| {
            sums.iter()
                .find(|(id, _)| id == workspace_id)
                .map_or_else(String::new, |(_, u)| tokens::figures(cpu(u), u, false))
        });
        // An agent whose processes could not be read keeps its place in the
        // tree, with the figures of the last run.
        let usage = usages
            .iter()
            .find(|(id, _)| *id == block.pane_id)
            .map(|(_, u)| (cpu(u), u));
        let tokens = tokens::agent(&block, usage, workspace);
        send(&block.pane_id, tokens);
    }

    let machine_cpu = previous
        .as_ref()
        .and_then(|p| machine::cpu_percent(p.machine?, system.cpu_times?))
        .map(format_cpu);
    println!(
        "{}",
        tokens::tab_bar(cpu(&total), &total, machine_cpu, system.used_memory)
    );
    if mode == Mode::Tick {
        let mut sample = system.sample(now);
        sample.shells = shells;
        write_sample(&sample_path, &sample);
    }
    Ok(())
}

/// The pane's shell: the one the last run found, while its terminal and its
/// process are the same; otherwise Herdr is asked.
fn shell_of(pane: &Pane, known: Option<&Shell>, table: &Table) -> Option<Shell> {
    if let Some(known) = known
        && known.terminal_id == pane.terminal_id
        && table.start_time(known.pid) == Some(known.start_time)
    {
        return Some(known.clone());
    }
    let pid: u32 = request_field::<Value>(
        "pane.process_info",
        json!({ "pane_id": pane.pane_id }),
        "process_info",
    )
    .ok()?
    .get("shell_pid")?
    .as_u64()
    .and_then(|pid| u32::try_from(pid).ok())?;
    Some(Shell {
        terminal_id: pane.terminal_id.clone(),
        pid,
        start_time: table.start_time(pid)?,
    })
}

/// A failed report only loses one refresh: it is logged, never fatal.
fn report(pane_id: &str, tokens: Map<String, Value>, seq: u64) {
    let params = json!({
        "pane_id": pane_id,
        "source": SOURCE,
        "tokens": tokens,
        "ttl_ms": TOKEN_TTL_MS,
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
        cpu_times: machine::cpu_times(),
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
