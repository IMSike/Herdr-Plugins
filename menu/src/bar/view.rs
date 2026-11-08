//! What the bar shows: its icons and the sidebar's totals, the disk
//! figures, or the reload of the configuration files open in an editor.

use super::geometry::{
    Grid, ICON_ROWS, LABEL_CELLS, bar_cells, button_pair, centered, covers, wanted_rows,
};
use super::{fit_height, folders, has_its_place, my_tab};
use crate::colors::{gradient, level_color, zone};
use crate::config::{self, Action, Entry, Menu, TotalsStyle};
use crate::herdr;
use crate::settings::files;
use crate::stats::{self, Measured, Stats};
use crate::ui::{Frame, Screen, fit};
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal;
use herdr_api::NumberFormat;
use herdr_api::sidebar::{Gauge, Level, MACHINE_GLYPH, Part, Totals};
use std::collections::HashSet;
use std::io::{self, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime};

/// Wait after a resize before checking the tab, so that a drag checks once.
const SETTLE: Duration = Duration::from_millis(400);
/// How long an error message stands in for the figures.
const MESSAGE_FOR: Duration = Duration::from_secs(5);
/// First row of the disk figures: a click there measures the folder again.
const FIGURES_TOP: u16 = 1;
/// After the figures: a click measures them again (Nerd Font `nf-md-refresh`).
const REFRESH: &str = "\u{F0450}";
/// When the bar first fits its height: once the swap that opened it is done.
const FIRST_FIT_AFTER: Duration = Duration::from_millis(300);
/// The longest wait for an event: news of the measuring thread and the
/// end of a message are shown within this.
const WAKE_EVERY: Duration = Duration::from_millis(500);
/// How often a bar checks whether another bar of the folder finished its reading.
const OTHER_READING_CHECK_EVERY: Duration = Duration::from_secs(1);
/// How often the totals of the sidebar and `menu.toml` are read again.
const READ_EVERY: Duration = Duration::from_secs(2);
/// Totals older than this are no longer shown: the sidebar stopped.
const TOTALS_FOR_AT_MOST: Duration = Duration::from_secs(30);
/// How long an entry, once started, does not start again.
const LAUNCH_AGAIN_AFTER: Duration = Duration::from_secs(1);

/// The glyphs of the back (nf-md-arrow_left) and reload (nf-md-reload)
/// buttons, for their icon alone and with their name.
macro_rules! glyph {
    (back) => {
        "\u{F004D}"
    };
    (reload) => {
        "\u{F0453}"
    };
}
const BACK_ICON: &str = glyph!(back);
const RELOAD_ICON: &str = glyph!(reload);
const BACK: &str = concat!(glyph!(back), " Retour");
const RELOAD: &str = concat!(glyph!(reload), " Recharger");

/// What the measuring thread tells the bar.
enum Measure {
    /// The explorer's folder is unknown: nothing to measure.
    NoFolder,
    Started,
    Done(Measured),
    /// The reading ended before the folder's first entry: it did not answer,
    /// or could not be read.
    NoAnswer,
    /// The figures could not be kept in the state folder.
    Unsaved(String),
}

/// The bar's link to its measuring thread.
struct Measurer {
    news: mpsc::Receiver<Measure>,
    /// A click on the figures: measure now.
    ask: mpsc::Sender<()>,
    /// Set when the page closes: a reading under way stops.
    stop: Arc<AtomicBool>,
}

