//! The settings screen, in its popup: the settings of the menu and of the
//! sidebar, each written to its file as soon as it changes; the
//! configuration files opened in an editor; Herdr's configuration reloaded.

pub mod files;
mod store;

use crate::config::{self, Layout, Menu};
use crate::ui::{Screen, fit};
use crossterm::cursor::MoveTo;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use files::Files;
use herdr_api::sidebar;
use std::io::{self, Write};
use std::path::Path;
use toml_edit::Value;

/// The `Tools` pane the screen was opened from, if any.
pub const BAR_ENV: &str = "HERDR_MENU_BAR";
/// First row of the list.
const TOP: u16 = 2;
/// Width of a setting's name, before its value.
const NAME_WIDTH: usize = 34;
/// A step of `ttl_seconds`, and its bounds.
const TTL_STEP: u64 = 5;
const TTL_MAX: u64 = 600;

/// What a row of the screen holds.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Row {
    Header(&'static str),
    Blank,
    Item(Item),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Item {
    DecimalComma,
    PercentSpace,
    Layout,
    Ttl,
    Shells,
    TabBar,
    OpenFiles,
    Reload,
}

/// The sidebar's settings the screen shows.
#[derive(Debug, Clone, PartialEq)]
struct SidebarValues {
    ttl_seconds: u64,
    shells: Vec<String>,
    tab_bar: bool,
}

impl SidebarValues {
    /// What `sidebar.toml` sets, its defaults for the rest.
    fn read(path: &Path) -> (Self, Option<String>) {
        let mut values = Self {
            ttl_seconds: sidebar::TTL_SECONDS,
            shells: sidebar::SHELLS.iter().map(|s| s.to_string()).collect(),
            tab_bar: false,
        };
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (values, None),
            Err(e) => {
                return (
                    values,
                    Some(format!("{} illisible : {e}", sidebar::SETTINGS_FILE)),
                );
            }
        };
        let table: toml::Table = match toml::from_str(&text) {
            Ok(table) => table,
            Err(e) => {
                let error = format!("{} invalide : {}", sidebar::SETTINGS_FILE, e.message());
                return (values, Some(error));
            }
        };
        if let Some(ttl) = table.get("ttl_seconds").and_then(toml::Value::as_integer) {
            values.ttl_seconds = ttl.max(1) as u64;
        }
        if let Some(shells) = table.get("shells").and_then(toml::Value::as_array) {
            values.shells = shells
                .iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect();
        }
        if let Some(tab_bar) = table.get("tab_bar").and_then(toml::Value::as_bool) {
            values.tab_bar = tab_bar;
        }
        (values, None)
    }
}

struct State {
    files: Files,
    menu: Menu,
    /// `None` when the sidebar is not installed.
    sidebar: Option<SidebarValues>,
    selected: usize,
    /// The last thing done, or why it failed (in red).
    status: Option<(String, bool)>,
    /// The shells being typed.
    typing: Option<String>,
}

impl State {
    fn rows(&self) -> Vec<Row> {
        let mut rows = vec![
            Row::Header("Format des nombres"),
            Row::Item(Item::DecimalComma),
            Row::Item(Item::PercentSpace),
            Row::Header("Pane Tools"),
            Row::Item(Item::Layout),
        ];
        if self.sidebar.is_some() {
            rows.extend([
                Row::Header("Sidebar"),
                Row::Item(Item::Ttl),
                Row::Item(Item::Shells),
                Row::Item(Item::TabBar),
            ]);
        }
        rows.extend([
            Row::Blank,
            Row::Item(Item::OpenFiles),
            Row::Item(Item::Reload),
        ]);
        rows
    }

    fn items(&self) -> Vec<Item> {
        self.rows()
            .into_iter()
            .filter_map(|row| match row {
                Row::Item(item) => Some(item),
                _ => None,
            })
            .collect()
    }

    fn name_and_value(&self, item: Item) -> (&'static str, String) {
        let yes_no = |b: bool| if b { "oui" } else { "non" }.to_string();
        let sidebar = self.sidebar.as_ref();
        match item {
            Item::DecimalComma => (
                "Virgule décimale (2,4G)",
                yes_no(self.menu.numbers.decimal_comma),
            ),
            Item::PercentSpace => (
                "Espace avant % (4 %)",
                yes_no(self.menu.numbers.percent_space),
            ),
            Item::Layout => (
                "Icônes",
                match self.menu.layout {
                    Layout::Single => "une ligne, nom au survol",
                    Layout::Double => "deux lignes, avec les noms",
                }
                .into(),
            ),
            Item::Ttl => (
                "Durée de vie des valeurs",
                format!("{} s", sidebar.map_or(0, |s| s.ttl_seconds)),
            ),
            Item::Shells => (
                "Shells",
                match &self.typing {
                    Some(text) => format!("{text}▏"),
                    None => sidebar.map_or_else(String::new, |s| s.shells.join(", ")),
                },
            ),
            Item::TabBar => (
                "Totaux dans la barre d'onglets",
                yes_no(sidebar.is_some_and(|s| s.tab_bar)),
            ),
            Item::OpenFiles => ("Ouvrir les fichiers de configuration…", String::new()),
            Item::Reload => ("Recharger la configuration de Herdr", String::new()),
        }
    }

