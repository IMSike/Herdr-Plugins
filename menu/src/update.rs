//! The `Update` entry. Herdr refuses `herdr update` inside one of its panes
//! (`HERDR_ENV`), so the entry opens a terminal window of its own, outside
//! Herdr, which runs `herdr-menu update`: what has a new version (see
//! `check`), ticked, to untick; the update of what stays ticked; then, when
//! the server must restart to run Herdr's new version, a confirmation that
//! says what stops.

pub mod check;

use crate::herdr;
use herdr_api::Taken;
use herdr_api::env::INSIDE as INSIDE_HERDR;
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

/// Held by the update that runs, in the plugin's state folder: a second
/// click opens no second update.
const LOCK: &str = "update.lock";
const ALREADY_RUNNING: &str = "une mise à jour est déjà en cours dans une autre fenêtre";
/// How long the window's terminal is watched for an early end.
const WINDOW_CHECK: std::time::Duration = std::time::Duration::from_secs(1);
/// The variable that hands `cmd` the program's path.
#[cfg(windows)]
const SELF_ENV: &str = "HERDR_MENU_SELF";

/// The update's lock: the update runs without it when there is none to take.
fn lock() -> Taken {
    herdr_api::state_dir().map_or(Taken::Unavailable, |dir| {
        herdr_api::Lock::try_take(&dir.join(LOCK))
    })
}

/// Opens the window that runs the update, and returns at once.
pub fn open_window() -> Result<(), String> {
    if matches!(lock(), Taken::Busy) {
        return Err(ALREADY_RUNNING.into());
    }
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = terminal_window(&me)?;
    command
        .env_remove(INSIDE_HERDR)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
        .spawn()
        .map_err(|e| format!("fenêtre de mise à jour impossible : {e}"))?;
    // A terminal that cannot open its window ends at once, in error; one
    // that hands the window to a server of its own ends at once too, fine.
    let started = std::time::Instant::now();
    while started.elapsed() < WINDOW_CHECK {
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                return Err(format!(
                    "la fenêtre de mise à jour s'est fermée ({status}) : lancez « herdr update » hors de Herdr"
                ));
            }
            Ok(Some(_)) | Err(_) => return Ok(()),
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// A new console window running `me update` (Windows), opened by `start`:
/// the window's console is the update's input and output, whatever handles
/// the bar's pane would pass on. `cmd` itself opens no window, and ends
/// once the window is open.
#[cfg(windows)]
fn terminal_window(me: &std::path::Path) -> Result<Command, String> {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("cmd");
    command.env(SELF_ENV, me).raw_arg(window_line());
    crate::launch::no_window(&mut command);
    Ok(command)
}

/// `cmd`'s command line. `cmd` reads `%…%` in its line as a variable, and
/// `&` or `^` outside quotes as its own: the path, which may hold any of
/// them, comes in a variable, whose value `cmd` does not read again, and in
/// quotes. The empty title: `start` takes a first quoted word for one.
#[cfg(windows)]
fn window_line() -> String {
    format!("/c start \"\" \"%{SELF_ENV}%\" update")
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
    // Over SSH, without a display, no terminal can open a window.
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Err("pas d'affichage graphique : lancez « herdr update » hors de Herdr".into());
    }
    let (terminal, path) = TERMINALS
        .iter()
        .find_map(|t| Some((t, crate::launch::find_on_path(t[0])?)))
        .ok_or("aucun terminal trouvé : lancez « herdr update » hors de Herdr")?;
    let mut command = Command::new(path);
    command.args(&terminal[1..]).arg(me).arg("update");
    Ok(command)
}

/// Runs `herdr <args>` with its output on screen, and this window's input,
/// without the variables of the bar's pane: Herdr started again here would
/// hand them to every pane it opens.
fn herdr_shown(args: &[&str]) -> Result<(), String> {
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
        Taken::Busy => Err(ALREADY_RUNNING.to_string()),
        Taken::Held(lock) => update(Some(lock)),
        Taken::Unavailable => update(None),
    };
    let message = match &result {
        Ok(message) => message.clone(),
        Err(e) => format!("Erreur : {e}"),
    };
    crate::ui::pause(&format!("\n{message}"));
    result.map(|_| ())
}

