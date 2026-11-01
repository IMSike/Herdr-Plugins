//! The bar (the `Tools` pane, called the bar in the code): a short pane of
//! clickable icons stacked above the file explorer
//! of gcaudan/herdr-explorer, in its column, with figures on the workspace
//! folder under them. It stands in for the popup menu in a tab that shows the
//! explorer.
//!
//! The bar opens with the explorer (`pane.created` event) and closes when the
//! explorer is gone or when no other pane is left beside them (`pane.closed`
//! event), so that the explorer can close its tab as it does on its own.
//! `prefix+space` closes and reopens it.
//!
//! Herdr only splits to the right or downwards: the bar opens below the
//! explorer, then swaps places with it. Herdr gives a side of a split a tenth
//! of it at least, so the bar is never shorter than a tenth of the column.

mod geometry;

use crate::config::{Entry, Menu};
use crate::herdr::{self, Context, Pane};
use crate::stats::{self, Measured, Stats};
use crate::ui::{Screen, fit};
use crossterm::cursor::MoveTo;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use geometry::{Layout, ROWS, centered, ratio_change, slot_at, slots};
use herdr_api::NumberFormat;
use serde_json::{Value, json};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Title of the `bar` pane in the manifest, which Herdr gives as its label.
const LABEL: &str = "Tools";
/// Label of the explorer's tree pane.
const EXPLORER: &str = "Explorer";
/// Labels of the explorer plugin's own panes: none of them is a terminal.
const EXPLORER_PANES: &[&str] = &[
    "Explorer",
    "Preview",
    "Search",
    "Source Control",
    "Changes",
    "Editor",
];
/// Wait after a resize before checking the tab, so that a drag checks once.
const SETTLE: Duration = Duration::from_millis(400);
/// How long an error message stands in for the figures.
const MESSAGE_FOR: Duration = Duration::from_secs(5);
/// First row of the figures (with the gap above them): a click there measures
/// the folder again.
const FIGURES_TOP: u16 = 2;
/// After the figures: a click measures them again (Nerd Font `nf-md-refresh`).
const REFRESH: &str = "\u{F0450}";
/// A bar just opened may not run its program yet, nor one slowed down by an
/// antivirus scanning a new binary: how long to wait for it.
const STARTING: Duration = Duration::from_secs(5);
/// How often a bar that does not run its program yet is checked again.
const STARTING_CHECK_EVERY: Duration = Duration::from_millis(500);
/// How long a new explorer may keep moving before its bar opens anyway.
const SETTLE_AT_MOST: Duration = Duration::from_secs(4);
/// How long a new explorer must keep the same place to count as settled.
const SETTLED_FOR: Duration = Duration::from_millis(600);
/// How often a new explorer's place is read while it settles.
const SETTLE_CHECK_EVERY: Duration = Duration::from_millis(200);
/// When the bar first fits its height: once the swap that opened it is done.
const FIRST_FIT_AFTER: Duration = Duration::from_millis(300);
/// The longest wait for an event: news of the measuring thread and the
/// end of a message are shown within this.
const WAKE_EVERY: Duration = Duration::from_millis(500);
/// How often a bar checks whether another bar of the folder finished its reading.
const OTHER_READING_CHECK_EVERY: Duration = Duration::from_secs(1);

fn label_is(pane: &Pane, label: &str) -> bool {
    pane.label.as_deref() == Some(label)
}

/// Whether a bar runs its program. Herdr restores a pane as a plain shell
/// after a restart: that one keeps the label, not the program. A pane Herdr
/// tells nothing about counts as running: it is kept.
fn is_live(pane: &Pane) -> bool {
    let Ok(info) = herdr_api::process_info(&pane.pane_id) else {
        return true;
    };
    info.foreground_processes
        .is_none_or(|processes| processes.iter().any(|p| crate::is_this_program(&p.name)))
}

/// The bars among `bars` that do not run the program, and still do not
/// after `STARTING`: a restored shell never starts it.
fn dead(bars: Vec<Pane>) -> Vec<Pane> {
    let deadline = Instant::now() + STARTING;
    let mut dead: Vec<Pane> = bars.into_iter().filter(|p| !is_live(p)).collect();
    while !dead.is_empty() && Instant::now() < deadline {
        std::thread::sleep(STARTING_CHECK_EVERY);
        dead.retain(|p| !is_live(p));
    }
    dead
}

/// Closes the tab's bar, or opens one above the tab's explorer. `false` when
/// the tab has neither: the caller shows the popup menu instead.
pub fn toggle(context: &Context) -> Result<bool, String> {
    let (Some(workspace), Some(tab)) = (&context.workspace_id, &context.tab_id) else {
        return Ok(false);
    };
    let panes = herdr::tab_panes(workspace, tab)?;
    let bars: Vec<&Pane> = panes.iter().filter(|p| label_is(p, LABEL)).collect();
    if !bars.is_empty() {
        for bar in bars {
            herdr::close(&bar.pane_id)?;
        }
        return Ok(true);
    }
    let Some(explorer) = panes.iter().find(|p| label_is(p, EXPLORER)) else {
        return Ok(false);
    };
    open_above(explorer)?;
    Ok(true)
}

