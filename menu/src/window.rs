//! A terminal window of its own, outside Herdr, running one of this
//! program's commands (`update`, `recover`): what has to go on once the
//! server stops, or what Herdr refuses to run inside one of its panes
//! (`HERDR_ENV`). And what such a window shares: `herdr` run on its screen,
//! what stops with the server, the command that starts Herdr again put in
//! the clipboard.

use crate::herdr;
use herdr_api::env::INSIDE as INSIDE_HERDR;
use std::ffi::OsStr;
use std::io::Write;
use std::process::{Command, Stdio};

/// How long the window's terminal is watched for an early end.
const WINDOW_CHECK: std::time::Duration = std::time::Duration::from_secs(1);
/// The variable that hands `cmd` the program's path.
#[cfg(windows)]
const SELF_ENV: &str = "HERDR_MENU_SELF";

/// Opens the window that runs `herdr-menu <subcommand>`, with `env` on top
/// of this process's environment, and returns at once. `outside`: what to
/// do instead when no window opens (`lancez « herdr update » hors de
/// Herdr`).
pub fn open(subcommand: &str, env: &[(&str, &OsStr)], outside: &str) -> Result<(), String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = terminal_window(&me, subcommand, outside)?;
    command
        .envs(env.iter().copied())
        .env_remove(INSIDE_HERDR)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
        .spawn()
        .map_err(|e| format!("fenêtre impossible : {e}"))?;
    // A terminal that cannot open its window ends at once, in error; one
    // that hands the window to a server of its own ends at once too, fine.
    let started = std::time::Instant::now();
    while started.elapsed() < WINDOW_CHECK {
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                return Err(format!("la fenêtre s'est fermée ({status}) : {outside}"));
            }
            Ok(Some(_)) | Err(_) => return Ok(()),
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// A new console window running `me <subcommand>` (Windows), opened by
/// `start`: the window's console is the command's input and output,
/// whatever handles the bar's pane would pass on. `cmd` itself opens no
/// window, and ends once the window is open.
#[cfg(windows)]
fn terminal_window(
    me: &std::path::Path,
    subcommand: &str,
    _outside: &str,
) -> Result<Command, String> {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("cmd");
    command.env(SELF_ENV, me).raw_arg(window_line(subcommand));
    crate::launch::no_window(&mut command);
    Ok(command)
}

/// `cmd`'s command line. `cmd` reads `%…%` in its line as a variable, and
/// `&` or `^` outside quotes as its own: the path, which may hold any of
/// them, comes in a variable, whose value `cmd` does not read again, and in
/// quotes. The empty title: `start` takes a first quoted word for one.
/// `subcommand` is one of this program's, a plain word.
#[cfg(windows)]
fn window_line(subcommand: &str) -> String {
    format!("/c start \"\" \"%{SELF_ENV}%\" {subcommand}")
}

/// The first terminal emulator found, running `me <subcommand>`.
#[cfg(not(windows))]
fn terminal_window(
    me: &std::path::Path,
    subcommand: &str,
    outside: &str,
) -> Result<Command, String> {
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
    // Over SSH, without a display, no terminal can open a window.
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Err(format!("pas d'affichage graphique : {outside}"));
    }
    let (terminal, path) = TERMINALS
        .iter()
        .find_map(|t| Some((t, crate::launch::find_on_path(t[0])?)))
        .ok_or_else(|| format!("aucun terminal trouvé : {outside}"))?;
    let mut command = Command::new(path);
    command.args(&terminal[1..]).arg(me).arg(subcommand);
    Ok(command)
}

/// Runs `herdr <args>` with its output on screen, and this window's input,
/// without the variables of the bar's pane: Herdr started again here would
/// hand them to every pane it opens.
pub fn herdr_shown(args: &[&str]) -> Result<(), String> {
    let mut command = herdr::command(args);
    crate::launch::leave_the_pane(&mut command);
    let status = command
        .stdin(Stdio::inherit())
        .status()
        .map_err(|e| format!("impossible de lancer « herdr {} » : {e}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("« herdr {} » a échoué ({status})", args.join(" ")))
    }
}

/// What stops with the server: `5 panes, dont 2 agents (1 au travail)`.
pub fn stopping() -> Result<String, String> {
    let panes = herdr::all_panes()?;
    let agents: Vec<_> = panes.iter().filter(|p| p.agent.is_some()).collect();
    let working = agents
        .iter()
        .filter(|p| p.agent_status.as_deref() == Some("working"))
        .count();
    Ok(count_stopping(panes.len(), agents.len(), working))
}

fn count_stopping(panes: usize, agents: usize, working: usize) -> String {
    let mut text = plural(panes, "pane");
    if agents > 0 {
        text.push_str(&format!(", dont {}", plural(agents, "agent")));
        if working > 0 {
            text.push_str(&format!(" ({working} au travail)"));
        }
    }
    text
}

/// `1 pane`, `2 panes`.
fn plural(n: usize, word: &str) -> String {
    if n > 1 {
        format!("{n} {word}s")
    } else {
        format!("{n} {word}")
    }
}

/// Puts `start`, the command that starts Herdr again, in the clipboard;
/// what to tell, the command shown either way.
pub fn start_from_clipboard(start: &str) -> String {
    if copy_to_clipboard(start) {
        format!(
            "« {start} » est copié dans le presse-papiers : le coller dans un terminal relance Herdr."
        )
    } else {
        format!("Relancez Herdr avec « {start} ».")
    }
}

/// Puts `text` in the system clipboard; `false` when no clipboard tool works.
fn copy_to_clipboard(text: &str) -> bool {
    let tools: &[&[&str]] = if cfg!(windows) {
        &[&["clip"]]
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

    #[test]
    fn says_what_stops() {
        assert_eq!(count_stopping(1, 0, 0), "1 pane");
        assert_eq!(
            count_stopping(5, 2, 1),
            "5 panes, dont 2 agents (1 au travail)"
        );
        assert_eq!(count_stopping(3, 1, 0), "3 panes, dont 1 agent");
    }
}
