//! How the settings screen is laid out: a title bar, the settings in
//! sections, each value in a column of its own (every choice shown, the
//! current one highlighted), a scrollbar when the list does not fit, the
//! selected setting's description, the last thing done, and the keys.
//!
//! Drawn on a `Canvas` without a terminal; `Drawn::at` tells what a click
//! lands on.

use super::canvas::{Canvas, Style};
use super::{Item, Row, Section, Shown, State};
use crate::colors::{TRACK_COLOR, level_color};
use crate::ui::{fit, width};
use crossterm::style::Color;
use herdr_api::sidebar::Level;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb { r, g, b }
}

// Gruvbox (dark), as the bars of the `Tools` pane.
const DARK: Color = rgb(0x1d, 0x20, 0x21);
const ON_ACCENT: Color = rgb(0x28, 0x28, 0x28);
pub(super) const BG1: Color = TRACK_COLOR;
pub(super) const BG2: Color = rgb(0x50, 0x49, 0x45);
const BG3: Color = rgb(0x66, 0x5c, 0x54);
pub(super) const FG: Color = rgb(0xeb, 0xdb, 0xb2);
pub(super) const FG4: Color = rgb(0xa8, 0x99, 0x84);
pub(super) const GREY: Color = rgb(0x92, 0x83, 0x74);
pub(super) const YELLOW: Color = level_color(Level::Yellow);
pub(super) const AQUA: Color = rgb(0x8e, 0xc0, 0x7c);
const GREEN: Color = level_color(Level::Green);
pub(super) const RED: Color = level_color(Level::Red);

/// The levels between thresholds, in order.
const LEVELS: [Level; 4] = [Level::Green, Level::Yellow, Level::Orange, Level::Red];

/// The widest the settings are drawn: a wider popup centers them.
const MAX_CONTENT: u16 = 88;
/// The widest a setting's name takes, as a share of the content (percent).
const MAX_LABEL_SHARE: u16 = 45;
/// nf-md-cog, the menu's icon for the settings.
const TITLE_ICON: &str = "\u{F0493}";

/// What a click lands on: an item (by its index among the items), one of
/// its choices, or a step of its number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    Item(usize),
    Choose(usize, usize),
    Step(usize, bool),
}

impl Target {
    /// The index of the item it belongs to.
    pub fn item(self) -> usize {
        match self {
            Self::Item(i) | Self::Choose(i, _) | Self::Step(i, _) => i,
        }
    }
}

/// A part of the screen that answers a click: row `y`, columns `x` to `end`.
struct Hit {
    y: u16,
    x: u16,
    end: u16,
    target: Target,
}

/// The screen drawn, what its parts answer, and the first row of the list
/// it shows.
pub struct Drawn {
    pub canvas: Canvas,
    hits: Vec<Hit>,
    pub top: usize,
}

impl Drawn {
    /// What a click at column `x`, row `y` lands on: the narrowest part.
    pub fn at(&self, x: u16, y: u16) -> Option<Target> {
        self.hits
            .iter()
            .filter(|h| h.y == y && (h.x..h.end).contains(&x))
            .min_by_key(|h| h.end - h.x)
            .map(|h| h.target)
    }
}

/// The rows of the screen, from its height: the smaller it is, the fewer
/// rows around the list.
struct Areas {
    title: Option<u16>,
    /// First row of the list, and how many rows it has.
    list: u16,
    rows: u16,
    rule: Option<u16>,
    about: Option<u16>,
    status: Option<u16>,
    footer: Option<u16>,
}

fn areas(height: u16) -> Areas {
    let h = height;
    let none = Areas {
        title: None,
        list: 0,
        rows: h,
        rule: None,
        about: None,
        status: None,
        footer: None,
    };
    match h {
        12.. => Areas {
            title: Some(0),
            list: 2,
            rows: h - 6,
            rule: Some(h - 4),
            about: Some(h - 3),
            status: Some(h - 2),
            footer: Some(h - 1),
        },
        8..=11 => Areas {
            title: Some(0),
            list: 1,
            rows: h - 4,
            about: Some(h - 3),
            status: Some(h - 2),
            footer: Some(h - 1),
            ..none
        },
        5..=7 => Areas {
            title: Some(0),
            list: 1,
            rows: h - 3,
            status: Some(h - 2),
            footer: Some(h - 1),
            ..none
        },
        2..=4 => Areas {
            rows: h - 1,
            footer: Some(h - 1),
            ..none
        },
        _ => none,
    }
}