    /// Writes `key` to `menu.toml`.
    fn set_menu(&self, key: &str, value: Value) -> Result<(), String> {
        let path = self
            .files
            .menu
            .as_ref()
            .ok_or("dossier de configuration du menu inconnu")?;
        store::set(path, files::MENU_TEMPLATE, key, value)
    }

    /// Writes `key` to `sidebar.toml`, when the sidebar is installed.
    fn set_sidebar(&self, key: &str, value: Value) -> Result<(), String> {
        match &self.files.sidebar {
            Some(path) => store::set(path, files::SIDEBAR_TEMPLATE, key, value),
            None => Ok(()),
        }
    }

    /// Changes `item` by one step (`forward` or back); `Some(true)` when the
    /// screen should close.
    fn change(&mut self, item: Item, forward: bool) -> Option<bool> {
        let saved = |files: &[&str]| format!("enregistré dans {}", files.join(" et "));
        let both: Vec<&str> = std::iter::once(config::FILE_NAME)
            .chain(self.sidebar.as_ref().map(|_| sidebar::SETTINGS_FILE))
            .collect();
        let result = match item {
            Item::DecimalComma => {
                let value = !self.menu.numbers.decimal_comma;
                self.set_menu("decimal_comma", value.into())
                    .and_then(|()| self.set_sidebar("decimal_comma", value.into()))
                    .map(|()| {
                        self.menu.numbers.decimal_comma = value;
                        saved(&both)
                    })
            }
            Item::PercentSpace => {
                let value = !self.menu.numbers.percent_space;
                self.set_menu("percent_space", value.into())
                    .and_then(|()| self.set_sidebar("percent_space", value.into()))
                    .map(|()| {
                        self.menu.numbers.percent_space = value;
                        saved(&both)
                    })
            }
            Item::Layout => {
                let value = match self.menu.layout {
                    Layout::Single => Layout::Double,
                    Layout::Double => Layout::Single,
                };
                self.set_menu("layout", value.name().into()).map(|()| {
                    self.menu.layout = value;
                    saved(&[config::FILE_NAME])
                })
            }
            Item::Ttl => {
                let current = self.sidebar.as_ref()?.ttl_seconds;
                let value = if forward {
                    (current / TTL_STEP + 1) * TTL_STEP
                } else {
                    current.saturating_sub(1) / TTL_STEP * TTL_STEP
                }
                .clamp(TTL_STEP, TTL_MAX);
                self.set_sidebar("ttl_seconds", (value as i64).into())
                    .map(|()| {
                        if let Some(s) = &mut self.sidebar {
                            s.ttl_seconds = value;
                        }
                        saved(&[sidebar::SETTINGS_FILE])
                    })
            }
            Item::TabBar => {
                let value = !self.sidebar.as_ref()?.tab_bar;
                self.set_sidebar("tab_bar", value.into()).map(|()| {
                    if let Some(s) = &mut self.sidebar {
                        s.tab_bar = value;
                    }
                    saved(&[sidebar::SETTINGS_FILE])
                })
            }
            Item::Shells => {
                self.typing = Some(self.sidebar.as_ref()?.shells.join(", "));
                return None;
            }
            Item::OpenFiles => match files::open(&self.files) {
                Ok(_) => {
                    tell_the_bar();
                    return Some(true);
                }
                Err(e) => Err(e),
            },
            Item::Reload => files::reload(),
        };
        self.status = Some(match result {
            Ok(message) => (message, false),
            Err(e) => (e, true),
        });
        None
    }

    /// Saves the shells typed.
    fn save_shells(&mut self, text: &str) {
        let shells: Vec<String> = text
            .split([',', ' '])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        let array: toml_edit::Array = shells.iter().map(String::as_str).collect();
        self.status = Some(match self.set_sidebar("shells", array.into()) {
            Ok(()) => {
                if let Some(s) = &mut self.sidebar {
                    s.shells = shells;
                }
                (format!("enregistré dans {}", sidebar::SETTINGS_FILE), false)
            }
            Err(e) => (e, true),
        });
    }
}

/// The `Tools` pane the screen was opened from switches to its reload view.
fn tell_the_bar() {
    if let (Ok(bar), Ok(state)) = (std::env::var(BAR_ENV), herdr_api::state_dir()) {
        let _ = herdr_api::write_atomic(&files::opened_flag(&state, &bar), b"");
    }
}

