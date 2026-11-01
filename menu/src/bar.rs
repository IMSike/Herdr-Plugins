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
mod view;

use crate::config::Menu;
use crate::herdr::{self, Context, Pane};
use crossterm::terminal;
use geometry::{Layout, ratio_change};
use serde_json::{Value, json};
use std::path::PathBuf;
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
/// First row of the disk figures: a click there measures the folder again.
const FIGURES_TOP: u16 = 1;
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
    let want = geometry::wanted_rows(crate::config::load(crate::config_dir().as_deref()).0.layout);
    fit_height_to(&bar, want, |height| height.saturating_sub(1));
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
/// why `menu.toml` was ignored: it is shown first, in place of the totals.
pub fn run(menu: Menu, error: Option<String>) -> Result<(), String> {
    let me = herdr_api::var(herdr_api::env::PANE_ID)?;
    let result = view::show(&me, menu, error).map_err(|e| e.to_string());
    let _ = herdr::close(&me);
    result
}

fn layout_of(pane: &str) -> Option<Layout> {
    herdr_api::request_field("pane.layout", json!({ "pane_id": pane }), "layout").ok()
}

/// Brings the bar back to `want` rows by moving the border below it: Herdr
/// keeps each split's ratio, so a resized window changes the bar's height.
fn fit_height(me: &str, want: u16) {
    let Ok((_, rows)) = terminal::size() else {
        return;
    };
    fit_height_to(me, want, |_| rows);
}

/// `fit_height` with the rows of the bar's terminal, worked out from its
/// height in the layout.
fn fit_height_to(me: &str, want: u16, rows: impl Fn(u16) -> u16) {
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
    let Some(delta) = ratio_change(&layout, me, rows(height), want) else {
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
