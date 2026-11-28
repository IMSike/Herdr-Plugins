//! How the settings screen is laid out: a title bar with the tabs, the
//! settings in sections across the popup's width, each value in a column of
//! its own (every choice shown, the current one highlighted; a list element
//! by element, on as many lines as it takes), a button that puts a setting
//! changed back to its default, a scrollbar on the right edge when the list
//! does not fit, the selected setting's description, the last thing done
//! with a button that undoes it, and the keys.
//!
//! Drawn on a `Canvas` without a terminal; `Drawn::at` tells what a click
//! lands on.

use super::canvas::{Canvas, Style};
use super::{Item, Row, Section, Shown, State, Tab};
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

/// The widest a setting's name takes, as a share of the content (percent).
const MAX_LABEL_SHARE: u16 = 45;
/// nf-md-cog, the menu's icon for the settings.
const TITLE_ICON: &str = "\u{F0493}";
/// The button of a setting changed, on its selected row, and on the others.
const RESET: &str = " ↺ défaut ";
const RESET_MARK: &str = "↺";
/// The values need this much room for the button to keep its own.
const RESET_ROOM: usize = 30;
/// The element added after the last of a list.
const ADD: &str = "+";
/// Marks, at its end, the element of a list a click removes.
const REMOVE: &str = "✕";
/// A button of the status line, and under the mouse.
const BUTTON: (Style, Style) = (Style::fg(FG).on(BG2), Style::fg(FG).on(BG3));

/// What a click lands on: a tab (by its index among the tabs), an item (by
/// its index among the tab's items), one of its choices, a step of its
/// number, a part of it (a list's element, a threshold) or the `✕` that
/// removes an element, its button back to the default; the button that
/// undoes the last change, or one that answers a change waiting for Enter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    Tab(usize),
    Item(usize),
    Choose(usize, usize),
    Step(usize, bool),
    Part(usize, usize),
    Remove(usize, usize),
    Reset(usize),
    Undo,
    Confirm(bool),
}

