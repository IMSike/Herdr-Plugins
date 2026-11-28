//! The settings screen, in its popup: the settings of the menu, of the
//! sidebar and of the links plugin, each written to its file as soon as it
//! changes; the configuration files opened in an editor; Herdr's
//! configuration reloaded.

mod canvas;
mod edit;
pub mod files;
mod links;
mod sessions;
mod store;
mod view;

use crate::config::{self, Menu, TotalsStyle};
use crate::ui::{Frame, Screen, is_control};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use crossterm::terminal;
use files::Files;
use herdr_api::NumberFormat;
use herdr_api::links::FileAction;
use herdr_api::sidebar::{self, Levels, Settings, Thresholds, check_thresholds};
use std::io;
use toml_edit::Value;
use view::Target;

/// The `Tools` pane the screen was opened from, if any.
pub const BAR_ENV: &str = "HERDR_MENU_BAR";
/// A step of `ttl_seconds`, and its bounds.
const TTL_STEP: u64 = 5;
const TTL_MAX: u64 = 600;
/// The rows of the list a notch of the wheel scrolls.
const WHEEL_ROWS: usize = 3;
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

/// The tabs of the screen, each a few sections.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tab {
    Display,
    Sidebar,
    Links,
    System,
}

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Self::Display => "Affichage",
            Self::Sidebar => "Sidebar",
            Self::Links => "Liens",
            Self::System => "Système",
        }
    }
}

/// The sections of a tab.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Section {
    Numbers,
    Totals,
    Colors,
    Measures,
    Links,
    Danger,
    Updates,
    Files,
    Session,
}

impl Section {
    fn title(self) -> &'static str {
        match self {
            Self::Numbers => "Format des nombres",
            Self::Totals => "Totaux CPU / mémoire",
            Self::Colors => "Couleurs par seuil",
            Self::Measures => "Mesures",
            Self::Links => "Liens",
            Self::Danger => "Danger",
            Self::Updates => "Mises à jour",
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
    /// A number changed by steps, or typed after Enter.
    Number(String),
    /// A list, each element on its own: one chosen with the arrows, then
    /// changed or removed; one added after the last.
    List(Vec<String>),
    /// Three thresholds, one chosen with the arrows, then typed; and their
    /// unit.
    Levels(Vec<String>, &'static str),
    /// Something done on Enter.
    Action,
}

/// A change that waits for Enter: one that could do harm, or that empties
/// a list on purpose.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Confirm {
    RunPrograms,
    EmptyTypes,
}

impl Confirm {
    /// What the status line asks.
    fn question(self) -> &'static str {
        match self {
            Self::RunPrograms => {
                "Lancer les programmes ? Un Ctrl+clic pourra exécuter un programme ou un script."
            }
            Self::EmptyTypes => {
                "Vider la liste ? Plus aucun fichier ne s'ouvrira avec son application."
            }
        }
    }
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
    Update,
    Declined,
    Ttl,
    Shells,
    TabBar,
    Colors,
    Levels(Levels),
    LinkFile,
    OpenTypes,
    RunPrograms,
    ConfirmPrograms,
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
            Self::Totals => "Pane Tools",
            Self::CheckUpdates => "Signaler les mises à jour",
            // nf-md-update, the icon the `Update` entry had
            Self::Update => "\u{F06B0} Mettre à jour…",
            // nf-md-puzzle, a plugin
            Self::Declined => "\u{F0431} Reproposer les plugins refusés",
            Self::Ttl => "Durée de vie d'une valeur",
            Self::Shells => "Shells",
            Self::TabBar => "Barre d'onglets",
            Self::Colors => "Couleurs",
            Self::Levels(Levels::Cpu) => "Seuils CPU (Herdr)",
            Self::Levels(Levels::Memory) => "Seuils mémoire (Herdr)",
            Self::Levels(Levels::MachineCpu) => "Seuils CPU (machine)",
            Self::Levels(Levels::MachineMemory) => "Seuils mémoire (machine)",
            Self::LinkFile => "Ctrl+clic sur un fichier",
            Self::OpenTypes => "Types ouverts",
            Self::RunPrograms => "Lancer les programmes",
            Self::ConfirmPrograms => "Confirmer avant d'ouvrir",
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
        matches!(
            self,
            Self::Update | Self::Declined | Self::OpenFiles | Self::Reload | Self::Recover
        )
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
    /// `None` when the links plugin is not installed.
    links: Option<herdr_api::links::Settings>,
    /// `links.toml` cannot be used: the links plugin sets it aside, and the
    /// screen shows what it goes by instead. Nothing is written to it until
    /// it can be used again: the user's own values would be lost under the
    /// fallback ones.
    links_set_aside: bool,
    /// The tab on screen, and the selected item among its items.
    tab: Tab,
    selected: usize,
    /// The first row of the list on screen.
    top: usize,
    /// The list scrolls to show the selection: after a key, not after the
    /// mouse, which leaves the list where it is (the wheel moves it alone).
    follow: bool,
    /// The part of the selected item the arrows chose: an element of a list
    /// (its length for the one to add), or a threshold.
    part: usize,
    /// The last thing done, or why it failed (in red).
    status: Option<(String, bool)>,
    /// The text being typed for the selected item's part.
    typing: Option<String>,
    /// The text typed is the value it started from: the first character
    /// typed replaces it, as a selected text would be.
    fresh: bool,
    /// A change that waits for Enter, said on the status line.
    confirm: Option<Confirm>,
    /// The last change made to the files, to undo it; and whether it was
    /// undone already (undoing it again redoes it).
    undo: Option<edit::Change>,
    undone: bool,
    /// Where the mouse is: the part of the screen under it is highlighted.
    hover: Option<(u16, u16)>,
    /// Set when the screen closes to show the session's snapshots.
    recover: bool,
    /// What has a new version (`Herdr 0.9.6 · Explorer`), from the last
    /// check, when `check_updates` is on.
    news: Option<String>,
    /// The plugins of a repository left unticked by the update, not
    /// offered again: by id.
    declined: Vec<String>,
}