/// `herdr-menu settings`, in its popup.
pub fn run() -> Result<(), String> {
    let files = Files::find();
    let (menu, menu_error) = config::load(crate::config_dir().as_deref());
    let (sidebar, sidebar_error) = match &files.sidebar {
        Some(path) => {
            let (values, error) = SidebarValues::read(path);
            (Some(values), error)
        }
        None => (None, None),
    };
    let mut state = State {
        files,
        menu,
        sidebar,
        selected: 0,
        status: menu_error.or(sidebar_error).map(|e| (e, true)),
        typing: None,
    };
    show(&mut state).map_err(|e| e.to_string())
}

fn show(state: &mut State) -> io::Result<()> {
    let _screen = Screen::enter()?;
    loop {
        draw(state)?;
        let items = state.items();
        let item = items[state.selected.min(items.len() - 1)];
        let event = event::read()?;
        if let Some(text) = &mut state.typing {
            if let Event::Key(key) = event
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Enter => {
                        let text = std::mem::take(text);
                        state.typing = None;
                        state.save_shells(&text);
                    }
                    KeyCode::Esc => state.typing = None,
                    KeyCode::Backspace => {
                        text.pop();
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        text.push(c)
                    }
                    _ => {}
                }
            }
            continue;
        }
        let close = match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(true),
                KeyCode::Esc | KeyCode::Char('q') => Some(true),
                KeyCode::Up | KeyCode::Char('k') => {
                    state.selected = (state.selected + items.len() - 1) % items.len();
                    None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    state.selected = (state.selected + 1) % items.len();
                    None
                }
                KeyCode::Left | KeyCode::Char('h') => state.change(item, false),
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
                    state.change(item, true)
                }
                _ => None,
            },
            Event::Mouse(mouse) => {
                let rows = state.rows();
                let hit = usize::from(mouse.row.wrapping_sub(TOP));
                let item_at = rows.get(hit).and_then(|row| match row {
                    Row::Item(item) => items.iter().position(|i| i == item),
                    _ => None,
                });
                match (mouse.kind, item_at) {
                    (MouseEventKind::Moved, Some(i)) => {
                        state.selected = i;
                        None
                    }
                    (MouseEventKind::Down(MouseButton::Left), Some(i)) => {
                        state.selected = i;
                        state.change(items[i], true)
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if close == Some(true) {
            return Ok(());
        }
    }
}

fn draw(state: &State) -> io::Result<()> {
    let (width, height) = terminal::size()?;
    let width = usize::from(width);
    let mut out = io::stdout();
    queue!(
        out,
        Clear(ClearType::All),
        MoveTo(1, 0),
        SetAttribute(Attribute::Bold),
        Print("Réglages"),
        SetAttribute(Attribute::Reset)
    )?;
    let items = state.items();
    let selected = items[state.selected.min(items.len() - 1)];
    for (i, row) in state.rows().into_iter().enumerate() {
        let y = TOP + i as u16;
        if y + 2 >= height {
            break;
        }
        match row {
            Row::Blank => {}
            Row::Header(title) => {
                queue!(
                    out,
                    MoveTo(1, y),
                    SetForegroundColor(Color::DarkGrey),
                    Print(fit(title, width.saturating_sub(1))),
                    ResetColor
                )?;
            }
            Row::Item(item) => {
                let (name, value) = state.name_and_value(item);
                let marker = if item == selected { "›" } else { " " };
                let line = format!(" {marker} {name:<NAME_WIDTH$}{value}");
                if item == selected {
                    queue!(out, SetForegroundColor(Color::Yellow))?;
                }
                queue!(out, MoveTo(0, y), Print(fit(&line, width)), ResetColor)?;
            }
        }
    }
    if let Some((status, error)) = &state.status {
        let color = if *error { Color::Red } else { Color::Green };
        queue!(
            out,
            MoveTo(1, height.saturating_sub(2)),
            SetForegroundColor(color),
            Print(fit(status, width.saturating_sub(1))),
            ResetColor
        )?;
    }
    let help = if state.typing.is_some() {
        " noms séparés par des virgules · Entrée valider · Échap annuler"
    } else {
        " ↑↓ choisir · ←→ Entrée changer · Échap fermer"
    };
    queue!(
        out,
        MoveTo(0, height.saturating_sub(1)),
        SetAttribute(Attribute::Dim),
        Print(fit(help, width)),
        SetAttribute(Attribute::Reset)
    )?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_sidebar_settings_with_their_defaults() {
        let dir = std::env::temp_dir().join(format!("herdr-menu-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(sidebar::SETTINGS_FILE);
        let (values, error) = SidebarValues::read(&path);
        assert!(error.is_none());
        assert_eq!(values.ttl_seconds, sidebar::TTL_SECONDS);
        std::fs::write(&path, "ttl_seconds = 30\nshells = [\"bash\"]\n").unwrap();
        let (values, _) = SidebarValues::read(&path);
        assert_eq!(
            (values.ttl_seconds, values.shells),
            (30, vec!["bash".into()])
        );
        assert!(!values.tab_bar);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
