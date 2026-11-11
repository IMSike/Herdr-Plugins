//! The screens drawn in a pane: the terminal taken over (`Screen`), a frame
//! drawn in one write (`Frame`), text cut to the cells it takes (`fit`,
//! `width`), the popup menu (one row per entry, chosen by key, arrows or
//! click), a list of boxes to tick (`pick`), and the plain prompts of a
//! terminal (`ask`, `pause`).

use crate::config::{Action, Entry};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::{execute, queue};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

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

/// A screen drawn in one write: a terminal never shows it cleared but not
/// drawn again yet, which blinks.
pub struct Frame(Vec<u8>);

impl Frame {
    /// A cleared screen.
    pub fn new() -> io::Result<Self> {
        let mut out = Vec::new();
        queue!(out, BeginSynchronizedUpdate, Clear(ClearType::All))?;
        Ok(Self(out))
    }

    /// Writes it to the terminal, unless it is `last`, the frame shown
    /// before: an event that changes nothing (a key released, the pointer
    /// moved within a row) sends nothing to Herdr.
    pub fn show_if_new(self, last: &mut Vec<u8>) -> io::Result<()> {
        if self.0 == *last {
            return Ok(());
        }
        last.clone_from(&self.0);
        self.show()
    }

    /// Writes it to the terminal.
    pub fn show(mut self) -> io::Result<()> {
        queue!(self.0, EndSynchronizedUpdate)?;
        let mut stdout = io::stdout().lock();
        stdout.write_all(&self.0)?;
        stdout.flush()
    }
}

impl Write for Frame {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Whether a key holds Ctrl, and not AltGr: Windows reports AltGr as
/// Ctrl+Alt, which types `@` or `{` on some keyboards.
pub fn is_control(modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::CONTROL) && !modifiers.contains(KeyModifiers::ALT)
}

/// What a key does in a list of `count` rows, once the list's own keys
/// (an entry's key, Space) are set aside.
enum Nav {
    Cancel,
    Confirm,
    /// The row to select.
    Select(usize),
    Nothing,
}

/// The keys every list shares: Esc, q or Ctrl+C close it, Enter confirms,
/// the arrows (or `k`, `j`) move the selection round.
fn nav(key: event::KeyEvent, selected: usize, count: usize) -> Nav {
    if is_control(key.modifiers) {
        return if key.code == KeyCode::Char('c') {
            Nav::Cancel
        } else {
            Nav::Nothing
        };
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Nav::Cancel,
        KeyCode::Enter if count > 0 => Nav::Confirm,
        KeyCode::Up | KeyCode::Char('k') if count > 0 => {
            Nav::Select((selected + count - 1) % count)
        }
        KeyCode::Down | KeyCode::Char('j') if count > 0 => Nav::Select((selected + 1) % count),
        _ => Nav::Nothing,
    }
}

/// The row of a list of `count` rows, drawn from the `top` one in
/// `visible` rows, that the screen row `row` shows: only a drawn row
/// answers the pointer.
fn hit(row: u16, top: usize, visible: usize, count: usize) -> Option<usize> {
    let row = usize::from(row.checked_sub(LIST_TOP)?);
    Some(top + row).filter(|&i| row < visible && i < count)
}

/// Shows the menu; returns the index of the chosen entry, or `None` when
/// closed. `news`: what has a new version, shown on the `Update` entry.
pub fn choose(
    title: &str,
    entries: &[Entry],
    error: Option<&str>,
    news: Option<&str>,
) -> io::Result<Option<usize>> {
    let _screen = Screen::enter()?;
    let mut selected = 0usize;
    // The first entry drawn: the list scrolls to keep the selection visible.
    let mut top = 0usize;
    let mut shown = Vec::new();
    let count = entries.len();
    loop {
        let (_, height) = terminal::size()?;
        let visible = visible_rows(height);
        top = scrolled(top, selected, visible);
        draw(title, entries, top, error, selected, visible, news)?.show_if_new(&mut shown)?;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                // An entry's own key wins over the navigation keys.
                if let KeyCode::Char(c) = key.code
                    && !is_control(key.modifiers)
                    && let Some(i) = entries.iter().position(|e| e.key == c)
                {
                    return Ok(Some(i));
                }
                match nav(key, selected, count) {
                    Nav::Cancel => return Ok(None),
                    Nav::Confirm => return Ok(Some(selected)),
                    Nav::Select(i) => selected = i,
                    Nav::Nothing => {}
                }
            }
            Event::Mouse(mouse) => match (mouse.kind, hit(mouse.row, top, visible, count)) {
                (MouseEventKind::Down(MouseButton::Left), Some(i)) => return Ok(Some(i)),
                (MouseEventKind::Moved, Some(i)) => selected = i,
                _ => {}
            },
            _ => {}
        }
    }
}