impl Drop for Measurer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Measures the folder in the background whenever its figures are due (see
/// `stats::lifetime`) or asked for. Figures kept in the plugin's state folder,
/// by this bar or another one of the folder, are shown without a reading.
/// The thread ends with the page, a reading under way included. The folder
/// is the explorer's of the bar `me`'s tab, asked from the thread: the bar
/// never waits for Herdr.
fn measure_in_background(me: String) -> Measurer {
    let (send, news) = mpsc::channel();
    let (ask, asked) = mpsc::channel::<()>();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = Arc::clone(&stop);
    std::thread::spawn(move || {
        let Some(root) = folders(&me).0 else {
            let _ = send.send(Measure::NoFolder);
            return;
        };
        let file = herdr_api::state_dir()
            .ok()
            .map(|dir| stats::cache_file(&dir, &root));
        let mut last: Option<Measured> = None;
        let mut manual = false;
        loop {
            let kept = file.as_deref().and_then(|f| stats::load(f, &root));
            last = Measured::newest(last.take(), kept);
            if !manual && let Some(known) = &last {
                if send.send(Measure::Done(known.clone())).is_err() {
                    return;
                }
                let due_in = known.due_in();
                if due_in > 0 {
                    match asked.recv_timeout(Duration::from_secs(due_in)) {
                        Ok(()) => manual = true,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                    continue;
                }
            }
            // Another bar of the folder measures it: its figures come
            // through the file.
            let lock = file
                .as_deref()
                .map_or(herdr_api::Taken::Unavailable, stats::reading_lock);
            if matches!(lock, herdr_api::Taken::Busy) {
                match asked.recv_timeout(OTHER_READING_CHECK_EVERY) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
                continue;
            }
            if send.send(Measure::Started).is_err() {
                return;
            }
            let limit = if manual {
                stats::Limit::manual()
            } else {
                stats::Limit::default()
            };
            let stats = stats::measure(&root, limit, &stopped);
            if stopped.load(Ordering::Relaxed) {
                return;
            }
            // Cut short before the folder's first entry, the reading tells
            // nothing: kept, it would stand for the folder for a day. The
            // next one waits for a click.
            if !stats.complete && stats.entries == 0 {
                drop(lock);
                if send.send(Measure::NoAnswer).is_err() || asked.recv().is_err() {
                    return;
                }
                manual = true;
                continue;
            }
            let measured = Measured::now(&root, stats);
            if let Some(file) = &file
                && let Err(e) = stats::save(file, &measured)
                && send.send(Measure::Unsaved(e)).is_err()
            {
                return;
            }
            if send.send(Measure::Done(measured.clone())).is_err() {
                return;
            }
            drop(lock);
            last = Some(measured);
            // Clicks made during a full reading are answered by it; one made
            // during an automatic, limited reading asks for a full one.
            let clicked = asked.try_iter().count() > 0;
            manual = clicked && !manual;
        }
    });
    Measurer { news, ask, stop }
}

/// What the bar shows.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Page {
    /// The icons, and the totals of the sidebar under them.
    Tools,
    /// The figures on the workspace folder.
    Disk,
    /// The configuration files are open in an editor: back, or reload.
    Files,
}

/// A button of the `Disk` and `Files` pages.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Button {
    Back,
    Reload,
}

/// The buttons of a page: each with its row, its first column and its text.
fn buttons(page: Page, width: u16) -> Vec<(Button, u16, u16, &'static str)> {
    match page {
        Page::Tools => Vec::new(),
        Page::Disk => vec![(Button::Back, 0, 1, BACK)],
        Page::Files => {
            let [(back_at, back), (reload_at, reload)] =
                button_pair(width, [BACK, BACK_ICON], [RELOAD, RELOAD_ICON]);
            vec![
                (Button::Back, 1, back_at, back),
                (Button::Reload, 1, reload_at, reload),
            ]
        }
    }
}

fn button_at(page: Page, width: u16, row: u16, column: u16) -> Option<Button> {
    buttons(page, width)
        .into_iter()
        .find(|&(_, r, left, text)| r == row && covers(left, text, column))
        .map(|(button, ..)| button)
}

#[derive(Clone, PartialEq)]
struct View {
    menu: Menu,
    page: Page,
    hover: Option<usize>,
    hover_button: Option<Button>,
    /// The pointer is over the figures.
    hover_figures: bool,
    /// A message, and its color, in place of the totals or the figures.
    message: Option<(String, Color)>,
    totals: Option<Totals>,
    stats: Option<Stats>,
    /// How old the figures are, once old enough to say.
    age: Option<String>,
    measuring: bool,
}

/// What an event asks for.
#[derive(Default)]
struct Asked {
    entry: Option<usize>,
    button: Option<Button>,
    measure_again: bool,
}

