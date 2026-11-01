//! The menu drawn in its popup: one row per entry, chosen by key, arrows or click.

use crate::config::Entry;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Write};

/// First screen row of the entry list.
const LIST_TOP: u16 = 2;

/// Draws on the alternate screen, which keeps no scrollback: a redraw leaves
/// nothing to scroll back to. Puts the terminal back as it was, even on an
/// early return or a panic.
pub struct Screen;

impl Screen {
    pub fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide, EnableMouseCapture)?;
        Ok(Screen)
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            Show,
            ResetColor,
            LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}

/// Shows the menu; returns the index of the chosen entry, or `None` when closed.
pub fn choose(title: &str, entries: &[Entry], error: Option<&str>) -> io::Result<Option<usize>> {
    let _screen = Screen::enter()?;
    let mut selected = 0usize;
    // The first entry drawn: the list scrolls to keep the selection visible.
    let mut top = 0usize;
    loop {
        let (_, height) = terminal::size()?;
        let visible = visible_rows(height);
        top = scrolled(top, selected, visible);
        draw(title, &entries[top..], error, selected - top, visible)?;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    if key.code == KeyCode::Char('c') {
                        return Ok(None);
                    }
                    continue;
                }
                // An entry's own key wins over the navigation keys below.
                if let KeyCode::Char(c) = key.code
                    && let Some(i) = entries.iter().position(|e| e.key == c)
                {
                    return Ok(Some(i));
                }
                let count = entries.len();
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                    KeyCode::Up | KeyCode::Char('k') if count > 0 => {
                        selected = (selected + count - 1) % count;
                    }
                    KeyCode::Down | KeyCode::Char('j') if count > 0 => {
                        selected = (selected + 1) % count;
                    }
                    KeyCode::Enter if count > 0 => return Ok(Some(selected)),
                    _ => {}
                }
            }
            Event::Mouse(mouse) => {
                // Only a drawn row answers a click.
                let row = mouse.row.checked_sub(LIST_TOP).map(usize::from);
                let hit = row
                    .filter(|&r| r < visible)
                    .map(|r| top + r)
                    .filter(|&i| i < entries.len());
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some(i) = hit {
                            return Ok(Some(i));
                        }
                    }
                    MouseEventKind::Moved => {
                        if let Some(i) = hit {
                            selected = i;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// Rows of the list: the title and a gap above, the error and help lines below.
fn visible_rows(height: u16) -> usize {
    usize::from(height.saturating_sub(LIST_TOP + 2))
}

/// The first entry to draw, moved just enough to show `selected`.
fn scrolled(top: usize, selected: usize, visible: usize) -> usize {
    if visible == 0 || selected < top {
        selected
    } else if selected >= top + visible {
        selected + 1 - visible
    } else {
        top
    }
}

/// Draws `entries` (from the first visible one) in `visible` rows.
fn draw(
    title: &str,
    entries: &[Entry],
    error: Option<&str>,
    selected: usize,
    visible: usize,
) -> io::Result<()> {
    let (width, height) = terminal::size()?;
    let width = usize::from(width);
    let mut out = io::stdout();
    queue!(out, Clear(ClearType::All), MoveTo(0, 0))?;
    queue!(
        out,
        SetAttribute(Attribute::Bold),
        Print(fit(&format!(" {title}"), width)),
        SetAttribute(Attribute::Reset)
    )?;
    for (i, entry) in entries.iter().take(visible).enumerate() {
        let row = LIST_TOP + i as u16;
        queue!(out, MoveTo(0, row))?;
        if i == selected {
            queue!(out, SetAttribute(Attribute::Reverse))?;
        }
        let line = format!("  {}  {}", entry.key, entry.label);
        queue!(
            out,
            Print(pad(&line, width)),
            SetAttribute(Attribute::Reset)
        )?;
    }
    if let Some(error) = error {
        queue!(
            out,
            MoveTo(0, height.saturating_sub(2)),
            SetForegroundColor(Color::Red),
            Print(fit(&format!(" {error}"), width)),
            ResetColor
        )?;
    }
    queue!(
        out,
        MoveTo(0, height.saturating_sub(1)),
        SetAttribute(Attribute::Dim),
        Print(fit(" touche, ↑↓ Entrée, clic · Échap", width)),
        SetAttribute(Attribute::Reset)
    )?;
    out.flush()
}

/// Cuts `text` to `width` characters.
pub(crate) fn fit(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

/// Cuts or pads `text` to exactly `width` characters, so a highlight spans the row.
fn pad(text: &str, width: usize) -> String {
    let text = fit(text, width);
    let len = text.chars().count();
    format!("{text}{}", " ".repeat(width - len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_and_fit_count_characters() {
        assert_eq!(pad("é", 3), "é  ");
        assert_eq!(pad("abcdef", 3), "abc");
        assert_eq!(fit("Échap", 2), "Éc");
    }

    #[test]
    fn the_list_scrolls_to_the_selection() {
        assert_eq!(visible_rows(14), 10);
        assert_eq!(scrolled(0, 3, 10), 0);
        assert_eq!(scrolled(0, 12, 10), 3);
        assert_eq!(scrolled(3, 1, 10), 1);
        assert_eq!(scrolled(0, 5, 0), 5);
    }
}
