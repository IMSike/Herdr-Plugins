//! What the bar shows: its icons and the sidebar's totals, the disk
//! figures, or the reload of the configuration files open in an editor.

use super::geometry::{Grid, centered, wanted_rows};
use super::{
    FIGURES_TOP, FIRST_FIT_AFTER, MESSAGE_FOR, OTHER_READING_CHECK_EVERY, REFRESH, SETTLE,
    WAKE_EVERY, fit_height, folders, has_its_place, my_tab,
};
use crate::config::{self, Action, Entry, Layout as IconLayout, Menu};
use crate::herdr;
use crate::settings::files;
use crate::stats::{self, Measured, Stats};
use crate::ui::{Screen, fit};
use crossterm::cursor::MoveTo;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use herdr_api::sidebar::{Level, MACHINE_GLYPH, Totals};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

/// What the measuring thread tells the bar.
enum Measure {
    Started,
    Done(Measured),
    /// The figures could not be kept in the state folder.
    Unsaved(String),
}

/// The bar's link to its measuring thread.
struct Measurer {
    news: mpsc::Receiver<Measure>,
    /// A click on the figures: measure now.
    ask: mpsc::Sender<()>,
}

/// Measures the folder in the background whenever its figures are due (see
/// `stats::lifetime`) or asked for. Figures kept in the plugin's state folder,
/// by this bar or another one of the folder, are shown without a reading.
/// The thread ends with the bar.
fn measure_in_background(root: PathBuf) -> Measurer {
    let (send, news) = mpsc::channel();
    let (ask, asked) = mpsc::channel::<()>();
    let file = herdr_api::state_dir()
        .ok()
        .map(|dir| stats::cache_file(&dir, &root));
    std::thread::spawn(move || {
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
            let lock = match &file {
                Some(file) => stats::ReadingLock::take(file),
                None => None,
            };
            if file.is_some() && lock.is_none() {
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
            let measured = Measured::now(&root, stats::measure(&root, limit));
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
    Measurer { news, ask }
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
/// How often the totals of the sidebar and `menu.toml` are read again.
const READ_EVERY: Duration = Duration::from_secs(2);
/// Totals older than this are no longer shown: the sidebar stopped.
const TOTALS_FOR_AT_MOST: Duration = Duration::from_secs(30);

/// The buttons of a page: each with its row, its first column and its text.
fn buttons(page: Page, width: u16) -> Vec<(Button, u16, u16, &'static str)> {
    let len = |text: &str| text.chars().count() as u16;
    match page {
        Page::Tools => Vec::new(),
        Page::Disk => vec![(Button::Back, 0, 1, BACK)],
        Page::Files => {
            let gap = 4;
            let left = width.saturating_sub(len(BACK) + gap + len(RELOAD)) / 2;
            vec![
                (Button::Back, 1, left, BACK),
                (Button::Reload, 1, left + len(BACK) + gap, RELOAD),
            ]
        }
    }
}

fn button_at(page: Page, width: u16, row: u16, column: u16) -> Option<Button> {
    buttons(page, width)
        .into_iter()
        .find(|&(_, r, left, text)| {
            r == row && column >= left && column < left + text.chars().count() as u16
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
    totals: Option<Vec<(String, Option<Level>)>>,
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
    // The disk figures are measured while their page is open only.
    let mut measurer: Option<Measurer> = None;
    // Launches run on threads of their own: the bar keeps answering while a
    // popup waits for the previous one to close. They send back their error.
    let (launch_failed, launch_errors) = mpsc::channel::<String>();
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
                Measure::Started => view.measuring = true,
                Measure::Done(measured) => {
                    view.measuring = false;
                    view.stats = Some(measured.stats);
                    measured_at = Some(measured.at);
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
        if Instant::now() >= read_at {
            read_at = Instant::now() + READ_EVERY;
            view.totals = sidebar_state
                .as_deref()
                .and_then(Totals::load)
                .and_then(|t| {
                    let age = herdr_api::unix_millis().saturating_sub(t.at_ms);
                    (age <= TOTALS_FOR_AT_MOST.as_millis() as u64)
                        .then(|| t.parts(&view.menu.numbers))
                });
            let changed = modified(menu_file.as_deref());
            if changed != menu_changed {
                menu_changed = changed;
                let (menu, error) = config::load(crate::config_dir().as_deref());
                if menu.layout != view.menu.layout {
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
        if drawn.as_ref() != Some(&(view.clone(), size)) {
            draw(&view)?;
            drawn = Some((view.clone(), size));
        }
        let until_check = check_at.map_or(Duration::MAX, |at| {
            at.saturating_duration_since(Instant::now())
        });
        if !event::poll(until_check.min(WAKE_EVERY))? {
            if check_at.is_some_and(|at| Instant::now() >= at) {
                check_at = None;
                let tab = my_tab(me);
                if tab.is_some_and(|tab| !has_its_place(&tab.iter().collect::<Vec<_>>())) {
                    return Ok(());
                }
                fit_height(me, wanted_rows(view.menu.layout));
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
                if key.modifiers.contains(KeyModifiers::CONTROL) {
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
        if measure_again && let Some(m) = &measurer {
            let _ = m.ask.send(());
        }
        match pressed {
            Some(Button::Back) => {
                view.page = Page::Tools;
                view.message = None;
                view.hover_button = None;
                measurer = None;
            }
            Some(Button::Reload) => {
                let (message, color) = match files::reload() {
                    Ok(message) => {
                        view.page = Page::Tools;
                        (message, Color::Green)
                    }
                    Err(e) => (e, Color::Red),
                };
                view.message = Some((message, color));
                message_until = Instant::now() + MESSAGE_FOR;
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
            measurer = folders(me).0.map(measure_in_background);
            if measurer.is_none() {
                view.message = Some(("dossier de l'explorateur inconnu".into(), Color::Red));
                message_until = Instant::now() + MESSAGE_FOR;
            }
            continue;
        }
        let (me, failed) = (me.to_string(), launch_failed.clone());
        std::thread::spawn(move || {
            if let Err(error) = launch(&me, &entry) {
                let _ = failed.send(error);
            }
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
    let (explorer, terminal) = folders(me);
    let cwd = herdr::start_folder(entry.cwd, explorer, terminal)
        .map(|p| p.to_string_lossy().into_owned());
    crate::start(entry, cwd.as_deref(), Some(me))
}

fn draw(view: &View) -> io::Result<()> {
    let (width, rows) = terminal::size()?;
    let mut out = io::stdout();
    queue!(out, Clear(ClearType::All))?;
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
    out.flush()
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
        let hovered = view.hover == Some(i);
        let color = if hovered { Color::Yellow } else { Color::Reset };
        // The icon (one cell in most fonts) over the middle of the name.
        let icon = entry.icon();
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
        let whole: String = totals.iter().map(|(text, _)| text.as_str()).collect();
        if row < rows && whole.chars().count() <= usize::from(width) {
            queue!(out, MoveTo(centered(0, width, &whole), row))?;
            for (text, level) in totals {
                let shown = text.chars().any(char::is_numeric) || text.contains(MACHINE_GLYPH);
                draw_figure(out, text, *level, shown)?;
            }
        } else if row < rows {
            let text = fit(&whole, usize::from(width));
            queue!(out, MoveTo(centered(0, width, &text), row), Print(text))?;
        }
    }
    Ok(())
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

/// The colors of the levels, those of the sidebar's example rules.
fn level_color(level: Level) -> Color {
    let (r, g, b) = match level {
        Level::Green => (0xb8, 0xbb, 0x26),
        Level::Yellow => (0xfa, 0xbd, 0x2f),
        Level::Orange => (0xfe, 0x80, 0x19),
        Level::Red => (0xfb, 0x49, 0x34),
    };
    Color::Rgb { r, g, b }
}
