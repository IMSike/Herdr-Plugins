//! The menu's settings and entries: read from `menu.toml` in the plugin's
//! config directory, or built in.

use herdr_api::{MemoryUnit, NumberFormat};
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
    /// The folder of the focused pane; from the `Tools` pane, of the tab's
    /// first terminal.
    Pane,
}

/// What the menu does itself, rather than start a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// Updates Herdr and the plugins installed from GitHub, in a terminal
    /// window of its own, outside Herdr. Not among the built-in entries:
    /// the settings screen offers it; still allowed in `menu.toml`.
    Update,
    /// Opens the settings screen.
    Settings,
    /// Shows the figures on the workspace folder (the `Tools` pane only).
    Disk,
}

/// How the `Tools` pane shows the sidebar's totals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TotalsStyle {
    /// One line: `6% 2,4G · 󰍹 35% 21,3G`.
    #[default]
    Line,
    /// Four rows, each a bar and its value.
    Bars,
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

    /// Whether the entry turns green when Herdr or a plugin has a new
    /// version: the settings, which hold the update, or an `update` entry.
    pub fn shows_news(&self) -> bool {
        matches!(self.action, Some(Action::Update | Action::Settings))
    }
}

/// What `menu.toml` sets.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub entries: Vec<Entry>,
    /// How the `Tools` pane writes its figures.
    pub numbers: NumberFormat,
    pub totals: TotalsStyle,
    /// Whether the `Update` entry turns green when Herdr or a plugin has a
    /// new version.
    pub check_updates: bool,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            entries: defaults(),
            numbers: NumberFormat::default(),
            totals: TotalsStyle::default(),
            check_updates: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct File {
    entries: Vec<Entry>,
    decimal_comma: bool,
    percent_space: bool,
    cpu_decimal: bool,
    memory_unit: MemoryUnit,
    memory_space: bool,
    totals: TotalsStyle,
    check_updates: bool,
    /// The icons' layout of earlier versions, which had two: still read so
    /// that a file that sets it stays valid, and ignored.
    #[serde(rename = "layout", skip_serializing)]
    _layout: Option<serde::de::IgnoredAny>,
}

impl Default for File {
    fn default() -> Self {
        let numbers = NumberFormat::default();
        Self {
            entries: Vec::new(),
            decimal_comma: numbers.decimal_comma,
            percent_space: numbers.percent_space,
            cpu_decimal: numbers.cpu_decimal,
            memory_unit: numbers.memory_unit,
            memory_space: numbers.memory_space,
            totals: TotalsStyle::default(),
            check_updates: true,
            _layout: None,
        }
    }
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
        // nf-md-harddisk
        action('d', "Disk", Action::Disk, "\u{F02CA}"),
        // nf-md-chart_line
        entry('b', "btop", &[btop()], "\u{F012A}", true),
        // nf-md-cog
        action('s', "Settings", Action::Settings, "\u{F0493}"),
    ]
}

/// The plugin's tool actions (`[[actions]]` in `herdr-plugin.toml`) and the
/// label of the entry each one starts.
const TOOL_ACTIONS: [(&str, &str); 4] = [
    ("lazygit", "lazygit"),
    ("gh-dash", "gh-dash"),
    ("vscode", "VS Code"),
    ("btop", "btop"),
];

/// The entry the tool action `id` (`HERDR_PLUGIN_ACTION_ID`: the manifest's
/// id, unqualified) starts: the menu's entry of the same label (letter case
/// aside), else the built-in one.
pub fn tool_entry(id: &str, entries: &[Entry]) -> Option<Entry> {
    let (_, label) = TOOL_ACTIONS.iter().find(|(action, _)| *action == id)?;
    let named = |e: &Entry| e.action.is_none() && e.label.eq_ignore_ascii_case(label);
    entries
        .iter()
        .find(|e| named(e))
        .cloned()
        .or_else(|| defaults().into_iter().find(|e| named(e)))
}

/// btop's program on this system (see `btop_among`).
fn btop() -> &'static str {
    btop_among(cfg!(windows), |name| {
        crate::launch::find_on_path(name).is_some()
    })
}

/// On Windows btop is btop4win: scoop installs it as `btop`, winget as
/// `btop` when it can make that link (Developer Mode or administrator),
/// and otherwise puts its folder on `PATH`, where it is `btop4win`.
/// `btop` when neither is `found`, for the message to name it.
fn btop_among(windows: bool, found: impl Fn(&str) -> bool) -> &'static str {
    if windows && !found("btop") && found("btop4win") {
        "btop4win"
    } else {
        "btop"
    }
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
            cpu_decimal: file.cpu_decimal,
            memory_unit: file.memory_unit,
            memory_space: file.memory_space,
        },
        totals: file.totals,
        check_updates: file.check_updates,
    })
}

