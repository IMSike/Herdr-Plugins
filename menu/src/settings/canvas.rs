//! A screen drawn cell by cell before it is written: text placed at a
//! column, cut at the screen's edge, each cell with its colors. The layout
//! is computed without a terminal, and tested as plain text.

use crate::ui::fit;
use crossterm::queue;
use crossterm::style::{
    Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

/// How a cell is drawn; `None` colors are the terminal's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
}

impl Style {
    pub const fn fg(color: Color) -> Self {
        Self {
            fg: Some(color),
            bg: None,
            bold: false,
            italic: false,
        }
    }

    pub const fn on(self, color: Color) -> Self {
        Self {
            bg: Some(color),
            ..self
        }
    }

    pub const fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    pub const fn italic(self) -> Self {
        Self {
            italic: true,
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Cell {
    /// Its character, with the combining ones after it; empty for the
    /// second half of a wide character.
    text: String,
    style: Style,
}

impl Cell {
    fn blank(style: Style) -> Self {
        Self {
            text: " ".into(),
            style,
        }
    }
}

pub struct Canvas {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl Canvas {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::blank(Style::default()); usize::from(width) * usize::from(height)],
        }
    }

    fn cell(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        if x < self.width && y < self.height {
            self.cells
                .get_mut(usize::from(y) * usize::from(self.width) + usize::from(x))
        } else {
            None
        }
    }

    /// Paints `count` cells from `x` blank, in `style`.
    pub fn fill(&mut self, x: u16, y: u16, count: u16, style: Style) {
        for i in 0..count {
            if let Some(cell) = self.cell(x.saturating_add(i), y) {
                *cell = Cell::blank(style);
            }
        }
    }

    /// Writes `text` from `x`, no further than `limit` (excluded) nor the
    /// screen's edge; the column after it.
    pub fn text_to(&mut self, x: u16, y: u16, text: &str, style: Style, limit: u16) -> u16 {
        let room = usize::from(limit.min(self.width).saturating_sub(x));
        let mut at = x;
        for c in fit(text, room).chars() {
            match c.width().unwrap_or(0) {
                0 => {
                    if let Some(cell) = self.cell(at.wrapping_sub(1), y) {
                        cell.text.push(c);
                    }
                }
                w => {
                    if let Some(cell) = self.cell(at, y) {
                        *cell = Cell {
                            text: c.to_string(),
                            style,
                        };
                    }
                    for i in 1..w as u16 {
                        if let Some(cell) = self.cell(at + i, y) {
                            *cell = Cell {
                                text: String::new(),
                                style,
                            };
                        }
                    }
                    at += w as u16;
                }
            }
        }
        at
    }

    /// Writes `text` from `x`, up to the screen's edge.
    pub fn text(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        self.text_to(x, y, text, style, self.width)
    }

    /// Row `y` as plain text, its trailing blanks left out.
    #[cfg(test)]
    fn row_text(&self, y: u16) -> String {
        let start = usize::from(y) * usize::from(self.width);
        let row = &self.cells[start..start + usize::from(self.width)];
        let text: String = row.iter().map(|c| c.text.as_str()).collect();
        text.trim_end().to_string()
    }

    /// The whole screen as plain text, one line per row.
    #[cfg(test)]
    pub fn to_text(&self) -> String {
        (0..self.height)
            .map(|y| self.row_text(y))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Queues the screen to `out`, row by row, a style change only where
    /// the style changes.
    pub fn write_to(&self, out: &mut impl Write) -> io::Result<()> {
        for y in 0..self.height {
            queue!(out, crossterm::cursor::MoveTo(0, y))?;
            let mut current: Option<Style> = None;
            let start = usize::from(y) * usize::from(self.width);
            for cell in &self.cells[start..start + usize::from(self.width)] {
                if current != Some(cell.style) {
                    set_style(out, cell.style)?;
                    current = Some(cell.style);
                }
                queue!(out, Print(&cell.text))?;
            }
            queue!(out, SetAttribute(Attribute::Reset))?;
        }
        Ok(())
    }
}

fn set_style(out: &mut impl Write, style: Style) -> io::Result<()> {
    queue!(
        out,
        SetAttribute(Attribute::Reset),
        SetForegroundColor(style.fg.unwrap_or(Color::Reset)),
        SetBackgroundColor(style.bg.unwrap_or(Color::Reset))
    )?;
    if style.bold {
        queue!(out, SetAttribute(Attribute::Bold))?;
    }
    if style.italic {
        queue!(out, SetAttribute(Attribute::Italic))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_cut_at_its_limit_and_the_edge() {
        let mut canvas = Canvas::new(8, 2);
        assert_eq!(canvas.text(2, 0, "réglages", Style::default()), 8);
        assert_eq!(canvas.row_text(0), "  réglag");
        assert_eq!(canvas.text_to(1, 1, "abcdef", Style::default(), 4), 4);
        assert_eq!(canvas.row_text(1), " abc");
        // Out of the screen: nothing, no panic.
        canvas.text(9, 5, "x", Style::default());
        canvas.fill(6, 9, 4, Style::default());
    }

    #[test]
    fn a_wide_character_takes_two_cells() {
        let mut canvas = Canvas::new(6, 1);
        assert_eq!(canvas.text(0, 0, "日本語", Style::default()), 6);
        assert_eq!(canvas.row_text(0), "日本語");
        let mut out = Vec::new();
        canvas.write_to(&mut out).unwrap();
        assert!(String::from_utf8(out).unwrap().contains("日本語"));
    }
}
