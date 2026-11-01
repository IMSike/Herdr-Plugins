//! Menu entries: read from `menu.toml` in the plugin's config directory, or built in.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// File name of the user's menu, in `HERDR_PLUGIN_CONFIG_DIR`.
const FILE_NAME: &str = "menu.toml";

/// Directory a tool starts in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StartIn {
    /// The workspace's root folder.
    #[default]
    Workspace,
    /// The folder of the pane that had the focus.
    Pane,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Single character that opens the entry from the menu.
    pub key: char,
    pub label: String,
    /// Program and its arguments, run without a shell.
    pub command: Vec<String>,
    #[serde(default)]
    pub cwd: StartIn,
    /// What the bar shows for the entry (a Nerd Font glyph); its key without one.
    #[serde(default)]
    pub icon: Option<String>,
    /// `false` for a program that opens its own window: it starts on its own,
    /// without the popup.
    #[serde(default = "yes")]
    pub popup: bool,
}

fn yes() -> bool {
    true
}

impl Entry {
    pub fn icon(&self) -> String {
        self.icon.clone().unwrap_or_else(|| self.key.to_string())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    entries: Vec<Entry>,
}

/// Entries shown when the user has no `menu.toml`. The icons are Material
/// Design glyphs of Nerd Fonts, drawn at the same size.
pub fn defaults() -> Vec<Entry> {
    let entry = |key, label: &str, command: &[&str], icon: &str, popup| Entry {
        key,
        label: label.into(),
        command: command.iter().map(|s| s.to_string()).collect(),
        cwd: StartIn::Workspace,
        icon: Some(icon.into()),
        popup,
    };
    vec![
        // nf-md-git
        entry('l', "lazygit", &["lazygit"], "\u{F02A2}", true),
        // nf-md-github
        entry('g', "gh-dash", &["gh", "dash"], "\u{F02A4}", true),
        // nf-md-microsoft_visual_studio_code
        entry('c', "VS Code", &["code", "."], "\u{F0A1E}", false),
    ]
}

/// Parses a `menu.toml` and checks its entries.
pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let file: File = toml::from_str(text).map_err(|e| e.message().to_string())?;
    for (i, entry) in file.entries.iter().enumerate() {
        if entry.command.is_empty() {
            return Err(format!("entrée « {} » : commande vide", entry.label));
        }
        if file.entries[..i].iter().any(|e| e.key == entry.key) {
            return Err(format!("touche « {} » utilisée deux fois", entry.key));
        }
    }
    Ok(file.entries)
}

/// The user's entries, the built-in ones when the file does not exist, and the
/// built-in ones with an error message when it cannot be used.
pub fn load(config_dir: Option<&Path>) -> (Vec<Entry>, Option<String>) {
    let Some(dir) = config_dir else {
        return (defaults(), None);
    };
    match std::fs::read_to_string(dir.join(FILE_NAME)) {
        Ok(text) => match parse(&text) {
            Ok(entries) => (entries, None),
            Err(e) => (defaults(), Some(format!("{FILE_NAME} ignoré : {e}"))),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (defaults(), None),
        Err(e) => (defaults(), Some(format!("{FILE_NAME} illisible : {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_with_default_cwd() {
        let entries = parse(
            r#"
            [[entries]]
            key = "l"
            label = "lazygit"
            command = ["lazygit"]

            [[entries]]
            key = "t"
            label = "btop"
            command = ["btop"]
            cwd = "pane"
            icon = "X"
            popup = false
            "#,
        )
        .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].cwd, StartIn::Workspace);
        assert_eq!(entries[1].cwd, StartIn::Pane);
        assert_eq!(entries[0].icon(), "l");
        assert_eq!(entries[1].icon(), "X");
        assert!(entries[0].popup && !entries[1].popup);
    }

    #[test]
    fn rejects_duplicate_keys() {
        let err = parse(
            r#"
            [[entries]]
            key = "l"
            label = "a"
            command = ["a"]
            [[entries]]
            key = "l"
            label = "b"
            command = ["b"]
            "#,
        )
        .unwrap_err();
        assert!(err.contains("deux fois"));
    }

    #[test]
    fn rejects_empty_command_and_unknown_fields() {
        assert!(parse("[[entries]]\nkey = \"a\"\nlabel = \"a\"\ncommand = []").is_err());
        assert!(
            parse("[[entries]]\nkey = \"a\"\nlabel = \"a\"\ncommand = [\"a\"]\nfoo = 1").is_err()
        );
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = std::env::temp_dir().join("herdr-menu-test-missing");
        let (entries, error) = load(Some(&dir));
        assert_eq!(entries, defaults());
        assert!(error.is_none());
    }
}
