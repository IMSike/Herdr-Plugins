//! Herdr plugin `imsike.menu`: a menu that opens a tool in a large popup.
//!
//! In a tab that shows the file explorer of gcaudan/herdr-explorer, the menu
//! is a `Tools` pane of icons above it (see `bar`); elsewhere it is a popup.
//!
//! Herdr shows one popup at a time and refuses to open another while one is on
//! screen, so the popup menu cannot open the tool itself. The action drives both:
//!
//! 1. `open` (action) opens the `menu` popup and waits for its choice file.
//! 2. `menu` (popup) writes its process id, then the chosen entry and the
//!    folder to start in, and exits.
//! 3. `open` reads the choice and opens the `run` popup, retrying while the menu
//!    popup is being torn down. It stops waiting as soon as the menu's process
//!    is gone, however the popup was closed.
//! 4. `run` (popup) runs the entry's command in that folder.

mod bar;
mod config;
mod herdr;
mod launch;
mod stats;
mod ui;

use config::Entry;
use herdr::Context;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// Choice file the `menu` popup writes for the `open` action.
const CHOICE_ENV: &str = "HERDR_MENU_CHOICE";
/// The entry (as JSON) and the start folder handed to the `run` popup.
pub(crate) const ENTRY_ENV: &str = "HERDR_MENU_ENTRY";
pub(crate) const CWD_ENV: &str = "HERDR_MENU_CWD";
/// How long the action waits for the menu to start.
const START_TIMEOUT: Duration = Duration::from_secs(10);
/// How often the action checks that the menu is still running.
const ALIVE_EVERY: Duration = Duration::from_secs(1);
/// Files of the state folder left by a process killed midway are removed
/// after this long: no menu stays open a day.
const STALE_AFTER: Duration = Duration::from_secs(24 * 3600);

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        Some("open") => open(),
        Some("menu") => menu(),
        Some("run") => run(),
        Some("bar") => bar::run(&entries().0),
        Some("startup") => bar::startup(),
        Some("pane-created") => bar::on_pane_created(),
        Some("pane-closed") => bar::on_pane_closed(),
        _ => Err("usage : herdr-menu open|menu|run|bar|startup|pane-created|pane-closed".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("herdr-menu : {message}");
            ExitCode::FAILURE
        }
    }
}

fn entries() -> (Vec<Entry>, Option<String>) {
    let dir = std::env::var_os(herdr_api::env::CONFIG_DIR).map(PathBuf::from);
    config::load(dir.as_deref())
}

/// A choice: the entry as the menu showed it, and the folder to start in.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Choice {
    entry: Entry,
    cwd: Option<String>,
}

impl Choice {
    fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// `None` when the menu was closed without a choice (empty file).
    fn decode(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }
}

fn open() -> Result<(), String> {
    // The bar when the tab has the explorer; the popup when it does not, or
    // when the bar cannot open.
    match bar::toggle(&Context::from_env()) {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(message) => eprintln!("herdr-menu : pane Tools impossible, menu en popup : {message}"),
    }
    let state = herdr_api::state_dir().ok_or(format!("{} absent", herdr_api::env::STATE_DIR))?;
    // What an action or a figures file killed midway left behind.
    herdr_api::remove_stale(&state, STALE_AFTER, |name| {
        name.starts_with("choice-") || name.ends_with(".partial")
    });
    let choice_file = state.join(format!("choice-{}", std::process::id()));
    let pid_file = pid_file(&choice_file);
    let _ = std::fs::remove_file(&choice_file);
    let _ = std::fs::remove_file(&pid_file);
    let choice_path = choice_file.to_string_lossy().into_owned();
    herdr::open_pane("menu", &[(CHOICE_ENV, &choice_path)]).map_err(|e| e.to_string())?;

    let text = wait_for_choice(&choice_file, &pid_file);
    let _ = std::fs::remove_file(&choice_file);
    let _ = std::fs::remove_file(&pid_file);
    let Some(choice) = text?.as_deref().and_then(Choice::decode) else {
        return Ok(());
    };
    let cwd = choice.cwd.as_deref();
    // A program with a window of its own starts without the popup.
    if !choice.entry.popup {
        return launch::detach(&choice.entry, cwd.map(Path::new));
    }
    run_in_popup(&choice.entry, cwd)
}