/// The columns of the list: from `left` to `right` (excluded), the values
/// from `value`, the scrollbar at `bar`.
struct Columns {
    left: u16,
    right: u16,
    label: u16,
    value: u16,
    bar: Option<u16>,
}

fn columns(width: u16, label_width: usize, scrolls: bool) -> Columns {
    let content = width.saturating_sub(2).min(MAX_CONTENT);
    let left = (width - content) / 2;
    let mut right = left + content;
    let bar = (scrolls && content > 4).then(|| {
        right -= 2;
        right + 1
    });
    let room = right - left;
    let label = u16::try_from(label_width)
        .unwrap_or(u16::MAX)
        .min(room * MAX_LABEL_SHARE / 100);
    Columns {
        left,
        right,
        label,
        value: (left + 2 + label + 2).min(right),
        bar,
    }
}

/// The first row of the list to show in `visible` rows, from `top`, moved
/// just enough to show row `at`, and the row above it when that is its
/// section's header.
fn scrolled(top: usize, at: usize, visible: usize, len: usize, header_above: bool) -> usize {
    if visible == 0 {
        return at;
    }
    let mut top = top.min(len.saturating_sub(visible));
    let from = if header_above {
        at.saturating_sub(1)
    } else {
        at
    };
    if from < top {
        top = from;
    }
    if at >= top + visible {
        top = at + 1 - visible;
    }
    top
}

/// The thumb of a scrollbar of `visible` rows over `len`, from `top`: its
/// first row and its length.
fn thumb(len: usize, visible: usize, top: usize) -> (usize, usize) {
    if len <= visible || visible == 0 {
        return (0, visible);
    }
    let size = (visible * visible / len).max(1);
    let start = (top * (visible - size)).div_ceil(len - visible);
    (start.min(visible - size), size)
}

/// `text` cut to `cells`, with `…` when it is cut.
pub(super) fn ellipsize(text: &str, cells: usize) -> String {
    if width(text) <= cells {
        text.to_string()
    } else if cells == 0 {
        String::new()
    } else {
        format!("{}…", fit(text, cells - 1))
    }
}

/// The end of `text` that fits in `cells`: what is typed, its last part
/// in view.
fn tail(text: &str, cells: usize) -> String {
    let mut used = 0;
    let mut kept: Vec<char> = text
        .chars()
        .rev()
        .take_while(|c| {
            used += width(&c.to_string());
            used <= cells
        })
        .collect();
    kept.reverse();
    kept.into_iter().collect()
}

/// The keys of the footer, for `item` shown as `shown`.
fn hints(item: Item, shown: &Shown, typing: bool) -> Vec<(&'static str, &'static str)> {
    if typing {
        return vec![
            ("Entrée", "valider"),
            ("Échap", "annuler"),
            ("⌫", "effacer"),
        ];
    }
    let mut hints = vec![("↑↓", "naviguer")];
    match shown {
        Shown::Choice { .. } => hints.extend([("←→", "choisir"), ("Espace", "suivant")]),
        Shown::Number(_) => hints.push(("←→", "−/+ 5 s")),
        Shown::Text(_) | Shown::Levels(..) => hints.push(("Entrée", "modifier")),
        Shown::Action if item == Item::Reload => hints.push(("Entrée", "recharger")),
        Shown::Action => hints.push(("Entrée", "ouvrir")),
    }
    hints.extend([("Tab", "section"), ("Échap", "fermer")]);
    hints
}

