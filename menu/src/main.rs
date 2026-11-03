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
//!
//! The built-in entries: `Update` opens a terminal window outside Herdr
//! (`update`), `Settings` the `settings` popup, which opens the files in an
//! editor (`edit-tab`, `edit` for a terminal one), `Disk` a view of the bar.

mod bar;
mod config;
mod herdr;
mod launch;
mod settings;
mod stats;
mod ui;
mod update;

use config::{Action, Entry};
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
/// How often the action looks for the choice file.
const CHOICE_CHECK_EVERY: Duration = Duration::from_millis(50);

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        Some("open") => open(),
        Some("menu") => menu(),
        Some("run") => run(),
        Some("bar") => {
            let (menu, error) = menu_config();
            bar::run(menu, error)
        }
        Some("settings") => settings::run(),
        Some("edit-tab") => settings::files::open_tab(),
        Some("edit") => settings::files::edit_in_tab(),
        Some("update") => update::run(),
        Some("startup") => bar::startup(),
        Some("pane-created") => bar::on_pane_created(),
        Some("pane-closed") => bar::on_pane_closed(),
        _ => Err(
            "usage : herdr-menu open|menu|run|bar|settings|edit-tab|edit|update|startup|pane-created|pane-closed"
                .into(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("herdr-menu : {message}");
            ExitCode::FAILURE
        }
    }
}

/// The user's menu, from the configuration folder Herdr gives the plugin
/// (see `config::load`).
pub(crate) fn menu_config() -> (config::Menu, Option<String>) {
    config::load(config_dir().as_deref())
}

pub(crate) fn config_dir() -> Option<PathBuf> {
    herdr_api::config_dir().ok()
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
    let state = herdr_api::state_dir()?;
    // What an action, or a write of the figures, killed midway left behind.
    herdr_api::remove_stale(&state, STALE_AFTER, |name| {
        name.starts_with("choice-") || name.starts_with("files-open-") || name.ends_with(".partial")
    });
    let choice_file = state.join(format!("choice-{}", std::process::id()));
    let files = [
        choice_file.clone(),
        beside(&choice_file, ".lock"),
        beside(&choice_file, ".started"),
    ];
    let remove = || {
        for file in &files {
            let _ = std::fs::remove_file(file);
        }
    };
    remove();
    let choice_path = choice_file.to_string_lossy().into_owned();
    herdr::open_pane("menu", &[(CHOICE_ENV, &choice_path)]).map_err(|e| e.to_string())?;

    let text = wait_for_choice(&files[0], &files[1], &files[2]);
    remove();
    let Some(choice) = text?.as_deref().and_then(Choice::decode) else {
        return Ok(());
    };
    start(&choice.entry, choice.cwd.as_deref(), None)
}

/// Starts an entry chosen in the menu or the bar (`bar`: its pane id): a
/// built-in action, a program in the `run` popup, or one with a window of
/// its own on its own. The disk view is the bar's own.
pub(crate) fn start(entry: &Entry, cwd: Option<&str>, bar: Option<&str>) -> Result<(), String> {
    match entry.action {
        Some(Action::Update) => update::open_window(),
        Some(Action::Settings) => {
            let env: Vec<(&str, &str)> = bar.map(|b| (settings::BAR_ENV, b)).into_iter().collect();
            herdr::open_pane_when_free("settings", &env).map_err(|e| e.to_string())
        }
        Some(Action::Disk) => Ok(()),
        None if !entry.popup => launch::detach(entry, cwd.map(Path::new)),
        None => run_in_popup(entry, cwd),
    }
}

