//! What the bar shows: its icons and the sidebar's totals, the disk
//! figures, or the reload of the configuration files open in an editor.

use super::colors::{gradient, level_color, zone};
use super::geometry::{Grid, centered, wanted_rows};
use super::{
    FIGURES_TOP, FIRST_FIT_AFTER, MESSAGE_FOR, OTHER_READING_CHECK_EVERY, REFRESH, SETTLE,
    WAKE_EVERY, fit_height, folders, has_its_place, my_tab,
};
use crate::config::{self, Action, Entry, IconLayout, Menu, TotalsStyle};
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
use herdr_api::sidebar::{Gauge, Level, MACHINE_GLYPH, Totals};
use std::collections::HashSet;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime};

/// What the measuring thread tells the bar.
enum Measure {
    /// The explorer's folder is unknown: nothing to measure.
    NoFolder,
    Started,
    Done(Measured),
    /// The reading ended before the folder's first entry: it did not answer.
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

/// Back (nf-md-arrow_left) and reload (nf-md-reload) buttons.
const BACK: &str = "\u{F004D} Retour";
const RELOAD: &str = "\u{F0453} Recharger";
const BACK_ICON: &str = "\u{F004D}";
const RELOAD_ICON: &str = "\u{F0453}";
/// How often the totals of the sidebar and `menu.toml` are read again.
const READ_EVERY: Duration = Duration::from_secs(2);
/// Totals older than this are no longer shown: the sidebar stopped.
const TOTALS_FOR_AT_MOST: Duration = Duration::from_secs(30);
/// How long an entry, once started, does not start again.
const LAUNCH_AGAIN_AFTER: Duration = Duration::from_secs(1);

/// The buttons of a page: each with its row, its first column and its text.
fn buttons(page: Page, width: u16) -> Vec<(Button, u16, u16, &'static str)> {
    let len = text_cells;
    match page {
        Page::Tools => Vec::new(),
        Page::Disk => vec![(Button::Back, 0, 1, BACK)],
        Page::Files => {
            // Their icons only when the names do not fit.
            let (back, reload, gap) = if len(BACK) + 4 + len(RELOAD) <= width {
                (BACK, RELOAD, 4)
            } else {
                (BACK_ICON, RELOAD_ICON, 2)
            };
            let left = width.saturating_sub(len(back) + gap + len(reload)) / 2;
            vec![
                (Button::Back, 1, left, back),
                (Button::Reload, 1, left + len(back) + gap, reload),
            ]
        }
    }
}

/// The cells `text` takes on screen (see `ui::width`).
fn text_cells(text: &str) -> u16 {
    u16::try_from(crate::ui::width(text)).unwrap_or(u16::MAX)
}

fn button_at(page: Page, width: u16, row: u16, column: u16) -> Option<Button> {
    buttons(page, width)
        .into_iter()
        .find(|&(_, r, left, text)| {
            r == row && column >= left && column < left.saturating_add(text_cells(text))
        })
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

pub(super) fn show(me: &str, menu: Menu, error: Option<String>) -> io::Result<()> {
    let _screen = Screen::enter()?;
    let menu_file = crate::config_dir().map(|dir| dir.join(config::FILE_NAME));
    let mut menu_changed = modified(menu_file.as_deref());
    let sidebar_state = herdr_api::sidebar::state_dir().ok();
    let files_flag = herdr_api::state_dir()
        .ok()
        .map(|dir| files::opened_flag(&dir, me));
    // One left by a bar of the same pane id that closed before reading it
    // is not for this one.
    if let Some(flag) = &files_flag {
        let _ = std::fs::remove_file(flag);
    }
    // The disk figures are measured while their page is open only.
    let mut measurer: Option<Measurer> = None;
    // Launches run on threads of their own: the bar keeps answering while a
    // popup waits for the previous one to close. They send back their error.
    let (launch_failed, launch_errors) = mpsc::channel::<String>();
    // The checks of the bar's place and the reloads of Herdr's configuration
    // run on threads too; a check sends whether the bar keeps its place.
    let (place_checked, place_news) = mpsc::channel::<bool>();
    let (reloaded, reloads) = mpsc::channel::<Result<String, String>>();
    // The keys of the entries starting, and started less than
    // `LAUNCH_AGAIN_AFTER` ago: a double click or a held key starts once.
    let launching: Arc<Mutex<HashSet<char>>> = Arc::default();
    let mut measured_at = None;
    // When the message gives its place back.
    let mut message_until = Instant::now() + MESSAGE_FOR;
    let mut view = View {
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
    };
    // The first check fits the height once the swap is done.
    let mut check_at = Some(Instant::now() + FIRST_FIT_AFTER);
    let mut read_at = Instant::now();
    // Drawn again only when something changed.
    let mut drawn: Option<(View, (u16, u16))> = None;
    loop {
        for news in measurer.iter().flat_map(|m| m.news.try_iter()) {
            match news {
                Measure::NoFolder => {
                    view.message = Some(("dossier de l'explorateur inconnu".into(), Color::Red));
                    message_until = Instant::now() + MESSAGE_FOR;
                }
                Measure::Started => view.measuring = true,
                Measure::Done(measured) => {
                    view.measuring = false;
                    view.stats = Some(measured.stats);
                    measured_at = Some(measured.at);
                }
                Measure::NoAnswer => {
                    view.measuring = false;
                    view.message = Some(("le dossier ne répond pas".into(), Color::Red));
                    message_until = Instant::now() + MESSAGE_FOR;
                }
                Measure::Unsaved(error) => {
                    view.message =
                        Some((format!("chiffres non enregistrés : {error}"), Color::Red));
                    message_until = Instant::now() + MESSAGE_FOR;
                }
            }
        }
        for error in launch_errors.try_iter() {
            view.message = Some((error, Color::Red));
            message_until = Instant::now() + MESSAGE_FOR;
        }
        if place_news.try_iter().any(|kept| !kept) {
            return Ok(());
        }
        for result in reloads.try_iter() {
            let (message, color) = match result {
                Ok(message) => {
                    view.page = Page::Tools;
                    (message, Color::Green)
                }
                Err(e) => (e, Color::Red),
            };
            view.message = Some((message, color));
            message_until = Instant::now() + MESSAGE_FOR;
        }
        if Instant::now() >= read_at {
            read_at = Instant::now() + READ_EVERY;
            view.totals = sidebar_state
                .as_deref()
                .and_then(Totals::load)
                // Stamped in the future (the clock was set back): not fresh.
                .filter(|t| {
                    herdr_api::unix_millis()
                        .checked_sub(t.at_ms)
                        .is_some_and(|age| age <= TOTALS_FOR_AT_MOST.as_millis() as u64)
                });
            let changed = modified(menu_file.as_deref());
            if changed != menu_changed {
                menu_changed = changed;
                let (menu, error) = crate::menu_config();
                if (menu.layout, menu.totals) != (view.menu.layout, view.menu.totals) {
                    check_at = Some(Instant::now());
                }
                view.menu = menu;
                view.hover = None;
                if let Some(error) = error {
                    view.message = Some((error, Color::Red));
                    message_until = Instant::now() + MESSAGE_FOR;
                }
            }
        }
        if let Some(flag) = &files_flag
            && std::fs::remove_file(flag).is_ok()
        {
            view.page = Page::Files;
            view.message = None;
            measurer = None;
        }
        if view.message.is_some() && Instant::now() >= message_until {
            view.message = None;
        }
        view.age =
            measured_at.and_then(|at| stats::age_label(stats::unix_now().saturating_sub(at)));
        let size = terminal::size()?;
        if drawn.as_ref().is_none_or(|(v, s)| *v != view || *s != size) {
            draw(&view)?;
            drawn = Some((view.clone(), size));
        }
        let until_check = check_at.map_or(Duration::MAX, |at| {
            at.saturating_duration_since(Instant::now())
        });
        if !event::poll(until_check.min(WAKE_EVERY))? {
            if check_at.is_some_and(|at| Instant::now() >= at) {
                check_at = None;
                let (me, rows, checked) = (
                    me.to_string(),
                    wanted_rows(&view.menu),
                    place_checked.clone(),
                );
                std::thread::spawn(move || {
                    let tab = my_tab(&me);
                    let known = super::registry::Bars::new();
                    if tab
                        .is_some_and(|tab| !has_its_place(&tab.iter().collect::<Vec<_>>(), &known))
                    {
                        let _ = checked.send(false);
                        return;
                    }
                    fit_height(&me, rows);
                });
            }
            continue;
        }
        let (width, _) = terminal::size()?;
        let grid = Grid::new(width, &view.menu.entries, view.menu.layout);
        // What the event asks for: an entry, a button, a new reading.
        let mut chosen = None;
        let mut pressed = None;
        let mut measure_again = false;
        match event::read()? {
            Event::Resize(..) => check_at = Some(Instant::now() + SETTLE),
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if crate::ui::is_control(key.modifiers) {
                    if key.code == KeyCode::Char('c') {
                        return Ok(());
                    }
                    continue;
                }
                match (view.page, key.code) {
                    // An entry's own key wins over `q`.
                    (Page::Tools, KeyCode::Char(c))
                        if view.menu.entries.iter().any(|e| e.key == c) =>
                    {
                        chosen = view.menu.entries.iter().position(|e| e.key == c);
                    }
                    (Page::Tools, KeyCode::Esc | KeyCode::Char('q')) => return Ok(()),
                    (_, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace) => {
                        pressed = Some(Button::Back);
                    }
                    (Page::Files, KeyCode::Char('r') | KeyCode::Enter) => {
                        pressed = Some(Button::Reload);
                    }
                    (Page::Disk, KeyCode::Char('r')) => measure_again = true,
                    _ => {}
                }
            }
            Event::Mouse(mouse) => {
                let button = button_at(view.page, width, mouse.row, mouse.column);
                let hit = match view.page {
                    Page::Tools => grid.at(mouse.row, mouse.column),
                    _ => None,
                };
                // Not while a message stands in for the figures.
                let on_figures = view.page == Page::Disk
                    && view.message.is_none()
                    && view.stats.is_some()
                    && mouse.row >= FIGURES_TOP;
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        chosen = hit;
                        pressed = button;
                        measure_again = on_figures;
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
        // A measuring thread that ended (the explorer's folder was unknown)
        // starts again.
        if measure_again && measurer.as_ref().is_none_or(|m| m.ask.send(()).is_err()) {
            measurer = Some(measure_in_background(me.to_string()));
        }
        match pressed {
            Some(Button::Back) => {
                view.page = Page::Tools;
                view.message = None;
                view.hover_button = None;
                measurer = None;
            }
            Some(Button::Reload) => {
                let reloaded = reloaded.clone();
                std::thread::spawn(move || {
                    let _ = reloaded.send(files::reload());
                });
            }
            None => {}
        }
        let Some(i) = chosen else {
            continue;
        };
        view.message = None;
        let entry = view.menu.entries[i].clone();
        if entry.action == Some(Action::Disk) {
            view.page = Page::Disk;
            view.hover = None;
            measurer = Some(measure_in_background(me.to_string()));
            continue;
        }
        let key = entry.key;
        let starting = || launching.lock().unwrap_or_else(|e| e.into_inner());
        if !starting().insert(key) {
            continue;
        }
        let (me, failed, launching) = (me.to_string(), launch_failed.clone(), launching.clone());
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
            Page::Tools => Grid::new(width, &view.menu.entries, view.menu.layout).rows(),
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

/// The icons, their names, and the totals of the sidebar under them.
fn draw_tools(out: &mut impl Write, view: &View, width: u16, rows: u16) -> io::Result<()> {
    let grid = Grid::new(width, &view.menu.entries, view.menu.layout);
    let slot = grid.slot();
    for (i, entry) in view.menu.entries.iter().enumerate() {
        let (left, top) = grid.place(i);
        if top >= rows {
            break;
        }
        // More icons than columns: those past the edge are left out.
        if left >= width {
            continue;
        }
        let hovered = view.hover == Some(i);
        let color = if hovered { Color::Yellow } else { Color::Reset };
        // The icon (one cell in most fonts) over the middle of the name.
        let icon = fit(&entry.icon(), usize::from(slot));
        queue!(
            out,
            MoveTo(centered(left, slot, &icon), top),
            SetForegroundColor(color),
            Print(&icon),
            ResetColor
        )?;
        if view.menu.layout == IconLayout::Double && top + 1 < rows {
            let name = fit(&entry.label, usize::from(slot));
            queue!(out, MoveTo(centered(left, slot, &name), top + 1))?;
            if hovered {
                queue!(out, SetForegroundColor(Color::Yellow))?;
            } else {
                queue!(out, SetAttribute(Attribute::Dim))?;
            }
            queue!(out, Print(name), SetAttribute(Attribute::Reset), ResetColor)?;
        }
    }
    // One row of icons: the name of the one under the pointer below it.
    if view.menu.layout == IconLayout::Single
        && let Some(entry) = view.hover.and_then(|i| view.menu.entries.get(i))
    {
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
        let row = grid.rows();
        match view.menu.totals {
            TotalsStyle::Line => draw_line(out, totals, &view.menu.numbers, width, row, rows)?,
            TotalsStyle::Bars => draw_bars(out, totals, &view.menu.numbers, width, row, rows)?,
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
    let whole: String = parts.iter().map(|(text, _)| text.as_str()).collect();
    if crate::ui::width(&whole) > usize::from(width) {
        let text = fit(&whole, usize::from(width));
        return queue!(out, MoveTo(centered(0, width, &text), row), Print(text));
    }
    queue!(out, MoveTo(centered(0, width, &whole), row))?;
    for (text, level) in &parts {
        let shown = text.chars().any(char::is_numeric) || text.contains(MACHINE_GLYPH);
        draw_figure(out, text, *level, shown)?;
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
    // `CPU 󰍹 :`: the glyph keeps a space after it, a terminal may draw it
    // over two cells.
    const LABEL_CELLS: usize = 7;
    let bar = usize::from(width).saturating_sub(1 + LABEL_CELLS + 1 + 1 + value_width + 1);
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
        if 1 + LABEL_CELLS + 1 + value_width > usize::from(width) {
            let value = fit(text, usize::from(width));
            queue!(out, MoveTo(centered(0, width, &value), y))?;
            draw_figure(out, &value, level, true)?;
            continue;
        }
        let pad = LABEL_CELLS.saturating_sub(crate::ui::width(&label));
        queue!(
            out,
            MoveTo(1, y),
            SetAttribute(Attribute::Bold),
            Print(&label),
            SetAttribute(Attribute::Reset),
            Print(" ".repeat(pad + 1))
        )?;
        if bar >= 3 {
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
    figure: bool,
) -> io::Result<()> {
    let color = match (figure, level) {
        (false, _) => Color::DarkGrey,
        (true, None) => Color::Reset,
        (true, Some(level)) => level_color(level),
    };
    if figure {
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