/// Draws the screen of `state` in `width` × `height` cells.
pub fn render(state: &State, width: u16, height: u16) -> Drawn {
    let mut canvas = Canvas::new(width, height);
    let mut hits = Vec::new();
    let areas = areas(height);
    let rows = state.rows();
    let items = state.items();
    let index = state.selected.min(items.len() - 1);
    let selected = items[index];
    let at = rows
        .iter()
        .position(|row| *row == Row::Item(selected))
        .unwrap_or(0);
    let header_above = at > 0 && matches!(rows[at - 1], Row::Header(_));
    let visible = usize::from(areas.rows);
    let top = scrolled(state.top, at, visible, rows.len(), header_above);
    // The actions, without a value, take the whole row.
    let label_width = items
        .iter()
        .filter(|item| !item.is_action())
        .map(|item| self::width(item.label()))
        .max()
        .unwrap_or(0);
    let cols = columns(width, label_width, rows.len() > visible);
    let shown = state.shown(selected);

    if let Some(y) = areas.title {
        let bar = Style::default().on(BG1);
        canvas.fill(0, y, width, bar);
        canvas.text(1, y, TITLE_ICON, Style::fg(YELLOW).on(BG1));
        canvas.text(3, y, "Réglages", Style::fg(FG).on(BG1).bold());
        let position = format!("{}/{} ", index + 1, items.len());
        let x = width.saturating_sub(self::width(&position) as u16);
        if x > 13 {
            canvas.text(x, y, &position, Style::fg(FG4).on(BG1));
        }
    }

    for (offset, row) in rows.iter().skip(top).take(visible).enumerate() {
        let y = areas.list + offset as u16;
        match *row {
            Row::Blank => {}
            Row::Header(section) => header(&mut canvas, &cols, y, section, state),
            Row::Item(item) => {
                let i = items.iter().position(|&it| it == item).unwrap_or(0);
                hits.push(Hit {
                    y,
                    x: cols.left,
                    end: cols.right,
                    target: Target::Item(i),
                });
                let typing = state.typing.as_deref().filter(|_| item == selected);
                let row = RowDraw {
                    cols: &cols,
                    y,
                    index: i,
                    selected: item == selected,
                };
                row.draw(&mut canvas, &mut hits, state, item, typing);
            }
        }
    }

    if let Some(x) = cols.bar {
        let (start, size) = thumb(rows.len(), visible, top);
        for r in 0..visible {
            let (glyph, color) = if (start..start + size).contains(&r) {
                ("┃", FG4)
            } else {
                ("│", BG2)
            };
            canvas.text(x, areas.list + r as u16, glyph, Style::fg(color));
        }
    }

    if let Some(y) = areas.rule {
        let rule = "─".repeat(usize::from(cols.right - cols.left));
        canvas.text(cols.left, y, &rule, Style::fg(BG2));
    }

    if let Some(y) = areas.about {
        let (what, key) = state.about(selected);
        let x = cols.left + 2;
        let room = usize::from(cols.right.saturating_sub(x));
        let key_room = room.saturating_sub(self::width(what) + 3);
        if self::width(&key) <= key_room {
            let kx = cols.right - self::width(&key) as u16;
            canvas.text(kx, y, &key, Style::fg(GREY));
            canvas.text_to(x, y, what, Style::fg(FG4).italic(), kx);
        } else {
            canvas.text(x, y, &ellipsize(what, room), Style::fg(FG4).italic());
        }
    }

    if let (Some(y), Some((message, error))) = (areas.status, &state.status) {
        let (mark, color) = if *error { ("✗", RED) } else { ("✓", GREEN) };
        let x = cols.left + 2;
        let room = usize::from(cols.right.saturating_sub(x + 2));
        canvas.text(x, y, mark, Style::fg(color).bold());
        canvas.text(x + 2, y, &ellipsize(message, room), Style::fg(color));
    }

    if let Some(y) = areas.footer {
        let bar = Style::default().on(BG1);
        canvas.fill(0, y, width, bar);
        let mut x = 1;
        for (key, what) in hints(selected, &shown, state.typing.is_some()) {
            let cells = (self::width(key) + 1 + self::width(what)) as u16;
            if x + cells >= width {
                break;
            }
            x = canvas.text(x, y, key, Style::fg(YELLOW).on(BG1).bold());
            x = canvas.text(x + 1, y, what, Style::fg(FG4).on(BG1)) + 3;
        }
    }

    Drawn { canvas, hits, top }
}

/// A section's title, a rule after it, and its note at the end.
fn header(canvas: &mut Canvas, cols: &Columns, y: u16, section: Section, state: &State) {
    let end = canvas.text_to(
        cols.left,
        y,
        section.title(),
        Style::fg(AQUA).bold(),
        cols.right,
    );
    let mut right = cols.right;
    if let Some(note) = state.note(section) {
        let note = format!(" {note}");
        let cells = width(&note) as u16;
        if end + 4 + cells <= cols.right {
            right = cols.right - cells;
            canvas.text(right, y, &note, Style::fg(FG4));
        }
    }
    let rule = "─".repeat(usize::from(right.saturating_sub(end + 1)));
    canvas.text_to(end + 1, y, &rule, Style::fg(BG2), right);
}