impl Target {
    /// The index of the item it belongs to, if any.
    pub fn item(self) -> Option<usize> {
        match self {
            Self::Tab(_) | Self::Undo | Self::Confirm(_) => None,
            Self::Item(i)
            | Self::Choose(i, _)
            | Self::Step(i, _)
            | Self::Part(i, _)
            | Self::Remove(i, _)
            | Self::Reset(i) => Some(i),
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

/// The screen drawn, what its parts answer, the first row of the list it
/// shows and the last row that can be first.
pub struct Drawn {
    pub canvas: Canvas,
    hits: Vec<Hit>,
    pub top: usize,
    pub max_top: usize,
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

/// Where the screen draws, to tell what is under the mouse.
struct Pen<'a> {
    canvas: &'a mut Canvas,
    hits: &'a mut Vec<Hit>,
    hover: Option<(u16, u16)>,
}

impl Pen<'_> {
    /// Whether the mouse is on columns `x` to `end` of row `y`.
    fn over(&self, x: u16, end: u16, y: u16) -> bool {
        self.hover
            .is_some_and(|(hx, hy)| hy == y && (x..end).contains(&hx))
    }

    /// `text` at column `x` of row `y` until `limit`, as a part to click:
    /// in the second style under the mouse, the first otherwise. Where it
    /// ends.
    fn button(
        &mut self,
        (x, y): (u16, u16),
        text: &str,
        (style, hovered): (Style, Style),
        limit: u16,
        target: Target,
    ) -> u16 {
        let end = (x + width(text) as u16).min(limit);
        let style = if self.over(x, end, y) { hovered } else { style };
        let end = self.canvas.text_to(x, y, text, style, limit);
        self.hits.push(Hit { y, x, end, target });
        end
    }
}

/// The rows of the screen, from its height: the smaller it is, the fewer
/// rows around the list; the taller, the more for the description.
struct Areas {
    title: Option<u16>,
    /// First row of the list, and how many rows it has.
    list: u16,
    rows: u16,
    rule: Option<u16>,
    /// First row of the description, and how many rows it has.
    about: Option<u16>,
    about_rows: u16,
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
        about_rows: 0,
        status: None,
        footer: None,
    };
    match h {
        12.. => {
            let about_rows = match h {
                20.. => 3,
                14.. => 2,
                _ => 1,
            };
            Areas {
                title: Some(0),
                list: 2,
                rows: h - 5 - about_rows,
                rule: Some(h - 3 - about_rows),
                about: Some(h - 2 - about_rows),
                about_rows,
                status: Some(h - 2),
                footer: Some(h - 1),
            }
        }
        8..=11 => Areas {
            title: Some(0),
            list: 1,
            rows: h - 4,
            about: Some(h - 3),
            about_rows: 1,
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
/// from `value`, the scrollbar at `bar`, on the last column.
struct Columns {
    left: u16,
    right: u16,
    label: u16,
    value: u16,
    bar: Option<u16>,
}

impl Columns {
    /// The last column a row draws in (excluded).
    fn limit(&self) -> u16 {
        self.right.saturating_sub(1)
    }

    /// Where the values end (excluded): before the button back to the
    /// default, when they have room for it.
    fn values_end(&self) -> u16 {
        let room = usize::from(self.limit().saturating_sub(self.value));
        if room >= RESET_ROOM {
            self.limit() - width(RESET) as u16 - 1
        } else {
            self.limit()
        }
    }
}

fn columns(width: u16, label_width: usize, scrolls: bool) -> Columns {
    // A column of margin on each side, the scrollbar in the right one.
    let left = u16::from(width > 2);
    let mut right = width.saturating_sub(1).max(left);
    let bar = (scrolls && right - left > 4).then(|| {
        right -= 1;
        width - 1
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

/// The lines an item takes, `first` to `last`, and whether the line above
/// is its section's header.
#[derive(Clone, Copy)]
struct Span {
    first: usize,
    last: usize,
    header_above: bool,
}

/// The first line of the list to show in `visible` lines, from `top`,
/// moved just enough to show the lines of `span`, and the line above them
/// when that is their section's header; their first line when they do not
/// all fit.
fn scrolled(top: usize, span: Span, visible: usize, len: usize) -> usize {
    if visible == 0 {
        return span.first;
    }
    let mut top = top.min(len.saturating_sub(visible));
    let from = if span.header_above {
        span.first.saturating_sub(1)
    } else {
        span.first
    };
    if from < top {
        top = from;
    }
    if span.last >= top + visible {
        top = (span.last + 1 - visible).min(span.first);
    }
    top
}

/// An element of a list as drawn: its index (the list's length for the
/// one to add) and its text, padded.
type Chip = (usize, String);

/// The elements of `list`, then the one to add, cut into lines of `cells`
/// between them; an element too long for a line is cut with `…`.
fn chip_lines(list: &[String], cells: usize) -> Vec<Vec<Chip>> {
    let chips = list
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(ADD))
        .enumerate()
        .map(|(i, text)| (i, format!(" {} ", ellipsize(text, cells.saturating_sub(2)))));
    let mut lines: Vec<Vec<Chip>> = vec![Vec::new()];
    let mut used = 0;
    for chip in chips {
        let cells_of = width(&chip.1);
        let line = lines.last_mut().expect("a line");
        if !line.is_empty() && used + 1 + cells_of > cells {
            lines.push(vec![chip]);
            used = cells_of;
        } else {
            used += usize::from(!line.is_empty()) + cells_of;
            line.push(chip);
        }
    }
    lines
}

/// A line of the list: a row, or a line of an item's list (`part`) in
/// `wrapped`, the lists cut into lines.
#[derive(Clone, Copy)]
struct Line {
    row: Row,
    part: usize,
    wrap: Option<usize>,
}

/// The lines of `rows` in `cols`: a list (shells, types) on as many lines
/// as it takes, unless it is being typed; and the lists cut into lines.
fn lines(
    state: &State,
    rows: &[Row],
    cols: &Columns,
    selected: Item,
) -> (Vec<Line>, Vec<Vec<Vec<Chip>>>) {
    let room = usize::from(cols.values_end().saturating_sub(cols.value));
    let mut lines = Vec::new();
    let mut wrapped = Vec::new();
    for &row in rows {
        let list = match row {
            Row::Item(item) if !(state.typing.is_some() && item == selected) && room > 2 => {
                match state.shown(item) {
                    Shown::List(list) => Some(chip_lines(&list, room)),
                    _ => None,
                }
            }
            _ => None,
        };
        match list {
            Some(list) => {
                for part in 0..list.len() {
                    lines.push(Line {
                        row,
                        part,
                        wrap: Some(wrapped.len()),
                    });
                }
                wrapped.push(list);
            }
            None => lines.push(Line {
                row,
                part: 0,
                wrap: None,
            }),
        }
    }
    (lines, wrapped)
}

/// `text` cut into lines of `cells` between its words, `rows` at most, the
/// last cut with `…` when the text goes on.
fn wrap_words(text: &str, cells: usize, rows: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.is_empty() {
            line = word.to_string();
        } else if width(&line) + 1 + width(word) <= cells {
            line.push(' ');
            line.push_str(word);
        } else {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        }
    }
    lines.push(line);
    if lines.len() > rows {
        lines.truncate(rows);
        if let Some(last) = lines.last_mut() {
            *last = ellipsize(&format!("{last} …"), cells);
        }
    }
    lines.iter().map(|l| ellipsize(l, cells)).collect()
}

/// The title bar: the icon, the title and the tabs, the current one
/// highlighted, until column `end`; without the title when they do not
/// fit, and only the current tab when they still do not.
fn title_bar(pen: &mut Pen, state: &State, end: u16, y: u16) {
    const TITLE: &str = "Réglages";
    pen.canvas.text(1, y, TITLE_ICON, Style::fg(YELLOW).on(BG1));
    let current = state.current_tab();
    let tabs: Vec<(usize, Tab)> = state.tabs().into_iter().enumerate().collect();
    let cells = |tabs: &[(usize, Tab)]| -> usize {
        tabs.iter()
            .map(|(_, t)| width(t.title()) + 3)
            .sum::<usize>()
    };
    let after_title = 3 + width(TITLE) as u16 + 2;
    let end_cells = usize::from(end);
    let (mut x, tabs) = if usize::from(after_title) + cells(&tabs) <= end_cells {
        pen.canvas.text(3, y, TITLE, Style::fg(FG).on(BG1).bold());
        (after_title, tabs)
    } else if 3 + cells(&tabs) <= end_cells {
        (3, tabs)
    } else {
        let only: Vec<_> = tabs.into_iter().filter(|&(_, t)| t == current).collect();
        (3, only)
    };
    for (i, tab) in tabs {
        let (style, hovered) = if tab == current {
            let on = Style::fg(ON_ACCENT).on(YELLOW).bold();
            (on, on)
        } else {
            (Style::fg(FG4).on(BG1), Style::fg(FG).on(BG3))
        };
        let text = format!(" {} ", tab.title());
        x = pen.button((x, y), &text, (style, hovered), end, Target::Tab(i)) + 1;
    }
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
fn hints(state: &State, item: Item, shown: &Shown) -> Vec<(&'static str, &'static str)> {
    if state.confirm.is_some() {
        return vec![("Entrée", "confirmer"), ("Échap", "annuler")];
    }
    if state.typing.is_some() {
        return vec![
            ("Entrée", "valider"),
            ("Échap", "annuler"),
            ("⌫", "effacer"),
        ];
    }
    let mut hints = vec![("↑↓", "naviguer")];
    match shown {
        Shown::Choice { .. } => hints.extend([("←→", "choisir"), ("Espace", "suivant")]),
        Shown::Number(_) => hints.extend([("←→", "−/+ 5 s"), ("Entrée", "saisir")]),
        Shown::List(list) if state.part_of(item) >= list.len() => {
            hints.extend([("←→", "élément"), ("Entrée", "ajouter")]);
        }
        Shown::List(_) => {
            hints.extend([
                ("←→", "élément"),
                ("Entrée", "modifier"),
                ("Suppr", "retirer"),
            ]);
        }
        Shown::Levels(..) => hints.extend([("←→", "seuil"), ("Entrée", "saisir")]),
        Shown::Action if item == Item::Reload => hints.push(("Entrée", "recharger")),
        Shown::Action => hints.push(("Entrée", "ouvrir")),
    }
    if !state.is_default(item) {
        hints.push(("r", "défaut"));
    }
    if state.undo.is_some() {
        hints.push(("u", if state.undone { "refaire" } else { "annuler" }));
    }
    hints.extend([("Tab", "onglet"), ("Échap", "fermer")]);
    hints
}

/// Draws the screen of `state` in `width` × `height` cells.
pub fn render(state: &State, width: u16, height: u16) -> Drawn {
    let mut canvas = Canvas::new(width, height);
    let mut hits = Vec::new();
    let mut pen = Pen {
        canvas: &mut canvas,
        hits: &mut hits,
        hover: state.hover,
    };
    let areas = areas(height);
    let rows = state.rows();
    let items = state.items();
    let index = state.selected.min(items.len() - 1);
    let selected = items[index];
    let visible = usize::from(areas.rows);
    // The actions, without a value, take the whole row.
    let label_width = items
        .iter()
        .filter(|item| !item.is_action())
        .map(|item| self::width(item.label()))
        .max()
        .unwrap_or(0);
    // The scrollbar when the lines do not fit, which takes a column from them.
    let mut cols = columns(width, label_width, false);
    let (mut lines, mut wrapped) = self::lines(state, &rows, &cols, selected);
    if lines.len() > visible {
        cols = columns(width, label_width, true);
        (lines, wrapped) = self::lines(state, &rows, &cols, selected);
    }
    let first = lines
        .iter()
        .position(|line| line.row == Row::Item(selected))
        .unwrap_or(0);
    let span = Span {
        first,
        last: lines
            .iter()
            .rposition(|line| line.row == Row::Item(selected))
            .unwrap_or(first),
        header_above: first > 0 && matches!(lines[first - 1].row, Row::Header(_)),
    };
    let max_top = lines.len().saturating_sub(visible);
    // The wheel moves the list alone; a key brings the selection back.
    let top = if state.follow {
        scrolled(state.top, span, visible, lines.len())
    } else {
        state.top.min(max_top)
    };
    let shown = state.shown(selected);

    if let Some(y) = areas.title {
        let bar = Style::default().on(BG1);
        pen.canvas.fill(0, y, width, bar);
        let position = format!("{}/{} ", index + 1, items.len());
        let mut end = width.saturating_sub(self::width(&position) as u16);
        if end > 13 {
            pen.canvas.text(end, y, &position, Style::fg(FG4).on(BG1));
            end -= 1;
        } else {
            end = width;
        }
        title_bar(&mut pen, state, end, y);
    }

    for (offset, line) in lines.iter().skip(top).take(visible).enumerate() {
        let y = areas.list + offset as u16;
        match line.row {
            Row::Blank => {}
            Row::Header(section) => header(pen.canvas, &cols, y, section, state),
            Row::Item(item) => {
                let i = items.iter().position(|&it| it == item).unwrap_or(0);
                pen.hits.push(Hit {
                    y,
                    x: cols.left,
                    end: cols.right,
                    target: Target::Item(i),
                });
                let row = RowDraw {
                    cols: &cols,
                    y,
                    index: i,
                    selected: item == selected,
                    chips: line.wrap.map(|w| wrapped[w].as_slice()),
                    part: line.part,
                };
                row.draw(&mut pen, state, item);
            }
        }
    }

    if let Some(x) = cols.bar {
        let (start, size) = thumb(lines.len(), visible, top);
        for r in 0..visible {
            let (glyph, color) = if (start..start + size).contains(&r) {
                ("┃", FG4)
            } else {
                ("│", BG2)
            };
            pen.canvas
                .text(x, areas.list + r as u16, glyph, Style::fg(color));
        }
    }

    let (what, place) = state.about(selected);
    if let Some(y) = areas.rule {
        // The setting and its files, at the end of the rule.
        let mut end = cols.right;
        let place = format!(" {place}");
        if place.len() > 1 && usize::from(cols.right - cols.left) >= self::width(&place) + 8 {
            end = cols.right - self::width(&place) as u16;
            pen.canvas.text(end, y, &place, Style::fg(GREY));
        }
        let rule = "─".repeat(usize::from(end - cols.left));
        pen.canvas.text_to(cols.left, y, &rule, Style::fg(BG2), end);
    }

    if let Some(y) = areas.about {
        let x = cols.left + 2;
        let room = usize::from(cols.right.saturating_sub(x));
        let rows = usize::from(areas.about_rows);
        let mut what = what.to_string();
        // Without the rule, the setting and its files after the text.
        if areas.rule.is_none() && !place.is_empty() {
            what = format!("{what} ({place})");
        }
        for (r, text) in wrap_words(&what, room, rows).iter().enumerate() {
            pen.canvas
                .text(x, y + r as u16, text, Style::fg(FG4).italic());
        }
    }

    if let Some(y) = areas.status {
        status_line(&mut pen, state, &cols, y);
    }

    if let Some(y) = areas.footer {
        let bar = Style::default().on(BG1);
        pen.canvas.fill(0, y, width, bar);
        let mut x = 1;
        for (key, what) in hints(state, selected, &shown) {
            let cells = (self::width(key) + 1 + self::width(what)) as u16;
            if x + cells >= width {
                break;
            }
            x = pen.canvas.text(x, y, key, Style::fg(YELLOW).on(BG1).bold());
            x = pen.canvas.text(x + 1, y, what, Style::fg(FG4).on(BG1)) + 3;
        }
    }

    Drawn {
        canvas,
        hits,
        top,
        max_top,
    }
}

/// The status line: a change waiting for Enter, with its buttons; or the
/// last thing done (`✓`, or `✗` in red), with the button that undoes it.
fn status_line(pen: &mut Pen, state: &State, cols: &Columns, y: u16) {
    let x = cols.left + 2;
    let end = cols.right;
    if let Some(confirm) = state.confirm {
        const YES: &str = " Confirmer ";
        const NO: &str = " Annuler ";
        let buttons = (self::width(YES) + 1 + self::width(NO)) as u16;
        let text_end = end.saturating_sub(buttons + 1).max(x);
        pen.canvas.text(x, y, "⚠", Style::fg(RED).bold());
        let room = usize::from(text_end.saturating_sub(x + 2));
        pen.canvas.text(
            x + 2,
            y,
            &ellipsize(confirm.question(), room),
            Style::fg(RED),
        );
        if text_end > x {
            let yes = Style::fg(ON_ACCENT).on(YELLOW).bold();
            let after = pen.button(
                (text_end + 1, y),
                YES,
                (yes, yes),
                end,
                Target::Confirm(true),
            );
            pen.button((after + 1, y), NO, BUTTON, end, Target::Confirm(false));
        }
        return;
    }
    let mut text_end = end;
    if state.undo.is_some() {
        let label = if state.undone {
            " ↷ refaire "
        } else {
            " ↶ annuler "
        };
        let bx = end.saturating_sub(self::width(label) as u16);
        if bx > x + 12 {
            pen.button((bx, y), label, BUTTON, end, Target::Undo);
            text_end = bx - 1;
        }
    }
    if let Some((message, error)) = &state.status {
        let (mark, color) = if *error { ("✗", RED) } else { ("✓", GREEN) };
        let room = usize::from(text_end.saturating_sub(x + 2));
        pen.canvas.text(x, y, mark, Style::fg(color).bold());
        pen.canvas
            .text(x + 2, y, &ellipsize(message, room), Style::fg(color));
    }
}

/// A section's title (Danger's in red), a rule after it, and its note at
/// the end.
fn header(canvas: &mut Canvas, cols: &Columns, y: u16, section: Section, state: &State) {
    let color = if section == Section::Danger {
        RED
    } else {
        AQUA
    };
    let end = canvas.text_to(
        cols.left,
        y,
        section.title(),
        Style::fg(color).bold(),
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
    /// The item's list cut into lines, and the line drawn here.
    chips: Option<&'a [Vec<Chip>]>,
    part: usize,
}

impl RowDraw<'_> {
    fn draw(&self, pen: &mut Pen, state: &State, item: Item) {
        let (cols, y) = (self.cols, self.y);
        // A threshold without colors to set is drawn, but greyed.
        let inert =
            matches!(item, Item::Levels(_)) && state.sidebar.as_ref().is_some_and(|s| !s.colors);
        let text = if inert { GREY } else { FG };
        let base = if self.selected {
            Style::fg(text).on(BG2)
        } else {
            Style::fg(text)
        };
        if self.selected {
            pen.canvas.fill(cols.left, y, cols.right - cols.left, base);
            pen.canvas
                .text(cols.left, y, "▌", Style::fg(YELLOW).on(BG2));
        }
        let label_style = if self.selected { base.bold() } else { base };
        let limit = cols.limit();
        let end = cols.values_end();
        let dim = Style {
            bg: base.bg,
            ..Style::fg(GREY)
        };
        let typing = state.typing.as_deref().filter(|_| self.selected);
        // The next lines of a list: its elements alone, under the first.
        if self.part > 0 {
            self.chips_line(pen, state, item, base);
            return;
        }
        if item.is_action() {
            // No value: the name takes the row, its key at the end.
            const KEY: &str = "Entrée ↵";
            let room = usize::from(limit.saturating_sub(cols.left + 2));
            let key_room = room.saturating_sub(width(KEY) + 2);
            let shows_key = self.selected && key_room >= 12;
            let label_room = if shows_key { key_room } else { room };
            // The update in green, followed by what has a new version.
            let (label, label_style) = match state.news.as_deref() {
                Some(news) if item == Item::Update => (
                    format!("{} · {news}", item.label()),
                    Style {
                        fg: Some(GREEN),
                        ..label_style
                    },
                ),
                _ => (item.label().to_string(), label_style),
            };
            let label = ellipsize(&label, label_room);
            pen.canvas
                .text_to(cols.left + 2, y, &label, label_style, cols.right);
            if shows_key {
                let x = limit - width(KEY) as u16;
                pen.canvas.text_to(x, y, KEY, dim, limit);
            }
            return;
        }
        let label = ellipsize(item.label(), usize::from(cols.label));
        pen.canvas
            .text_to(cols.left + 2, y, &label, label_style, cols.right);
        if cols.value >= end {
            return;
        }
        self.reset_button(pen, state, item, base);
        if let Some(text) = typing {
            self.field(pen, state, item, text, end);
            return;
        }
        let room = usize::from(end - cols.value);
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
                        let hovered = if i == current {
                            style
                        } else {
                            Style::fg(FG).on(BG3)
                        };
                        let target = Target::Choose(self.index, i);
                        let text = format!(" {option} ");
                        x = pen.button((x, y), &text, (style, hovered), end, target) + 1;
                    }
                } else {
                    let current = options.get(current).map_or("", String::as_str);
                    self.stepper(pen, current, base, dim, end);
                }
            }
            Shown::Number(text) => self.stepper(pen, &text, base, dim, end),
            Shown::List(list) if self.chips.is_none() => {
                // No room to show the elements: how many there are.
                let text = format!("{} éléments", list.len());
                pen.canvas
                    .text_to(cols.value + 1, y, &ellipsize(&text, room - 1), dim, end);
            }
            Shown::List(_) => self.chips_line(pen, state, item, base),
            Shown::Levels(values, unit) => {
                let part = state.part_of(item);
                let mut x = cols.value + 1;
                for (i, level) in LEVELS.into_iter().enumerate() {
                    let color = if inert { BG3 } else { level_color(level) };
                    let bar = Style {
                        bg: base.bg,
                        ..Style::fg(color)
                    };
                    x = pen.canvas.text_to(x, y, "━━", bar, end);
                    if let Some(value) = values.get(i) {
                        let style = if self.selected && i == part {
                            Style::fg(ON_ACCENT).on(YELLOW).bold()
                        } else {
                            base.bold()
                        };
                        let hovered = Style::fg(FG).on(BG3).bold();
                        let target = Target::Part(self.index, i);
                        x = pen.button(
                            (x, y),
                            &format!(" {value} "),
                            (style, hovered),
                            end,
                            target,
                        );
                    }
                }
                pen.canvas.text_to(x + 1, y, unit, dim, end);
            }
            // Drawn above.
            Shown::Action => {}
        }
    }

    /// The line `part` of the item's list: its elements, the chosen one in
    /// yellow on the selected row, a `✕` at the end of the chosen or
    /// hovered one, which removes it; `+` after the last, to add one.
    fn chips_line(&self, pen: &mut Pen, state: &State, item: Item, base: Style) {
        let Some(line) = self.chips.and_then(|lines| lines.get(self.part)) else {
            return;
        };
        let (cols, y) = (self.cols, self.y);
        let end = cols.values_end();
        let chosen = self.selected.then(|| state.part_of(item));
        let count = match state.shown(item) {
            Shown::List(list) => list.len(),
            _ => 0,
        };
        let mut x = cols.value;
        for (i, text) in line {
            let cells = width(text) as u16;
            let is_add = *i >= count;
            let hovered = pen.over(x, x + cells, y);
            let style = match (chosen == Some(*i), is_add) {
                (true, _) => Style::fg(ON_ACCENT).on(YELLOW).bold(),
                (false, true) => Style::fg(AQUA).on(base.bg.unwrap_or(BG1)).bold(),
                (false, false) => Style::fg(FG).on(if self.selected { BG3 } else { BG1 }),
            };
            let style = if hovered && chosen != Some(*i) {
                Style::fg(FG).on(BG3).bold()
            } else {
                style
            };
            let shown = if (hovered || chosen == Some(*i)) && !is_add {
                // The `✕` in place of the last space: the width stays.
                format!("{}{REMOVE}", fit(text, usize::from(cells) - 1))
            } else {
                text.clone()
            };
            let after = pen.canvas.text_to(x, y, &shown, style, end);
            pen.hits.push(Hit {
                y,
                x,
                end: after,
                target: Target::Part(self.index, *i),
            });
            if !is_add && after > x {
                pen.hits.push(Hit {
                    y,
                    x: after - 1,
                    end: after,
                    target: Target::Remove(self.index, *i),
                });
            }
            x = after + 1;
        }
    }

    /// The button that puts a setting changed back to its default: named on
    /// the selected row, a mark on the others; none at its default.
    fn reset_button(&self, pen: &mut Pen, state: &State, item: Item, base: Style) {
        let cols = self.cols;
        if cols.values_end() == cols.limit() || state.is_default(item) {
            return;
        }
        let label = if self.selected { RESET } else { RESET_MARK };
        let x = cols.limit() - width(label) as u16;
        let style = if self.selected {
            Style::fg(FG).on(BG3)
        } else {
            Style {
                bg: base.bg,
                ..Style::fg(FG4)
            }
        };
        let hovered = Style::fg(ON_ACCENT).on(YELLOW).bold();
        let target = Target::Reset(self.index);
        pen.button((x, self.y), label, (style, hovered), cols.limit(), target);
    }

    /// The field typing the item's chosen part: what it types, then the
    /// text, its end in view, highlighted while the first key replaces it.
    fn field(&self, pen: &mut Pen, state: &State, item: Item, text: &str, end: u16) {
        let (cols, y) = (self.cols, self.y);
        let field = Style::fg(FG).on(DARK);
        pen.canvas.fill(cols.value, y, end - cols.value, field);
        let prompt = state.typing_prompt(item);
        let x = pen
            .canvas
            .text_to(cols.value + 1, y, &prompt, Style::fg(GREY).on(DARK), end);
        let room = usize::from(end.saturating_sub(x)).saturating_sub(1);
        let style = if state.fresh {
            Style::fg(ON_ACCENT).on(FG4)
        } else {
            field
        };
        let x = pen.canvas.text_to(x, y, &tail(text, room), style, end);
        pen.canvas
            .text_to(x, y, "▏", Style::fg(YELLOW).on(DARK), end);
    }

    /// `‹ value ›`: a click on an arrow steps back or forward, on the value
    /// types it.
    fn stepper(&self, pen: &mut Pen, value: &str, base: Style, dim: Style, end: u16) {
        let (cols, y) = (self.cols, self.y);
        let arrow = Style::fg(FG).on(BG3);
        let styles = (dim, arrow);
        let back = pen.button(
            (cols.value, y),
            "‹ ",
            styles,
            end,
            Target::Step(self.index, false),
        );
        let x = pen.canvas.text_to(back, y, value, base.bold(), end);
        pen.button((x, y), " ›", styles, end, Target::Step(self.index, true));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_around_the_list_shrink_with_the_screen() {
        let tall = areas(30);
        assert_eq!((tall.list, tall.rows, tall.about_rows), (2, 22, 3));
        assert_eq!(tall.footer, Some(29));
        assert_eq!(tall.about, Some(25));
        assert_eq!(tall.rule, Some(24));
        let medium = areas(16);
        assert_eq!((medium.rows, medium.about_rows), (9, 2));
        let small = areas(12);
        assert_eq!((small.rows, small.rule, small.about), (6, Some(8), Some(9)));
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
    fn the_settings_take_the_width_and_the_scrollbar_its_edge() {
        let wide = columns(140, 30, false);
        assert_eq!((wide.left, wide.right, wide.bar), (1, 139, None));
        assert_eq!(wide.value, 1 + 2 + 30 + 2);
        // The button back to the default keeps its room.
        assert_eq!(wide.values_end(), 138 - 10 - 1);
        let narrow = columns(40, 30, true);
        assert_eq!((narrow.left, narrow.right, narrow.bar), (1, 38, Some(39)));
        // The name gives way to the value.
        assert_eq!(narrow.label, 16);
        assert_eq!(narrow.values_end(), narrow.limit());
        let none = columns(0, 30, true);
        assert_eq!((none.left, none.right, none.value), (0, 0, 0));
    }

    #[test]
    fn the_list_scrolls_to_the_selection_and_its_header() {
        let at = |line, header_above| Span {
            first: line,
            last: line,
            header_above,
        };
        // Within view: no move.
        assert_eq!(scrolled(0, at(5, false), 10, 30), 0);
        // Below: just enough.
        assert_eq!(scrolled(0, at(12, false), 10, 30), 3);
        // Above: its header too.
        assert_eq!(scrolled(8, at(6, true), 10, 30), 5);
        assert_eq!(scrolled(8, at(6, false), 10, 30), 6);
        // A taller screen: no rows left blank at the end.
        assert_eq!(scrolled(25, at(26, false), 10, 30), 20);
        assert_eq!(scrolled(4, at(3, false), 0, 30), 3);
        // A list on several lines: all of them, or its first ones.
        let list = |first, last| Span {
            first,
            last,
            header_above: false,
        };
        assert_eq!(scrolled(0, list(8, 11), 10, 30), 2);
        assert_eq!(scrolled(0, list(8, 30), 10, 40), 8);
    }

    #[test]
    fn a_list_wraps_between_its_elements_then_offers_one_more() {
        let list = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let texts = |lines: Vec<Vec<Chip>>| -> Vec<Vec<String>> {
            lines
                .into_iter()
                .map(|l| l.into_iter().map(|(_, t)| t).collect())
                .collect()
        };
        assert_eq!(
            texts(chip_lines(&list(&["txt", "md"]), 20)),
            [[" txt ", " md ", " + "]]
        );
        assert_eq!(
            texts(chip_lines(&list(&["txt", "md", "rst"]), 11)),
            [vec![" txt ", " md "], vec![" rst ", " + "]]
        );
        // Too long for a line: cut.
        assert_eq!(
            texts(chip_lines(&list(&["markdown"]), 6)),
            [vec![" mar… "], vec![" + "]]
        );
        let lines = chip_lines(&list(&["a", "b"]), 30);
        assert_eq!(lines[0].last().unwrap().0, 2);
    }

    #[test]
    fn a_description_wraps_between_its_words() {
        assert_eq!(wrap_words("un deux trois", 20, 2), ["un deux trois"]);
        assert_eq!(wrap_words("un deux trois", 8, 2), ["un deux", "trois"]);
        assert_eq!(
            wrap_words("un deux trois quatre", 8, 2),
            ["un deux", "trois …"]
        );
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
            max_top: 0,
        };
        assert_eq!(drawn.at(5, 1), Some(Target::Choose(2, 1)));
        assert_eq!(drawn.at(1, 1), Some(Target::Item(2)));
        assert_eq!(drawn.at(7, 1), Some(Target::Item(2)));
        assert_eq!(drawn.at(5, 0), None);
    }
}