/// `pane.created`: a new explorer gets its bar, once the explorer has taken
/// its place (it moves itself to the left side of the tab).
pub fn on_pane_created() -> Result<(), String> {
    let Some(explorer) = event_pane() else {
        return Ok(());
    };
    if !label_is(&explorer, EXPLORER) || !settled(&explorer.pane_id) {
        return Ok(());
    }
    let panes = herdr::tab_panes(&explorer.workspace_id, &explorer.tab_id)?;
    let bars: Vec<Pane> = panes.into_iter().filter(|p| label_is(p, LABEL)).collect();
    let count = bars.len();
    let dead = dead(bars);
    if dead.len() < count {
        return Ok(());
    }
    for bar in dead {
        let _ = herdr::close(&bar.pane_id);
    }
    open_above(&explorer)
}

/// `pane.closed`: the bars of the workspace that lost their place close at once.
pub fn on_pane_closed() -> Result<(), String> {
    let Some(workspace) = event_data().and_then(|d| d["workspace_id"].as_str().map(str::to_string))
    else {
        return Ok(());
    };
    let panes = herdr::workspace_panes(&workspace)?;
    for bar in panes.iter().filter(|p| label_is(p, LABEL)) {
        let tab: Vec<&Pane> = panes.iter().filter(|p| p.tab_id == bar.tab_id).collect();
        if !has_its_place(&tab) {
            let _ = herdr::close(&bar.pane_id);
        }
    }
    Ok(())
}

/// Closes the bars Herdr restored as plain shells after a restart; the
/// explorer reopens with a new bar.
pub fn startup() -> Result<(), String> {
    let bars = herdr::all_panes()?
        .into_iter()
        .filter(|p| label_is(p, LABEL))
        .collect();
    for bar in dead(bars) {
        let _ = herdr::close(&bar.pane_id);
    }
    Ok(())
}

/// `data` of `HERDR_PLUGIN_EVENT_JSON`.
fn event_data() -> Option<Value> {
    let json = std::env::var(herdr_api::env::EVENT_JSON).ok()?;
    let event: Value = serde_json::from_str(&json).ok()?;
    Some(event.get("data").cloned().unwrap_or(event))
}

fn event_pane() -> Option<Pane> {
    let data = event_data()?;
    let pane = data.get("pane").cloned().unwrap_or(data);
    serde_json::from_value(pane).ok()
}

/// Waits until the pane stops moving: the explorer swaps and resizes itself
/// right after it opens. `false` when it closed meanwhile.
fn settled(pane: &str) -> bool {
    let start = Instant::now();
    let mut last = None;
    while start.elapsed() < SETTLE_AT_MOST {
        let Some(layout) = layout_of(pane) else {
            return false;
        };
        let Some(now) = layout.panes.iter().find(|p| p.pane_id == pane) else {
            return false;
        };
        let now = (now.rect.x, now.rect.y, now.rect.width, now.rect.height);
        if last == Some(now) && start.elapsed() >= SETTLED_FOR {
            return true;
        }
        last = Some(now);
        std::thread::sleep(SETTLE_CHECK_EVERY);
    }
    true
}

/// Opens the bar below the explorer and swaps them, leaving the focus where it was.
fn open_above(explorer: &Pane) -> Result<(), String> {
    let focused = layout_of(&explorer.pane_id).and_then(|l| l.focused_pane_id);
    let options = json!({
        "placement": "split",
        "target_pane_id": explorer.pane_id,
        "direction": "down",
        "focus": false,
    });
    let bar = herdr::open_plugin_pane("bar", options, &[])?
        .ok_or("le pane Tools n'a pas d'identifiant")?;
    herdr_api::request(
        "pane.swap",
        json!({ "source_pane_id": bar, "target_pane_id": explorer.pane_id }),
    )?;
    // Down to its height at once, before its program draws: it opens with half
    // of the column. Its border is one row; the bar corrects it once it runs.
    fit_height_to(&bar, |height| height.saturating_sub(1));
    // The swap moves the focus to the bar: give it back.
    if let Some(focused) = focused {
        let _ = herdr_api::request("pane.focus", json!({ "pane_id": focused }));
    }
    Ok(())
}

/// A pane that is neither the bar nor one of the explorer's.
fn is_terminal(pane: &Pane) -> bool {
    pane.label
        .as_deref()
        .is_none_or(|l| l != LABEL && !EXPLORER_PANES.contains(&l))
}

