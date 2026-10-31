//! Herdr plugin `imsike.menu`: a popup menu that opens a tool in a full-size popup.
//!
//! Herdr shows one popup at a time and refuses to open another while one is on
//! screen, so the menu cannot open the tool itself. The action drives both:
//!
//! 1. `open` (action) opens the `menu` popup and waits for its choice file.
//! 2. `menu` (popup) writes the chosen entry and the folder to start in, then exits.
//! 3. `open` reads the choice and opens the `run` popup, retrying while the menu
//!    popup is being torn down.
//! 4. `run` (popup) runs the entry's command in that folder.

mod config;
mod herdr;
mod ui;

use config::Entry;
use herdr::Context;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

/// Choice file the `menu` popup writes for the `open` action.
const CHOICE_ENV: &str = "HERDR_MENU_CHOICE";
/// Entry key and start folder the `open` action hands to the `run` popup.
const ENTRY_ENV: &str = "HERDR_MENU_ENTRY";
const CWD_ENV: &str = "HERDR_MENU_CWD";
/// How long the action waits for a choice before giving up.
const CHOICE_TIMEOUT: Duration = Duration::from_secs(60 * 60);

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        Some("open") => open(),
        Some("menu") => menu(),
        Some("run") => run(),
        _ => Err("usage : herdr-menu open|menu|run".into()),
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
    let dir = std::env::var_os("HERDR_PLUGIN_CONFIG_DIR").map(PathBuf::from);
    config::load(dir.as_deref())
}

/// A choice: the entry's key and the folder to start in, one per line.
#[derive(Debug, PartialEq, Eq)]
struct Choice {
    key: char,
    cwd: Option<String>,
}

impl Choice {
    fn encode(&self) -> String {
        format!("{}\n{}", self.key, self.cwd.as_deref().unwrap_or(""))
    }

    /// `None` when the menu was closed without a choice (empty file).
    fn decode(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        let key = lines.next()?.chars().next()?;
        let cwd = lines.next().filter(|l| !l.is_empty()).map(str::to_string);
        Some(Choice { key, cwd })
    }
}

fn open() -> Result<(), String> {
    let state = std::env::var_os("HERDR_PLUGIN_STATE_DIR")
        .map(PathBuf::from)
        .ok_or("HERDR_PLUGIN_STATE_DIR absent")?;
    let choice_file = state.join(format!("choice-{}", std::process::id()));
    let _ = std::fs::remove_file(&choice_file);
    let choice_path = choice_file.to_string_lossy().into_owned();
    herdr::open_pane("menu", &[(CHOICE_ENV, &choice_path)]).map_err(|e| e.to_string())?;

    let deadline = Instant::now() + CHOICE_TIMEOUT;
    let text = loop {
        if let Ok(text) = std::fs::read_to_string(&choice_file) {
            let _ = std::fs::remove_file(&choice_file);
            break text;
        }
        if Instant::now() >= deadline {
            return Err("aucun choix reçu du menu".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let Some(choice) = Choice::decode(&text) else {
        return Ok(());
    };
    let key = choice.key.to_string();
    let mut env = vec![(ENTRY_ENV, key.as_str())];
    if let Some(cwd) = &choice.cwd {
        env.push((CWD_ENV, cwd.as_str()));
    }
    herdr::open_pane_when_free("run", &env).map_err(|e| e.to_string())
}

/// Writes the choice file all at once, so the action never reads half of it.
fn write_choice(path: &Path, choice: Option<&Choice>) -> io::Result<()> {
    let partial = path.with_extension("partial");
    std::fs::write(&partial, choice.map(Choice::encode).unwrap_or_default())?;
    std::fs::rename(&partial, path)
}

fn menu() -> Result<(), String> {
    let choice_file = std::env::var_os(CHOICE_ENV)
        .map(PathBuf::from)
        .ok_or(format!("{CHOICE_ENV} absent"))?;
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
        key: entries[i].key,
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

fn run_entry() -> Result<(), String> {
    let key = std::env::var(ENTRY_ENV).map_err(|_| format!("{ENTRY_ENV} absent"))?;
    let (entries, _) = entries();
    let entry = entries
        .iter()
        .find(|e| key.chars().eq([e.key]))
        .ok_or_else(|| format!("aucune entrée pour la touche « {key} »"))?;
    let mut command = Command::new(&entry.command[0]);
    command.args(&entry.command[1..]);
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
        let with_cwd = Choice {
            key: 'l',
            cwd: Some("C:\\repo\\app".into()),
        };
        assert_eq!(Choice::decode(&with_cwd.encode()), Some(with_cwd));
        let without_cwd = Choice {
            key: 'g',
            cwd: None,
        };
        assert_eq!(Choice::decode(&without_cwd.encode()), Some(without_cwd));
    }

    #[test]
    fn empty_choice_means_closed() {
        assert_eq!(Choice::decode(""), None);
    }
}