impl State {
    fn new(
        files: Files,
        menu: Menu,
        sidebar: Option<Settings>,
        links: Option<herdr_api::links::Settings>,
    ) -> Self {
        Self {
            files,
            menu,
            sidebar,
            links,
            links_set_aside: false,
            tab: Tab::Display,
            selected: 0,
            top: 0,
            follow: true,
            part: 0,
            status: None,
            typing: None,
            fresh: false,
            confirm: None,
            undo: None,
            undone: false,
            hover: None,
            recover: false,
            news: None,
            declined: Vec::new(),
        }
    }

    /// The tabs shown: Sidebar and Liens when their plugin is installed.
    fn tabs(&self) -> Vec<Tab> {
        let mut tabs = vec![Tab::Display];
        if self.sidebar.is_some() {
            tabs.push(Tab::Sidebar);
        }
        if self.links.is_some() {
            tabs.push(Tab::Links);
        }
        tabs.push(Tab::System);
        tabs
    }

    /// The tab on screen: the first when its plugin is gone.
    fn current_tab(&self) -> Tab {
        if self.tabs().contains(&self.tab) {
            self.tab
        } else {
            Tab::Display
        }
    }

    /// Shows `tab`, from its first item; the screen stays open.
    fn show_tab(&mut self, tab: Tab) -> bool {
        if tab != self.current_tab() {
            self.tab = tab;
            self.selected = 0;
            self.part = 0;
            self.top = 0;
        }
        false
    }

    /// Shows the next tab (`forward`) or the one before, round the tabs.
    fn next_tab(&mut self, forward: bool) -> bool {
        let tabs = self.tabs();
        let at = tabs
            .iter()
            .position(|&t| t == self.current_tab())
            .unwrap_or(0);
        let next = if forward {
            (at + 1) % tabs.len()
        } else {
            (at + tabs.len() - 1) % tabs.len()
        };
        self.show_tab(tabs[next])
    }

    /// The rows of the tab on screen.
    fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        match self.current_tab() {
            Tab::Display => {
                rows.extend([
                    Row::Header(Section::Numbers),
                    Row::Item(Item::DecimalComma),
                    Row::Item(Item::PercentSpace),
                    Row::Item(Item::CpuDecimal),
                    Row::Item(Item::MemoryUnit),
                    Row::Item(Item::MemorySpace),
                    Row::Blank,
                    Row::Header(Section::Totals),
                    Row::Item(Item::Totals),
                ]);
                if self.sidebar.is_some() {
                    rows.extend([
                        Row::Item(Item::TabBar),
                        Row::Blank,
                        Row::Header(Section::Colors),
                        Row::Item(Item::Colors),
                    ]);
                    rows.extend(Levels::ALL.map(|l| Row::Item(Item::Levels(l))));
                }
            }
            Tab::Sidebar => rows.extend([
                Row::Header(Section::Measures),
                Row::Item(Item::Ttl),
                Row::Item(Item::Shells),
            ]),
            Tab::Links => {
                rows.extend([
                    Row::Header(Section::Links),
                    Row::Item(Item::LinkFile),
                    Row::Item(Item::OpenTypes),
                    Row::Blank,
                    Row::Header(Section::Danger),
                    Row::Item(Item::RunPrograms),
                ]);
                // Only meaningful once programs may run.
                if self.links.as_ref().is_some_and(|l| l.run_programs) {
                    rows.push(Row::Item(Item::ConfirmPrograms));
                }
            }
            Tab::System => {
                rows.extend([
                    Row::Header(Section::Updates),
                    Row::Item(Item::CheckUpdates),
                    Row::Item(Item::Update),
                ]);
                if !self.declined.is_empty() {
                    rows.push(Row::Item(Item::Declined));
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
            }
        }
        rows
    }