/// The choice file's content once the menu wrote it; `None` when the menu
/// went away without writing it (its popup closed by something else).
fn wait_for_choice(choice_file: &Path, pid_file: &Path) -> Result<Option<String>, String> {
    let started = Instant::now();
    let mut next_check = started + ALIVE_EVERY;
    loop {
        if let Ok(text) = std::fs::read_to_string(choice_file) {
            return Ok(Some(text));
        }
        if Instant::now() >= next_check {
            next_check += ALIVE_EVERY;
            let pid = std::fs::read_to_string(pid_file)
                .ok()
                .and_then(|text| text.trim().parse::<u32>().ok());
            let gone = match pid {
                Some(pid) => !is_running(pid),
                None => started.elapsed() >= START_TIMEOUT,
            };
            if gone {
                // It may have written its choice just before exiting.
                return Ok(std::fs::read_to_string(choice_file).ok());
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn pid_file(choice_file: &Path) -> PathBuf {
    let mut name = choice_file.as_os_str().to_os_string();
    name.push(".pid");
    PathBuf::from(name)
}

/// Whether the menu still runs under `pid`. The menu is this same program: a
/// process of another program given the same pid since does not count.
fn is_running(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let Ok(mut system) = System::new() else {
        return true;
    };
    let pid = Pid::from_u32(pid);
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing(),
    );
    let this = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
    system.process(pid).is_some_and(|p| {
        this.as_deref()
            .is_none_or(|this| same_program(this, &p.name().to_string_lossy()))
    })
}

/// Linux gives a process name of 15 characters at most.
fn same_program(a: &str, b: &str) -> bool {
    let short = |name: &str| name.to_lowercase().chars().take(15).collect::<String>();
    short(a) == short(b)
}

/// Opens the `run` popup for the entry.
pub(crate) fn run_in_popup(entry: &Entry, cwd: Option<&str>) -> Result<(), String> {
    let entry = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    let mut env = vec![(ENTRY_ENV, entry.as_str())];
    if let Some(cwd) = cwd {
        env.push((CWD_ENV, cwd));
    }
    herdr::open_pane_when_free("run", &env).map_err(|e| e.to_string())
}

/// Writes the choice file all at once, so the action never reads half of it.
fn write_choice(path: &Path, choice: Option<&Choice>) -> io::Result<()> {
    herdr_api::write_atomic(
        path,
        choice.map(Choice::encode).unwrap_or_default().as_bytes(),
    )
}

fn menu() -> Result<(), String> {
    let choice_file = std::env::var_os(CHOICE_ENV)
        .map(PathBuf::from)
        .ok_or(format!("{CHOICE_ENV} absent"))?;
    // Tells the waiting action that the menu runs, and which process to watch.
    let _ = herdr_api::write_atomic(
        &pid_file(&choice_file),
        std::process::id().to_string().as_bytes(),
    );
    let (entries, error) = entries();
    let context = Context::from_env();
    let title = context
        .workspace_cwd
        .as_deref()
        .and_then(|p| p.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Outils".into());
    let chosen = ui::choose(&title, &entries, error.as_deref());
    // Always answer the waiting action, even when the menu failed.
    let choice = chosen.as_ref().ok().copied().flatten().map(|i| Choice {
        entry: entries[i].clone(),
        cwd: context
            .cwd(entries[i].cwd)
            .map(|p| p.to_string_lossy().into_owned()),
    });
    write_choice(&choice_file, choice.as_ref()).map_err(|e| e.to_string())?;
    chosen.map(|_| ()).map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    let result = run_entry();
    if let Err(message) = &result {
        // The popup closes with this process: leave the message on screen until a key.
        println!("{message}\n\nAppuyer sur Entrée pour fermer.");
        let _ = io::stdout().flush();
        let _ = io::stdin().read_line(&mut String::new());
    }
    result
}

/// Runs the entry the menu or the bar showed, not one read again from
/// `menu.toml`, which may have changed since.
fn run_entry() -> Result<(), String> {
    let json = std::env::var(ENTRY_ENV).map_err(|_| format!("{ENTRY_ENV} absent"))?;
    let entry: Entry =
        serde_json::from_str(&json).map_err(|e| format!("{ENTRY_ENV} illisible : {e}"))?;
    let mut command = launch::command(&entry.command)?;
    if let Some(cwd) = std::env::var_os(CWD_ENV) {
        command.current_dir(cwd);
    }
    let status = command
        .status()
        .map_err(|e| format!("impossible de lancer « {} » : {e}", entry.command.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "« {} » s'est terminé en erreur ({status})",
            entry.label
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choice_round_trips() {
        let entries = config::defaults();
        let with_cwd = Choice {
            entry: entries[0].clone(),
            cwd: Some("C:\\repo\\app".into()),
        };
        assert_eq!(Choice::decode(&with_cwd.encode()), Some(with_cwd));
        let without_cwd = Choice {
            entry: entries[1].clone(),
            cwd: None,
        };
        assert_eq!(Choice::decode(&without_cwd.encode()), Some(without_cwd));
    }

    #[test]
    fn empty_choice_means_closed() {
        assert_eq!(Choice::decode(""), None);
    }

    #[test]
    fn this_process_runs() {
        assert!(is_running(std::process::id()));
        assert!(same_program("herdr-menu.exe", "HERDR-MENU.EXE"));
        assert!(same_program(
            "herdr_menu-0123456789abcdef",
            "herdr_menu-0123"
        ));
        assert!(!same_program("herdr-menu.exe", "notepad.exe"));
    }
}
