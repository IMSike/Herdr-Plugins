//! The bar: a short pane of clickable icons stacked above the file explorer
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

use crate::config::{Entry, StartIn};
use crate::herdr::{self, Context, Pane};
use crate::stats::{self, Stats};
use crate::ui::Screen;
use crossterm::cursor::MoveTo;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Title of the `bar` pane in the manifest, which Herdr gives as its label.
pub const LABEL: &str = "Tools";
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
/// Rows the bar asks for: icons, names, a gap and two lines of figures.
const ROWS: u16 = 5;
/// Narrowest column of one entry.
const MIN_SLOT: u16 = 9;
/// Wait after a resize before checking the tab, so that a drag checks once.
const SETTLE: Duration = Duration::from_millis(400);
/// How often the figures on the folder are measured again.
const MEASURE_EVERY: Duration = Duration::from_secs(120);
/// Smallest share Herdr gives a side of a split.
const MIN_RATIO: f64 = 0.1;

fn label_is(pane: &Pane, label: &str) -> bool {
    pane.label.as_deref() == Some(label)
}

/// A bar whose program runs. Herdr restores a pane as a plain shell after a
/// restart: that one keeps the label, not the program.
fn is_live(pane: &Pane) -> bool {
    herdr::call("pane.process_info", json!({ "pane_id": pane.pane_id }))
        .ok()
        .and_then(|r| {
            let processes = r["process_info"]["foreground_processes"]
                .as_array()?
                .clone();
            Some(processes.iter().any(|p| {
                p["name"]
                    .as_str()
                    .is_some_and(|n| n.to_ascii_lowercase().starts_with("herdr-menu"))
            }))
        })
        .unwrap_or(false)
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
    for bar in panes.iter().filter(|p| label_is(p, LABEL)) {
        if is_live(bar) {
            return Ok(());
        }
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
    for pane in herdr::all_panes()? {
        if label_is(&pane, LABEL) && !is_live(&pane) {
            let _ = herdr::close(&pane.pane_id);
        }
    }
    Ok(())
}

/// `data` of `HERDR_PLUGIN_EVENT_JSON`.
fn event_data() -> Option<Value> {
    let json = std::env::var("HERDR_PLUGIN_EVENT_JSON").ok()?;
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
    while start.elapsed() < Duration::from_secs(4) {
        let Some(layout) = layout_of(pane) else {
            return false;
        };
        let Some(now) = layout.panes.iter().find(|p| p.pane_id == pane) else {
            return false;
        };
        let now = (now.rect.x, now.rect.y, now.rect.width, now.rect.height);
        if last == Some(now) && start.elapsed() >= Duration::from_millis(600) {
            return true;
        }
        last = Some(now);
        std::thread::sleep(Duration::from_millis(200));
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
        .ok_or("le pane de la barre n'a pas d'identifiant")?;
    herdr::call(
        "pane.swap",
        json!({ "source_pane_id": bar, "target_pane_id": explorer.pane_id }),
    )?;
    // Down to its height at once, before its program draws: it opens with half
    // of the column. Its border is one row; the bar corrects it once it runs.
    fit_rows(&bar, |height| height.saturating_sub(1));
    // The swap moves the focus to the bar: give it back.
    if let Some(focused) = focused {
        focus(&bar, &focused);
    }
    Ok(())
}

/// Moves the focus to `target`: Herdr only moves it to a neighbour, so walk
/// towards it a few steps.
fn focus(bar: &str, target: &str) {
    for _ in 0..6 {
        let Some(layout) = layout_of(bar) else {
            return;
        };
        let Some(now) = layout.focused_pane_id.as_deref() else {
            return;
        };
        if now == target {
            return;
        }
        let rect_of = |id: &str| {
            layout
                .panes
                .iter()
                .find(|p| p.pane_id == id)
                .map(|p| &p.rect)
        };
        let (Some(from), Some(to)) = (rect_of(now), rect_of(target)) else {
            return;
        };
        let direction = if to.x + to.width <= from.x {
            "left"
        } else if to.x >= from.x + from.width {
            "right"
        } else if to.y + to.height <= from.y {
            "up"
        } else {
            "down"
        };
        if herdr::call(
            "pane.focus_direction",
            json!({ "direction": direction, "pane_id": now }),
        )
        .is_err()
        {
            return;
        }
    }
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

/// Runs the bar until it is closed or no longer has a place.
pub fn run(entries: &[Entry]) -> Result<(), String> {
    let me = std::env::var("HERDR_PANE_ID").map_err(|_| "HERDR_PANE_ID absent".to_string())?;
    let result = show(&me, entries).map_err(|e| e.to_string());
    let _ = herdr::close(&me);
    result
}

/// Measures the folder now and every `MEASURE_EVERY`, in the background. A
/// folder too large to be read in full is measured once: the next readings
/// would stop at the same limit.
fn measure_in_background(root: PathBuf) -> mpsc::Receiver<Stats> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let stats = stats::measure(&root, stats::Limit::default());
            if send.send(stats).is_err() || !stats.complete {
                return;
            }
            std::thread::sleep(MEASURE_EVERY);
        }
    });
    receive
}

