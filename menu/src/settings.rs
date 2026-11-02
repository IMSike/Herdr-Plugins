//! The settings screen, in its popup: the settings of the menu and of the
//! sidebar, each written to its file as soon as it changes; the
//! configuration files opened in an editor; Herdr's configuration reloaded.

pub mod files;
mod store;

use crate::config::{self, IconLayout, Menu, TotalsStyle};
use crate::ui::{Frame, Screen, fit, is_control};
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal;
use files::Files;
use herdr_api::NumberFormat;
use herdr_api::sidebar::{self, Levels, Settings, Thresholds, check_thresholds};
use std::io;
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

/// The name of a set of thresholds on the screen.
fn levels_name(levels: Levels) -> &'static str {
    match levels {
        Levels::Cpu => "Seuils CPU (Workspaces)",
        Levels::Memory => "Seuils mémoire (Workspaces)",
        Levels::MachineCpu => "Seuils CPU (machine)",
        Levels::MachineMemory => "Seuils mémoire (machine)",
    }
}

/// The unit of a set of thresholds on the screen.
fn levels_unit(levels: Levels) -> &'static str {
    match levels {
        Levels::Memory => "Go",
        _ => "%",
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

/// Thresholds as they are written: whole numbers as integers (`[10, 25,
/// 50]`, not `[10.0, …]`), unless too large for one to hold them exactly,
/// which would change them.
fn levels_array(thresholds: &Thresholds) -> toml_edit::Array {
    /// Past this, an `f64` no longer holds every whole number.
    const EXACT: f64 = 9_007_199_254_740_992.0;
    let mut array = toml_edit::Array::new();
    for &value in thresholds {
        if value.fract() == 0.0 && value.abs() < EXACT {
            array.push(value as i64);
        } else {
            array.push(value);
        }
    }
    array
}

struct State {
    files: Files,
    menu: Menu,
    /// `None` when the sidebar is not installed.
    sidebar: Option<Settings>,
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
                    IconLayout::Single => "une ligne, nom au survol",
                    IconLayout::Double => "deux lignes, avec les noms",
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
                levels_name(which),
                match typed {
                    Some(text) => format!("{text}▏"),
                    None => sidebar.map_or_else(String::new, |s| {
                        let values: Vec<String> = which.of(s).iter().map(|v| number(*v)).collect();
                        format!("{} {}", values.join(" · "), levels_unit(which))
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
        store::set(path, files::MENU_TEMPLATE, key, value)?;
        // A file the menu still ignores (an unknown setting…) is said so.
        match config::load(path.parent()) {
            (_, Some(e)) => Err(format!("enregistré, mais {e}")),
            (_, None) => Ok(()),
        }
    }

    /// Writes `key` to `sidebar.toml`, when the sidebar is installed.
    fn set_sidebar(&self, key: &str, value: Value) -> Result<(), String> {
        match &self.files.sidebar {
            Some(path) => {
                store::set(path, files::SIDEBAR_TEMPLATE, key, value)?;
                // A file the sidebar still ignores is said so.
                Settings::read(path)
                    .map(|_| ())
                    .map_err(|e| format!("enregistré, mais {e}"))
            }
            None => Ok(()),
        }
    }

    /// Writes a number format setting to both files; the screen shows it
    /// once `menu.toml` has it, even when `sidebar.toml` cannot be written.
    fn set_number(
        &mut self,
        key: &str,
        value: Value,
        apply: impl FnOnce(&mut NumberFormat),
    ) -> Result<String, String> {
        self.set_menu(key, value.clone())?;
        apply(&mut self.menu.numbers);
        self.set_sidebar(key, value).map_err(|e| {
            format!(
                "enregistré dans {}, pas dans {} : {e}",
                config::FILE_NAME,
                sidebar::SETTINGS_FILE
            )
        })?;
        let files: Vec<&str> = std::iter::once(config::FILE_NAME)
            .chain(self.sidebar.as_ref().map(|_| sidebar::SETTINGS_FILE))
            .collect();
        Ok(saved(&files))
    }

    /// Turns a yes-or-no number format setting over.
    fn toggle_number(
        &mut self,
        key: &str,
        field: fn(&mut NumberFormat) -> &mut bool,
    ) -> Result<String, String> {
        let value = !*field(&mut self.menu.numbers);
        self.set_number(key, value.into(), |n| *field(n) = value)
    }

    /// Writes a setting of `sidebar.toml`, then shows it; `None` when the
    /// sidebar is not installed.
    fn set_in_sidebar(
        &mut self,
        key: &str,
        value: Value,
        apply: impl FnOnce(&mut Settings),
    ) -> Option<Result<String, String>> {
        self.sidebar.as_ref()?;
        Some(self.set_sidebar(key, value).map(|()| {
            if let Some(s) = &mut self.sidebar {
                apply(s);
            }
            saved(&[sidebar::SETTINGS_FILE])
        }))
    }

    /// Turns a yes-or-no setting of `sidebar.toml` over.
    fn toggle_sidebar(
        &mut self,
        key: &str,
        field: fn(&mut Settings) -> &mut bool,
    ) -> Option<Result<String, String>> {
        let value = !*field(self.sidebar.as_mut()?);
        self.set_in_sidebar(key, value.into(), |s| *field(s) = value)
    }

    /// Changes `item` by one step (`forward` or back); whether the screen
    /// should close.
    fn change(&mut self, item: Item, forward: bool) -> bool {
        if item == Item::OpenFiles {
            match files::open(&self.files) {
                Ok(_) => {
                    tell_the_bar();
                    return true;
                }
                Err(e) => self.show(Err(e)),
            }
        } else if let Some(result) = self.step(item, forward) {
            self.show(result);
        }
        false
    }

    /// What changing `item` by one step did, to show; `None` when there is
    /// nothing to show (no sidebar, a value to type).
    fn step(&mut self, item: Item, forward: bool) -> Option<Result<String, String>> {
        Some(match item {
            Item::DecimalComma => self.toggle_number("decimal_comma", |n| &mut n.decimal_comma),
            Item::PercentSpace => self.toggle_number("percent_space", |n| &mut n.percent_space),
            Item::CpuDecimal => self.toggle_number("cpu_decimal", |n| &mut n.cpu_decimal),
            Item::MemorySpace => self.toggle_number("memory_space", |n| &mut n.memory_space),
            Item::MemoryUnit => {
                let mut value = self.menu.numbers.memory_unit.next();
                if !forward {
                    // Back: two steps forward among three.
                    value = value.next();
                }
                self.set_number("memory_unit", value.name().into(), |n| {
                    n.memory_unit = value
                })
            }
            Item::Layout => {
                let value = match self.menu.layout {
                    IconLayout::Single => IconLayout::Double,
                    IconLayout::Double => IconLayout::Single,
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
                // From a value out of the range, which the file may hold, a
                // step never goes the other way.
                if (forward && value < current) || (!forward && value > current) {
                    return None;
                }
                self.set_in_sidebar("ttl_seconds", (value as i64).into(), |s| {
                    s.ttl_seconds = value
                })?
            }
            Item::TabBar => self.toggle_sidebar("tab_bar", |s| &mut s.tab_bar)?,
            Item::Colors => self.toggle_sidebar("colors", |s| &mut s.colors)?,
            Item::Shells => {
                self.typing = Some(self.sidebar.as_ref()?.shells.join(", "));
                return None;
            }
            Item::Levels(which) => {
                let values = which.of(self.sidebar.as_ref()?);
                let text: Vec<String> = values.iter().map(|v| number(*v)).collect();
                self.typing = Some(text.join(" "));
                return None;
            }
            // See `change`: it closes the screen.
            Item::OpenFiles => return None,
            Item::Reload => files::reload(),
        })
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
        self.set_in_sidebar("shells", array.into(), |s| s.shells = shells)
            .unwrap_or_else(|| Ok(String::new()))
    }

    fn save_levels(&mut self, which: Levels, text: &str) -> Result<String, String> {
        let thresholds = parse_levels(text)?;
        self.set_in_sidebar(which.key(), levels_array(&thresholds).into(), |s| {
            *which.of_mut(s) = thresholds
        })
        .unwrap_or_else(|| Ok(String::new()))
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
    let (menu, menu_error) = crate::menu_config();
    let (sidebar, sidebar_error) = match &files.sidebar {
        // Shown as the sidebar uses them: the defaults when it ignores the
        // file, and why.
        Some(path) => match Settings::read(path) {
            Ok(settings) => (Some(settings), None),
            Err(e) => (Some(Settings::default()), Some(e)),
        },
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
    let mut shown = Vec::new();
    loop {
        draw(state)?.show_if_new(&mut shown)?;
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
                    KeyCode::Char(c) if !is_control(key.modifiers) => text.push(c),
                    _ => {}
                }
            }
            continue;
        }
        let close = match event {
            // Ctrl+L and the like are no l: only Ctrl+C means something.
            Event::Key(key) if key.kind == KeyEventKind::Press && is_control(key.modifiers) => {
                key.code == KeyCode::Char('c')
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => true,
                KeyCode::Up | KeyCode::Char('k') => {
                    state.selected = (state.selected + items.len() - 1) % items.len();
                    false
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    state.selected = (state.selected + 1) % items.len();
                    false
                }
                KeyCode::Left | KeyCode::Char('h') => state.change(item, false),
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
                    state.change(item, true)
                }
                _ => false,
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
                        false
                    }
                    (MouseEventKind::Down(MouseButton::Left), Some(i)) => {
                        state.selected = i;
                        state.change(items[i], true)
                    }
                    (MouseEventKind::ScrollDown, _) => {
                        state.selected = (state.selected + 1).min(items.len() - 1);
                        false
                    }
                    (MouseEventKind::ScrollUp, _) => {
                        state.selected = state.selected.saturating_sub(1);
                        false
                    }
                    _ => false,
                }
            }
            _ => false,
        };
        if close {
            return Ok(());
        }
    }
}

fn draw(state: &State) -> io::Result<Frame> {
    let (width, height) = terminal::size()?;
    let width = usize::from(width);
    let mut out = Frame::new()?;
    queue!(
        out,
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
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn typed_thresholds_read_back_as_typed() {
        let file =
            std::env::temp_dir().join(format!("herdr-menu-levels-{}.toml", std::process::id()));
        for text in ["10 25 50", "0,5 1 2.5", "1e19 2e19 3e19"] {
            let thresholds = parse_levels(text).unwrap();
            let line = format!("cpu_levels = {}\n", levels_array(&thresholds));
            std::fs::write(&file, line).unwrap();
            let read = Settings::read(&file).unwrap();
            assert_eq!(*Levels::Cpu.of(&read), thresholds, "{text}");
        }
        assert_eq!(
            levels_array(&[10.0, 25.0, 50.0]).to_string(),
            "[10, 25, 50]"
        );
        let _ = std::fs::remove_file(file);
    }
}
