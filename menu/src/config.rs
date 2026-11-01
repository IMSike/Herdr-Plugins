//! The menu's settings and entries: read from `menu.toml` in the plugin's
//! config directory, or built in.

use herdr_api::NumberFormat;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// File name of the user's menu, in `HERDR_PLUGIN_CONFIG_DIR`.
pub const FILE_NAME: &str = "menu.toml";

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

/// What the menu does itself, rather than start a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// Updates Herdr, and restarts its server once confirmed.
    UpdateHerdr,
    /// Opens the settings files in an editor, then reloads Herdr's.
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Single character that opens the entry from the menu.
    pub key: char,
    pub label: String,
    /// Program and its arguments, run without a shell; empty for an `action`.
    #[serde(default)]
    pub command: Vec<String>,
    /// A built-in action instead of a command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
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

    /// Whether the entry runs in the popup: an action always does.
    pub fn in_popup(&self) -> bool {
        self.popup || self.action.is_some()
    }
}

/// What `menu.toml` sets.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub entries: Vec<Entry>,
    /// How the `Tools` pane writes its figures.
    pub numbers: NumberFormat,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            entries: defaults(),
            numbers: NumberFormat::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    entries: Vec<Entry>,
    #[serde(default = "yes")]
    decimal_comma: bool,
    #[serde(default)]
    percent_space: bool,
}

/// Entries shown when the user has no `menu.toml`, or one without entries.
/// The icons are Material Design glyphs of Nerd Fonts, drawn at the same size.
pub fn defaults() -> Vec<Entry> {
    let entry = |key, label: &str, command: &[&str], icon: &str, popup| Entry {
        key,
        label: label.into(),
        command: command.iter().map(|s| s.to_string()).collect(),
        action: None,
        cwd: StartIn::Workspace,
        icon: Some(icon.into()),
        popup,
    };
    let action = |key, label: &str, action, icon: &str| Entry {
        action: Some(action),
        ..entry(key, label, &[], icon, true)
    };
    vec![
        // nf-md-git
        entry('l', "lazygit", &["lazygit"], "\u{F02A2}", true),
        // nf-md-github
        entry('g', "gh-dash", &["gh", "dash"], "\u{F02A4}", true),
        // nf-md-microsoft_visual_studio_code
        entry('c', "VS Code", &["code", "."], "\u{F0A1E}", false),
        // nf-md-update
        action('u', "Herdr", Action::UpdateHerdr, "\u{F06B0}"),
        // nf-md-cog
        action('s', "Réglages", Action::Settings, "\u{F0493}"),
    ]
}

/// Parses a `menu.toml` and checks its entries. A file without entries
/// keeps the built-in ones.
pub fn parse(text: &str) -> Result<Menu, String> {
    let file: File = toml::from_str(text).map_err(|e| e.message().to_string())?;
    for (i, entry) in file.entries.iter().enumerate() {
        match (entry.command.is_empty(), entry.action.is_some()) {
            (true, false) => return Err(format!("entrée « {} » : commande vide", entry.label)),
            (false, true) => {
                return Err(format!(
                    "entrée « {} » : command et action à la fois",
                    entry.label
                ));
            }
            _ => {}
        }
        if file.entries[..i].iter().any(|e| e.key == entry.key) {
            return Err(format!("touche « {} » utilisée deux fois", entry.key));
        }
    }
    let entries = if file.entries.is_empty() {
        defaults()
    } else {
        file.entries
    };
    Ok(Menu {
        entries,
        numbers: NumberFormat {
            decimal_comma: file.decimal_comma,
            percent_space: file.percent_space,
        },
    })
}

/// The user's menu, the built-in one when the file does not exist, and the
/// built-in one with an error message when it cannot be used.
pub fn load(config_dir: Option<&Path>) -> (Menu, Option<String>) {
    let Some(dir) = config_dir else {
        return (Menu::default(), None);
    };
    match std::fs::read_to_string(dir.join(FILE_NAME)) {
        Ok(text) => match parse(&text) {
            Ok(menu) => (menu, None),
            Err(e) => (Menu::default(), Some(format!("{FILE_NAME} ignoré : {e}"))),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Menu::default(), None),
        Err(e) => (
            Menu::default(),
            Some(format!("{FILE_NAME} illisible : {e}")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_with_default_cwd() {
        let menu = parse(
            r#"
            percent_space = true

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

            [[entries]]
            key = "u"
            label = "Herdr"
            action = "update-herdr"
            popup = false
            "#,
        )
        .unwrap();
        let entries = &menu.entries;
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].cwd, StartIn::Workspace);
        assert_eq!(entries[1].cwd, StartIn::Pane);
        assert_eq!(entries[0].icon(), "l");
        assert_eq!(entries[1].icon(), "X");
        assert!(entries[0].popup && !entries[1].popup);
        assert_eq!(entries[2].action, Some(Action::UpdateHerdr));
        assert!(entries[2].in_popup());
        assert!(menu.numbers.decimal_comma && menu.numbers.percent_space);
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
        assert!(
            parse(
                "[[entries]]\nkey = \"a\"\nlabel = \"a\"\ncommand = [\"a\"]\naction = \"settings\""
            )
            .is_err()
        );
    }

    #[test]
    fn a_file_without_entries_keeps_the_built_in_ones() {
        let menu = parse("# Only comments.\ndecimal_comma = false\n").unwrap();
        assert_eq!(menu.entries, defaults());
        assert!(!menu.numbers.decimal_comma);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = std::env::temp_dir().join("herdr-menu-test-missing");
        let (menu, error) = load(Some(&dir));
        assert_eq!(menu, Menu::default());
        assert!(error.is_none());
    }
}