/// The bar stays while its tab has the explorer and a pane of its own.
fn has_its_place(tab: &[&Pane]) -> bool {
    tab.iter().any(|p| label_is(p, EXPLORER)) && tab.iter().any(|p| is_terminal(p))
}

/// The panes of the bar's tab; `None` when Herdr does not answer.
fn my_tab(me: &str) -> Option<Vec<Pane>> {
    let pane = herdr::pane(me).ok()?;
    herdr::tab_panes(&pane.workspace_id, &pane.tab_id).ok()
}

/// Folders the entries start in, read from the tab: the explorer's, the
/// first terminal's.
fn folders(me: &str) -> (Option<PathBuf>, Option<PathBuf>) {
    let tab = my_tab(me).unwrap_or_default();
    let explorer = tab
        .iter()
        .find(|p| label_is(p, EXPLORER))
        .and_then(|p| p.cwd.clone());
    let terminal = tab
        .iter()
        .find(|p| is_terminal(p))
        .and_then(|p| p.cwd.clone());
    (explorer, terminal)
}

/// Runs the bar until it is closed or no longer has a place. `error` says
/// why `menu.toml` was ignored: it is shown first, in place of the figures.
pub fn run(menu: &Menu, error: Option<String>) -> Result<(), String> {
    let me = herdr_api::var(herdr_api::env::PANE_ID)?;
    let result = show(&me, &menu.entries, menu.numbers, error).map_err(|e| e.to_string());
    let _ = herdr::close(&me);
    result
}

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

#[derive(Clone, PartialEq)]
struct View<'a> {
    entries: &'a [Entry],
    numbers: NumberFormat,
    hover: Option<usize>,
    /// The pointer is over the figures.
    hover_figures: bool,
    message: Option<String>,
    stats: Option<Stats>,
    /// How old the figures are, once old enough to say.
    age: Option<String>,
    measuring: bool,
}

fn show(
    me: &str,
    entries: &[Entry],
    numbers: NumberFormat,
    error: Option<String>,
) -> io::Result<()> {
    let _screen = Screen::enter()?;
    let measurer = folders(me).0.map(measure_in_background);
    // Launches run on threads of their own: the bar keeps answering while a
    // popup waits for the previous one to close. They send back their error.
    let (launch_failed, launch_errors) = mpsc::channel::<String>();
    let mut measured_at = None;
    // When the error message gives its place back to the figures.
    let mut message_until = Instant::now() + MESSAGE_FOR;
    let mut view = View {
        entries,
        numbers,
        hover: None,
        hover_figures: false,
        message: error,
        stats: None,
        age: None,
        measuring: false,
    };
    // The first check fits the height once the swap is done.
    let mut check_at = Some(Instant::now() + FIRST_FIT_AFTER);
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
                    view.message = Some(format!("chiffres non enregistrés : {error}"));
                    message_until = Instant::now() + MESSAGE_FOR;
                }
            }
        }
        for error in launch_errors.try_iter() {
            view.message = Some(error);
            message_until = Instant::now() + MESSAGE_FOR;
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
                fit_height(me);
            }
            continue;
        }
        let (width, _) = terminal::size()?;
        let chosen = match event::read()? {
            Event::Resize(..) => {
                check_at = Some(Instant::now() + SETTLE);
                None
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    if key.code == KeyCode::Char('c') {
                        return Ok(());
                    }
                    continue;
                }
                // An entry's own key wins over `q`.
                if let KeyCode::Char(c) = key.code
                    && let Some(i) = entries.iter().position(|e| e.key == c)
                {
                    Some(i)
                } else if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    return Ok(());
                } else {
                    None
                }
            }
            Event::Mouse(mouse) => {
                let hit = slot_at(
                    slots(width, entries),
                    entries.len(),
                    mouse.row,
                    mouse.column,
                );
                // Not while a message stands in for the figures.
                let on_figures =
                    view.message.is_none() && view.stats.is_some() && mouse.row >= FIGURES_TOP;
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if on_figures && let Some(m) = &measurer {
                            let _ = m.ask.send(());
                        }
                        hit
                    }
                    MouseEventKind::Moved => {
                        view.hover = hit;
                        view.hover_figures = on_figures;
                        None
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(i) = chosen {
            view.message = None;
            let (me, entry, failed) = (me.to_string(), entries[i].clone(), launch_failed.clone());
            std::thread::spawn(move || {
                if let Err(error) = launch(&me, &entry) {
                    let _ = failed.send(error);
                }
            });
        }
    }
}