/// The user's menu, the built-in one when the file does not exist, and the
/// built-in one with an error message when it cannot be used.
pub fn load(config_dir: Option<&Path>) -> (Menu, Option<String>) {
    let Some(dir) = config_dir else {
        return (Menu::default(), None);
    };
    match herdr_api::read_settings(&dir.join(FILE_NAME), FILE_NAME, parse) {
        Ok(menu) => (menu, None),
        Err(e) => (Menu::default(), Some(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_entry_shows_new_versions() {
        let entries = defaults();
        assert!(entries.iter().all(|e| e.action != Some(Action::Update)));
        let shown: Vec<&str> = entries
            .iter()
            .filter(|e| e.shows_news())
            .map(|e| e.label.as_str())
            .collect();
        assert_eq!(shown, ["Settings"]);
    }

    #[test]
    fn tool_actions_start_the_built_in_entries() {
        for (id, label) in TOOL_ACTIONS {
            let entry = tool_entry(id, &[]).expect(id);
            assert_eq!(entry.label, label);
            assert!(entry.action.is_none());
        }
        assert_eq!(tool_entry("open", &[]), None);
    }

    #[test]
    fn tool_action_prefers_the_menu_entry_of_its_name() {
        let menu = parse(
            r#"
            [[entries]]
            key = "x"
            label = "Lazygit"
            command = ["lazygit", "--debug"]
            cwd = "pane"
            "#,
        )
        .unwrap();
        let entry = tool_entry("lazygit", &menu.entries).unwrap();
        assert_eq!(entry.command, ["lazygit", "--debug"]);
        assert_eq!(entry.cwd, StartIn::Pane);
        // Not in the menu: the built-in entry.
        assert_eq!(
            tool_entry("btop", &menu.entries).map(|e| e.label),
            Some("btop".to_string())
        );
    }

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
            label = "Update"
            action = "update"
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
        assert_eq!(entries[2].action, Some(Action::Update));
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
        assert!(parse("totals = \"triple\"").is_err());
    }

    #[test]
    fn the_layout_of_earlier_versions_is_ignored() {
        for text in ["layout = \"double\"", "layout = \"single\"", "layout = 2"] {
            assert_eq!(parse(text).unwrap(), Menu::default(), "{text}");
        }
    }

    #[test]
    fn a_file_without_entries_keeps_the_built_in_ones() {
        let menu = parse("# Only comments.\ndecimal_comma = false\n").unwrap();
        assert_eq!(menu.entries, defaults());
        assert!(!menu.numbers.decimal_comma);
    }

    #[test]
    fn the_template_gives_the_defaults() {
        let template = crate::settings::files::MENU_TEMPLATE;
        assert_eq!(parse(template).unwrap(), Menu::default());
        // Its commented settings, before the entries, are the default values.
        let settings: String = template
            .lines()
            .take_while(|line| !line.contains("[[entries]]"))
            .filter_map(|line| line.strip_prefix("# "))
            .filter(|line| line.contains(" = "))
            .map(|line| format!("{line}\n"))
            .collect();
        assert_eq!(parse(&settings).unwrap(), Menu::default());
        // A line for every setting.
        let mut in_template: Vec<&str> = settings
            .lines()
            .filter_map(|line| line.split(" = ").next())
            .collect();
        in_template.sort_unstable();
        let file = serde_json::to_value(File::default()).unwrap();
        let mut keys: Vec<&str> = file
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .filter(|key| *key != "entries")
            .collect();
        keys.sort_unstable();
        assert_eq!(in_template, keys);
    }

    #[test]
    fn btop_is_btop4win_on_windows_only_when_only_it_is_found() {
        let only = |name: &'static str| move |n: &str| n == name;
        assert_eq!(btop_among(true, only("btop4win")), "btop4win");
        assert_eq!(btop_among(true, |_| true), "btop");
        assert_eq!(btop_among(true, only("btop")), "btop");
        assert_eq!(btop_among(true, |_| false), "btop");
        assert_eq!(btop_among(false, only("btop4win")), "btop");
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = std::env::temp_dir().join("herdr-menu-test-missing");
        let (menu, error) = load(Some(&dir));
        assert_eq!(menu, Menu::default());
        assert!(error.is_none());
    }
}
