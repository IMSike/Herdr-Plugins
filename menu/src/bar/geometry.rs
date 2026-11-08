//! Where the bar draws and what a click hits, and how far to move the split
//! it tops to keep its height: arithmetic on sizes and Herdr's layout only.

use crate::config::{Entry, Menu, TotalsStyle};
use serde::Deserialize;

/// Widest column of one icon.
const MAX_ICON_SLOT: u16 = 6;
/// Smallest share Herdr gives a side of a split.
const MIN_RATIO: f64 = 0.1;
/// Rows of the entries: the icons, then the name of the hovered one.
pub const ICON_ROWS: u16 = 2;

/// Rows the bar asks for: its icons and the name under them, then the
/// totals of the sidebar (one line, or four bars).
pub fn wanted_rows(menu: &Menu) -> u16 {
    ICON_ROWS + totals_rows(menu.totals)
}

/// Rows of the totals.
pub fn totals_rows(style: TotalsStyle) -> u16 {
    match style {
        TotalsStyle::Line => 1,
        TotalsStyle::Bars => 4,
    }
}

/// Where the entries are drawn: one row of icons, centered, the name of
/// the hovered one under it.
pub struct Grid {
    slot: u16,
    count: usize,
    width: u16,
}

impl Grid {
    pub fn new(width: u16, entries: &[Entry]) -> Self {
        let count = entries.len();
        let slot = (width / count_cells(count.max(1))).clamp(1, MAX_ICON_SLOT);
        Self { slot, count, width }
    }

    /// Columns of one entry.
    pub fn slot(&self) -> u16 {
        self.slot
    }

    /// The first column of entry `i`'s slot.
    pub fn left(&self, i: usize) -> u16 {
        self.start()
            .saturating_add(self.slot.saturating_mul(count_cells(i)))
    }

    fn start(&self) -> u16 {
        let slots = self.slot.saturating_mul(count_cells(self.count));
        self.width.saturating_sub(slots) / 2
    }

    /// The entry under a click: its icon or the row under it.
    pub fn at(&self, row: u16, column: u16) -> Option<usize> {
        if row >= ICON_ROWS {
            return None;
        }
        let index = usize::from(column.checked_sub(self.start())? / self.slot);
        (index < self.count).then_some(index)
    }
}

/// A count as screen cells: past what a screen holds, it stays the most
/// one can (a cast would wrap, to zero for a divisor).
fn count_cells(count: usize) -> u16 {
    u16::try_from(count).unwrap_or(u16::MAX)
}

/// The cells `text` takes on screen (see `ui::width`).
fn text_cells(text: &str) -> u16 {
    count_cells(crate::ui::width(text))
}

/// The column `text` starts at, centered in `width` columns from `start`.
pub fn centered(start: u16, width: u16, text: &str) -> u16 {
    start.saturating_add(width.saturating_sub(text_cells(text)) / 2)
}

/// Whether `text`, drawn from column `left`, covers `column`.
pub fn covers(left: u16, text: &str, column: u16) -> bool {
    column >= left && column < left.saturating_add(text_cells(text))
}

/// Two buttons side by side, centered in `width`: each given as its name
/// and its icon, drawn by name 4 cells apart when they fit, else by icon 2
/// apart. The first column of each, and its text.
pub fn button_pair<'a>(
    width: u16,
    [first, first_icon]: [&'a str; 2],
    [second, second_icon]: [&'a str; 2],
) -> [(u16, &'a str); 2] {
    let len = text_cells;
    let (first, second, gap) = if len(first) + 4 + len(second) <= width {
        (first, second, 4)
    } else {
        (first_icon, second_icon, 2)
    };
    let left = width.saturating_sub(len(first) + gap + len(second)) / 2;
    [(left, first), (left + len(first) + gap, second)]
}

/// The cells of a totals' label (`CPU 󰍹 :`): the glyph keeps a space after
/// it, a terminal may draw it over two cells.
pub const LABEL_CELLS: usize = 7;
/// The narrowest bar drawn.
const MIN_BAR: usize = 3;

/// The cells of the totals' bars, in `width` columns, the widest value
/// taking `value_cells`: a cell, the label, a space, the bar, a space,
/// the value, a cell. `Some(0)` when there is no room for a bar, `None`
/// when there is none for the label either: the value alone.
pub fn bar_cells(width: u16, value_cells: usize) -> Option<usize> {
    let width = usize::from(width);
    if 1 + LABEL_CELLS + 1 + value_cells > width {
        return None;
    }
    let bar = width.saturating_sub(1 + LABEL_CELLS + 1 + 1 + value_cells + 1);
    Some(if bar >= MIN_BAR { bar } else { 0 })
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
        let rows = |totals| {
            wanted_rows(&Menu {
                totals,
                ..Menu::default()
            })
        };
        assert_eq!(rows(TotalsStyle::Line), 3);
        assert_eq!(rows(TotalsStyle::Bars), 6);
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
    fn one_row_of_icons_fits_the_width() {
        let grid = Grid::new(40, &entries());
        assert_eq!(grid.slot(), MAX_ICON_SLOT);
        assert_eq!(grid.left(1), 8);
        assert_eq!(centered(2, 6, "x"), 4);
        // Too narrow: the slots share the width.
        assert_eq!(Grid::new(18, &entries()).slot(), 3);
        // More entries than a screen has columns: nothing wraps or divides by zero.
        let many = vec![entry("x"); 65_536];
        let grid = Grid::new(40, &many);
        assert_eq!(grid.slot(), 1);
        assert_eq!(grid.left(65_535), u16::MAX);
        // No entry at all.
        assert_eq!(Grid::new(40, &[]).at(0, 20), None);
    }

    #[test]
    fn clicks_map_to_entries() {
        let grid = Grid::new(40, &entries());
        assert_eq!(grid.at(0, 2), Some(0));
        assert_eq!(grid.at(0, 8), Some(1));
        assert_eq!(grid.at(1, 8), Some(1));
        assert_eq!(grid.at(2, 8), None);
        assert_eq!(grid.at(0, 1), None);
        assert_eq!(grid.at(0, 38), None);
    }

    #[test]
    fn two_buttons_by_name_or_by_icon() {
        let named = button_pair(20, ["< Back", "<"], ["Reload", "R"]);
        // 6 + 4 + 6 = 16 cells, from column 2.
        assert_eq!(named, [(2, "< Back"), (12, "Reload")]);
        let icons = button_pair(10, ["< Back", "<"], ["Reload", "R"]);
        assert_eq!(icons, [(3, "<"), (6, "R")]);
        assert!(covers(12, "Reload", 12) && covers(12, "Reload", 17));
        assert!(!covers(12, "Reload", 11) && !covers(12, "Reload", 18));
    }

    #[test]
    fn the_bars_take_what_the_label_and_value_leave() {
        // 1 + 7 + 1 + bar + 1 + 4 + 1.
        assert_eq!(bar_cells(30, 4), Some(15));
        assert_eq!(bar_cells(17, 4), Some(0));
        assert_eq!(bar_cells(18, 4), Some(3));
        assert_eq!(bar_cells(13, 4), Some(0));
        assert_eq!(bar_cells(12, 4), None);
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
