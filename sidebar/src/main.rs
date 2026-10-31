//! Herdr sidebar plugin: a workspace → agents tree in the Agents panel, with
//! CPU, memory and shell counts, as sidebar tokens.
//!
//! - `install` (startup hook and action) copies this binary into the plugin
//!   state folder, so a `ui.tab_bar_right` command entry has a fixed path,
//!   and sorts the Agents panel by the tree order.
//! - `tick` (run by that entry every few seconds) measures every pane's
//!   process tree, reports the tokens and prints the total for the tab bar.
//! - `event` (agent, focus and workspace hooks) reports the same tokens right away.
//! - `show` prints the tokens without reporting them.

mod herdr;
mod tree;
mod usage;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{Map, Value, json};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use tree::Space;
use usage::{Proc, Sample, Table, Usage, cpu_percent, format_cpu, format_memory};

const SOURCE: &str = "imsike.sidebar";
const PLUGIN_ID_ENV: &str = "HERDR_PLUGIN_ID";
const STATE_DIR_ENV: &str = "HERDR_PLUGIN_STATE_DIR";
const SAMPLE_FILE: &str = "sample.json";
/// Tokens vanish when no run refreshes them (Herdr closed the tab bar
/// entry, or the plugin was removed).
const TOKEN_TTL_MS: u64 = 20_000;
/// A previous sample older than this gives no meaningful CPU average.
const MAX_SAMPLE_AGE_MS: u64 = 120_000;

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        Some("install") => install(),
        Some("tick") => tick(Mode::Tick),
        Some("event") => tick(Mode::Event),
        Some("show") => tick(Mode::Show),
        _ => Err("usage: herdr-sidebar install|tick|event|show".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("herdr-sidebar: {e}");
            ExitCode::FAILURE
        }
    }
}

fn install() -> Result<(), String> {
    copy_binary()?;
    // Agents follow the `order` token: each workspace's agents after its header.
    let plugin = std::env::var(PLUGIN_ID_ENV).map_err(|_| format!("{PLUGIN_ID_ENV} absent"))?;
    herdr::request(
        "agent.view.set",
        json!({
            "source": format!("plugin:{plugin}"),
            "label": "arbre",
            "sort": [{ "field": { "token": "order" }, "order": "asc" }],
        }),
    )?;
    Ok(())
}

/// Copies the running binary into the state folder, unless it is already there.
fn copy_binary() -> Result<(), String> {
    let dir = PathBuf::from(std::env::var_os(STATE_DIR_ENV).ok_or("état du plugin inconnu")?);
    let source = std::env::current_exe().map_err(|e| e.to_string())?;
    let target = dir.join(source.file_name().ok_or("binaire sans nom")?);
    if fs::read(&target).ok() == Some(fs::read(&source).map_err(|e| e.to_string())?) {
        return Ok(());
    }
    let partial = target.with_extension("partial");
    fs::copy(&source, &partial).map_err(|e| e.to_string())?;
    // A running binary cannot be overwritten on Windows, but can be renamed.
    let old = target.with_extension("old");
    let _ = fs::remove_file(&old);
    let _ = fs::rename(&target, &old);
    fs::rename(&partial, &target).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&old);
    Ok(())
}

#[derive(Deserialize)]
struct Pane {
    pane_id: String,
    workspace_id: String,
    tab_id: String,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    agent_status: String,
}

#[derive(Deserialize)]
struct Tab {
    tab_id: String,
    #[serde(default)]
    label: String,
}

#[derive(Deserialize)]
struct Workspace {
    workspace_id: String,
    number: u64,
    #[serde(default)]
    label: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Run by the tab bar entry: report, save the CPU sample, print the total.
    Tick,
    /// Run on an agent state change: report only, right away.
    Event,
    /// Print the tokens instead of reporting them, for a manual check.
    Show,
}