#[derive(Clone, PartialEq)]
struct View<'a> {
    entries: &'a [Entry],
    hover: Option<usize>,
    message: Option<String>,
    stats: Option<Stats>,
}

fn show(me: &str, entries: &[Entry]) -> io::Result<()> {
    let _screen = Screen::enter()?;
    let measures = folders(me).0.map(measure_in_background);
    let mut view = View {
        entries,
        hover: None,
        message: None,
        stats: None,
    };
    // The first check fits the height once the swap is done.
    let mut check_at = Some(Instant::now() + Duration::from_millis(300));
    // Drawn again only when something changed.
    let mut drawn: Option<(View, (u16, u16))> = None;
    loop {
        if let Some(stats) = measures.as_ref().and_then(|m| m.try_iter().last()) {
            view.stats = Some(stats);
        }
        let size = terminal::size()?;
        if drawn.as_ref() != Some(&(view.clone(), size)) {
            draw(&view)?;
            drawn = Some((view.clone(), size));
        }
        let until_check = check_at.map_or(Duration::MAX, |at| {
            at.saturating_duration_since(Instant::now())
        });
        if !event::poll(until_check.min(Duration::from_millis(500)))? {
            if check_at.is_some_and(|at| Instant::now() >= at) {
                check_at = None;
                let tab = my_tab(me);
                if tab.is_some_and(|tab| !has_its_place(&tab.iter().collect::<Vec<_>>())) {
                    return Ok(());
                }
                fit(me);
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
                match key.code {
                    KeyCode::Char(c) if entries.iter().any(|e| e.key == c) => {
                        entries.iter().position(|e| e.key == c)
                    }
                    KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
                    _ => None,
                }
            }
            Event::Mouse(mouse) => {
                let hit = slot_at(
                    slots(width, entries),
                    entries.len(),
                    mouse.row,
                    mouse.column,
                );
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => hit,
                    MouseEventKind::Moved => {
                        view.hover = hit;
                        None
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(i) = chosen {
            view.message = launch(me, &entries[i]).err();
        }
    }
}

/// Columns of one entry and the column the first one starts at, centred.
fn slots(width: u16, entries: &[Entry]) -> (u16, u16) {
    let count = entries.len().max(1) as u16;
    let longest = entries
        .iter()
        .map(|e| e.label.chars().count() as u16)
        .max()
        .unwrap_or(0);
    let slot = (longest + 2).max(MIN_SLOT).min(width / count).max(1);
    (slot, width.saturating_sub(slot * count) / 2)
}

/// The entry under a click: its icon or its name.
fn slot_at((slot, start): (u16, u16), count: usize, row: u16, column: u16) -> Option<usize> {
    if row > 1 {
        return None;
    }
    let index = usize::from(column.checked_sub(start)? / slot);
    (index < count).then_some(index)
}

/// The column `text` starts at, centred in `width` columns from `start`.
fn centred(start: u16, width: u16, text: &str) -> u16 {
    let len = text.chars().count() as u16;
    start + width.saturating_sub(len) / 2
}

fn draw(view: &View) -> io::Result<()> {
    let (width, rows) = terminal::size()?;
    let (slot, start) = slots(width, view.entries);
    let mut out = io::stdout();
    queue!(out, Clear(ClearType::All))?;
    for (i, entry) in view.entries.iter().enumerate() {
        let left = start + slot * i as u16;
        let hovered = view.hover == Some(i);
        let colour = if hovered { Color::Yellow } else { Color::Reset };
        // The icon (one cell in most fonts) over the middle of the name.
        let icon = entry.icon();
        queue!(
            out,
            MoveTo(centred(left, slot, &icon), 0),
            SetForegroundColor(colour),
            Print(&icon),
            ResetColor
        )?;
        if rows > 1 {
            let name: String = entry.label.chars().take(usize::from(slot)).collect();
            queue!(out, MoveTo(centred(left, slot, &name), 1))?;
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
    let cut = |text: String| -> String { text.chars().take(usize::from(width)).collect() };
    let lines: Vec<(String, Color)> = match (&view.message, &view.stats) {
        (Some(message), _) => vec![(cut(message.clone()), Color::Red)],
        (None, Some(stats)) => vec![
            (cut(stats::size_line(stats)), Color::DarkGrey),
            (cut(stats::files_line(stats)), Color::DarkGrey),
        ],
        (None, None) => Vec::new(),
    };
    for (row, (text, colour)) in (first..rows).zip(lines) {
        queue!(
            out,
            MoveTo(centred(0, width, &text), row),
            SetForegroundColor(colour),
            Print(text),
            ResetColor
        )?;
    }
    out.flush()
}

/// Opens the entry's tool in the plugin's full-size popup, or on its own.
fn launch(me: &str, entry: &Entry) -> Result<(), String> {
    let (explorer, terminal) = folders(me);
    let cwd = match entry.cwd {
        StartIn::Workspace => explorer.or(terminal),
        StartIn::Pane => terminal.or(explorer),
    };
    if !entry.popup {
        return crate::launch::detach(entry, cwd.as_deref());
    }
    let cwd = cwd.map(|p| p.to_string_lossy().into_owned());
    crate::run_in_popup(entry, cwd.as_deref())
}

#[derive(Deserialize)]
struct Rect {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

#[derive(Deserialize)]
struct LayoutPane {
    pane_id: String,
    rect: Rect,
}

#[derive(Deserialize)]
struct Split {
    direction: String,
    ratio: f64,
    rect: Rect,
}

#[derive(Deserialize)]
struct Layout {
    panes: Vec<LayoutPane>,
    splits: Vec<Split>,
    #[serde(default)]
    zoomed: bool,
    #[serde(default)]
    focused_pane_id: Option<String>,
}

fn layout_of(pane: &str) -> Option<Layout> {
    herdr::call("pane.layout", json!({ "pane_id": pane }))
        .ok()
        .and_then(|r| serde_json::from_value(r["layout"].clone()).ok())
}

/// Brings the bar back to its rows by moving the border below it: Herdr keeps
/// each split's ratio, so a resized window changes the bar's height.
fn fit(me: &str) {
    let Ok((_, rows)) = terminal::size() else {
        return;
    };
    fit_rows(me, |_| rows);
}

/// `fit` with the rows of the bar's terminal, worked out from its height in
/// the layout.
fn fit_rows(me: &str, rows: impl Fn(u16) -> u16) {
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
    let _ = herdr::call(
        "pane.resize",
        json!({ "pane_id": me, "direction": direction, "amount": delta.abs() }),
    );
}

/// How far to move the ratio of the split the bar tops, for its rows to be
/// `ROWS`; `None` when it already has them or is not on top of a split.
fn ratio_change(layout: &Layout, me: &str, rows: u16) -> Option<f64> {
    let rect = &layout.panes.iter().find(|p| p.pane_id == me)?.rect;
    let split = layout.splits.iter().find(|s| {
        s.direction == "down"
            && s.rect.x == rect.x
            && s.rect.y == rect.y
            && s.rect.width == rect.width
            && s.rect.height > rect.height
    })?;
    // What the pane takes beyond its terminal's rows: its border.
    let border = rect.height.saturating_sub(rows);
    let want = ROWS + border;
    if rect.height == want {
        return None;
    }
    // Herdr keeps a side of a split at a tenth at least: on a short screen
    // the bar gets more rows than it asks for.
    let ratio = ((f64::from(want) + 0.25) / f64::from(split.rect.height)).max(MIN_RATIO);
    let delta = ratio - split.ratio;
    (delta.abs() * f64::from(split.rect.height) >= 0.5).then_some(delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: u16, y: u16, width: u16, height: u16) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn entry(label: &str) -> Entry {
        Entry {
            key: 'x',
            label: label.into(),
            command: vec!["x".into()],
            cwd: StartIn::Workspace,
            icon: None,
            popup: true,
        }
    }

    #[test]
    fn slots_are_centred_and_fit_the_width() {
        let entries = [entry("lazygit"), entry("gh-dash"), entry("VS Code")];
        // 3 slots of 9 in 38 columns: 5 columns on the left.
        assert_eq!(slots(38, &entries), (9, 5));
        // Too narrow: the slots share the width.
        assert_eq!(slots(20, &entries), (6, 1));
        assert_eq!(centred(5, 9, "lazygit"), 6);
    }

    #[test]
    fn clicks_map_to_slots() {
        let geometry = (9, 5);
        assert_eq!(slot_at(geometry, 3, 0, 5), Some(0));
        assert_eq!(slot_at(geometry, 3, 1, 14), Some(1));
        assert_eq!(slot_at(geometry, 3, 0, 31), Some(2));
        assert_eq!(slot_at(geometry, 3, 0, 4), None);
        assert_eq!(slot_at(geometry, 3, 0, 32), None);
        assert_eq!(slot_at(geometry, 3, 2, 10), None);
    }

    #[test]
    fn shrinks_the_bar_to_its_rows() {
        let layout = Layout {
            panes: vec![
                LayoutPane {
                    pane_id: "bar".into(),
                    rect: rect(160, 0, 40, 30),
                },
                LayoutPane {
                    pane_id: "explorer".into(),
                    rect: rect(160, 30, 40, 20),
                },
            ],
            splits: vec![Split {
                direction: "down".into(),
                ratio: 0.6,
                rect: rect(160, 0, 40, 50),
            }],
            zoomed: false,
            focused_pane_id: None,
        };
        // 29 rows inside 30: a border of 1, so 6 rows in all.
        let delta = ratio_change(&layout, "bar", 29).unwrap();
        assert!((delta - (6.25 / 50.0 - 0.6)).abs() < 1e-9);
        assert_eq!(ratio_change(&layout, "explorer", 19), None);
        // Taller than it asks for, but already at Herdr's smallest share.
        let mut low = layout;
        low.splits[0].ratio = MIN_RATIO;
        low.splits[0].rect.height = 60;
        low.panes[0].rect.height = 6;
        assert_eq!(ratio_change(&low, "bar", 5), None);
    }

    #[test]
    fn only_other_panes_are_terminals() {
        let pane = |label: Option<&str>| Pane {
            pane_id: "p".into(),
            tab_id: "t".into(),
            workspace_id: "w".into(),
            label: label.map(str::to_string),
            cwd: None,
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
