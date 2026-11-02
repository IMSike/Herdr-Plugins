//! Where the bar draws and what a click hits, and how far to move the split
//! it tops to keep its height: arithmetic on sizes and Herdr's layout only.

use crate::config::{Entry, IconLayout, Menu, TotalsStyle};
use serde::Deserialize;

/// Narrowest column of one entry with its name under it.
const MIN_SLOT: u16 = 9;
/// Widest column of one icon alone, on one row.
const MAX_ICON_SLOT: u16 = 6;
/// Smallest share Herdr gives a side of a split.
const MIN_RATIO: f64 = 0.1;

/// Rows the bar asks for: its icons and their names, then the totals of
/// the sidebar (one line, or four bars).
pub fn wanted_rows(menu: &Menu) -> u16 {
    let icons = match menu.layout {
        IconLayout::Single => 2,
        IconLayout::Double => 4,
    };
    icons + totals_rows(menu.totals)
}

/// Rows of the totals.
pub fn totals_rows(style: TotalsStyle) -> u16 {
    match style {
        TotalsStyle::Line => 1,
        TotalsStyle::Bars => 4,
    }
}

/// Where the entries are drawn: one row of icons, or two rows of icons with
/// their names, each row centered.
pub struct Grid {
    slot: u16,
    per_row: usize,
    count: usize,
    width: u16,
    layout: IconLayout,
}

impl Grid {
    pub fn new(width: u16, entries: &[Entry], layout: IconLayout) -> Self {
        let count = entries.len();
        let (per_row, slot) = match layout {
            IconLayout::Single => {
                let per_row = count.max(1);
                (per_row, (width / cells(per_row)).clamp(1, MAX_ICON_SLOT))
            }
            IconLayout::Double => {
                let per_row = count.div_ceil(2).max(1);
                let longest = entries
                    .iter()
                    .map(|e| cells(crate::ui::width(&e.label)))
                    .max()
                    .unwrap_or(0);
                let slot = longest
                    .saturating_add(2)
                    .max(MIN_SLOT)
                    .min(width / cells(per_row));
                (per_row, slot.max(1))
            }
        };
        Self {
            slot,
            per_row,
            count,
            width,
            layout,
        }
    }

    /// Columns of one entry.
    pub fn slot(&self) -> u16 {
        self.slot
    }

    /// Rows the entries take: each row of icons and the row of names under it.
    pub fn rows(&self) -> u16 {
        cells(self.count.div_ceil(self.per_row).max(1)).saturating_mul(2)
    }

    /// The first column of entry `i`'s slot, and the row of its icon.
    pub fn place(&self, i: usize) -> (u16, u16) {
        let line = i / self.per_row;
        let first = line * self.per_row;
        let start = self.start(first);
        (
            start.saturating_add(self.slot.saturating_mul(cells(i - first))),
            cells(line).saturating_mul(2),
        )
    }

    fn start(&self, first: usize) -> u16 {
        let in_line = cells((self.count - first).min(self.per_row));
        self.width.saturating_sub(self.slot.saturating_mul(in_line)) / 2
    }

    /// The entry under a click: its icon or its name (the row under it).
    pub fn at(&self, row: u16, column: u16) -> Option<usize> {
        let line = usize::from(row / 2);
        if self.layout == IconLayout::Single && line > 0 {
            return None;
        }
        let first = line * self.per_row;
        if first >= self.count {
            return None;
        }
        let index = usize::from(column.checked_sub(self.start(first))? / self.slot);
        let in_line = (self.count - first).min(self.per_row);
        (index < in_line).then_some(first + index)
    }
}

/// A count as screen cells: past what a screen holds, it stays the most
/// one can (a cast would wrap, to zero for a divisor).
fn cells(count: usize) -> u16 {
    u16::try_from(count).unwrap_or(u16::MAX)
}