    /// The items of every tab.
    #[cfg(test)]
    fn all_items(&mut self) -> Vec<Item> {
        let (tab, selected) = (self.tab, self.selected);
        let mut items = Vec::new();
        for t in self.tabs() {
            self.tab = t;
            items.extend(self.items());
        }
        (self.tab, self.selected) = (tab, selected);
        items
    }

    /// Shows the tab of `item`, and selects it.
    #[cfg(test)]
    fn focus(&mut self, item: Item) {
        for t in self.tabs() {
            self.tab = t;
            if let Some(index) = self.items().iter().position(|&i| i == item) {
                self.selected = index;
                return;
            }
        }
        panic!("{item:?} on no tab");
    }

    /// Selects the item `index`, from its first part; the screen stays open.
    fn select(&mut self, index: usize) -> bool {
        if index != self.selected {
            self.selected = index;
            self.part = 0;
        }
        false
    }

    /// Shows the list from row `top`, the selection left where it is; the
    /// screen stays open.
    fn scroll(&mut self, top: usize) -> bool {
        self.top = top;
        false
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
            Item::Shells => Shown::List(sidebar.map_or_else(Vec::new, |s| s.shells.clone())),
            Item::TabBar => yes_no(sidebar.is_some_and(|s| s.tab_bar)),
            Item::Colors => yes_no(sidebar.is_some_and(|s| s.colors)),
            Item::Levels(which) => Shown::Levels(
                sidebar.map_or_else(Vec::new, |s| {
                    which.of(s).iter().map(|v| number(*v)).collect()
                }),
                levels_unit(which),
            ),
            Item::LinkFile => Shown::Choice {
                options: FileAction::ALL
                    .map(|a| links::file_label(a).into())
                    .to_vec(),
                current: self.links.as_ref().map_or(0, |l| links::file_index(l.file)),
            },
            Item::OpenTypes => Shown::List(
                self.links
                    .as_ref()
                    .map_or_else(Vec::new, |l| l.open_types.clone()),
            ),
            Item::RunPrograms => yes_no(self.links.as_ref().is_some_and(|l| l.run_programs)),
            Item::ConfirmPrograms => {
                yes_no(self.links.as_ref().is_some_and(|l| l.confirm_programs))
            }
            Item::Update | Item::Declined | Item::OpenFiles | Item::Reload | Item::Recover => {
                Shown::Action
            }
        }
    }

    /// What an item does, and the setting and files it writes.
    fn about(&self, item: Item) -> (&'static str, String) {
        let menu = config::FILE_NAME;
        let side = sidebar::SETTINGS_FILE;
        let link = herdr_api::links::SETTINGS_FILE;
        let both = match self.sidebar {
            Some(_) => format!("{menu}, {side}"),
            None => menu.to_string(),
        };
        let (what, key, file) = match item {
            Item::DecimalComma => (
                "Sépare les décimales par une virgule (2,4 Go) ou par un point (2.4 Go), dans le pane Tools comme dans la sidebar.",
                "decimal_comma",
                both,
            ),
            Item::PercentSpace => (
                "Met une espace fine entre un pourcentage et son signe : 4 % plutôt que 4%.",
                "percent_space",
                both,
            ),
            Item::CpuDecimal => (
                "Écrit les parts de CPU des totaux avec une décimale (4,2%) ou arrondies à l'unité (4%).",
                "cpu_decimal",
                both,
            ),
            Item::MemoryUnit => (
                "Comment s'écrivent les tailles : la lettre seule (K, M, G), à la française (Ko, Mo, Go) ou à l'anglaise (KB, MB, GB).",
                "memory_unit",
                both,
            ),
            Item::MemorySpace => (
                "Met une espace fine entre une taille et son unité : 2,4 Go plutôt que 2,4Go.",
                "memory_space",
                both,
            ),
            Item::Totals => (
                "Comment le pane Tools montre les totaux CPU et mémoire, de Herdr et de la machine, sous ses icônes : une ligne de texte, ou quatre barres qui se remplissent.",
                "totals",
                menu.into(),
            ),
            Item::CheckUpdates => (
                "Cherche les nouvelles versions de Herdr et des plugins installés depuis GitHub, au démarrage puis toutes les 4 h tant qu'un pane Tools est ouvert : Settings et « Mettre à jour » passent alors en vert.",
                "check_updates",
                menu.into(),
            ),
            Item::Update => (
                "Met à jour Herdr et les plugins installés depuis GitHub, dans une fenêtre à part, et ferme cet écran.",
                "",
                String::new(),
            ),
            Item::Declined => (
                "Les plugins d'un dépôt laissés décochés à la mise à jour : proposés de nouveau à la prochaine vérification.",
                "",
                String::new(),
            ),
            Item::Ttl => (
                "Combien de secondes la sidebar garde une valeur (CPU, mémoire, shells) quand plus rien ne la mesure : passé ce délai, elle disparaît plutôt que de rester périmée. De 1 à 600 s ; Entrée pour la taper.",
                "ttl_seconds",
                side.into(),
            ),
            Item::Shells => (
                "Les noms de programmes que la sidebar compte comme des shells sous un agent (sans .exe, majuscules ou non) : un shell sous un agent signale qu'il est au travail.",
                "shells",
                side.into(),
            ),
            Item::TabBar => (
                "Montre aussi les totaux CPU et mémoire, de Herdr et de la machine, à droite de la barre d'onglets.",
                "tab_bar",
                side.into(),
            ),
            Item::Colors => (
                "Colore les chiffres de la sidebar en vert, jaune, orange ou rouge selon les seuils ci-dessous ; sur non, les seuils sont sans effet.",
                "colors",
                side.into(),
            ),
            Item::Levels(Levels::Cpu) => (
                "La part de CPU de Herdr et de tout ce qu'il lance où la couleur passe au jaune, à l'orange, puis au rouge : trois valeurs croissantes, en %.",
                Levels::Cpu.key(),
                side.into(),
            ),
            Item::Levels(Levels::Memory) => (
                "La mémoire utilisée par Herdr et tout ce qu'il lance où la couleur passe au jaune, à l'orange, puis au rouge : trois valeurs croissantes, en Go.",
                Levels::Memory.key(),
                side.into(),
            ),
            Item::Levels(Levels::MachineCpu) => (
                "La part de CPU de toute la machine où la couleur passe au jaune, à l'orange, puis au rouge : trois valeurs croissantes, en %.",
                Levels::MachineCpu.key(),
                side.into(),
            ),
            Item::Levels(Levels::MachineMemory) => (
                "La part de mémoire utilisée de toute la machine où la couleur passe au jaune, à l'orange, puis au rouge : trois valeurs croissantes, en %.",
                Levels::MachineMemory.key(),
                side.into(),
            ),
            Item::LinkFile => (
                "Ce que fait un Ctrl+clic sur un fichier affiché par dx : l'aperçu de herdr-explorer (sans lui, l'application), sa sélection dans l'Explorateur de fichiers, ou son ouverture avec son application.",
                "file",
                link.into(),
            ),
            Item::OpenTypes => (
                "Les extensions qu'un Ctrl+clic peut ouvrir avec leur application par défaut ; un fichier d'un autre type est seulement sélectionné. Seule la dernière extension compte (gz, pas tar.gz).",
                "open_types",
                link.into(),
            ),
            Item::RunPrograms => (
                "DANGER : un fichier d'un autre type s'ouvre aussi avec son programme : un Ctrl+clic sur un programme ou un script le lance. Demande une confirmation.",
                "run_programs",
                link.into(),
            ),
            Item::ConfirmPrograms => (
                "Avant d'ouvrir un fichier d'un autre type, montre son vrai chemin et attend Entrée.",
                "confirm_programs",
                link.into(),
            ),
            Item::OpenFiles => (
                "Ouvre les fichiers de configuration (Herdr et les plugins) dans un éditeur, et ferme cet écran.",
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
    /// stops at its first or last one otherwise, as its row shows them; the
    /// arrows choose an element of a list or a threshold. Only `round`
    /// (Enter, a click) types a value or runs an action.
    fn change(&mut self, item: Item, forward: bool, round: bool) -> bool {
        match self.shown(item) {
            Shown::Choice { options, current } if !round => {
                let at_end = if forward {
                    current + 1 >= options.len()
                } else {
                    current == 0
                };
                if at_end {
                    return false;
                }
            }
            // The element to add, after the last, is one more part.
            Shown::List(list) if !round => {
                self.move_part(forward, list.len() + 1);
                return false;
            }
            Shown::Levels(values, _) if !round => {
                self.move_part(forward, values.len());
                return false;
            }
            Shown::List(_) | Shown::Levels(..) | Shown::Number(_) if round => {
                self.start_typing(item);
                return false;
            }
            Shown::Action if !round => return false,
            _ => {}
        }
        if item == Item::Recover {
            self.recover = true;
            return true;
        }
        if item == Item::Update {
            return match crate::update::open_window() {
                Ok(()) => true,
                Err(e) => {
                    self.report(Err(e));
                    false
                }
            };
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
        // From the value the file holds now, which the screen may not show.
        if matches!(
            item,
            Item::LinkFile | Item::RunPrograms | Item::ConfirmPrograms
        ) && let Some(Err(e)) = self.check_links_file()
        {
            return self.report(Err(e));
        }
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
            // Typed, see `change`.
            Item::Shells | Item::Levels(_) | Item::OpenTypes => return None,
            Item::LinkFile => self.step_link_file(forward)?,
            // As the file holds it now, which the screen may not show yet:
            // letting programs run waits for Enter, stopping them does not.
            Item::RunPrograms => match self.check_links_file()? {
                Err(e) => Err(e),
                Ok(()) if self.links.as_ref()?.run_programs => self.set_run_programs(false)?,
                Ok(()) => {
                    self.confirm = Some(Confirm::RunPrograms);
                    return None;
                }
            },
            Item::ConfirmPrograms => {
                self.toggle_links("confirm_programs", |l| l.confirm_programs)?
            }
            // See `change`: they close the screen.
            Item::Update | Item::OpenFiles | Item::Recover => return None,
            Item::Reload => files::reload(),
            Item::Declined => self.forget_declined(),
        })
    }

    /// Forgets the plugins declined: the update offers them again. The
    /// line goes, the selection stays on the update's.
    fn forget_declined(&mut self) -> Result<String, String> {
        let ids = crate::update::discover::forget_declined()?;
        self.declined.clear();
        self.selected = self.selected.saturating_sub(1);
        Ok(format!(
            "proposés de nouveau à la prochaine vérification : {}",
            ids.join(", ")
        ))
    }

    /// Writes `shells` to `sidebar.toml`.
    fn save_shells(&mut self, shells: Vec<String>) -> Result<String, String> {
        let array: toml_edit::Array = shells.iter().map(String::as_str).collect();
        self.set_in_sidebar("shells", array.into(), |s| s.shells = shells)
            .unwrap_or_else(|| Ok(String::new()))
    }

    /// Writes a set of thresholds to `sidebar.toml`, once they are in order.
    fn save_levels(&mut self, which: Levels, thresholds: Thresholds) -> Result<String, String> {
        check_thresholds(&thresholds)?;
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
    let news = crate::update::check::news(menu.check_updates);
    let links_file = files.links.clone();
    let mut state = State::new(files, menu, sidebar, None);
    // Shown as the links plugin uses them too: a file it sets aside only
    // selects.
    let links_error = links_file.and_then(|path| state.reread_links(&path));
    state.news = news;
    state.declined = crate::update::discover::declined();
    state.status = menu_error
        .or(sidebar_error)
        .or(links_error)
        .map(|e| (e, true));
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
        let event = event::read()?;
        match event {
            Event::Key(_) => state.follow = true,
            Event::Mouse(mouse) => {
                state.follow = false;
                state.hover = Some((mouse.column, mouse.row));
            }
            _ => {}
        }
        if handle(state, &drawn, &event) {
            return Ok(());
        }
    }
}

/// Answers `event` on the screen drawn as `drawn`; whether the screen
/// should close.
fn handle(state: &mut State, drawn: &view::Drawn, event: &Event) -> bool {
    let items = state.items();
    let last = items.len() - 1;
    // A row gone since (after an undo, say): no selection past the last.
    state.selected = state.selected.min(last);
    let item = items[state.selected];
    let press = match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => Some(*key),
        _ => None,
    };
    let click = match event {
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
            Some(drawn.at(mouse.column, mouse.row))
        }
        _ => None,
    };
    // A change waiting for Enter: Enter or its button does it, any other
    // key or click drops it.
    if state.confirm.is_some() {
        match (press, click) {
            (Some(key), _) => state.tracked(|s| s.answer(key.code == KeyCode::Enter)),
            (_, Some(Some(Target::Confirm(yes)))) => state.tracked(|s| s.answer(yes)),
            (_, Some(_)) => state.answer(false),
            _ => {}
        }
        return false;
    }
    if let Some(text) = &mut state.typing {
        if let Some(key) = press {
            let fresh = std::mem::take(&mut state.fresh);
            match key.code {
                KeyCode::Enter => {
                    let text = std::mem::take(text);
                    state.typing = None;
                    state.tracked(|s| s.save_typed(item, &text));
                }
                KeyCode::Esc => state.typing = None,
                // The value it started from goes at once.
                KeyCode::Backspace if fresh => text.clear(),
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Char(c) if !is_control(key.modifiers) => {
                    if fresh {
                        text.clear();
                    }
                    text.push(c);
                }
                _ => state.fresh = fresh,
            }
        }
        return false;
    }
    if let Some(key) = press {
        // Ctrl+L and the like are no l: only Ctrl+C means something.
        if is_control(key.modifiers) {
            return key.code == KeyCode::Char('c');
        }
        return match key.code {
            KeyCode::Esc | KeyCode::Char('q') => true,
            KeyCode::Up | KeyCode::Char('k') => state.select((state.selected + last) % items.len()),
            KeyCode::Down | KeyCode::Char('j') => state.select((state.selected + 1) % items.len()),
            KeyCode::Home | KeyCode::Char('g') => state.select(0),
            KeyCode::End | KeyCode::Char('G') => state.select(last),
            KeyCode::Tab => state.next_tab(true),
            KeyCode::BackTab => state.next_tab(false),
            KeyCode::Char(c @ '1'..='9') => match state.tabs().get(usize::from(c as u8 - b'1')) {
                Some(&tab) => state.show_tab(tab),
                None => false,
            },
            KeyCode::Left | KeyCode::Char('h') => state.change(item, false, false),
            KeyCode::Right | KeyCode::Char('l') => state.change(item, true, false),
            KeyCode::Enter | KeyCode::Char(' ') => state.tracked(|s| s.change(item, true, true)),
            KeyCode::Delete | KeyCode::Backspace => {
                state.tracked(|s| s.remove_part(item));
                false
            }
            KeyCode::Char('r') if !state.is_default(item) => {
                state.tracked(|s| s.reset(item));
                false
            }
            KeyCode::Char('u') => {
                state.undo();
                false
            }
            _ => false,
        };
    }
    let Event::Mouse(mouse) = event else {
        return false;
    };
    match (mouse.kind, click) {
        (MouseEventKind::Moved, _) => {
            match drawn.at(mouse.column, mouse.row).and_then(Target::item) {
                Some(index) => state.select(index),
                None => false,
            }
        }
        (_, Some(Some(target))) => {
            if let Some(index) = target.item() {
                state.select(index);
            }
            match target {
                Target::Tab(tab) => match state.tabs().get(tab) {
                    Some(&tab) => state.show_tab(tab),
                    None => false,
                },
                // A list waits for its element to be chosen.
                Target::Item(index) if matches!(state.shown(items[index]), Shown::List(_)) => false,
                Target::Item(index) => state.tracked(|s| s.change(items[index], true, true)),
                Target::Choose(index, option) => {
                    state.tracked(|s| s.choose(items[index], option));
                    false
                }
                Target::Step(index, forward) => {
                    state.tracked(|s| s.change(items[index], forward, false))
                }
                // A list's element is chosen; the one to add, or a threshold,
                // typed at once.
                Target::Part(index, part) => {
                    state.part = part;
                    let at_end = matches!(state.shown(items[index]), Shown::List(list) if part >= list.len());
                    if at_end || matches!(items[index], Item::Levels(_)) {
                        state.start_typing(items[index]);
                    }
                    false
                }
                Target::Remove(index, part) => {
                    state.part = part;
                    state.tracked(|s| s.remove_part(items[index]));
                    false
                }
                Target::Reset(index) => {
                    state.tracked(|s| s.reset(items[index]));
                    false
                }
                Target::Undo => {
                    state.undo();
                    false
                }
                Target::Confirm(_) => false,
            }
        }
        (MouseEventKind::ScrollDown, _) => {
            state.scroll((drawn.top + WHEEL_ROWS).min(drawn.max_top))
        }
        (MouseEventKind::ScrollUp, _) => state.scroll(drawn.top.saturating_sub(WHEEL_ROWS)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The screen on the files of `dir`, the sidebar and the links plugin
    /// installed.
    pub(super) fn state_in(dir: &std::path::Path) -> State {
        let files = Files {
            menu: Some(dir.join(config::FILE_NAME)),
            sidebar: Some(dir.join(sidebar::SETTINGS_FILE)),
            links: Some(dir.join(herdr_api::links::SETTINGS_FILE)),
        };
        State::new(
            files,
            Menu::default(),
            Some(Settings::default()),
            Some(herdr_api::links::Settings::default()),
        )
    }

    /// The screen without files, the sidebar and the links plugin installed.
    pub(super) fn state() -> State {
        let files = Files {
            menu: None,
            sidebar: None,
            links: None,
        };
        State::new(
            files,
            Menu::default(),
            Some(Settings::default()),
            Some(herdr_api::links::Settings::default()),
        )
    }

    /// The screen without files, as `width` × `height` cells of text.
    pub(super) fn rendered(state: &State, width: u16, height: u16) -> String {
        view::render(state, width, height).canvas.to_text()
    }

    #[test]
    fn the_screen_shows_every_setting_in_its_section() {
        let mut state = state();
        state.status = Some(("enregistré dans menu.toml".into(), false));
        let text = rendered(&state, 92, 60);
        for expected in [
            "Réglages",
            "Format des nombres",
            "Virgule décimale",
            " 2.4G   2,4G",
            " 2,4G   2,4Go   2,4GB",
            "━━ 10 ━━ 25 ━━ 50 ━━ %",
            " Affichage ",
            " Sidebar ",
            " Liens ",
            " Système ",
            "Barre d'onglets",
            "Couleurs par seuil",
            "Seuils mémoire (Herdr)",
            "✓ enregistré dans menu.toml",
            "decimal_comma · menu.toml, sidebar.toml",
            "↑↓ naviguer",
        ] {
            assert!(text.contains(expected), "{expected:?} missing:\n{text}");
        }
        assert!(!text.contains("Mesures"), "{text}");
        state.next_tab(true);
        let text = rendered(&state, 92, 60);
        assert!(
            text.contains("Mesures") && text.contains("‹ 20 s ›"),
            "{text}"
        );
        // Without the sidebar, its tab and sections are gone.
        state.sidebar = None;
        let text = rendered(&state, 92, 60);
        assert!(
            !text.contains("Sidebar") && !text.contains("Seuils"),
            "{text}"
        );
        assert!(text.contains("Format des nombres"), "{text}");
    }

    #[test]
    fn tab_goes_from_tab_to_tab() {
        let mut state = state();
        assert_eq!(
            state.tabs(),
            [Tab::Display, Tab::Sidebar, Tab::Links, Tab::System]
        );
        state.selected = 3;
        state.next_tab(true);
        assert_eq!((state.tab, state.selected), (Tab::Sidebar, 0));
        state.next_tab(false);
        state.next_tab(false);
        assert_eq!(state.tab, Tab::System);
        state.next_tab(true);
        assert_eq!(state.tab, Tab::Display);
        // The same tab keeps its selection.
        state.selected = 2;
        state.show_tab(Tab::Display);
        assert_eq!(state.selected, 2);
        // Without the links plugin, its tab is gone.
        state.links = None;
        state.show_tab(Tab::System);
        state.next_tab(false);
        assert_eq!(state.tab, Tab::Sidebar);
        state.tab = Tab::Links;
        assert_eq!(state.current_tab(), Tab::Display);
    }

    #[test]
    fn a_long_list_shows_on_several_lines() {
        let mut state = state();
        state.focus(Item::OpenTypes);
        if let Some(links) = &mut state.links {
            links.open_types = (1..=30).map(|i| format!("ext{i}")).collect();
        }
        let text = rendered(&state, 70, 40);
        assert!(text.contains(" ext1✕  ext2 "), "{text}");
        assert!(text.contains(" ext30   +"), "{text}");
        let list = text
            .lines()
            .filter(|line| line.contains("ext") && !line.contains("extension"));
        assert!(list.clone().count() > 1, "{text}");
        assert!(list.clone().all(|line| !line.contains('…')), "{text}");
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
            for tab in state.tabs() {
                state.tab = tab;
                for selected in 0..state.items().len() {
                    state.selected = selected;
                    state.typing = (selected % 2 == 0).then(|| "pwsh, bash, zsh, fish".into());
                    let drawn = view::render(&state, w, h);
                    state.top = drawn.top;
                }
            }
        }
    }

    #[test]
    fn the_wheel_scrolls_the_list_not_the_selection() {
        let mut state = state();
        let drawn = view::render(&state, 70, 14);
        assert_eq!(drawn.top, 0);
        // Scrolled past its end: stops at the last rows.
        state.scroll(usize::MAX);
        state.follow = false;
        let drawn = view::render(&state, 70, 14);
        assert_eq!(drawn.top, drawn.max_top);
        assert!(drawn.max_top > 0);
        assert_eq!(state.selected, 0);
        let text = drawn.canvas.to_text();
        assert!(!text.contains("Virgule décimale"), "{text}");
        // A key brings the selection back into view.
        state.follow = true;
        let text = rendered(&state, 70, 14);
        assert!(text.contains("Virgule décimale"), "{text}");
    }

    #[test]
    fn a_short_screen_scrolls_to_the_selection() {
        let mut state = state();
        state.selected = state.items().len() - 1;
        let text = rendered(&state, 70, 14);
        assert!(text.contains("Seuils mémoire (machine)"), "{text}");
        assert!(!text.contains("Virgule décimale"), "{text}");
        // The scrollbar's thumb, at the bottom.
        assert!(text.contains('┃'), "{text}");
    }

    #[test]
    fn typed_text_shows_its_end_and_the_keys_to_save_it() {
        let mut state = state();
        state.focus(Item::Shells);
        state.typing = Some("pwsh, bash, zsh, fish, nu, elvish, xonsh, tcsh".into());
        let text = rendered(&state, 56, 11);
        assert!(text.contains("tcsh▏"), "{text}");
        assert!(text.contains("Entrée valider"), "{text}");
    }

    #[test]
    fn the_update_says_what_has_a_new_version() {
        let mut state = state();
        state.show_tab(Tab::System);
        let text = rendered(&state, 92, 40);
        assert!(text.contains("Mises à jour") && text.contains("Mettre à jour…"));
        assert!(!text.contains("Herdr 0.9.6"));
        state.news = Some("Herdr 0.9.6 · Explorer".into());
        let text = rendered(&state, 92, 40);
        assert!(
            text.contains("Mettre à jour… · Herdr 0.9.6 · Explorer"),
            "{text}"
        );
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
    fn thresholds_show_as_written() {
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
        for item in state.all_items() {
            let said = match item {
                Item::Update | Item::Declined | Item::OpenFiles | Item::Reload | Item::Recover => {
                    continue;
                }
                Item::Shells => state.save_shells(vec!["pwsh".into(), "bash".into()]),
                Item::Levels(which) => state.save_levels(which, [1.0, 2.0, 3.0]),
                Item::OpenTypes => {
                    let types = ["md", "txt", "rs"].map(String::from).to_vec();
                    let written = state.write_types(types).unwrap();
                    assert!(written.is_ok(), "{written:?}");
                    let added = state.add_type(".$$$");
                    assert!(added.is_ok(), "{added:?}");
                    state.remove_type("TXT")
                }
                Item::RunPrograms => state.set_run_programs(true).unwrap(),
                _ => state.step(item, true).unwrap(),
            };
            assert!(said.is_ok(), "{item:?}: {said:?}");
        }
        // Shown once programs may run.
        assert!(state.all_items().contains(&Item::ConfirmPrograms));
        let said = state.step(Item::ConfirmPrograms, true).unwrap();
        assert!(said.is_ok(), "{said:?}");
        let written = herdr_api::links::Settings::read(&dir.join(herdr_api::links::SETTINGS_FILE))
            .unwrap()
            .settings;
        assert_eq!(written, *state.links.as_ref().unwrap());
        assert_eq!(written.file, herdr_api::links::FileAction::Select);
        assert_eq!(written.open_types, ["md", "rs", "$$$"]);
        assert!(written.run_programs && !written.confirm_programs);
    }

    #[test]
    fn thresholds_written_read_back_as_they_are() {
        let file =
            std::env::temp_dir().join(format!("herdr-menu-levels-{}.toml", std::process::id()));
        for thresholds in [[10.0, 25.0, 50.0], [0.5, 1.0, 2.5], [1e19, 2e19, 3e19]] {
            let line = format!("cpu_levels = {}\n", levels_array(&thresholds));
            std::fs::write(&file, line).unwrap();
            let read = Settings::read(&file).unwrap();
            assert_eq!(*Levels::Cpu.of(&read), thresholds, "{thresholds:?}");
        }
        assert_eq!(
            levels_array(&[10.0, 25.0, 50.0]).to_string(),
            "[10, 25, 50]"
        );
        let _ = std::fs::remove_file(file);
    }
}
