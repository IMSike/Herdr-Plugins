//! The settings screen, in its popup: the settings of the menu and of the
//! sidebar, each written to its file as soon as it changes; the
//! configuration files opened in an editor; Herdr's configuration reloaded.

mod canvas;
pub mod files;
mod sessions;
mod store;
mod view;

use crate::config::{self, Menu, TotalsStyle};
use crate::ui::{Frame, Screen, is_control};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use crossterm::terminal;
use files::Files;
use herdr_api::NumberFormat;
use herdr_api::sidebar::{self, Levels, Settings, Thresholds, check_thresholds};
use std::io;
use toml_edit::Value;
use view::Target;

/// The `Tools` pane the screen was opened from, if any.
pub const BAR_ENV: &str = "HERDR_MENU_BAR";
/// A step of `ttl_seconds`, and its bounds.
const TTL_STEP: u64 = 5;
const TTL_MAX: u64 = 600;
/// The size the number formats are shown with: 2.4 GiB, and 853 MiB.
const SAMPLE_SIZE: u64 = 2_576_980_378;
const SAMPLE_SMALL_SIZE: u64 = 853 * 1024 * 1024;
/// The CPU share they are shown with.
const SAMPLE_CPU: f64 = 4.2;
/// The units of a size, in the order `MemoryUnit::next` goes through them.
const MEMORY_UNITS: [herdr_api::MemoryUnit; 3] = [
    herdr_api::MemoryUnit::M,
    herdr_api::MemoryUnit::Mo,
    herdr_api::MemoryUnit::MB,
];

/// What a row of the screen holds.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Row {
    Header(Section),
    Blank,
    Item(Item),
}

/// The sections of the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Section {
    Numbers,
    Tools,
    Sidebar,
    Colors,
    Files,
    Session,
}

impl Section {
    fn title(self) -> &'static str {
        match self {
            Self::Numbers => "Format des nombres",
            Self::Tools => "Pane Tools",
            Self::Sidebar => "Sidebar",
            Self::Colors => "Couleurs par seuil",
            Self::Files => "Configuration",
            Self::Session => "Session",
        }
    }
}

