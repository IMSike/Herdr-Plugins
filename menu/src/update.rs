//! The `Update` entry. Herdr refuses `herdr update` inside one of its panes
//! (`HERDR_ENV`), so the entry opens a terminal window of its own, outside
//! Herdr, which runs `herdr-menu update`: the update, then, when the server
//! must restart to run the new version, a confirmation that says what stops.

use crate::herdr;
use herdr_api::env::INSIDE as INSIDE_HERDR;
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

/// Held by the update that runs, in the plugin's state folder: a second
/// click opens no second update.
const LOCK: &str = "update.lock";
const ALREADY_RUNNING: &str = "une mise à jour est déjà en cours dans une autre fenêtre";

/// The update's lock; `Ok(None)` while another update holds it, an error
/// when it cannot be taken (the update runs without it).
fn lock() -> Result<Option<herdr_api::Lock>, String> {
    let file = herdr_api::state_dir()?.join(LOCK);
    herdr_api::Lock::take(&file).map_err(|e| format!("verrou impossible : {e}"))
}

/// Opens the window that runs the update, and returns at once.
pub fn open_window() -> Result<(), String> {
    if matches!(lock(), Ok(None)) {
        return Err(ALREADY_RUNNING.into());
    }
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = terminal_window(&me)?;
    command.env_remove(INSIDE_HERDR);
    // The new console window is the update's input and output on Windows:
    // given null ones, it would show nothing and read no answer.
    #[cfg(not(windows))]
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
        .spawn()
        .map_err(|e| format!("fenêtre de mise à jour impossible : {e}"))?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// A new console window running `me update` (Windows).
#[cfg(windows)]
fn terminal_window(me: &std::path::Path) -> Result<Command, String> {
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    let mut command = Command::new(me);
    command.arg("update");
    std::os::windows::process::CommandExt::creation_flags(&mut command, CREATE_NEW_CONSOLE);
    Ok(command)
}

/// The first terminal emulator found, running `me update`.
#[cfg(not(windows))]
fn terminal_window(me: &std::path::Path) -> Result<Command, String> {
    const TERMINALS: &[&[&str]] = &[
        &["x-terminal-emulator", "-e"],
        &["gnome-terminal", "--"],
        &["konsole", "-e"],
        &["xfce4-terminal", "-x"],
        &["kitty"],
        &["alacritty", "-e"],
        &["wezterm", "start", "--"],
        &["foot"],
        &["xterm", "-e"],
    ];
    let terminal = TERMINALS
        .iter()
        .find(|t| crate::launch::find_on_path(t[0]).is_some())
        .ok_or("aucun terminal trouvé : lancez « herdr update » hors de Herdr")?;
    let mut command = Command::new(terminal[0]);
    command.args(&terminal[1..]).arg(me).arg("update");
    Ok(command)
}

fn herdr(args: &[&str]) -> Command {
    let mut command = Command::new(herdr::binary());
    command.args(args);
    command
}

/// Runs `herdr <args>` with its output on screen.
fn herdr_shown(args: &[&str]) -> Result<(), String> {
    let status = herdr(args)
        .status()
        .map_err(|e| format!("impossible de lancer « herdr {} » : {e}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("« herdr {} » a échoué ({status})", args.join(" ")))
    }
}

/// `herdr status --json`.
fn status() -> Result<Value, String> {
    let output = herdr::command(&["status", "--json"])
        .output()
        .map_err(|e| format!("impossible de lancer « herdr status » : {e}"))?;
    serde_json::from_slice(&output.stdout).map_err(|e| format!("état de Herdr illisible : {e}"))
}

/// Whether the server runs an older binary than the one installed.
fn restart_needed(status: &Value) -> bool {
    ["restart_needed", "server_binary_stale"]
        .iter()
        .any(|key| status["update"][key] == true || status["server"][key] == true)
}

/// `herdr-menu update`, in the window `open_window` opened. The window
/// closes with it, so it ends on a key press.
pub fn run() -> Result<(), String> {
    let result = match lock() {
        Ok(None) => Err(ALREADY_RUNNING.to_string()),
        Ok(lock) => update(lock),
        Err(_) => update(None),
    };
    let message = match &result {
        Ok(message) => message.clone(),
        Err(e) => format!("Erreur : {e}"),
    };
    crate::pause(&format!("\n{message}\n\nAppuyer sur Entrée pour fermer."));
    result.map(|_| ())
}

/// The update, holding `lock`; what to tell once done.
fn update(lock: Option<herdr_api::Lock>) -> Result<String, String> {
    let before = status()?;
    // Where the server can hand its sessions over, nothing stops.
    let handoff = before["server"]["capabilities"]["live_handoff"] == true;
    println!("Mise à jour de Herdr…\n");
    if handoff {
        herdr_shown(&["update", "--handoff"])?;
    } else {
        herdr_shown(&["update"])?;
    }
    if !restart_needed(&status()?) {
        return Ok(
            "Herdr est à jour. Le client déjà ouvert garde son ancienne version : \
                   détachez-vous (prefix+q) puis relancez « herdr » pour passer à la nouvelle."
                .into(),
        );
    }
    let panes = herdr::all_panes()?;
    let agents: Vec<_> = panes.iter().filter(|p| p.agent.is_some()).collect();
    let working = agents
        .iter()
        .filter(|p| p.agent_status.as_deref() == Some("working"))
        .count();
    println!(
        "\nLa nouvelle version demande de redémarrer le serveur Herdr.\n\
         Tous les panes se ferment : {}.",
        stopping(panes.len(), agents.len(), working)
    );
    if !crate::ask("Redémarrer maintenant ? [o/N] ", false) {
        return Ok(
            "Redémarrage annulé : la nouvelle version servira au prochain \
                   démarrage du serveur Herdr."
                .into(),
        );
    }
    herdr_shown(&["server", "stop"])?;
    if crate::ask(
        "\nServeur arrêté. Relancer Herdr dans cette fenêtre ? [o/N] ",
        false,
    ) {
        // Herdr runs in this window for as long as it is used: the update
        // is over.
        drop(lock);
        herdr_shown(&[])?;
        return Ok("Herdr s'est fermé.".into());
    }
    Ok(if copy_to_clipboard("herdr") {
        "« herdr » est copié dans le presse-papiers : le coller dans un terminal relance Herdr."
    } else {
        "Relancez Herdr avec « herdr »."
    }
    .into())
}

/// `5 panes, dont 2 agents (1 au travail)`.
fn stopping(panes: usize, agents: usize, working: usize) -> String {
    let plural = |n: usize, word: &str| {
        if n > 1 {
            format!("{n} {word}s")
        } else {
            format!("{n} {word}")
        }
    };
    let mut text = plural(panes, "pane");
    if agents > 0 {
        text.push_str(&format!(", dont {}", plural(agents, "agent")));
        if working > 0 {
            text.push_str(&format!(" ({working} au travail)"));
        }
    }
    text
}

/// Puts `text` in the system clipboard; `false` when no clipboard tool works.
fn copy_to_clipboard(text: &str) -> bool {
    let tools: &[&[&str]] = if cfg!(windows) {
        &[&["clip"]]
    } else if cfg!(target_os = "macos") {
        &[&["pbcopy"]]
    } else {
        &[
            &["wl-copy"],
            &["xclip", "-selection", "clipboard"],
            &["xsel", "--clipboard", "--input"],
        ]
    };
    tools.iter().any(|tool| pipe_to(tool, text))
}

fn pipe_to(argv: &[&str], text: &str) -> bool {
    let Ok(mut child) = Command::new(argv[0])
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let written = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
    child.wait().is_ok_and(|status| status.success()) && written
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn says_what_stops() {
        assert_eq!(stopping(1, 0, 0), "1 pane");
        assert_eq!(stopping(5, 2, 1), "5 panes, dont 2 agents (1 au travail)");
        assert_eq!(stopping(3, 1, 0), "3 panes, dont 1 agent");
    }

    #[test]
    fn reads_whether_a_restart_is_needed() {
        assert!(!restart_needed(
            &json!({"update": {"restart_needed": false, "server_binary_stale": false}})
        ));
        assert!(restart_needed(&json!({"update": {"restart_needed": true}})));
        assert!(restart_needed(
            &json!({"server": {"server_binary_stale": true}})
        ));
        assert!(!restart_needed(&json!({})));
    }
}
