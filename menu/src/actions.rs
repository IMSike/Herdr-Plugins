//! The menu's built-in entries: updating Herdr, and opening the settings.
//! Both run in the `run` popup, which shows what they print.

use crate::config::{self, Action};
use crate::herdr;
use crate::launch;
use serde_json::Value;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The sidebar plugin, whose settings the `Réglages` entry opens too.
const SIDEBAR_ID: &str = "imsike.sidebar";
/// The sidebar's settings file, in its config directory.
const SIDEBAR_FILE: &str = "sidebar.toml";
/// What a new `menu.toml` holds: every setting, commented out.
const MENU_TEMPLATE: &str = include_str!("../templates/menu.toml");
/// What a new `sidebar.toml` holds.
const SIDEBAR_TEMPLATE: &str = include_str!("../templates/sidebar.toml");

pub fn run(action: Action) -> Result<(), String> {
    match action {
        Action::UpdateHerdr => update_herdr(),
        Action::Settings => settings(),
    }
}

/// The `herdr` program: the one that runs this Herdr, else the one on `PATH`.
fn herdr_command() -> Command {
    let program = std::env::var_os("HERDR_BIN_PATH").unwrap_or_else(|| "herdr".into());
    Command::new(program)
}

/// Runs `herdr <args>` with its output on screen.
fn herdr_shown(args: &[&str]) -> Result<(), String> {
    let status = herdr_command()
        .args(args)
        .status()
        .map_err(|e| format!("impossible de lancer « herdr {} » : {e}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("« herdr {} » a échoué ({status})", args.join(" ")))
    }
}

/// What `herdr <args>` prints.
fn herdr_output(args: &[&str]) -> Result<String, String> {
    let output = herdr_command()
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("impossible de lancer « herdr {} » : {e}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "« herdr {} » a échoué : {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `herdr status --json`.
fn status() -> Result<Value, String> {
    let text = herdr_output(&["status", "--json"])?;
    serde_json::from_str(&text).map_err(|e| format!("état de Herdr illisible : {e}"))
}

/// Whether the server runs an older binary than the one installed.
fn restart_needed(status: &Value) -> bool {
    ["restart_needed", "server_binary_stale"]
        .iter()
        .any(|key| status["update"][key] == true || status["server"][key] == true)
}

/// Installs the latest Herdr. When the server must restart to run it, says
/// which agents will stop and asks first; then puts `herdr` in the clipboard,
/// to start it again, and stops the server.
fn update_herdr() -> Result<(), String> {
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
        crate::pause("\nHerdr est à jour. Appuyer sur Entrée pour fermer.");
        return Ok(());
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
    if !confirm("Redémarrer maintenant ? [o/N] ") {
        crate::pause(
            "\nRedémarrage annulé : la nouvelle version servira au prochain \
             lancement de Herdr. Appuyer sur Entrée pour fermer.",
        );
        return Ok(());
    }
    let relaunch = if copy_to_clipboard("herdr") {
        "« herdr » est copié dans le presse-papiers : le coller dans le terminal relance Herdr."
    } else {
        "Relancer Herdr ensuite avec « herdr »."
    };
    crate::pause(&format!(
        "\n{relaunch}\nAppuyer sur Entrée pour arrêter le serveur."
    ));
    herdr_shown(&["server", "stop"])
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

/// Asks a yes-or-no question; anything but yes is no.
fn confirm(question: &str) -> bool {
    print!("{question}");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(
        answer.trim().to_lowercase().as_str(),
        "o" | "oui" | "y" | "yes"
    )
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

/// Opens Herdr's `config.toml`, the menu's `menu.toml` and, when the sidebar
/// is installed, its `sidebar.toml` in an editor; the plugins' files start
/// from a commented template. Then checks and reloads Herdr's configuration.
fn settings() -> Result<(), String> {
    let mut files = vec![herdr_config_path()];
    if let Some(dir) = crate::config_dir() {
        let menu = dir.join(config::FILE_NAME);
        create_if_missing(&menu, MENU_TEMPLATE)?;
        files.push(menu);
    }
    if herdr::has_plugin(SIDEBAR_ID).unwrap_or(false) {
        let dir = herdr_output(&["plugin", "config-dir", SIDEBAR_ID])?;
        let sidebar = PathBuf::from(dir.trim()).join(SIDEBAR_FILE);
        create_if_missing(&sidebar, SIDEBAR_TEMPLATE)?;
        files.push(sidebar);
    }
    edit(&files)?;
    // The menu reads its file when it opens: say now what it will refuse.
    if let Some(menu) = files.iter().find(|f| f.ends_with(config::FILE_NAME))
        && let Ok(text) = std::fs::read_to_string(menu)
        && let Err(e) = config::parse(&text)
    {
        return Err(format!("{} ignoré : {e}", config::FILE_NAME));
    }
    herdr_shown(&["config", "check"])?;
    herdr_shown(&["server", "reload-config"])
}

/// Herdr's `config.toml`: the path Herdr's help gives, which follows
/// `HERDR_CONFIG_PATH`; its usual place otherwise.
fn herdr_config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("HERDR_CONFIG_PATH") {
        return path.into();
    }
    if let Ok(help) = herdr_output(&["--help"])
        && let Some(path) = help
            .lines()
            .find_map(|line| line.strip_prefix("Config:"))
            .map(str::trim)
            .filter(|path| !path.is_empty())
    {
        return path.into();
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")))
    };
    base.unwrap_or_default().join("herdr").join("config.toml")
}

fn create_if_missing(path: &Path, template: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    }
    herdr_api::write_atomic(path, template.as_bytes())
        .map_err(|e| format!("{} : {e}", path.display()))
}

/// Opens the files and waits until the editor is closed: `VISUAL` or
/// `EDITOR` when set; else VS Code, or Notepad one file after the other, on
/// Windows; nano, else vi, elsewhere.
fn edit(files: &[PathBuf]) -> Result<(), String> {
    let configured = ["VISUAL", "EDITOR"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|v| v.to_string_lossy().into_owned())
        .find(|v| !v.trim().is_empty());
    let editor: Vec<String> = match configured {
        Some(editor) => editor.split_whitespace().map(str::to_string).collect(),
        None if cfg!(windows) => {
            if launch::find_on_path("code").is_some() {
                vec!["code".into(), "--wait".into()]
            } else {
                for file in files {
                    open_in(&["notepad".into()], std::slice::from_ref(file))?;
                }
                return Ok(());
            }
        }
        None if launch::find_on_path("nano").is_some() => vec!["nano".into()],
        None => vec!["vi".into()],
    };
    open_in(&editor, files)
}

fn open_in(editor: &[String], files: &[PathBuf]) -> Result<(), String> {
    let mut command = launch::command(editor)?;
    command.args(files);
    let status = command
        .status()
        .map_err(|e| format!("impossible de lancer l'éditeur « {} » : {e}", editor[0]))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("l'éditeur « {} » a échoué ({status})", editor[0]))
    }
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

    #[test]
    fn templates_parse() {
        let menu = config::parse(MENU_TEMPLATE).unwrap();
        assert_eq!(menu.entries, config::defaults());
        assert!(toml::from_str::<toml::Table>(SIDEBAR_TEMPLATE).is_ok());
    }
}
