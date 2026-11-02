//! The configuration files the settings screen opens in an editor (Herdr's
//! `config.toml`, `menu.toml`, `sidebar.toml`), how it opens them without
//! holding Herdr, and the reload of Herdr's configuration.

use crate::{config, herdr, launch};
use herdr_api::sidebar;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

/// What a new `menu.toml` holds: every setting, commented out.
pub const MENU_TEMPLATE: &str = include_str!("../../templates/menu.toml");
/// What a new `sidebar.toml` holds.
pub const SIDEBAR_TEMPLATE: &str = sidebar::TEMPLATE;
/// The editor (as JSON) and the files (as JSON) handed to the `edit` pane.
const EDITOR_ENV: &str = "HERDR_MENU_EDITOR";
const FILES_ENV: &str = "HERDR_MENU_FILES";
/// Editors that open a window of their own: they start on their own. Any
/// other one runs in a terminal, in a Herdr tab.
const WINDOW_EDITORS: &[&str] = &[
    "code",
    "code-insiders",
    "codium",
    "cursor",
    "windsurf",
    "zed",
    "subl",
    "sublime_text",
    "notepad",
    "notepad++",
    "gedit",
    "gnome-text-editor",
    "kate",
    "kwrite",
    "mousepad",
    "gvim",
];

/// The plugins' settings files; Herdr's is only looked for when the files
/// are opened.
pub struct Files {
    /// `None` outside a plugin run (no config folder).
    pub menu: Option<PathBuf>,
    /// `None` when the sidebar is not installed.
    pub sidebar: Option<PathBuf>,
}

impl Files {
    pub fn find() -> Self {
        Self {
            menu: crate::config_dir().map(|dir| dir.join(config::FILE_NAME)),
            sidebar: sidebar_config_dir().map(|dir| dir.join(sidebar::SETTINGS_FILE)),
        }
    }

    /// All of them, Herdr's first; the plugins' created from their template
    /// when missing.
    fn prepared(&self) -> Result<Vec<PathBuf>, String> {
        let mut all = vec![herdr_config_path()];
        for (file, template) in [
            (&self.menu, MENU_TEMPLATE),
            (&self.sidebar, SIDEBAR_TEMPLATE),
        ] {
            if let Some(file) = file {
                create_if_missing(file, template)?;
                all.push(file.clone());
            }
        }
        Ok(all)
    }
}

