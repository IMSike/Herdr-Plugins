//! The settings screen, in its popup: the settings of the menu and of the
//! sidebar, each written to its file as soon as it changes; the
//! configuration files opened in an editor; Herdr's configuration reloaded.

pub mod files;
mod store;

use crate::config::{self, Layout, Menu, TotalsStyle};
use crate::ui::{Screen, fit};
use crossterm::cursor::MoveTo;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use files::Files;
use herdr_api::sidebar::{self, Thresholds, check_thresholds};
use serde::Deserialize;
use std::io::{self, Write};
use std::path::Path;
use toml_edit::Value;

/// The `Tools` pane the screen was opened from, if any.
pub const BAR_ENV: &str = "HERDR_MENU_BAR";
/// First row of the list.
const TOP: u16 = 2;
/// Rows under the list: the status and the help.
const BOTTOM: u16 = 2;
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
    CpuDecimal,
    MemoryUnit,
    MemorySpace,
    Layout,
    Totals,
    Ttl,
    Shells,
    TabBar,
    Colors,
    Levels(Levels),
    OpenFiles,
    Reload,
}

/// One of the sidebar's sets of thresholds.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Levels {
    Cpu,
    Memory,
    MachineCpu,
    MachineMemory,
}

impl Levels {
    const ALL: [Self; 4] = [
        Self::Cpu,
        Self::Memory,
        Self::MachineCpu,
        Self::MachineMemory,
    ];

    /// Its setting in `sidebar.toml`.
    fn key(self) -> &'static str {
        match self {
            Self::Cpu => "cpu_levels",
            Self::Memory => "memory_levels",
            Self::MachineCpu => "machine_cpu_levels",
            Self::MachineMemory => "machine_memory_levels",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Cpu => "Seuils CPU (Workspaces)",
            Self::Memory => "Seuils mémoire (Workspaces)",
            Self::MachineCpu => "Seuils CPU (machine)",
            Self::MachineMemory => "Seuils mémoire (machine)",
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Self::Memory => "Go",
            _ => "%",
        }
    }
}

/// The sidebar's settings the screen shows; what `sidebar.toml` sets, the
/// defaults for the rest.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
struct SidebarValues {
    ttl_seconds: u64,
    shells: Vec<String>,
    tab_bar: bool,
    colors: bool,
    cpu_levels: Thresholds,
    memory_levels: Thresholds,
    machine_cpu_levels: Thresholds,
    machine_memory_levels: Thresholds,
}

impl Default for SidebarValues {
    fn default() -> Self {
        Self {
            ttl_seconds: sidebar::TTL_SECONDS,
            shells: sidebar::SHELLS.iter().map(|s| s.to_string()).collect(),
            tab_bar: false,
            colors: false,
            cpu_levels: sidebar::CPU_LEVELS,
            memory_levels: sidebar::MEMORY_LEVELS,
            machine_cpu_levels: sidebar::MACHINE_CPU_LEVELS,
            machine_memory_levels: sidebar::MACHINE_MEMORY_LEVELS,
        }
    }
}

impl SidebarValues {
    /// The values of `path`, the defaults when it cannot be read (and why).
    fn read(path: &Path) -> (Self, Option<String>) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Self::default(), None),
            Err(e) => {
                let error = format!("{} illisible : {e}", sidebar::SETTINGS_FILE);
                return (Self::default(), Some(error));
            }
        };
        match toml::from_str::<Self>(&text) {
            Ok(values) => (values, None),
            Err(e) => {
                let error = format!("{} invalide : {}", sidebar::SETTINGS_FILE, e.message());
                (Self::default(), Some(error))
            }
        }
    }

    fn levels(&self, which: Levels) -> &Thresholds {
        match which {
            Levels::Cpu => &self.cpu_levels,
            Levels::Memory => &self.memory_levels,
            Levels::MachineCpu => &self.machine_cpu_levels,
            Levels::MachineMemory => &self.machine_memory_levels,
        }
    }

    fn levels_mut(&mut self, which: Levels) -> &mut Thresholds {
        match which {
            Levels::Cpu => &mut self.cpu_levels,
            Levels::Memory => &mut self.memory_levels,
            Levels::MachineCpu => &mut self.machine_cpu_levels,
            Levels::MachineMemory => &mut self.machine_memory_levels,
        }
    }
}