/// The bar while it runs: what it shows, the files it reads again, and its
/// links to the threads working for it.
struct State<'a> {
    me: &'a str,
    view: View,
    /// When the message gives its place back.
    message_until: Instant,
    menu_file: Option<PathBuf>,
    menu_changed: Option<SystemTime>,
    sidebar_state: Option<PathBuf>,
    /// The totals, read again only once the sidebar wrote them again.
    totals_file: Option<PathBuf>,
    totals_changed: Option<SystemTime>,
    totals: Option<Totals>,
    read_at: Instant,
    files_flag: Option<PathBuf>,
    /// The disk figures are measured while their page is open only.
    measurer: Option<Measurer>,
    measured_at: Option<u64>,
    /// Launches run on threads of their own: the bar keeps answering while
    /// a popup waits for the previous one to close. They send back their
    /// error.
    launch_failed: mpsc::Sender<String>,
    launch_errors: mpsc::Receiver<String>,
    /// The keys of the entries starting, and started less than
    /// `LAUNCH_AGAIN_AFTER` ago: a double click or a held key starts once.
    launching: Arc<Mutex<HashSet<char>>>,
    /// The checks of the bar's place and the reloads of Herdr's
    /// configuration run on threads too; a check sends whether the bar
    /// keeps its place.
    place_checked: mpsc::Sender<bool>,
    place_news: mpsc::Receiver<bool>,
    reloaded: mpsc::Sender<Result<String, String>>,
    reloads: mpsc::Receiver<Result<String, String>>,
    /// When the bar checks its place next.
    check_at: Option<Instant>,
}

pub(super) fn show(me: &str, menu: Menu, error: Option<String>) -> io::Result<()> {
    let _screen = Screen::enter()?;
    let mut state = State::new(me, menu, error);
    // Drawn again only when something changed.
    let mut drawn: Option<(View, (u16, u16))> = None;
    loop {
        if state.take_news().is_break() {
            return Ok(());
        }
        state.read_again();
        state.age_and_message();
        let size = terminal::size()?;
        if drawn
            .as_ref()
            .is_none_or(|(v, s)| *v != state.view || *s != size)
        {
            draw(&state.view)?;
            drawn = Some((state.view.clone(), size));
        }
        let until_check = state.check_at.map_or(Duration::MAX, |at| {
            at.saturating_duration_since(Instant::now())
        });
        if !event::poll(until_check.min(WAKE_EVERY))? {
            if state.check_at.is_some_and(|at| Instant::now() >= at) {
                state.check_at = None;
                state.check_place();
            }
            continue;
        }
        let event = event::read()?;
        state.on_event(event, size.0);
    }
}

impl<'a> State<'a> {
    fn new(me: &'a str, menu: Menu, error: Option<String>) -> Self {
        let menu_file = crate::config_dir().map(|dir| dir.join(config::FILE_NAME));
        let sidebar_state = herdr_api::sidebar::session_dir().ok();
        let files_flag = herdr_api::session_dir()
            .ok()
            .map(|dir| files::opened_flag(&dir, me));
        // One left by a bar of the same pane id that closed before reading
        // it is not for this one.
        if let Some(flag) = &files_flag {
            let _ = std::fs::remove_file(flag);
        }
        let (launch_failed, launch_errors) = mpsc::channel();
        let (place_checked, place_news) = mpsc::channel();
        let (reloaded, reloads) = mpsc::channel();
        Self {
            me,
            view: View {
                menu,
                page: Page::Tools,
                hover: None,
                hover_button: None,
                hover_figures: false,
                message: error.map(|e| (e, Color::Red)),
                totals: None,
                stats: None,
                age: None,
                measuring: false,
            },
            message_until: Instant::now() + MESSAGE_FOR,
            menu_changed: modified(menu_file.as_deref()),
            menu_file,
            totals_file: sidebar_state.as_deref().map(Totals::file),
            sidebar_state,
            totals_changed: None,
            totals: None,
            read_at: Instant::now(),
            files_flag,
            measurer: None,
            measured_at: None,
            launch_failed,
            launch_errors,
            launching: Arc::default(),
            place_checked,
            place_news,
            reloaded,
            reloads,
            // The first check fits the height once the swap is done.
            check_at: Some(Instant::now() + FIRST_FIT_AFTER),
        }
    }

    /// Shows `text` in place of the totals or the figures for a while.
    fn say(&mut self, text: String, color: Color) {
        self.view.message = Some((text, color));
        self.message_until = Instant::now() + MESSAGE_FOR;
    }