/// Shows `items`, each with a box ticked at first, and lets them be
/// unticked; returns the ticks once confirmed, `None` when cancelled.
pub fn pick(title: &str, items: &[String]) -> io::Result<Option<Vec<bool>>> {
    let _screen = Screen::enter()?;
    let mut ticked = vec![true; items.len()];
    let mut selected = 0usize;
    let mut top = 0usize;
    let mut shown = Vec::new();
    let count = items.len();
    loop {
        let (_, height) = terminal::size()?;
        let visible = visible_rows(height);
        top = scrolled(top, selected, visible);
        draw_pick(title, items, &ticked, top, selected, visible)?.show_if_new(&mut shown)?;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key.code == KeyCode::Char(' ') && count > 0 {
                    ticked[selected] = !ticked[selected];
                    continue;
                }
                match nav(key, selected, count) {
                    Nav::Cancel => return Ok(None),
                    Nav::Confirm => return Ok(Some(ticked)),
                    Nav::Select(i) => selected = i,
                    Nav::Nothing => {}
                }
            }
            Event::Mouse(mouse) => match (mouse.kind, hit(mouse.row, top, visible, count)) {
                (MouseEventKind::Down(MouseButton::Left), Some(i)) => {
                    selected = i;
                    ticked[i] = !ticked[i];
                }
                (MouseEventKind::Moved, Some(i)) => selected = i,
                _ => {}
            },
            _ => {}
        }
    }
}

/// A frame with `title` on its first row and `help` on its last; with the
/// terminal's width and height.
fn framed(title: &str, help: &str) -> io::Result<(Frame, usize, u16)> {
    let (width, height) = terminal::size()?;
    let width = usize::from(width);
    let mut out = Frame::new()?;
    queue!(
        out,
        MoveTo(0, 0),
        SetAttribute(Attribute::Bold),
        Print(fit(&format!(" {title}"), width)),
        SetAttribute(Attribute::Reset),
        MoveTo(0, height.saturating_sub(1)),
        SetAttribute(Attribute::Dim),
        Print(fit(help, width)),
        SetAttribute(Attribute::Reset)
    )?;
    Ok((out, width, height))
}

/// Draws the `row`th row of a list, highlighted when `selected`, in `color`
/// if any.
fn draw_row(
    out: &mut Frame,
    row: usize,
    line: &str,
    width: usize,
    selected: bool,
    color: Option<Color>,
) -> io::Result<()> {
    queue!(out, MoveTo(0, LIST_TOP + row as u16))?;
    if selected {
        queue!(out, SetAttribute(Attribute::Reverse))?;
    }
    if let Some(color) = color {
        queue!(out, SetForegroundColor(color))?;
    }
    queue!(
        out,
        Print(pad(line, width)),
        ResetColor,
        SetAttribute(Attribute::Reset)
    )
}

/// Draws the items of `pick`, from the `top` one in `visible` rows, and
/// their boxes.
fn draw_pick(
    title: &str,
    items: &[String],
    ticked: &[bool],
    top: usize,
    selected: usize,
    visible: usize,
) -> io::Result<Frame> {
    let (mut out, width, _) = framed(
        title,
        " Espace ou clic : cocher · Entrée : mettre à jour · Échap : annuler",
    )?;
    let rows = items.iter().zip(ticked).enumerate().skip(top).take(visible);
    for (i, (item, &on)) in rows {
        let tick = if on { "[x]" } else { "[ ]" };
        let line = format!("  {tick} {item}");
        draw_row(&mut out, i - top, &line, width, i == selected, None)?;
    }
    Ok(out)
}

/// Rows of the list: the title and a gap above, the error and help lines below.
fn visible_rows(height: u16) -> usize {
    usize::from(height.saturating_sub(LIST_TOP + 2))
}

/// The first entry to draw, moved just enough to show `selected`.
pub(crate) fn scrolled(top: usize, selected: usize, visible: usize) -> usize {
    if visible == 0 || selected < top {
        selected
    } else if selected >= top + visible {
        selected + 1 - visible
    } else {
        top
    }
}