/// `10`, `2.5`: a threshold as it is typed and written.
fn number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

/// Three thresholds typed with spaces between them (`10 25 50`), a comma
/// or a point before decimals.
fn parse_levels(text: &str) -> Result<Thresholds, String> {
    let values: Vec<f64> = text
        .split_whitespace()
        .map(|v| v.replace(',', ".").parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| "seuils : des nombres séparés par des espaces")?;
    let thresholds: Thresholds = values
        .try_into()
        .map_err(|_| "seuils : trois valeurs, séparées par des espaces")?;
    check_thresholds(&thresholds)?;
    Ok(thresholds)
}

struct State {
    files: Files,
    menu: Menu,
    /// `None` when the sidebar is not installed.
    sidebar: Option<SidebarValues>,
    selected: usize,
    /// The last thing done, or why it failed (in red).
    status: Option<(String, bool)>,
    /// The text being typed for the selected item (shells, thresholds).
    typing: Option<String>,
}

impl State {
    fn rows(&self) -> Vec<Row> {
        let mut rows = vec![
            Row::Header("Format des nombres"),
            Row::Item(Item::DecimalComma),
            Row::Item(Item::PercentSpace),
            Row::Item(Item::CpuDecimal),
            Row::Item(Item::MemoryUnit),
            Row::Item(Item::MemorySpace),
            Row::Header("Pane Tools"),
            Row::Item(Item::Layout),
            Row::Item(Item::Totals),
        ];
        if self.sidebar.is_some() {
            rows.extend([
                Row::Header("Sidebar"),
                Row::Item(Item::Ttl),
                Row::Item(Item::Shells),
                Row::Item(Item::TabBar),
                Row::Header("Couleurs par seuil (vert, jaune, orange, rouge)"),
                Row::Item(Item::Colors),
            ]);
            rows.extend(Levels::ALL.map(|l| Row::Item(Item::Levels(l))));
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

    fn name_and_value(&self, item: Item, selected: bool) -> (&'static str, String) {
        let yes_no = |b: bool| if b { "oui" } else { "non" }.to_string();
        let sidebar = self.sidebar.as_ref();
        let typed = self.typing.as_ref().filter(|_| selected);
        let numbers = &self.menu.numbers;
        match item {
            Item::DecimalComma => ("Virgule décimale (2,4G)", yes_no(numbers.decimal_comma)),
            Item::PercentSpace => ("Espace avant % (4 %)", yes_no(numbers.percent_space)),
            Item::CpuDecimal => (
                "CPU avec une décimale",
                if numbers.cpu_decimal { "4,2%" } else { "4%" }.into(),
            ),
            Item::MemoryUnit => (
                "Unités de la mémoire",
                match numbers.memory_unit {
                    herdr_api::MemoryUnit::M => "M, G",
                    herdr_api::MemoryUnit::Mo => "Mo, Go",
                    herdr_api::MemoryUnit::MB => "MB, GB",
                }
                .into(),
            ),
            Item::MemorySpace => (
                "Espace avant l'unité",
                numbers.size(2_576_980_378).replace('\u{202F}', " "),
            ),
            Item::Layout => (
                "Icônes",
                match self.menu.layout {
                    Layout::Single => "une ligne, nom au survol",
                    Layout::Double => "deux lignes, avec les noms",
                }
                .into(),
            ),
            Item::Totals => (
                "Totaux CPU / mémoire",
                match self.menu.totals {
                    TotalsStyle::Line => "une ligne",
                    TotalsStyle::Bars => "barres",
                }
                .into(),
            ),
            Item::Ttl => (
                "Durée de vie des valeurs",
                format!("{} s", sidebar.map_or(0, |s| s.ttl_seconds)),
            ),
            Item::Shells => (
                "Shells",
                match typed {
                    Some(text) => format!("{text}▏"),
                    None => sidebar.map_or_else(String::new, |s| s.shells.join(", ")),
                },
            ),
            Item::TabBar => (
                "Totaux dans la barre d'onglets",
                yes_no(sidebar.is_some_and(|s| s.tab_bar)),
            ),
            Item::Colors => ("Couleurs", yes_no(sidebar.is_some_and(|s| s.colors))),
            Item::Levels(which) => (
                which.name(),
                match typed {
                    Some(text) => format!("{text}▏"),
                    None => sidebar.map_or_else(String::new, |s| {
                        let values: Vec<String> =
                            s.levels(which).iter().map(|v| number(*v)).collect();
                        format!("{} {}", values.join(" · "), which.unit())
                    }),
                },
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

    /// Writes a number format setting to both files.
    fn set_both(&self, key: &str, value: Value) -> Result<String, String> {
        self.set_menu(key, value.clone())?;
        self.set_sidebar(key, value)?;
        let files: Vec<&str> = std::iter::once(config::FILE_NAME)
            .chain(self.sidebar.as_ref().map(|_| sidebar::SETTINGS_FILE))
            .collect();
        Ok(saved(&files))
    }

    /// Changes `item` by one step (`forward` or back); `Some(true)` when the
    /// screen should close.
    fn change(&mut self, item: Item, forward: bool) -> Option<bool> {
        let result = match item {
            Item::DecimalComma => {
                let value = !self.menu.numbers.decimal_comma;
                self.set_both("decimal_comma", value.into())
                    .inspect(|_| self.menu.numbers.decimal_comma = value)
            }
            Item::PercentSpace => {
                let value = !self.menu.numbers.percent_space;
                self.set_both("percent_space", value.into())
                    .inspect(|_| self.menu.numbers.percent_space = value)
            }
            Item::CpuDecimal => {
                let value = !self.menu.numbers.cpu_decimal;
                self.set_both("cpu_decimal", value.into())
                    .inspect(|_| self.menu.numbers.cpu_decimal = value)
            }
            Item::MemoryUnit => {
                let mut value = self.menu.numbers.memory_unit.next();
                if !forward {
                    // Back: two steps forward among three.
                    value = value.next();
                }
                self.set_both("memory_unit", value.name().into())
                    .inspect(|_| self.menu.numbers.memory_unit = value)
            }
            Item::MemorySpace => {
                let value = !self.menu.numbers.memory_space;
                self.set_both("memory_space", value.into())
                    .inspect(|_| self.menu.numbers.memory_space = value)
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
            Item::Totals => {
                let value = match self.menu.totals {
                    TotalsStyle::Line => TotalsStyle::Bars,
                    TotalsStyle::Bars => TotalsStyle::Line,
                };
                self.set_menu("totals", value.name().into()).map(|()| {
                    self.menu.totals = value;
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
            Item::Colors => {
                let value = !self.sidebar.as_ref()?.colors;
                self.set_sidebar("colors", value.into()).map(|()| {
                    if let Some(s) = &mut self.sidebar {
                        s.colors = value;
                    }
                    saved(&[sidebar::SETTINGS_FILE])
                })
            }
            Item::Shells => {
                self.typing = Some(self.sidebar.as_ref()?.shells.join(", "));
                return None;
            }
            Item::Levels(which) => {
                let values = self.sidebar.as_ref()?.levels(which);
                let text: Vec<String> = values.iter().map(|v| number(*v)).collect();
                self.typing = Some(text.join(" "));
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
        self.show(result);
        None
    }

    /// Saves what was typed for `item`.
    fn save_typed(&mut self, item: Item, text: &str) {
        let result = match item {
            Item::Shells => self.save_shells(text),
            Item::Levels(which) => self.save_levels(which, text),
            _ => return,
        };
        self.show(result);
    }

    fn save_shells(&mut self, text: &str) -> Result<String, String> {
        let shells: Vec<String> = text
            .split([',', ' '])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        let array: toml_edit::Array = shells.iter().map(String::as_str).collect();
        self.set_sidebar("shells", array.into())?;
        if let Some(s) = &mut self.sidebar {
            s.shells = shells;
        }
        Ok(saved(&[sidebar::SETTINGS_FILE]))
    }

    fn save_levels(&mut self, which: Levels, text: &str) -> Result<String, String> {
        let thresholds = parse_levels(text)?;
        let mut array = toml_edit::Array::new();
        for value in thresholds {
            // Whole numbers as integers: `[10, 25, 50]`, not `[10.0, …]`.
            if value.fract() == 0.0 {
                array.push(value as i64);
            } else {
                array.push(value);
            }
        }
        self.set_sidebar(which.key(), array.into())?;
        if let Some(s) = &mut self.sidebar {
            *s.levels_mut(which) = thresholds;
        }
        Ok(saved(&[sidebar::SETTINGS_FILE]))
    }

    fn show(&mut self, result: Result<String, String>) {
        self.status = Some(match result {
            Ok(message) => (message, false),
            Err(e) => (e, true),
        });
    }
}

/// `enregistré dans menu.toml et sidebar.toml`.
fn saved(files: &[&str]) -> String {
    format!("enregistré dans {}", files.join(" et "))
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

/// The first row of the list on screen: the selected item stays visible
/// when the list is taller than the screen.
fn scroll(state: &State, height: u16) -> usize {
    let visible = usize::from(height.saturating_sub(TOP + BOTTOM)).max(1);
    let items = state.items();
    let selected = items[state.selected.min(items.len() - 1)];
    let rows = state.rows();
    let at = rows
        .iter()
        .position(|row| *row == Row::Item(selected))
        .unwrap_or(0);
    // Show the header above the first item, the end of the list at the end.
    (at + 1)
        .saturating_sub(visible)
        .min(rows.len().saturating_sub(visible))
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
                        state.save_typed(item, &text);
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
                let (_, height) = terminal::size()?;
                let rows = state.rows();
                let in_list = mouse.row >= TOP && mouse.row < height.saturating_sub(BOTTOM);
                let hit = usize::from(mouse.row.wrapping_sub(TOP)) + scroll(state, height);
                let item_at = rows.get(hit).filter(|_| in_list).and_then(|row| match row {
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
                    (MouseEventKind::ScrollDown, _) => {
                        state.selected = (state.selected + 1).min(items.len() - 1);
                        None
                    }
                    (MouseEventKind::ScrollUp, _) => {
                        state.selected = state.selected.saturating_sub(1);
                        None
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
    let rows = state.rows();
    let first = scroll(state, height);
    let last = height.saturating_sub(BOTTOM);
    for (y, row) in (TOP..last).zip(rows.iter().skip(first)) {
        match *row {
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
                let (name, value) = state.name_and_value(item, item == selected);
                let marker = if item == selected { "›" } else { " " };
                let line = format!(" {marker} {name:<NAME_WIDTH$}{value}");
                if item == selected {
                    queue!(out, SetForegroundColor(Color::Yellow))?;
                }
                queue!(out, MoveTo(0, y), Print(fit(&line, width)), ResetColor)?;
            }
        }
    }
    // Rows above or below the screen: a hint in the corner.
    let more = (
        first > 0,
        first + usize::from(last.saturating_sub(TOP)) < rows.len(),
    );
    let arrows = match more {
        (true, true) => "↑↓",
        (true, false) => "↑",
        (false, true) => "↓",
        (false, false) => "",
    };
    if !arrows.is_empty() {
        queue!(
            out,
            MoveTo(width.saturating_sub(3) as u16, 0),
            SetForegroundColor(Color::DarkGrey),
            Print(arrows),
            ResetColor
        )?;
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
    let help = match (state.typing.is_some(), selected) {
        (true, Item::Shells) => " noms séparés par des virgules · Entrée valider · Échap annuler",
        (true, _) => {
            " trois valeurs croissantes, séparées par des espaces · Entrée valider · Échap annuler"
        }
        (false, _) => " ↑↓ choisir · ←→ Entrée changer · Échap fermer",
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
        assert_eq!(values, SidebarValues::default());
        std::fs::write(
            &path,
            "ttl_seconds = 30\nshells = [\"bash\"]\ncpu_levels = [5, 10.5, 20]\nunknown = 1\n",
        )
        .unwrap();
        let (values, error) = SidebarValues::read(&path);
        assert!(error.is_none());
        assert_eq!(
            (values.ttl_seconds, values.shells),
            (30, vec!["bash".into()])
        );
        assert_eq!(values.cpu_levels, [5.0, 10.5, 20.0]);
        assert!(!values.tab_bar && !values.colors);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn thresholds_are_typed_with_spaces() {
        assert_eq!(parse_levels("10 25 50"), Ok([10.0, 25.0, 50.0]));
        assert_eq!(parse_levels(" 0,5  1 2.5 "), Ok([0.5, 1.0, 2.5]));
        assert!(parse_levels("10 25").is_err());
        assert!(parse_levels("10 x 50").is_err());
        assert!(parse_levels("50 25 10").is_err());
        assert_eq!(number(10.0), "10");
        assert_eq!(number(2.5), "2.5");
    }
}