/// The column `text` starts at, centered in `width` columns from `start`.
pub fn centered(start: u16, width: u16, text: &str) -> u16 {
    let len = cells(crate::ui::width(text));
    start.saturating_add(width.saturating_sub(len) / 2)
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
/// `want`; `None` when it already has them or is not on top of a split.
pub fn ratio_change(layout: &Layout, me: &str, rows: u16, want: u16) -> Option<f64> {
    let rect = &layout.panes.iter().find(|p| p.pane_id == me)?.rect;
    // The innermost split the bar tops: the one `pane.resize` moves.
    let split = layout
        .splits
        .iter()
        .filter(|s| {
            s.direction == "down"
                && s.rect.x == rect.x
                && s.rect.y == rect.y
                && s.rect.width == rect.width
                && s.rect.height > rect.height
        })
        .min_by_key(|s| s.rect.height)?;
    // What the pane takes beyond its terminal's rows: its border.
    let border = rect.height.saturating_sub(rows);
    let want = want + border;
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

    #[test]
    fn the_bar_asks_for_its_icons_and_totals() {
        let rows = |layout, totals| {
            wanted_rows(&Menu {
                layout,
                totals,
                ..Menu::default()
            })
        };
        assert_eq!(rows(IconLayout::Single, TotalsStyle::Line), 3);
        assert_eq!(rows(IconLayout::Double, TotalsStyle::Line), 5);
        assert_eq!(rows(IconLayout::Single, TotalsStyle::Bars), 6);
        assert_eq!(rows(IconLayout::Double, TotalsStyle::Bars), 8);
    }

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
            action: None,
            cwd: StartIn::Workspace,
            icon: None,
            popup: true,
        }
    }

    fn entries() -> Vec<Entry> {
        [
            "lazygit", "gh-dash", "VS Code", "Disk", "Update", "Settings",
        ]
        .into_iter()
        .map(entry)
        .collect()
    }

    #[test]
    fn two_rows_of_entries_are_centered() {
        let grid = Grid::new(40, &entries(), IconLayout::Double);
        // "Settings" and a column on each side: 3 slots of 10 in 40 columns.
        assert_eq!(grid.slot(), 10);
        assert_eq!(grid.rows(), 4);
        assert_eq!(grid.place(0), (5, 0));
        assert_eq!(grid.place(5), (25, 2));
        assert_eq!(centered(5, 10, "lazygit"), 6);
        // An odd count, and slots of 9 without "Settings": the last row is
        // centered on its own.
        let five = &entries()[..5];
        assert_eq!(Grid::new(40, five, IconLayout::Double).place(3), (11, 2));
    }

    #[test]
    fn one_row_of_icons_fits_the_width() {
        let grid = Grid::new(40, &entries(), IconLayout::Single);
        assert_eq!(grid.slot(), MAX_ICON_SLOT);
        assert_eq!(grid.rows(), 2);
        assert_eq!(grid.place(1), (8, 0));
        // Too narrow: the slots share the width.
        assert_eq!(Grid::new(18, &entries(), IconLayout::Single).slot(), 3);
        // More entries than a screen has columns: nothing wraps or divides by zero.
        let many = vec![entry("x"); 65_536];
        let grid = Grid::new(40, &many, IconLayout::Single);
        assert_eq!(grid.slot(), 1);
        assert_eq!(grid.place(65_535), (u16::MAX, 0));
    }

    #[test]
    fn clicks_map_to_entries() {
        let grid = Grid::new(40, &entries(), IconLayout::Double);
        assert_eq!(grid.at(0, 5), Some(0));
        assert_eq!(grid.at(1, 16), Some(1));
        assert_eq!(grid.at(3, 34), Some(5));
        assert_eq!(grid.at(0, 4), None);
        assert_eq!(grid.at(2, 35), None);
        assert_eq!(grid.at(4, 10), None);
        let single = Grid::new(40, &entries(), IconLayout::Single);
        assert_eq!(single.at(0, 8), Some(1));
        assert_eq!(single.at(2, 8), None);
        assert_eq!(single.at(0, 38), None);
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
        let delta = ratio_change(&layout, "bar", 29, 5).unwrap();
        assert!((delta - (6.25 / 50.0 - 0.6)).abs() < 1e-9);
        assert_eq!(ratio_change(&layout, "explorer", 19, 5), None);
        // Taller than it asks for, but already at Herdr's smallest share.
        let mut low = layout;
        low.splits[0].ratio = MIN_RATIO;
        low.splits[0].rect.height = 60;
        low.panes[0].rect.height = 6;
        assert_eq!(ratio_change(&low, "bar", 5, 5), None);
    }

    #[test]
    fn moves_the_innermost_split() {
        // The column splits twice downwards: the bar, then the explorer
        // above a preview. Herdr lists the outer split first.
        let layout = Layout {
            panes: vec![LayoutPane {
                pane_id: "bar".into(),
                rect: rect(160, 0, 40, 20),
            }],
            splits: vec![
                Split {
                    direction: "down".into(),
                    ratio: 0.5,
                    rect: rect(160, 0, 40, 60),
                },
                Split {
                    direction: "down".into(),
                    ratio: 0.5,
                    rect: rect(160, 0, 40, 40),
                },
            ],
            zoomed: false,
            focused_pane_id: None,
        };
        let delta = ratio_change(&layout, "bar", 19, 5).unwrap();
        assert!((delta - (6.25 / 40.0 - 0.5)).abs() < 1e-9);
    }
}