fn tick(mode: Mode) -> Result<(), String> {
    let state_dir = match std::env::var_os(STATE_DIR_ENV) {
        Some(dir) => PathBuf::from(dir),
        // The tab bar entry gets no plugin context: it runs the copy that
        // `install` put in the state folder.
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .map(Path::to_path_buf)
            .ok_or("dossier du binaire inconnu")?,
    };
    // Lets Herdr drop a report older than one already applied.
    let seq = now_ms();
    let send = |method: &str, key: &str, id: &str, tokens: Map<String, Value>| match mode {
        Mode::Show => println!("{id}: {}", Value::Object(tokens)),
        _ => report(method, key, id, tokens, seq),
    };
    let sample_path = state_dir.join(SAMPLE_FILE);

    let panes: Vec<Pane> = list("pane.list", "panes")?;
    let tabs: Vec<Tab> = list("tab.list", "tabs")?;
    let spaces: Vec<Space> = list::<Workspace>("workspace.list", "workspaces")?
        .into_iter()
        .map(|w| Space {
            id: w.workspace_id,
            label: w.label,
            number: w.number,
        })
        .collect();

    let previous = match mode {
        Mode::Tick | Mode::Event => None,
        // No saved sample to compare with: take one a second earlier.
        Mode::Show => {
            let first = Sample::of(now_ms(), &read_processes()?);
            std::thread::sleep(std::time::Duration::from_secs(1));
            Some(first)
        }
    };
    let procs = read_processes()?;
    let now = now_ms();
    let previous = previous.or_else(|| read_sample(&sample_path, now));
    let elapsed = previous.as_ref().map(|s| now.saturating_sub(s.at_ms));
    let empty = Sample::default();
    let table = Table::new(&procs, previous.as_ref().unwrap_or(&empty));
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let cpu = |u: &Usage| elapsed.map(|e| format_cpu(cpu_percent(u.cpu_ms, e, cores)));

    let mut sums: Vec<(String, Usage)> = Vec::new();
    let mut total = Usage::default();
    let mut agents = Vec::new();
    let mut usages = Vec::new();
    for pane in &panes {
        let Some(root) = shell_pid(&pane.pane_id) else {
            continue;
        };
        let usage = table.tree(root);
        total += usage;
        match sums.iter_mut().find(|(id, _)| *id == pane.workspace_id) {
            Some((_, sum)) => *sum += usage,
            None => sums.push((pane.workspace_id.clone(), usage)),
        }
        if let Some(agent) = &pane.agent {
            // The tab name tells agents apart; an unnamed tab shows its number.
            let name = tabs
                .iter()
                .find(|t| t.tab_id == pane.tab_id)
                .map(|t| t.label.clone())
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| agent.clone());
            agents.push(tree::Agent {
                pane_id: pane.pane_id.clone(),
                workspace_id: pane.workspace_id.clone(),
                tab_id: pane.tab_id.clone(),
                name,
                status: pane.agent_status.clone(),
            });
            usages.push((pane.pane_id.clone(), pane.workspace_id.clone(), usage));
        }
    }

    for block in tree::blocks(&spaces, &agents) {
        let Some((_, workspace_id, usage)) = usages.iter().find(|(id, ..)| *id == block.pane_id)
        else {
            continue;
        };
        // A lone agent's figures already tell its workspace's.
        let several = agents
            .iter()
            .filter(|a| a.workspace_id == *workspace_id)
            .nth(1)
            .is_some();
        let workspace_stats = (block.header.is_some() && several).then(|| {
            sums.iter()
                .find(|(id, _)| id == workspace_id)
                .map_or_else(String::new, |(_, u)| figures(cpu(u), u, false))
        });
        let mut tokens = tokens(cpu(usage), usage);
        let shells = match usage.shells {
            0 => Value::Null,
            1 => "1 shell".into(),
            n => format!("{n} shells").into(),
        };
        tokens.insert("shells".into(), shells);
        tokens.insert("order".into(), block.order.into());
        tokens.insert("line".into(), block.line.into());
        tokens.insert("stats".into(), figures(cpu(usage), usage, true).into());
        tokens.insert("ws".into(), block.header.map_or(Value::Null, Value::from));
        tokens.insert(
            "ws_stats".into(),
            workspace_stats.map_or(Value::Null, Value::from),
        );
        send("pane.report_metadata", "pane_id", &block.pane_id, tokens);
    }
    for (id, usage) in &sums {
        send(
            "workspace.report_metadata",
            "workspace_id",
            id,
            tokens(cpu(usage), usage),
        );
    }

    // The local time, then the total of every pane: `14:05 6% 2.4G`.
    let tab_bar = || {
        let time = chrono::Local::now().format("%H:%M");
        format!("{time} {}", figures(cpu(&total), &total, false))
    };
    match mode {
        Mode::Event => {}
        Mode::Tick => {
            write_sample(&sample_path, &Sample::of(now, &procs));
            println!("{}", tab_bar());
        }
        Mode::Show => println!("{}", tab_bar()),
    }
    Ok(())
}