fn draw(view: &View) -> io::Result<()> {
    let (width, rows) = terminal::size()?;
    let (slot, start) = slots(width, view.entries);
    let mut out = io::stdout();
    queue!(out, Clear(ClearType::All))?;
    for (i, entry) in view.entries.iter().enumerate() {
        let left = start + slot * i as u16;
        let hovered = view.hover == Some(i);
        let color = if hovered { Color::Yellow } else { Color::Reset };
        // The icon (one cell in most fonts) over the middle of the name.
        let icon = entry.icon();
        queue!(
            out,
            MoveTo(centered(left, slot, &icon), 0),
            SetForegroundColor(color),
            Print(&icon),
            ResetColor
        )?;
        if rows > 1 {
            let name = fit(&entry.label, usize::from(slot));
            queue!(out, MoveTo(centered(left, slot, &name), 1))?;
            if hovered {
                queue!(out, SetForegroundColor(Color::Yellow))?;
            } else {
                queue!(out, SetAttribute(Attribute::Dim))?;
            }
            queue!(out, Print(name), SetAttribute(Attribute::Reset), ResetColor)?;
        }
    }
    // Figures, or an error, under the names, after a gap when there is room.
    let first = if rows >= ROWS { 3 } else { 2 };
    let cut = |text: &str| fit(text, usize::from(width));
    // Each line: its text, its color, and what follows in another color.
    let lines: Vec<(String, Color, String)> = match (&view.message, &view.stats) {
        (Some(message), _) => vec![(message.clone(), Color::Red, String::new())],
        (None, Some(stats)) => {
            let mut files = stats::files_line(stats);
            if let Some(age) = &view.age {
                files = format!("{files} · {age}");
            }
            let refresh = if view.measuring { "…" } else { REFRESH };
            vec![
                (
                    stats::size_line(stats, &view.numbers),
                    Color::DarkGrey,
                    String::new(),
                ),
                (files, Color::DarkGrey, format!(" {refresh}")),
            ]
        }
        (None, None) => Vec::new(),
    };
    let suffix_color = if view.hover_figures {
        Color::Yellow
    } else {
        Color::DarkGrey
    };
    for (row, (text, color, suffix)) in (first..rows).zip(lines) {
        let whole = cut(&format!("{text}{suffix}"));
        let text = cut(&text);
        let suffix: String = whole.chars().skip(text.chars().count()).collect();
        queue!(
            out,
            MoveTo(centered(0, width, &whole), row),
            SetForegroundColor(color),
            Print(text),
            SetForegroundColor(suffix_color),
            Print(suffix),
            ResetColor
        )?;
    }
    out.flush()
}

/// Opens the entry's tool in the plugin's large popup, or on its own.
fn launch(me: &str, entry: &Entry) -> Result<(), String> {
    let (explorer, terminal) = folders(me);
    let cwd = herdr::start_folder(entry.cwd, explorer, terminal);
    if !entry.in_popup() {
        return crate::launch::detach(entry, cwd.as_deref());
    }
    let cwd = cwd.map(|p| p.to_string_lossy().into_owned());
    crate::run_in_popup(entry, cwd.as_deref())
}

fn layout_of(pane: &str) -> Option<Layout> {
    herdr_api::request_field("pane.layout", json!({ "pane_id": pane }), "layout").ok()
}

/// Brings the bar back to its rows by moving the border below it: Herdr keeps
/// each split's ratio, so a resized window changes the bar's height.
fn fit_height(me: &str) {
    let Ok((_, rows)) = terminal::size() else {
        return;
    };
    fit_height_to(me, |_| rows);
}

/// `fit_height` with the rows of the bar's terminal, worked out from its
/// height in the layout.
fn fit_height_to(me: &str, rows: impl Fn(u16) -> u16) {
    let Some(layout) = layout_of(me) else {
        return;
    };
    if layout.zoomed {
        return;
    }
    let Some(height) = layout
        .panes
        .iter()
        .find(|p| p.pane_id == me)
        .map(|p| p.rect.height)
    else {
        return;
    };
    let Some(delta) = ratio_change(&layout, me, rows(height)) else {
        return;
    };
    let direction = if delta > 0.0 { "down" } else { "up" };
    let _ = herdr_api::request(
        "pane.resize",
        json!({ "pane_id": me, "direction": direction, "amount": delta.abs() }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_other_panes_are_terminals() {
        let pane = |label: Option<&str>| Pane {
            pane_id: "p".into(),
            tab_id: "t".into(),
            workspace_id: "w".into(),
            label: label.map(str::to_string),
            cwd: None,
            agent: None,
            agent_status: None,
        };
        assert!(is_terminal(&pane(None)));
        assert!(is_terminal(&pane(Some("lazygit"))));
        assert!(!is_terminal(&pane(Some(LABEL))));
        assert!(!is_terminal(&pane(Some("Preview"))));
        let (explorer, terminal) = (pane(Some(EXPLORER)), pane(None));
        assert!(has_its_place(&[&explorer, &terminal]));
        assert!(!has_its_place(&[&explorer]));
        assert!(!has_its_place(&[&terminal]));
    }
}