/// The update, holding `lock`; what to tell once done.
fn update(lock: Option<herdr_api::Lock>) -> Result<String, String> {
    println!("Recherche des mises à jour…");
    let found = check::now_and_keep();
    let offered = offers(&found);
    if offered.is_empty() {
        return Ok("Herdr et les plugins installés depuis GitHub sont à jour.".into());
    }
    let labels: Vec<String> = offered.iter().map(|(_, label)| label.clone()).collect();
    let ticked =
        crate::ui::pick("Mises à jour", &labels).map_err(|e| format!("liste impossible : {e}"))?;
    let chosen: Vec<&Offer> = offered
        .iter()
        .zip(ticked.unwrap_or_default())
        .filter(|(_, ticked)| *ticked)
        .map(|((offer, _), _)| offer)
        .collect();
    if chosen.is_empty() {
        return Ok("Rien n'a été mis à jour.".into());
    }
    let herdr = chosen.contains(&&Offer::Herdr);
    let plugins: Vec<String> = chosen
        .iter()
        .filter_map(|offer| match offer {
            Offer::Plugin(id) => Some(id.clone()),
            Offer::Herdr => None,
        })
        .collect();
    let installed = install(herdr, &plugins);
    // What is still new once done, even when a step failed midway.
    println!("\nVérification des versions…");
    check::now_and_keep();
    installed?;
    if !herdr {
        return Ok(
            "Plugins à jour : leurs panes et commandes prennent la nouvelle \
                   version dès leur prochain lancement."
                .into(),
        );
    }
    if !restart_needed(&check::status()?) {
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
    if !crate::ui::ask("Redémarrer maintenant ? [o/N] ", false) {
        return Ok(
            "Redémarrage annulé : la nouvelle version servira au prochain \
                   démarrage du serveur Herdr."
                .into(),
        );
    }
    herdr_shown(&["server", "stop"])?;
    if crate::ui::ask(
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

/// Updates Herdr when `herdr`, then `plugins`: Herdr first, since a
/// plugin's new version may need Herdr's.
fn install(herdr: bool, plugins: &[String]) -> Result<(), String> {
    if herdr {
        // Where the server can hand its sessions over, nothing stops.
        let handoff = check::status()?["server"]["capabilities"]["live_handoff"] == true;
        println!("Mise à jour de Herdr…\n");
        if handoff {
            herdr_shown(&["update", "--handoff"])?;
        } else {
            herdr_shown(&["update"])?;
        }
    }
    if !plugins.is_empty() {
        println!("\nMise à jour des plugins…\n");
        let mut args = vec!["plugin", "update"];
        args.extend(plugins.iter().map(String::as_str));
        herdr_shown(&args)?;
    }
    Ok(())
}

/// Something the window offers to update.
#[derive(Debug, PartialEq, Eq)]
enum Offer {
    Herdr,
    /// A plugin installed from GitHub, by its id.
    Plugin(String),
}

/// What the window offers, and how it names each: Herdr also when its new
/// version is not known (the preview channel, no answer), since
/// `herdr update` finds it on its own.
fn offers(found: &check::Found) -> Vec<(Offer, String)> {
    let herdr = match &found.herdr {
        check::Herdr::Newer { installed, latest } => Some(format!("Herdr {installed} → {latest}")),
        check::Herdr::Unknown(why) => Some(format!("Herdr (nouvelle version inconnue : {why})")),
        check::Herdr::UpToDate => None,
    };
    herdr
        .map(|label| (Offer::Herdr, label))
        .into_iter()
        .chain(found.plugins.iter().map(|p| {
            (
                Offer::Plugin(p.id.clone()),
                format!("Plugin {} ({})", p.name, p.id),
            )
        }))
        .collect()
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
    fn offers_herdr_unless_up_to_date_and_each_new_plugin() {
        let plugin = check::Plugin {
            id: "gcaudan.explorer".into(),
            name: "Explorer".into(),
        };
        let found = |herdr| check::Found {
            herdr,
            plugins: vec![plugin.clone()],
        };
        let newer = check::Herdr::Newer {
            installed: "0.9.5".into(),
            latest: "0.9.6".into(),
        };
        assert_eq!(
            offers(&found(newer)),
            [
                (Offer::Herdr, "Herdr 0.9.5 → 0.9.6".to_string()),
                (
                    Offer::Plugin("gcaudan.explorer".into()),
                    "Plugin Explorer (gcaudan.explorer)".to_string()
                ),
            ]
        );
        let unknown = offers(&found(check::Herdr::Unknown("canal preview".into())));
        assert_eq!(unknown[0].0, Offer::Herdr);
        assert_eq!(offers(&found(check::Herdr::UpToDate)).len(), 1);
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