    /// What the threads sent; `Break` when the bar lost its place.
    fn take_news(&mut self) -> ControlFlow<()> {
        let news: Vec<Measure> = self
            .measurer
            .iter()
            .flat_map(|m| m.news.try_iter())
            .collect();
        for news in news {
            match news {
                Measure::NoFolder => {
                    self.say("dossier de l'explorateur inconnu".into(), Color::Red)
                }
                Measure::Started => self.view.measuring = true,
                Measure::Done(measured) => {
                    self.view.measuring = false;
                    self.view.stats = Some(measured.stats);
                    self.measured_at = Some(measured.at);
                }
                Measure::NoAnswer => {
                    self.view.measuring = false;
                    self.say("dossier illisible ou qui ne répond pas".into(), Color::Red);
                }
                Measure::Unsaved(error) => {
                    self.say(format!("chiffres non enregistrés : {error}"), Color::Red);
                }
            }
        }
        let errors: Vec<String> = self.launch_errors.try_iter().collect();
        for error in errors {
            self.say(error, Color::Red);
        }
        if self.place_news.try_iter().any(|kept| !kept) {
            return ControlFlow::Break(());
        }
        let reloads: Vec<_> = self.reloads.try_iter().collect();
        for result in reloads {
            match result {
                Ok(message) => {
                    self.view.page = Page::Tools;
                    self.say(message, Color::Green);
                }
                Err(e) => self.say(e, Color::Red),
            }
        }
        ControlFlow::Continue(())
    }

    /// The totals and `menu.toml` every `READ_EVERY`, and the flag of the
    /// files opened from this bar.
    fn read_again(&mut self) {
        if Instant::now() >= self.read_at {
            self.read_at = Instant::now() + READ_EVERY;
            let changed = modified(self.totals_file.as_deref());
            if changed != self.totals_changed {
                self.totals_changed = changed;
                self.totals = self.sidebar_state.as_deref().and_then(Totals::load);
            }
            self.view.totals = self
                .totals
                .clone()
                // Stamped in the future (the clock was set back): not fresh.
                .filter(|t| {
                    herdr_api::unix_millis()
                        .checked_sub(t.at_ms)
                        .is_some_and(|age| age <= TOTALS_FOR_AT_MOST.as_millis() as u64)
                });
            let changed = modified(self.menu_file.as_deref());
            if changed != self.menu_changed {
                self.menu_changed = changed;
                let (menu, error) = crate::menu_config();
                if menu.totals != self.view.menu.totals {
                    self.check_at = Some(Instant::now());
                }
                self.view.menu = menu;
                self.view.hover = None;
                if let Some(error) = error {
                    self.say(error, Color::Red);
                }
            }
        }
        if let Some(flag) = &self.files_flag
            && std::fs::remove_file(flag).is_ok()
        {
            self.view.page = Page::Files;
            self.view.message = None;
            self.measurer = None;
        }
    }

    /// The figures' age, and the end of the message once its time is up.
    fn age_and_message(&mut self) {
        if self.view.message.is_some() && Instant::now() >= self.message_until {
            self.view.message = None;
        }
        self.view.age = self
            .measured_at
            .and_then(|at| stats::age_label(stats::unix_now().saturating_sub(at)));
    }

    /// Checks, on a thread, that the bar keeps its place, then fits its
    /// height.
    fn check_place(&self) {
        let (me, rows, checked) = (
            self.me.to_string(),
            wanted_rows(&self.view.menu),
            self.place_checked.clone(),
        );
        std::thread::spawn(move || {
            let tab = my_tab(&me);
            let known = super::registry::Bars::new();
            if tab.is_some_and(|tab| !has_its_place(tab.iter(), &known)) {
                let _ = checked.send(false);
                return;
            }
            fit_height(&me, rows);
        });
    }