/// How an item's value is shown.
#[derive(Debug, Clone, PartialEq)]
enum Shown {
    /// One of a few values, all shown, the `current` one highlighted.
    Choice {
        options: Vec<String>,
        current: usize,
    },
    /// A number changed by steps.
    Number(String),
    /// Text typed after Enter.
    Text(String),
    /// Three thresholds, typed after Enter, and their unit.
    Levels(Vec<String>, &'static str),
    /// Something done on Enter.
    Action,
}

/// A yes-or-no setting, shown as two choices.
fn yes_no(value: bool) -> Shown {
    Shown::Choice {
        options: vec!["non".into(), "oui".into()],
        current: usize::from(value),
    }
}

/// A size or a share as the screen shows it: the narrow no-break space of
/// the plugins as a plain one, which every terminal draws one cell wide.
fn plain(text: String) -> String {
    text.replace('\u{202F}', " ")
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Item {
    DecimalComma,
    PercentSpace,
    CpuDecimal,
    MemoryUnit,
    MemorySpace,
    Totals,
    CheckUpdates,
    Ttl,
    Shells,
    TabBar,
    Colors,
    Levels(Levels),
    OpenFiles,
    Reload,
    Recover,
}

impl Item {
    /// Its name on the screen.
    fn label(self) -> &'static str {
        match self {
            Self::DecimalComma => "Virgule décimale",
            Self::PercentSpace => "Espace avant %",
            Self::CpuDecimal => "CPU avec une décimale",
            Self::MemoryUnit => "Unités de la mémoire",
            Self::MemorySpace => "Espace avant l'unité",
            Self::Totals => "Totaux CPU / mémoire",
            Self::CheckUpdates => "Signaler les mises à jour",
            Self::Ttl => "Durée de vie des valeurs",
            Self::Shells => "Shells",
            Self::TabBar => "Totaux dans la barre d'onglets",
            Self::Colors => "Couleurs",
            Self::Levels(Levels::Cpu) => "Seuils CPU (Herdr)",
            Self::Levels(Levels::Memory) => "Seuils mémoire (Herdr)",
            Self::Levels(Levels::MachineCpu) => "Seuils CPU (machine)",
            Self::Levels(Levels::MachineMemory) => "Seuils mémoire (machine)",
            // nf-md-pencil
            Self::OpenFiles => "\u{F03EB} Ouvrir les fichiers de configuration…",
            // nf-md-reload, as the `Tools` pane's button
            Self::Reload => "\u{F0453} Recharger la configuration de Herdr",
            // nf-md-backup_restore
            Self::Recover => "\u{F006F} Récupérer une session…",
        }
    }

    /// Something done on Enter, without a value to show.
    fn is_action(self) -> bool {
        matches!(self, Self::OpenFiles | Self::Reload | Self::Recover)
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
    /// The first row of the list on screen.
    top: usize,
    /// The last thing done, or why it failed (in red).
    status: Option<(String, bool)>,
    /// The text being typed for the selected item (shells, thresholds).
    typing: Option<String>,
    /// Set when the screen closes to show the session's snapshots.
    recover: bool,
}

impl State {
    fn new(files: Files, menu: Menu, sidebar: Option<Settings>) -> Self {
        Self {
            files,
            menu,
            sidebar,
            selected: 0,
            top: 0,
            status: None,
            typing: None,
            recover: false,
        }
    }

    fn rows(&self) -> Vec<Row> {
        let mut rows = vec![
            Row::Header(Section::Numbers),
            Row::Item(Item::DecimalComma),
            Row::Item(Item::PercentSpace),
            Row::Item(Item::CpuDecimal),
            Row::Item(Item::MemoryUnit),
            Row::Item(Item::MemorySpace),
            Row::Blank,
            Row::Header(Section::Tools),
            Row::Item(Item::Totals),
            Row::Item(Item::CheckUpdates),
        ];
        if self.sidebar.is_some() {
            rows.extend([
                Row::Blank,
                Row::Header(Section::Sidebar),
                Row::Item(Item::Ttl),
                Row::Item(Item::Shells),
                Row::Item(Item::TabBar),
                Row::Blank,
                Row::Header(Section::Colors),
                Row::Item(Item::Colors),
            ]);
            rows.extend(Levels::ALL.map(|l| Row::Item(Item::Levels(l))));
        }
        rows.extend([
            Row::Blank,
            Row::Header(Section::Files),
            Row::Item(Item::OpenFiles),
            Row::Item(Item::Reload),
            Row::Blank,
            Row::Header(Section::Session),
            Row::Item(Item::Recover),
        ]);
        rows
    }

    /// The index, among the items, of the first item of each section.
    fn section_starts(&self) -> Vec<usize> {
        let mut starts = Vec::new();
        let mut items = 0;
        let mut after_header = false;
        for row in self.rows() {
            match row {
                Row::Header(_) => after_header = true,
                Row::Item(_) => {
                    if after_header {
                        starts.push(items);
                    }
                    after_header = false;
                    items += 1;
                }
                Row::Blank => {}
            }
        }
        starts
    }

    /// The first item of the next section (`forward`), or the first item of
    /// a section above the selected item: its own section's when it is not
    /// that one; round the list.
    fn next_section(&self, forward: bool) -> usize {
        let starts = self.section_starts();
        let found = if forward {
            starts.iter().find(|&&s| s > self.selected)
        } else {
            starts.iter().rev().find(|&&s| s < self.selected)
        };
        let round = if forward {
            starts.first()
        } else {
            starts.last()
        };
        found.or(round).copied().unwrap_or(0)
    }

    /// Selects the item `index`; the screen stays open.
    fn select(&mut self, index: usize) -> bool {
        self.selected = index;
        false
    }

    /// The selected one of `items`, the screen's items.
    fn selected_item(&self, items: &[Item]) -> Item {
        items[self.selected.min(items.len() - 1)]
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

    /// An item's value: a choice shows each value as the current number
    /// format writes it, so that it shows what changes.
    fn shown(&self, item: Item) -> Shown {
        let sidebar = self.sidebar.as_ref();
        let numbers = self.menu.numbers;
        let sizes = |format: fn(&mut NumberFormat, usize), count: usize, current: usize| {
            let options = (0..count)
                .map(|i| {
                    let mut n = numbers;
                    format(&mut n, i);
                    plain(n.size(SAMPLE_SIZE))
                })
                .collect();
            Shown::Choice { options, current }
        };
        let shares = |format: fn(&mut NumberFormat, bool), current: bool| {
            let options = [false, true]
                .into_iter()
                .map(|b| {
                    let mut n = numbers;
                    format(&mut n, b);
                    plain(n.cpu(SAMPLE_CPU))
                })
                .collect();
            Shown::Choice {
                options,
                current: usize::from(current),
            }
        };
        match item {
            Item::DecimalComma => sizes(
                |n, i| n.decimal_comma = i == 1,
                2,
                usize::from(numbers.decimal_comma),
            ),
            Item::PercentSpace => shares(|n, b| n.percent_space = b, numbers.percent_space),
            Item::CpuDecimal => shares(|n, b| n.cpu_decimal = b, numbers.cpu_decimal),
            Item::MemoryUnit => sizes(
                |n, i| n.memory_unit = MEMORY_UNITS[i],
                MEMORY_UNITS.len(),
                MEMORY_UNITS
                    .iter()
                    .position(|&u| u == numbers.memory_unit)
                    .unwrap_or(0),
            ),
            Item::MemorySpace => sizes(
                |n, i| n.memory_space = i == 1,
                2,
                usize::from(numbers.memory_space),
            ),
            Item::Totals => Shown::Choice {
                options: vec!["une ligne".into(), "barres".into()],
                current: usize::from(self.menu.totals == TotalsStyle::Bars),
            },
            Item::CheckUpdates => yes_no(self.menu.check_updates),
            Item::Ttl => Shown::Number(format!("{} s", sidebar.map_or(0, |s| s.ttl_seconds))),
            Item::Shells => Shown::Text(sidebar.map_or_else(String::new, |s| s.shells.join(", "))),
            Item::TabBar => yes_no(sidebar.is_some_and(|s| s.tab_bar)),
            Item::Colors => yes_no(sidebar.is_some_and(|s| s.colors)),
            Item::Levels(which) => Shown::Levels(
                sidebar.map_or_else(Vec::new, |s| {
                    which.of(s).iter().map(|v| number(*v)).collect()
                }),
                levels_unit(which),
            ),
            Item::OpenFiles | Item::Reload | Item::Recover => Shown::Action,
        }
    }

    /// What an item does, and the setting and files it writes.
    fn about(&self, item: Item) -> (&'static str, String) {
        let menu = config::FILE_NAME;
        let side = sidebar::SETTINGS_FILE;
        let both = match self.sidebar {
            Some(_) => format!("{menu}, {side}"),
            None => menu.to_string(),
        };
        let (what, key, file) = match item {
            Item::DecimalComma => (
                "Une virgule ou un point avant les décimales.",
                "decimal_comma",
                both,
            ),
            Item::PercentSpace => ("Une espace fine avant le signe %.", "percent_space", both),
            Item::CpuDecimal => (
                "Les parts de CPU des totaux, avec une décimale ou sans.",
                "cpu_decimal",
                both,
            ),
            Item::MemoryUnit => (
                "L'unité des tailles : K, M, G ; Ko, Mo, Go ; ou KB, MB, GB.",
                "memory_unit",
                both,
            ),
            Item::MemorySpace => (
                "Une espace fine entre une taille et son unité.",
                "memory_space",
                both,
            ),
            Item::Totals => (
                "Les totaux sous les icônes : une ligne, ou quatre barres.",
                "totals",
                menu.into(),
            ),
            Item::CheckUpdates => (
                "Update en vert, dans le pane Tools et le menu en popup, quand Herdr ou un plugin a une nouvelle version : vérifié au démarrage, puis toutes les 4 h tant qu'un pane Tools est ouvert.",
                "check_updates",
                menu.into(),
            ),
            Item::Ttl => (
                "Combien de temps une valeur dure sans mesure nouvelle (5 à 600 s).",
                "ttl_seconds",
                side.into(),
            ),
            Item::Shells => (
                "Les programmes comptés comme shells, séparés par des virgules.",
                "shells",
                side.into(),
            ),
            Item::TabBar => (
                "Les totaux CPU et mémoire à droite de la barre d'onglets.",
                "tab_bar",
                side.into(),
            ),
            Item::Colors => (
                "Les chiffres en vert, jaune, orange ou rouge selon leurs seuils.",
                "colors",
                side.into(),
            ),
            Item::Levels(which) => (
                "Trois seuils croissants, entre vert, jaune, orange et rouge.",
                which.key(),
                side.into(),
            ),
            Item::OpenFiles => (
                "Ouvre les fichiers dans un éditeur, et ferme cet écran.",
                "",
                String::new(),
            ),
            Item::Reload => (
                "Recharge le config.toml de Herdr et dit s'il l'a appliqué.",
                "",
                String::new(),
            ),
            Item::Recover => (
                "Les snapshots de la session, pour en restaurer un : Herdr redémarre, sur confirmation.",
                "",
                String::new(),
            ),
        };
        let place = if key.is_empty() {
            String::new()
        } else {
            format!("{key} · {file}")
        };
        (what, place)
    }

    /// What a section's header shows after its title: the numbers as their
    /// format writes them.
    fn note(&self, section: Section) -> Option<String> {
        let n = self.menu.numbers;
        (section == Section::Numbers).then(|| {
            plain(format!(
                "{} · {} · {}",
                n.cpu(SAMPLE_CPU),
                n.size(SAMPLE_SMALL_SIZE),
                n.size(SAMPLE_SIZE)
            ))
        })
    }

    /// Writes `key` to `menu.toml`; why the menu still ignores the file,
    /// if it does (an unknown setting…).
    fn set_menu(&self, key: &str, value: Value) -> Result<Option<String>, String> {
        let path = self
            .files
            .menu
            .as_ref()
            .ok_or("dossier de configuration du menu inconnu")?;
        store::set(path, files::MENU_TEMPLATE, key, value)?;
        Ok(config::load(path.parent()).1)
    }

    /// Writes `key` to `sidebar.toml`, when the sidebar is installed; why
    /// the sidebar still ignores the file, if it does.
    fn set_sidebar(&self, key: &str, value: Value) -> Result<Option<String>, String> {
        match &self.files.sidebar {
            Some(path) => {
                store::set(path, files::SIDEBAR_TEMPLATE, key, value)?;
                Ok(Settings::read(path).err())
            }
            None => Ok(None),
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
        let menu_ignored = self.set_menu(key, value.clone())?;
        apply(&mut self.menu.numbers);
        let sidebar_ignored = self.set_sidebar(key, value).map_err(|e| {
            format!(
                "enregistré dans {}, pas dans {} : {e}",
                config::FILE_NAME,
                sidebar::SETTINGS_FILE
            )
        })?;
        let files: Vec<&str> = std::iter::once(config::FILE_NAME)
            .chain(self.sidebar.as_ref().map(|_| sidebar::SETTINGS_FILE))
            .collect();
        written(menu_ignored.or(sidebar_ignored), &files)
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
        Some(self.set_sidebar(key, value).and_then(|ignored| {
            if let Some(s) = &mut self.sidebar {
                apply(s);
            }
            written(ignored, &[sidebar::SETTINGS_FILE])
        }))
    }

    /// Sets a setting of `menu.toml` to `value`, and the menu as `apply` does.
    fn switch_menu(
        &mut self,
        key: &str,
        value: Value,
        apply: impl FnOnce(&mut Menu),
    ) -> Result<String, String> {
        self.set_menu(key, value).and_then(|ignored| {
            apply(&mut self.menu);
            written(ignored, &[config::FILE_NAME])
        })
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
    /// should close. A choice goes round its values when `round`, and
    /// stops at its first or last one otherwise, as its row shows them;
    /// only `round` (Enter, a click) types a value or runs an action.
    fn change(&mut self, item: Item, forward: bool, round: bool) -> bool {
        if !round {
            match self.shown(item) {
                Shown::Choice { options, current } => {
                    let at_end = if forward {
                        current + 1 >= options.len()
                    } else {
                        current == 0
                    };
                    if at_end {
                        return false;
                    }
                }
                Shown::Number(_) => {}
                Shown::Text(_) | Shown::Levels(..) | Shown::Action => return false,
            }
        }
        if item == Item::Recover {
            self.recover = true;
            return true;
        }
        if item == Item::OpenFiles {
            match files::open(&self.files) {
                Ok(_) => {
                    tell_the_bar();
                    return true;
                }
                Err(e) => self.report(Err(e)),
            }
        } else if let Some(result) = self.step(item, forward) {
            self.report(result);
        }
        false
    }

    /// Sets the choice `item` to its value `option`, step by step, as long
    /// as each step changes it.
    fn choose(&mut self, item: Item, option: usize) {
        let current = |state: &Self| match state.shown(item) {
            Shown::Choice { current, .. } => Some(current),
            _ => None,
        };
        while let Some(before) = current(self).filter(|&c| c != option) {
            match self.step(item, true) {
                Some(result) => self.report(result),
                None => return,
            }
            if current(self) == Some(before) {
                return;
            }
        }
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
                self.set_number("memory_unit", herdr_api::name_of(&value).into(), |n| {
                    n.memory_unit = value
                })
            }
            Item::Totals => {
                let value = match self.menu.totals {
                    TotalsStyle::Line => TotalsStyle::Bars,
                    TotalsStyle::Bars => TotalsStyle::Line,
                };
                self.switch_menu("totals", herdr_api::name_of(&value).into(), |m| {
                    m.totals = value
                })
            }
            Item::CheckUpdates => {
                let value = !self.menu.check_updates;
                self.switch_menu("check_updates", value.into(), |m| m.check_updates = value)
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
            // See `change`: they close the screen.
            Item::OpenFiles | Item::Recover => return None,
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
        self.report(result);
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

    /// Shows `result` on the status line.
    fn report(&mut self, result: Result<String, String>) {
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

/// What the screen says of a setting written to `files`: saved, or saved
/// but in a file still `ignored` for that reason (an unknown setting…). The
/// screen shows the new value either way: the file holds it.
fn written(ignored: Option<String>, files: &[&str]) -> Result<String, String> {
    match ignored {
        Some(e) => Err(format!("enregistré, mais {e}")),
        None => Ok(saved(files)),
    }
}

/// The `Tools` pane the screen was opened from switches to its reload view.
fn tell_the_bar() {
    if let (Ok(bar), Ok(state)) = (std::env::var(BAR_ENV), herdr_api::session_dir()) {
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
    let mut state = State::new(files, menu, sidebar);
    state.status = menu_error.or(sidebar_error).map(|e| (e, true));
    loop {
        show(&mut state).map_err(|e| e.to_string())?;
        if !std::mem::take(&mut state.recover) {
            return Ok(());
        }
        // Back from the snapshots, the settings again.
        match sessions::browse().map_err(|e| e.to_string())? {
            sessions::Outcome::Back => {}
            sessions::Outcome::Restoring => return Ok(()),
        }
    }
}

/// Draws the screen; what its parts answer to a click.
fn draw(state: &mut State, shown: &mut Vec<u8>) -> io::Result<view::Drawn> {
    let (width, height) = terminal::size()?;
    let drawn = view::render(state, width, height);
    state.top = drawn.top;
    let mut frame = Frame::new()?;
    drawn.canvas.write_to(&mut frame)?;
    frame.show_if_new(shown)?;
    Ok(drawn)
}

fn show(state: &mut State) -> io::Result<()> {
    let _screen = Screen::enter()?;
    let mut shown = Vec::new();
    loop {
        let drawn = draw(state, &mut shown)?;
        let items = state.items();
        let item = state.selected_item(&items);
        let last = items.len() - 1;
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
                    state.select((state.selected + last) % items.len())
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    state.select((state.selected + 1) % items.len())
                }
                KeyCode::Home | KeyCode::Char('g') => state.select(0),
                KeyCode::End | KeyCode::Char('G') => state.select(last),
                KeyCode::Tab => state.select(state.next_section(true)),
                KeyCode::BackTab => state.select(state.next_section(false)),
                KeyCode::Left | KeyCode::Char('h') => state.change(item, false, false),
                KeyCode::Right | KeyCode::Char('l') => state.change(item, true, false),
                KeyCode::Enter | KeyCode::Char(' ') => state.change(item, true, true),
                _ => false,
            },
            Event::Mouse(mouse) => match (mouse.kind, drawn.at(mouse.column, mouse.row)) {
                (MouseEventKind::Moved, Some(target)) => state.select(target.item()),
                (MouseEventKind::Down(MouseButton::Left), Some(target)) => {
                    state.selected = target.item();
                    let item = items[target.item()];
                    match target {
                        Target::Item(_) => state.change(item, true, true),
                        Target::Choose(_, option) => {
                            state.choose(item, option);
                            false
                        }
                        Target::Step(_, forward) => state.change(item, forward, false),
                    }
                }
                (MouseEventKind::ScrollDown, _) => state.select((state.selected + 1).min(last)),
                (MouseEventKind::ScrollUp, _) => state.select(state.selected.saturating_sub(1)),
                _ => false,
            },
            _ => false,
        };
        if close {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The screen on the files of `dir`, the sidebar installed.
    fn state_in(dir: &std::path::Path) -> State {
        let files = Files {
            menu: Some(dir.join(config::FILE_NAME)),
            sidebar: Some(dir.join(sidebar::SETTINGS_FILE)),
        };
        State::new(files, Menu::default(), Some(Settings::default()))
    }

    /// The screen without files, the sidebar installed.
    fn state() -> State {
        let files = Files {
            menu: None,
            sidebar: None,
        };
        State::new(files, Menu::default(), Some(Settings::default()))
    }

    /// The screen without files, as `width` × `height` cells of text.
    fn rendered(state: &State, width: u16, height: u16) -> String {
        view::render(state, width, height).canvas.to_text()
    }

    #[test]
    fn the_screen_shows_every_setting_in_its_section() {
        let mut state = state();
        state.status = Some(("enregistré dans menu.toml".into(), false));
        let text = rendered(&state, 92, 40);
        for expected in [
            "Réglages",
            "Format des nombres",
            "Virgule décimale",
            " 2.4G   2,4G",
            " 2,4G   2,4Go   2,4GB",
            "━━ 10 ━━ 25 ━━ 50 ━━ %",
            "‹ 20 s ›",
            "Couleurs par seuil",
            "Seuils mémoire (Herdr)",
            "✓ enregistré dans menu.toml",
            "decimal_comma · menu.toml, sidebar.toml",
            "↑↓ naviguer",
        ] {
            assert!(text.contains(expected), "{expected:?} missing:\n{text}");
        }
        // Without the sidebar, its sections are gone.
        state.sidebar = None;
        let text = rendered(&state, 92, 40);
        assert!(!text.contains("Sidebar") && !text.contains("Seuils"));
    }

    #[test]
    fn any_size_draws_without_panic() {
        let mut state = state();
        state.status = Some((
            "une erreur longue, très longue, bien trop longue".into(),
            true,
        ));
        for (w, h) in [
            (0, 0),
            (1, 1),
            (3, 2),
            (10, 5),
            (20, 8),
            (30, 12),
            (200, 60),
        ] {
            for selected in 0..state.items().len() {
                state.selected = selected;
                state.typing = (selected % 2 == 0).then(|| "pwsh, bash, zsh, fish".into());
                let drawn = view::render(&state, w, h);
                state.top = drawn.top;
            }
        }
    }

    #[test]
    fn a_short_screen_scrolls_to_the_selection() {
        let mut state = state();
        state.selected = state.items().len() - 1;
        let text = rendered(&state, 70, 14);
        assert!(text.contains("Récupérer une session"), "{text}");
        assert!(!text.contains("Virgule décimale"), "{text}");
        // The scrollbar's thumb, at the bottom.
        assert!(text.contains('┃'), "{text}");
    }

    #[test]
    fn typed_text_shows_its_end_and_the_keys_to_save_it() {
        let mut state = state();
        state.selected = state
            .items()
            .iter()
            .position(|&i| i == Item::Shells)
            .unwrap();
        state.typing = Some("pwsh, bash, zsh, fish, nu, elvish, xonsh, tcsh".into());
        let text = rendered(&state, 56, 11);
        assert!(text.contains("tcsh▏"), "{text}");
        assert!(text.contains("Entrée valider"), "{text}");
    }

    #[test]
    fn tab_goes_from_section_to_section() {
        let state = state();
        let starts = state.section_starts();
        assert_eq!(starts.len(), 6);
        let items = state.items();
        assert_eq!(items[starts[1]], Item::Totals);
        assert_eq!(items[starts[4]], Item::OpenFiles);
        assert_eq!(state.next_section(true), starts[1]);
        assert_eq!(items[starts[5]], Item::Recover);
        // Back from the first section: round to the last.
        assert_eq!(state.next_section(false), starts[5]);
        let within = State {
            selected: starts[2] + 1,
            ..state
        };
        assert_eq!(within.next_section(false), starts[2]);
    }

    #[test]
    fn the_scrollbar_is_not_a_row_to_click() {
        let mut state = state();
        state.selected = state.items().len() - 1;
        let drawn = view::render(&state, 70, 14);
        let text = drawn.canvas.to_text();
        let bar = text
            .lines()
            .find_map(|line| line.chars().position(|c| c == '┃'))
            .expect("a scrollbar");
        for y in 0..14 {
            assert_eq!(drawn.at(u16::try_from(bar).unwrap(), y), None, "{text}");
        }
    }

    #[test]
    fn arrows_neither_type_nor_run_an_action() {
        let dir = herdr_api::TempDir::new("menu-arrows").unwrap();
        let mut state = state_in(&dir);
        for item in [
            Item::OpenFiles,
            Item::Reload,
            Item::Recover,
            Item::Shells,
            Item::Levels(Levels::Cpu),
        ] {
            for forward in [true, false] {
                assert!(!state.change(item, forward, false), "{item:?}");
                assert!(state.typing.is_none() && state.status.is_none(), "{item:?}");
            }
        }
    }

    #[test]
    fn arrows_stop_at_a_choice_s_ends_and_a_click_picks_one() {
        let dir = herdr_api::TempDir::new("menu-choices").unwrap();
        let mut state = state_in(&dir);
        let comma = |s: &State| s.menu.numbers.decimal_comma;
        // `decimal_comma` is on: its last value, right does nothing.
        assert!(comma(&state));
        state.change(Item::DecimalComma, true, false);
        assert!(comma(&state));
        state.change(Item::DecimalComma, false, false);
        assert!(!comma(&state));
        // Enter goes round.
        state.change(Item::DecimalComma, true, true);
        assert!(comma(&state));
        state.choose(Item::MemoryUnit, 2);
        assert_eq!(state.menu.numbers.memory_unit, herdr_api::MemoryUnit::MB);
        state.choose(Item::MemoryUnit, 0);
        assert_eq!(state.menu.numbers.memory_unit, herdr_api::MemoryUnit::M);
        assert!(matches!(
            state.shown(Item::MemoryUnit),
            Shown::Choice { current: 0, .. }
        ));
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

    #[test]
    fn a_setting_written_to_an_ignored_file_still_turns_over() {
        let dir = herdr_api::TempDir::new("menu-ignored").unwrap();
        // Ignored for another reason: an unknown setting.
        std::fs::write(dir.join(config::FILE_NAME), "unknown = 1\n").unwrap();
        std::fs::write(dir.join(sidebar::SETTINGS_FILE), "").unwrap();
        let mut state = state_in(&dir);
        let before = state.menu.numbers.decimal_comma;
        let said = state.step(Item::DecimalComma, true).unwrap();
        let sidebar = Settings::read(&dir.join(sidebar::SETTINGS_FILE));
        assert!(said.unwrap_err().starts_with("enregistré, mais"));
        // Shown as written, and written to the sidebar's file too.
        assert_eq!(state.menu.numbers.decimal_comma, !before);
        assert_eq!(sidebar.unwrap().decimal_comma, !before);
    }

    #[test]
    fn every_setting_written_is_one_the_plugins_read() {
        let dir = herdr_api::TempDir::new("menu-every-setting").unwrap();
        let mut state = state_in(&dir);
        // A file holding a setting its plugin does not know is ignored:
        // `written` says so.
        for item in state.items() {
            let said = match item {
                Item::OpenFiles | Item::Reload | Item::Recover => continue,
                Item::Shells => state.save_shells("pwsh, bash"),
                Item::Levels(which) => state.save_levels(which, "1 2 3"),
                _ => state.step(item, true).unwrap(),
            };
            assert!(said.is_ok(), "{item:?}: {said:?}");
        }
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