/// Draws `entries`, from the `top` one, in `visible` rows.
fn draw(
    title: &str,
    entries: &[Entry],
    top: usize,
    error: Option<&str>,
    selected: usize,
    visible: usize,
    news: Option<&str>,
) -> io::Result<Frame> {
    let (mut out, width, height) = framed(title, " touche, ↑↓ Entrée, clic · Échap")?;
    for (i, entry) in entries.iter().enumerate().skip(top).take(visible) {
        let news = news.filter(|_| entry.action == Some(Action::Update));
        let line = match news {
            Some(news) => format!("  {}  {} · {news}", entry.key, entry.label),
            None => format!("  {}  {}", entry.key, entry.label),
        };
        let color = news.map(|_| crate::colors::NEW_VERSION_COLOR);
        draw_row(&mut out, i - top, &line, width, i == selected, color)?;
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
    Ok(out)
}

/// The cells `text` takes on screen, as `fit` shows it: two for a wide
/// character (CJK), none for a combining one.
pub(crate) fn width(text: &str) -> usize {
    text.chars()
        .map(shown)
        .map(|c| c.width().unwrap_or(0))
        .sum()
}

/// How a character is shown: a control character (an escape, a line break,
/// in a label or a message) would act on the terminal instead, so it shows
/// as `�`, a tab as a space.
fn shown(c: char) -> char {
    match c {
        '\t' => ' ',
        c if c.is_control() => '\u{FFFD}',
        c => c,
    }
}

/// `text` cut to `width` cells, its control characters shown (see `shown`).
pub(crate) fn fit(text: &str, width: usize) -> String {
    let mut used = 0;
    text.chars()
        .map(shown)
        .take_while(|c| {
            used += c.width().unwrap_or(0);
            used <= width
        })
        .collect()
}

/// Cuts or pads `text` to exactly `width` cells, so a highlight spans the row.
fn pad(text: &str, width: usize) -> String {
    let text = fit(text, width);
    let used = self::width(&text);
    format!("{text}{}", " ".repeat(width.saturating_sub(used)))
}

/// Asks a yes-or-no question; an empty answer is `default`, an unreadable
/// one is no.
pub fn ask(question: &str, default: bool) -> bool {
    print!("{question}");
    let _ = io::Write::flush(&mut io::stdout());
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    is_yes(&answer, default)
}

fn is_yes(answer: &str, default: bool) -> bool {
    match answer.trim().to_lowercase().as_str() {
        "" => default,
        "o" | "oui" | "y" | "yes" => true,
        _ => false,
    }
}

/// Leaves `message` on screen until Enter, and says so: the popup or the
/// window closes with this process.
pub fn pause(message: &str) {
    println!("{message}\n\nAppuyer sur Entrée pour fermer.");
    let _ = io::stdout().flush();
    let _ = io::stdin().read_line(&mut String::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_answer_takes_the_default() {
        assert!(is_yes("\n", true) && !is_yes("\n", false));
        assert!(is_yes(" Oui\n", false) && is_yes("y", false));
        assert!(!is_yes("non", true) && !is_yes("x", true));
    }

    #[test]
    fn pad_and_fit_count_cells() {
        assert_eq!(pad("é", 3), "é  ");
        assert_eq!(pad("abcdef", 3), "abc");
        assert_eq!(fit("Échap", 2), "Éc");
        // A wide character takes two cells, and is not cut in half.
        assert_eq!(pad("日本", 5), "日本 ");
        assert_eq!(fit("日本", 3), "日");
        // Nothing acts on the terminal.
        assert_eq!(fit("a\u{1b}[2Jb\tc\n", 10), "a\u{FFFD}[2Jb c\u{FFFD}");
        assert_eq!(width("a\u{1b}b"), 3);
    }

    #[test]
    fn the_list_scrolls_to_the_selection() {
        assert_eq!(visible_rows(14), 10);
        assert_eq!(scrolled(0, 3, 10), 0);
        assert_eq!(scrolled(0, 12, 10), 3);
        assert_eq!(scrolled(3, 1, 10), 1);
        assert_eq!(scrolled(0, 5, 0), 5);
    }

    #[test]
    fn only_a_drawn_row_answers_the_pointer() {
        // Rows 2 to 11 show entries 3 to 12 of 20.
        assert_eq!(hit(2, 3, 10, 20), Some(3));
        assert_eq!(hit(11, 3, 10, 20), Some(12));
        assert_eq!(hit(12, 3, 10, 20), None);
        assert_eq!(hit(1, 3, 10, 20), None);
        assert_eq!(hit(4, 0, 10, 2), None);
    }
}