/// Nerd Font terminal glyph (nf-oct-terminal), as in CC-Plugins' dx. Some
/// terminals draw it two cells wide: the space after it keeps the count clear.
const SHELL_GLYPH: char = '\u{F489}';

/// `4% 853M  1`; the CPU share is missing on the first run.
fn figures(cpu: Option<String>, usage: &Usage, with_shells: bool) -> String {
    let mut parts: Vec<String> = cpu.into_iter().collect();
    parts.push(format_memory(usage.memory));
    if with_shells && usage.shells > 0 {
        parts.push(format!("{SHELL_GLYPH} {}", usage.shells));
    }
    parts.join(" ")
}

/// `cpu` is left out (not cleared) on the first run, which has no average yet.
fn tokens(cpu: Option<String>, usage: &Usage) -> Map<String, Value> {
    let mut tokens = Map::new();
    if let Some(cpu) = cpu {
        tokens.insert("cpu".into(), cpu.into());
    }
    tokens.insert("ram".into(), format_memory(usage.memory).into());
    tokens
}

fn list<T: serde::de::DeserializeOwned>(method: &str, field: &str) -> Result<Vec<T>, String> {
    let items = herdr::request(method, json!({}))?
        .get_mut(field)
        .map(Value::take)
        .unwrap_or_default();
    serde_json::from_value(items).map_err(|e| format!("{method}: {e}"))
}

/// A failed report only loses one refresh: it is logged, never fatal.
fn report(method: &str, key: &str, id: &str, tokens: Map<String, Value>, seq: u64) {
    let params = json!({
        key: id,
        "source": SOURCE,
        "tokens": tokens,
        "ttl_ms": TOKEN_TTL_MS,
        "seq": seq,
    });
    if let Err(e) = herdr::request(method, params) {
        eprintln!("herdr-sidebar: {e}");
    }
}

fn shell_pid(pane_id: &str) -> Option<u32> {
    let info = herdr::request("pane.process_info", json!({ "pane_id": pane_id })).ok()?;
    info.get("process_info")?
        .get("shell_pid")?
        .as_u64()
        .and_then(|pid| u32::try_from(pid).ok())
}

fn read_processes() -> Result<Vec<Proc>, String> {
    let mut system = System::new().map_err(|e| format!("processus illisibles: {e:?}"))?;
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cpu().with_memory(),
    );
    Ok(system
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
        .collect())
}

fn read_sample(path: &Path, now: u64) -> Option<Sample> {
    let sample: Sample = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    let age = now.checked_sub(sample.at_ms)?;
    (age > 0 && age <= MAX_SAMPLE_AGE_MS).then_some(sample)
}

fn write_sample(path: &Path, sample: &Sample) {
    let partial = path.with_extension("partial");
    let written = serde_json::to_vec(sample)
        .map_err(|e| e.to_string())
        .and_then(|bytes| fs::write(&partial, bytes).map_err(|e| e.to_string()))
        .and_then(|()| fs::rename(&partial, path).map_err(|e| e.to_string()));
    if let Err(e) = written {
        eprintln!("herdr-sidebar: échantillon non enregistré: {e}");
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}