/// What `herdr <args>` prints, when it succeeds.
fn herdr_output(args: &[&str]) -> Option<String> {
    let output = herdr::command(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The sidebar's config folder, when it is installed.
fn sidebar_config_dir() -> Option<PathBuf> {
    if !herdr::has_plugin(sidebar::ID).unwrap_or(false) {
        return None;
    }
    sidebar::config_dir().ok()
}

/// Herdr's `config.toml`: `HERDR_CONFIG_PATH`, else the path Herdr's help
/// gives, else its usual place.
fn herdr_config_path() -> PathBuf {
    if let Some(path) = std::env::var_os(herdr_api::env::CONFIG_PATH) {
        return path.into();
    }
    if let Some(help) = herdr_output(&["--help"])
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
    super::store::write(path, template)
}

/// How the files are opened.
#[derive(Debug, PartialEq)]
enum Editor {
    /// A program with a window of its own, all files at once.
    Window(Vec<String>),
    /// Notepad: one window per file.
    WindowPerFile(Vec<String>),
    /// A terminal editor, in a Herdr tab.
    Terminal(Vec<String>),
}

/// `VISUAL` or `EDITOR` when set; else VS Code or Notepad on Windows, nano
/// or vi elsewhere.
fn editor() -> Editor {
    let configured = ["VISUAL", "EDITOR"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|v| v.to_string_lossy().into_owned())
        .find(|v| !v.trim().is_empty());
    if let Some(editor) = configured {
        return classify(split_command(&editor));
    }
    if cfg!(windows) {
        if launch::find_on_path("code").is_some() {
            Editor::Window(vec!["code".into()])
        } else {
            Editor::WindowPerFile(vec!["notepad".into()])
        }
    } else if launch::find_on_path("nano").is_some() {
        Editor::Terminal(vec!["nano".into()])
    } else {
        Editor::Terminal(vec!["vi".into()])
    }
}

/// A command line split into its words; double quotes keep spaces in one
/// (`"C:\Program Files\Editor\editor.exe" --wait`).
fn split_command(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut started) = (false, false);
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

fn classify(argv: Vec<String>) -> Editor {
    let name = argv
        .first()
        .and_then(|p| Path::new(p).file_stem())
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match name.as_str() {
        "notepad" => Editor::WindowPerFile(argv),
        n if WINDOW_EDITORS.contains(&n) => Editor::Window(argv),
        _ => Editor::Terminal(argv),
    }
}

/// Opens the files in an editor without waiting for it; says where.
pub fn open(files: &Files) -> Result<String, String> {
    let paths = files.prepared()?;
    let with_files = |argv: &[String], files: &[PathBuf]| {
        let mut all: Vec<String> = argv.to_vec();
        all.extend(files.iter().map(|f| f.to_string_lossy().into_owned()));
        all
    };
    match editor() {
        Editor::Window(argv) => {
            launch::spawn_detached(&with_files(&argv, &paths), None)?;
            Ok(format!("fichiers ouverts dans {}", argv[0]))
        }
        Editor::WindowPerFile(argv) => {
            for path in &paths {
                launch::spawn_detached(&with_files(&argv, std::slice::from_ref(path)), None)?;
            }
            Ok(format!("fichiers ouverts dans {}", argv[0]))
        }
        Editor::Terminal(argv) => {
            // The settings popup is still on screen: a process of its own
            // opens the tab once it is gone.
            let me = std::env::current_exe().map_err(|e| e.to_string())?;
            let editor = serde_json::to_string(&argv).map_err(|e| e.to_string())?;
            let files = serde_json::to_string(&paths).map_err(|e| e.to_string())?;
            let mut command = Command::new(me);
            command
                .arg("edit-tab")
                .env(EDITOR_ENV, editor)
                .env(FILES_ENV, files);
            launch::detach_command(command)?;
            Ok(format!("fichiers ouverts dans {}, dans un onglet", argv[0]))
        }
    }
}

/// `herdr-menu edit-tab`: opens the `edit` pane as a tab, once the
/// settings popup is gone.
pub fn open_tab() -> Result<(), String> {
    let editor = herdr_api::var(EDITOR_ENV)?;
    let files = herdr_api::var(FILES_ENV)?;
    herdr::open_pane_when_free("edit", &[(EDITOR_ENV, &editor), (FILES_ENV, &files)])
        .map_err(|e| e.to_string())
}

/// `herdr-menu edit`, in its tab: the editor, then the reload of Herdr's
/// configuration when asked for.
pub fn edit_in_tab() -> Result<(), String> {
    let editor: Vec<String> = serde_json::from_str(&herdr_api::var(EDITOR_ENV)?)
        .map_err(|e| format!("{EDITOR_ENV} illisible : {e}"))?;
    let files: Vec<PathBuf> = serde_json::from_str(&herdr_api::var(FILES_ENV)?)
        .map_err(|e| format!("{FILES_ENV} illisible : {e}"))?;
    let mut command = launch::command(&editor)?;
    command.args(&files);
    command
        .status()
        .map_err(|e| format!("impossible de lancer l'éditeur « {} » : {e}", editor[0]))?;
    if crate::ask("Recharger la configuration de Herdr ? [O/n] ", true) {
        let message = reload().unwrap_or_else(|e| e);
        crate::pause(&message);
    }
    Ok(())
}

/// Reloads Herdr's configuration; says how it went. `Err` when Herdr
/// refused it: the previous configuration stays.
pub fn reload() -> Result<String, String> {
    let result =
        herdr_api::request("server.reload_config", json!({})).map_err(|e| e.to_string())?;
    let diagnostics: Vec<String> = result["diagnostics"]
        .as_array()
        .map(|all| all.iter().map(diagnostic).collect())
        .unwrap_or_default();
    if result["status"] != "applied" {
        return Err(format!(
            "config.toml refusé : {}",
            diagnostics
                .first()
                .map_or("raison inconnue", String::as_str)
        ));
    }
    Ok(match diagnostics.first() {
        Some(first) => format!("configuration rechargée, avec : {first}"),
        None => "configuration de Herdr rechargée".into(),
    })
}

fn diagnostic(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| value.to_string())
}

/// Left by the settings screen for the `Tools` pane it was opened from: the
/// files are open, the pane offers to reload.
pub fn opened_flag(state_dir: &Path, bar: &str) -> PathBuf {
    state_dir.join(format!("files-open-{}", bar.replace(':', "-")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_an_editor_command_with_quotes() {
        assert_eq!(split_command("code --wait"), ["code", "--wait"]);
        assert_eq!(
            split_command(r#""C:\Program Files\Ed\ed.exe" -n  "" x"#),
            [r"C:\Program Files\Ed\ed.exe", "-n", "", "x"]
        );
        assert!(split_command("  ").is_empty());
    }

    #[test]
    fn tells_window_editors_from_terminal_ones() {
        let argv = |s: &str| s.split(' ').map(str::to_string).collect::<Vec<_>>();
        assert_eq!(
            classify(argv("code --wait")),
            Editor::Window(argv("code --wait"))
        );
        assert_eq!(
            classify(argv("C:\\Tools\\Notepad.exe")),
            Editor::WindowPerFile(argv("C:\\Tools\\Notepad.exe"))
        );
        assert_eq!(classify(argv("nvim")), Editor::Terminal(argv("nvim")));
        assert_eq!(
            opened_flag(Path::new("s"), "w1:p2"),
            Path::new("s").join("files-open-w1-p2")
        );
    }
}
