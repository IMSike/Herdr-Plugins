//! Where the bar draws and what a click hits, and how far to move the split
//! it tops to keep its height: arithmetic on sizes and Herdr's layout only.

use crate::config::Entry;
use serde::Deserialize;

/// Rows the bar asks for: icons, names, a gap and two lines of figures.
pub const ROWS: u16 = 5;
/// Narrowest column of one entry.
pub const MIN_SLOT: u16 = 9;
/// Smallest share Herdr gives a side of a split.
pub const MIN_RATIO: f64 = 0.1;

/// Columns of one entry and the column the first one starts at, centred.
pub fn slots(width: u16, entries: &[Entry]) -> (u16, u16) {
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
pub fn slot_at((slot, start): (u16, u16), count: usize, row: u16, column: u16) -> Option<usize> {
    if row > 1 {
        return None;
    }
    let index = usize::from(column.checked_sub(start)? / slot);
    (index < count).then_some(index)
}

/// The column `text` starts at, centred in `width` columns from `start`.
pub fn centred(start: u16, width: u16, text: &str) -> u16 {
    let len = text.chars().count() as u16;
    start + width.saturating_sub(len) / 2
}

#[derive(Deserialize)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Deserialize)]
pub struct LayoutPane {
    pub pane_id: String,
    pub rect: Rect,
}

#[derive(Deserialize)]
pub struct Split {
    pub direction: String,
    pub ratio: f64,
    pub rect: Rect,
}

#[derive(Deserialize)]
pub struct Layout {
    pub panes: Vec<LayoutPane>,
    pub splits: Vec<Split>,
    #[serde(default)]
    pub zoomed: bool,
    #[serde(default)]
    pub focused_pane_id: Option<String>,
}

/// How far to move the ratio of the split the bar tops, for its rows to be
/// `ROWS`; `None` when it already has them or is not on top of a split.
pub fn ratio_change(layout: &Layout, me: &str, rows: u16) -> Option<f64> {
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
    use crate::config::StartIn;

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
}