/// Where an item's row is drawn.
struct RowDraw<'a> {
    cols: &'a Columns,
    y: u16,
    index: usize,
    selected: bool,
}

impl RowDraw<'_> {
    fn draw(
        &self,
        canvas: &mut Canvas,
        hits: &mut Vec<Hit>,
        state: &State,
        item: Item,
        typing: Option<&str>,
    ) {
        let (cols, y) = (self.cols, self.y);
        let base = if self.selected {
            Style::fg(FG).on(BG2)
        } else {
            Style::fg(FG)
        };
        if self.selected {
            canvas.fill(cols.left, y, cols.right - cols.left, base);
            canvas.text(cols.left, y, "▌", Style::fg(YELLOW).on(BG2));
        }
        let label_style = if self.selected { base.bold() } else { base };
        let limit = cols.right.saturating_sub(1);
        let dim = Style {
            bg: base.bg,
            ..Style::fg(GREY)
        };
        if item.is_action() {
            // No value: the name takes the row, its key at the end.
            const KEY: &str = "Entrée ↵";
            let room = usize::from(limit.saturating_sub(cols.left + 2));
            let key_room = room.saturating_sub(width(KEY) + 2);
            let shows_key = self.selected && key_room >= 12;
            let label_room = if shows_key { key_room } else { room };
            let label = ellipsize(item.label(), label_room);
            canvas.text_to(cols.left + 2, y, &label, label_style, cols.right);
            if shows_key {
                let x = limit - width(KEY) as u16;
                canvas.text_to(x, y, KEY, dim, limit);
            }
            return;
        }
        let label = ellipsize(item.label(), usize::from(cols.label));
        canvas.text_to(cols.left + 2, y, &label, label_style, cols.right);
        if cols.value >= limit {
            return;
        }
        let room = usize::from(limit - cols.value);
        if let Some(text) = typing {
            let field = Style::fg(FG).on(DARK);
            canvas.fill(cols.value, y, limit - cols.value, field);
            let x = canvas.text_to(
                cols.value + 1,
                y,
                &tail(text, room.saturating_sub(3)),
                field,
                limit,
            );
            canvas.text_to(x, y, "▏", Style::fg(YELLOW).on(DARK), limit);
            return;
        }
        match state.shown(item) {
            Shown::Choice { options, current } => {
                let chips: usize = options.iter().map(|o| width(o) + 3).sum();
                if chips <= room + 1 {
                    let mut x = cols.value;
                    for (i, option) in options.iter().enumerate() {
                        let style = match (i == current, self.selected) {
                            (true, true) => Style::fg(ON_ACCENT).on(YELLOW).bold(),
                            (true, false) => Style::fg(FG).on(BG3).bold(),
                            (false, _) => dim,
                        };
                        let end = canvas.text_to(x, y, &format!(" {option} "), style, limit);
                        hits.push(Hit {
                            y,
                            x,
                            end,
                            target: Target::Choose(self.index, i),
                        });
                        x = end + 1;
                    }
                } else {
                    let current = options.get(current).map_or("", String::as_str);
                    self.stepper(canvas, hits, current, base, dim, limit);
                }
            }
            Shown::Number(text) => self.stepper(canvas, hits, &text, base, dim, limit),
            Shown::Text(text) if text.is_empty() => {
                canvas.text_to(cols.value + 1, y, "aucun", dim.italic(), limit);
            }
            Shown::Text(text) => {
                canvas.text_to(cols.value + 1, y, &ellipsize(&text, room - 1), base, limit);
            }
            Shown::Levels(values, unit) => {
                let mut x = cols.value + 1;
                for (i, level) in LEVELS.into_iter().enumerate() {
                    let style = Style {
                        bg: base.bg,
                        ..Style::fg(level_color(level))
                    };
                    x = canvas.text_to(x, y, "━━", style, limit);
                    if let Some(value) = values.get(i) {
                        x = canvas.text_to(x, y, &format!(" {value} "), base.bold(), limit);
                    }
                }
                canvas.text_to(x + 1, y, unit, dim, limit);
            }
            // Drawn above.
            Shown::Action => {}
        }
    }

    /// `‹ value ›`: a click on an arrow steps back or forward.
    fn stepper(
        &self,
        canvas: &mut Canvas,
        hits: &mut Vec<Hit>,
        value: &str,
        base: Style,
        dim: Style,
        limit: u16,
    ) {
        let (cols, y) = (self.cols, self.y);
        let back = canvas.text_to(cols.value, y, "‹ ", dim, limit);
        let x = canvas.text_to(back, y, value, base.bold(), limit);
        let forward = canvas.text_to(x, y, " ›", dim, limit);
        hits.push(Hit {
            y,
            x: cols.value,
            end: back,
            target: Target::Step(self.index, false),
        });
        hits.push(Hit {
            y,
            x,
            end: forward,
            target: Target::Step(self.index, true),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_around_the_list_shrink_with_the_screen() {
        let tall = areas(30);
        assert_eq!((tall.list, tall.rows), (2, 24));
        assert_eq!(tall.footer, Some(29));
        assert_eq!(tall.about, Some(27));
        let short = areas(9);
        assert_eq!((short.list, short.rows, short.rule), (1, 5, None));
        let tiny = areas(6);
        assert_eq!(
            (tiny.about, tiny.status, tiny.footer),
            (None, Some(4), Some(5))
        );
        assert_eq!(areas(3).rows, 2);
        assert_eq!(areas(1).footer, None);
        assert_eq!(areas(0).rows, 0);
    }

    #[test]
    fn a_wide_screen_centers_the_settings() {
        let wide = columns(140, 30, false);
        assert_eq!((wide.left, wide.right), (26, 114));
        assert_eq!(wide.value, 26 + 2 + 30 + 2);
        let narrow = columns(40, 30, true);
        assert_eq!((narrow.left, narrow.right, narrow.bar), (1, 37, Some(38)));
        // The name gives way to the value.
        assert_eq!(narrow.label, 16);
        let none = columns(0, 30, true);
        assert_eq!((none.left, none.right, none.value), (0, 0, 0));
    }

    #[test]
    fn the_list_scrolls_to_the_selection_and_its_header() {
        // Within view: no move.
        assert_eq!(scrolled(0, 5, 10, 30, false), 0);
        // Below: just enough.
        assert_eq!(scrolled(0, 12, 10, 30, false), 3);
        // Above: its header too.
        assert_eq!(scrolled(8, 6, 10, 30, true), 5);
        assert_eq!(scrolled(8, 6, 10, 30, false), 6);
        // A taller screen: no rows left blank at the end.
        assert_eq!(scrolled(25, 26, 10, 30, false), 20);
        assert_eq!(scrolled(4, 3, 0, 30, false), 3);
    }

    #[test]
    fn the_thumb_follows_the_list() {
        assert_eq!(thumb(10, 20, 0), (0, 20));
        assert_eq!(thumb(40, 10, 0), (0, 2));
        assert_eq!(thumb(40, 10, 30), (8, 2));
        let (start, size) = thumb(40, 10, 15);
        assert!(start > 0 && start + size < 10);
    }

    #[test]
    fn text_is_cut_with_an_ellipsis_or_from_its_start() {
        assert_eq!(ellipsize("Couleurs", 20), "Couleurs");
        assert_eq!(ellipsize("Couleurs", 5), "Coul…");
        assert_eq!(ellipsize("Couleurs", 0), "");
        assert_eq!(tail("pwsh, bash, zsh", 8), "ash, zsh");
        assert_eq!(tail("abc", 8), "abc");
    }

    #[test]
    fn a_click_lands_on_the_narrowest_part() {
        let drawn = Drawn {
            canvas: Canvas::new(10, 2),
            hits: vec![
                Hit {
                    y: 1,
                    x: 0,
                    end: 10,
                    target: Target::Item(2),
                },
                Hit {
                    y: 1,
                    x: 4,
                    end: 7,
                    target: Target::Choose(2, 1),
                },
            ],
            top: 0,
        };
        assert_eq!(drawn.at(5, 1), Some(Target::Choose(2, 1)));
        assert_eq!(drawn.at(1, 1), Some(Target::Item(2)));
        assert_eq!(drawn.at(7, 1), Some(Target::Item(2)));
        assert_eq!(drawn.at(5, 0), None);
    }
}