/// The choice file's content once the menu wrote it; `None` when the menu
/// went away without writing it (its popup closed by something else). The
/// menu runs while it holds `lock`: the system releases it when the menu
/// ends, killed included.
fn wait_for_choice(
    choice_file: &Path,
    lock: &Path,
    started: &Path,
) -> Result<Option<String>, String> {
    let since = Instant::now();
    let mut next_check = since + ALIVE_EVERY;
    loop {
        if let Ok(text) = std::fs::read_to_string(choice_file) {
            return Ok(Some(text));
        }
        if Instant::now() >= next_check {
            next_check += ALIVE_EVERY;
            let gone = if started.exists() {
                // Held by the menu while it runs; taken here once it is gone.
                !matches!(herdr_api::Lock::take(lock), Ok(None))
            } else {
                since.elapsed() >= START_TIMEOUT
            };
            if gone {
                // It may have written its choice just before exiting.
                return Ok(std::fs::read_to_string(choice_file).ok());
            }
        }
        std::thread::sleep(CHOICE_CHECK_EVERY);
    }
}

/// A file beside the choice file: `.lock`, the lock the menu holds while it
/// runs; `.started`, written once it holds it.
fn beside(choice_file: &Path, extension: &str) -> PathBuf {
    let mut name = choice_file.as_os_str().to_os_string();
    name.push(extension);
    PathBuf::from(name)
}

/// Whether a process name is this program's: the menu, the bar and the
/// action are all this same binary.
pub(crate) fn is_this_program(name: &str) -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .is_none_or(|this| same_program(&this, name))
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
    let choice_file = PathBuf::from(herdr_api::var(CHOICE_ENV)?);
    // Tells the waiting action that the menu runs: it holds the lock until
    // it ends. Without the lock, the action waits for the choice until its
    // start timeout.
    let running = herdr_api::Lock::take(&beside(&choice_file, ".lock"))
        .ok()
        .flatten();
    if running.is_some() {
        let _ = herdr_api::write_atomic(&beside(&choice_file, ".started"), b"");
    }
    let (menu, error) = menu_config();
    // The disk figures belong to the bar: the explorer's folder.
    let entries: Vec<Entry> = menu
        .entries
        .into_iter()
        .filter(|e| e.action != Some(Action::Disk))
        .collect();
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
        pause(message);
    }
    result
}

/// Asks a yes-or-no question; an empty answer is `default`, an unreadable
/// one is no.
pub(crate) fn ask(question: &str, default: bool) -> bool {
    print!("{question}");
    let _ = io::Write::flush(&mut io::stdout());
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    is_yes(&answer, default)
}

fn is_yes(answer: &str, default: bool) -> bool {
    match answer.trim().to_lowercase().as_str() {
        "" => default,
        "o" | "oui" | "y" | "yes" => true,
        _ => false,
    }
}

/// Leaves `message` on screen until Enter, and says so: the popup or the
/// window closes with this process.
pub(crate) fn pause(message: &str) {
    println!("{message}\n\nAppuyer sur Entrée pour fermer.");
    let _ = io::stdout().flush();
    let _ = io::stdin().read_line(&mut String::new());
}

/// Runs the entry the menu or the bar showed, not one read again from
/// `menu.toml`, which may have changed since.
fn run_entry() -> Result<(), String> {
    let json = herdr_api::var(ENTRY_ENV)?;
    let entry: Entry =
        serde_json::from_str(&json).map_err(|e| format!("{ENTRY_ENV} illisible : {e}"))?;
    let mut command = launch::command(&entry.command)?;
    if let Some(cwd) = std::env::var_os(CWD_ENV) {
        command.current_dir(cwd);
    }
    let status = command
        .status()
        .map_err(|e| format!("impossible de lancer « {} » : {e}", entry.label))?;
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
    fn an_empty_answer_takes_the_default() {
        assert!(is_yes("\n", true) && !is_yes("\n", false));
        assert!(is_yes(" Oui\n", false) && is_yes("y", false));
        assert!(!is_yes("non", true) && !is_yes("x", true));
    }

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
    fn names_of_this_program() {
        assert!(same_program("herdr-menu.exe", "HERDR-MENU.EXE"));
        assert!(same_program(
            "herdr_menu-0123456789abcdef",
            "herdr_menu-0123"
        ));
        assert!(!same_program("herdr-menu.exe", "notepad.exe"));
    }
}