    /// Answers a key, a click, the pointer or a resize. No key closes the
    /// bar: `prefix+space` and the closing of the explorer do, so that a
    /// stray Escape never takes it away.
    fn on_event(&mut self, event: Event, width: u16) {
        let view = &mut self.view;
        let mut asked = Asked::default();
        match event {
            Event::Resize(..) => self.check_at = Some(Instant::now() + SETTLE),
            Event::Key(key)
                if key.kind == KeyEventKind::Press && !crate::ui::is_control(key.modifiers) =>
            {
                match (view.page, key.code) {
                    (Page::Tools, KeyCode::Char(c))
                        if view.menu.entries.iter().any(|e| e.key == c) =>
                    {
                        asked.entry = view.menu.entries.iter().position(|e| e.key == c);
                    }
                    (Page::Tools, _) => {}
                    (_, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace) => {
                        asked.button = Some(Button::Back);
                    }
                    (Page::Files, KeyCode::Char('r') | KeyCode::Enter) => {
                        asked.button = Some(Button::Reload);
                    }
                    (Page::Disk, KeyCode::Char('r')) => asked.measure_again = true,
                    _ => {}
                }
            }
            Event::Mouse(mouse) => {
                let button = button_at(view.page, width, mouse.row, mouse.column);
                let hit = match view.page {
                    Page::Tools => Grid::new(width, &view.menu.entries).at(mouse.row, mouse.column),
                    _ => None,
                };
                // Not while a message stands in for the figures.
                let on_figures = view.page == Page::Disk
                    && view.message.is_none()
                    && view.stats.is_some()
                    && mouse.row >= FIGURES_TOP;
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        asked = Asked {
                            entry: hit,
                            button,
                            measure_again: on_figures,
                        };
                    }
                    MouseEventKind::Moved => {
                        view.hover = hit;
                        view.hover_button = button;
                        view.hover_figures = on_figures;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        self.answer(asked);
    }

    fn answer(&mut self, asked: Asked) {
        // A measuring thread that ended (the explorer's folder was unknown)
        // starts again.
        if asked.measure_again
            && self
                .measurer
                .as_ref()
                .is_none_or(|m| m.ask.send(()).is_err())
        {
            self.measurer = Some(measure_in_background(self.me.to_string()));
        }
        match asked.button {
            Some(Button::Back) => {
                self.view.page = Page::Tools;
                self.view.message = None;
                self.view.hover_button = None;
                self.measurer = None;
            }
            Some(Button::Reload) => {
                let reloaded = self.reloaded.clone();
                std::thread::spawn(move || {
                    let _ = reloaded.send(files::reload());
                });
            }
            None => {}
        }
        if let Some(i) = asked.entry {
            self.choose(i);
        }
    }

    /// Opens the `Disk` page, or starts the entry on a thread.
    fn choose(&mut self, i: usize) {
        self.view.message = None;
        let entry = self.view.menu.entries[i].clone();
        if entry.action == Some(Action::Disk) {
            self.view.page = Page::Disk;
            self.view.hover = None;
            self.measurer = Some(measure_in_background(self.me.to_string()));
            return;
        }
        let key = entry.key;
        let starting = || self.launching.lock().unwrap_or_else(|e| e.into_inner());
        if !starting().insert(key) {
            return;
        }
        let (me, failed, launching) = (
            self.me.to_string(),
            self.launch_failed.clone(),
            self.launching.clone(),
        );
        std::thread::spawn(move || {
            if let Err(error) = launch(&me, &entry) {
                let _ = failed.send(error);
            }
            std::thread::sleep(LAUNCH_AGAIN_AFTER);
            launching
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&key);
        });
    }
}

/// When the file was last written; `None` when it cannot be read.
fn modified(file: Option<&Path>) -> Option<SystemTime> {
    std::fs::metadata(file?).and_then(|m| m.modified()).ok()
}

/// Starts the entry: the tool in the plugin's large popup or on its own, or
/// a built-in action.
fn launch(me: &str, entry: &Entry) -> Result<(), String> {
    // An action starts in no folder: asking Herdr for the tab's is not needed.
    let cwd = if entry.action.is_some() {
        None
    } else {
        let (explorer, terminal) = folders(me);
        herdr::start_folder(entry.cwd, explorer, terminal).map(|p| p.to_string_lossy().into_owned())
    };
    crate::start(entry, cwd.as_deref(), Some(me))
}

/// Draws the whole page in one write: a terminal never shows it cleared
/// but not drawn again yet (which blinks under the pointer).
fn draw(view: &View) -> io::Result<()> {
    let (width, rows) = terminal::size()?;
    let mut out = Frame::new()?;
    let cut = |text: &str| fit(text, usize::from(width));
    match view.page {
        Page::Tools => draw_tools(&mut out, view, width, rows)?,
        Page::Disk => {
            let lines: Vec<(String, String)> = match (&view.message, &view.stats) {
                (Some(_), _) => Vec::new(),
                (None, Some(stats)) => {
                    let mut files = stats::files_line(stats);
                    if let Some(age) = &view.age {
                        files = format!("{files} · {age}");
                    }
                    let refresh = if view.measuring { "…" } else { REFRESH };
                    vec![
                        (stats::size_line(stats, &view.menu.numbers), String::new()),
                        (files, format!(" {refresh}")),
                    ]
                }
                (None, None) => vec![("mesure…".into(), String::new())],
            };
            let suffix_color = if view.hover_figures {
                Color::Yellow
            } else {
                Color::DarkGrey
            };
            for (row, (text, suffix)) in (FIGURES_TOP..rows).zip(lines) {
                let whole = cut(&format!("{text}{suffix}"));
                let text = cut(&text);
                let suffix: String = whole.chars().skip(text.chars().count()).collect();
                queue!(
                    out,
                    MoveTo(centered(0, width, &whole), row),
                    SetForegroundColor(Color::DarkGrey),
                    Print(text),
                    SetForegroundColor(suffix_color),
                    Print(suffix),
                    ResetColor
                )?;
            }
        }
        Page::Files => {
            let text = cut("Fichiers de configuration ouverts");
            queue!(
                out,
                MoveTo(centered(0, width, &text), 0),
                SetForegroundColor(Color::DarkGrey),
                Print(text),
                ResetColor
            )?;
        }
    }
    for (button, row, left, text) in buttons(view.page, width) {
        let color = if view.hover_button == Some(button) {
            Color::Yellow
        } else {
            Color::Reset
        };
        queue!(
            out,
            MoveTo(left, row),
            SetForegroundColor(color),
            Print(cut(text)),
            ResetColor
        )?;
    }
    // A message under the rest: the last row the page asks for.
    if let Some((message, color)) = &view.message {
        let row = match view.page {
            Page::Tools => ICON_ROWS,
            _ => 2,
        };
        let text = cut(message);
        queue!(
            out,
            MoveTo(centered(0, width, &text), row.min(rows.saturating_sub(1))),
            SetForegroundColor(*color),
            Print(text),
            ResetColor
        )?;
    }
    out.show()
}

/// The icons, the name of the hovered one, and the totals of the sidebar
/// under them.
fn draw_tools(out: &mut impl Write, view: &View, width: u16, rows: u16) -> io::Result<()> {
    let grid = Grid::new(width, &view.menu.entries);
    let slot = grid.slot();
    for (i, entry) in view.menu.entries.iter().enumerate() {
        let left = grid.left(i);
        // More icons than columns: those past the edge are left out.
        if rows == 0 || left >= width {
            break;
        }
        let color = if view.hover == Some(i) {
            Color::Yellow
        } else {
            Color::Reset
        };
        // The icon (one cell in most fonts) in the middle of its slot.
        let icon = fit(&entry.icon(), usize::from(slot));
        queue!(
            out,
            MoveTo(centered(left, slot, &icon), 0),
            SetForegroundColor(color),
            Print(&icon),
            ResetColor
        )?;
    }
    // The name of the icon under the pointer, below the icons.
    if let Some(entry) = view.hover.and_then(|i| view.menu.entries.get(i)) {
        let name = fit(&entry.label, usize::from(width));
        queue!(
            out,
            MoveTo(centered(0, width, &name), 1),
            SetForegroundColor(Color::Yellow),
            Print(name),
            ResetColor
        )?;
    }
    if view.message.is_none()
        && let Some(totals) = &view.totals
    {
        let (numbers, row) = (&view.menu.numbers, ICON_ROWS);
        match view.menu.totals {
            TotalsStyle::Line => draw_line(out, totals, numbers, width, row, rows)?,
            TotalsStyle::Bars => draw_bars(out, totals, numbers, width, row, rows)?,
        }
    }
    Ok(())
}

/// The totals on one line, centered.
fn draw_line(
    out: &mut impl Write,
    totals: &Totals,
    numbers: &NumberFormat,
    width: u16,
    row: u16,
    rows: u16,
) -> io::Result<()> {
    if row >= rows {
        return Ok(());
    }
    let parts = totals.parts(numbers);
    let whole: String = parts.iter().map(Part::text).collect();
    if crate::ui::width(&whole) > usize::from(width) {
        let text = fit(&whole, usize::from(width));
        return queue!(out, MoveTo(centered(0, width, &text), row), Print(text));
    }
    queue!(out, MoveTo(centered(0, width, &whole), row))?;
    for part in &parts {
        let (level, bold) = match part {
            Part::Figure { level, .. } => (*level, true),
            Part::Glyph(_) => (None, true),
            Part::Gap(_) => (None, false),
        };
        draw_figure(out, part.text(), level, bold)?;
    }
    Ok(())
}

/// The totals as four bars: `CPU :`, `RAM :`, then the machine's, each
/// bar in a gradient through the colors of its thresholds, then its value.
fn draw_bars(
    out: &mut impl Write,
    totals: &Totals,
    numbers: &NumberFormat,
    width: u16,
    row: u16,
    rows: u16,
) -> io::Result<()> {
    let gauges = totals.gauges(numbers);
    let texts: Vec<String> = gauges
        .iter()
        .map(|g| g.text.clone().unwrap_or_else(|| "–".into()))
        .collect();
    let value_width = texts.iter().map(|t| crate::ui::width(t)).max().unwrap_or(0);
    let bar = bar_cells(width, value_width);
    for ((y, gauge), text) in (row..rows).zip(&gauges).zip(&texts) {
        let kind = if gauge.cpu { "CPU" } else { "RAM" };
        let label = if gauge.machine {
            format!("{kind} {MACHINE_GLYPH} :")
        } else {
            format!("{kind} :")
        };
        let level = gauge.level(totals.colors);
        // Right-aligned by cells: `format!` pads by characters.
        let value = format!(
            "{}{text}",
            " ".repeat(value_width.saturating_sub(crate::ui::width(text)))
        );
        // Too narrow for the label: the value alone.
        let Some(bar) = bar else {
            let value = fit(text, usize::from(width));
            queue!(out, MoveTo(centered(0, width, &value), y))?;
            draw_figure(out, &value, level, true)?;
            continue;
        };
        let pad = LABEL_CELLS.saturating_sub(crate::ui::width(&label));
        queue!(
            out,
            MoveTo(1, y),
            SetAttribute(Attribute::Bold),
            Print(&label),
            SetAttribute(Attribute::Reset),
            Print(" ".repeat(pad + 1))
        )?;
        if bar > 0 {
            draw_gauge(out, gauge, bar)?;
            queue!(out, Print(" "))?;
        }
        draw_figure(out, &value, level, true)?;
    }
    Ok(())
}

/// A thin bar of `width` cells, filled by half cells as far as the gauge
/// goes, each filled cell in the color its place takes in the gradient of
/// the thresholds; the empty part in the dimmed color of its level's zone.
fn draw_gauge(out: &mut impl Write, gauge: &Gauge, width: usize) -> io::Result<()> {
    let filled = gauge.fill.unwrap_or(0.0).clamp(0.0, 1.0) * width as f64;
    let mut current = None;
    for cell in 0..width {
        let covered = (filled - cell as f64).clamp(0.0, 1.0);
        let halves = (covered * 2.0).round() as usize;
        let at = (cell as f64 + 0.5) / width as f64 * gauge.full;
        let (symbol, color) = match halves {
            0 => ('─', zone(at, &gauge.thresholds)),
            1 => ('╸', gradient(at, &gauge.thresholds)),
            _ => ('━', gradient(at, &gauge.thresholds)),
        };
        if current != Some(color) {
            queue!(out, SetForegroundColor(color))?;
            current = Some(color);
        }
        queue!(out, Print(symbol))?;
    }
    queue!(out, ResetColor)
}

/// One piece of the totals: a figure in the color of its level (bold, in
/// the terminal's color without the threshold colors) or the machine's
/// glyph (bold), a separator in grey.
fn draw_figure(
    out: &mut impl Write,
    text: &str,
    level: Option<Level>,
    bold: bool,
) -> io::Result<()> {
    let color = match (bold, level) {
        (false, _) => Color::DarkGrey,
        (true, None) => Color::Reset,
        (true, Some(level)) => level_color(level),
    };
    if bold {
        queue!(out, SetAttribute(Attribute::Bold))?;
    }
    queue!(
        out,
        SetForegroundColor(color),
        Print(text),
        ResetColor,
        SetAttribute(Attribute::Reset)
    )
}
