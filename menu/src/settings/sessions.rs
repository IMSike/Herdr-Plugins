//! The screen of « Récupérer une session… »: the session's snapshots, the
//! latest first, each summed up (workspaces, tabs, panes, agents) with what
//! the session lacks now; the selected one in detail below, workspace by
//! workspace, tab by tab, pane by pane, with each agent and the title of its
//! Claude Code conversation. Nothing changes until Enter, which opens the
//! window that restores it (see `recover`).

use super::canvas::{Canvas, Style};
use super::view::{AQUA, BG1, BG2, FG, FG4, GREY, RED, YELLOW, ellipsize};
use crate::herdr::{self, Session};
use crate::recover::{self, Snapshot, claude, layout::Layout};
use crate::ui::{Frame, Screen, is_control, width};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use crossterm::terminal;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::time::SystemTime;

/// nf-md-backup_restore.
const TITLE_ICON: &str = "\u{F006F}";
/// The first row of the list.
const LIST_TOP: u16 = 4;

/// How the screen was left.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Back to the settings.
    Back,
    /// The window that restores a snapshot is open.
    Restoring,
}

struct Browser {
    /// `Err` when the session is not found: the screen only says why.
    session: Result<Session, String>,
    snapshots: Vec<Snapshot>,
    current: Result<Layout, String>,
    selected: usize,
    /// The first snapshot of the list on screen.
    top: usize,
    /// The first row of the details on screen.
    detail_top: usize,
    /// The titles of the Claude Code conversations, by id, once looked for.
    titles: HashMap<String, Option<String>>,
    /// Why the snapshot could not be restored.
    error: Option<String>,
    now: SystemTime,
}

impl Browser {
    fn new(session: Result<Session, String>) -> Self {
        let (snapshots, current) = match &session {
            Ok(s) => (recover::snapshots(s), recover::current(s)),
            Err(_) => (Vec::new(), Ok(Layout::default())),
        };
        Self {
            session,
            snapshots,
            current,
            selected: 0,
            top: 0,
            detail_top: 0,
            titles: HashMap::new(),
            error: None,
            now: SystemTime::now(),
        }
    }

    fn select(&mut self, index: usize) {
        if index != self.selected {
            self.selected = index;
            self.detail_top = 0;
            self.error = None;
        }
    }

    /// Looks for the titles of the selected snapshot's Claude conversations.
    fn find_titles(&mut self) {
        let Some(Ok(layout)) = self.snapshots.get(self.selected).map(|s| &s.layout) else {
            return;
        };
        for agent in layout.panes().filter_map(|p| p.agent.as_ref()) {
            if let Some(id) = agent
                .conversation
                .as_ref()
                .filter(|_| agent.name == "claude")
                && !self.titles.contains_key(id)
            {
                self.titles.insert(id.clone(), claude::title(id));
            }
        }
    }

    /// The panes of `snapshot` the session lacks now: none when its layout
    /// now is unknown.
    fn missing(&self, layout: &Layout) -> Vec<bool> {
        match &self.current {
            Ok(current) => layout.missing_from(current),
            Err(_) => vec![false; layout.counts().panes],
        }
    }
}

/// The screen drawn, and where its list is.
struct Drawn {
    canvas: Canvas,
    top: usize,
    /// The rows of the list on screen, from `LIST_TOP`.
    rows: usize,
    /// The first row of the details on screen.
    detail_top: usize,
}

impl Drawn {
    /// The snapshot at screen row `y`, if a drawn one.
    fn at(&self, y: u16, count: usize) -> Option<usize> {
        let row = usize::from(y.checked_sub(LIST_TOP)?);
        Some(self.top + row).filter(|&i| row < self.rows && i < count)
    }
}

/// `~/src/app` for a folder in the home folder.
fn short(path: &Path, home: Option<&Path>) -> String {
    let text = path.to_string_lossy();
    let text = text.trim_end_matches(['/', '\\']);
    let Some(home) = home.map(|h| {
        h.to_string_lossy()
            .trim_end_matches(['/', '\\'])
            .to_string()
    }) else {
        return text.to_string();
    };
    // `get` gives nothing past the end or inside a character.
    match (text.get(..home.len()), text.get(home.len()..)) {
        (Some(start), Some(rest))
            if !home.is_empty()
                && (rest.is_empty() || rest.starts_with(['/', '\\']))
                && if cfg!(windows) {
                    start.eq_ignore_ascii_case(&home)
                } else {
                    start == home
                } =>
        {
            format!("~{rest}")
        }
        _ => text.to_string(),
    }
}

/// A line of the details: its indent, its parts and their styles.
type Line = Vec<(String, Style)>;

/// The details of `layout`, `missing` marking the panes the session lacks.
fn details(
    layout: &Layout,
    missing: &[bool],
    titles: &HashMap<String, Option<String>>,
    home: Option<&Path>,
) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut flags = missing.iter();
    for workspace in &layout.workspaces {
        let mut line = vec![(workspace.name.clone(), Style::fg(AQUA).bold())];
        if let Some(cwd) = &workspace.cwd {
            line.push((format!("  {}", short(cwd, home)), Style::fg(GREY)));
        }
        lines.push(line);
        for tab in &workspace.tabs {
            lines.push(vec![(format!("  {}", tab.name), Style::fg(FG).bold())]);
            for pane in &tab.panes {
                let gone = flags.next().copied().unwrap_or(false);
                let (mark, style) = if gone {
                    ("+ ", Style::fg(YELLOW))
                } else {
                    ("  ", Style::fg(FG4))
                };
                let what = match &pane.agent {
                    Some(agent) => {
                        let mut what = agent.name.clone();
                        if let Some(id) = &agent.conversation {
                            let short_id: String = id.chars().take(8).collect();
                            match titles.get(id).cloned().flatten() {
                                Some(title) => what.push_str(&format!(" · {title} ({short_id})")),
                                None => what.push_str(&format!(" {short_id}")),
                            }
                        }
                        what
                    }
                    None => "shell".into(),
                };
                let cwd = pane
                    .cwd
                    .as_deref()
                    .map(|p| short(p, home))
                    .unwrap_or_default();
                lines.push(vec![
                    (format!("    {mark}"), style.bold()),
                    (
                        what,
                        if pane.agent.is_some() {
                            style.bold()
                        } else {
                            style
                        },
                    ),
                    (format!("  {cwd}"), Style::fg(GREY)),
                ]);
            }
        }
    }
    lines
}

/// The keys of the footer.
fn hints(can_restore: bool) -> Vec<(&'static str, &'static str)> {
    let mut hints = vec![("↑↓", "choisir"), ("PgUp PgDn", "détails")];
    if can_restore {
        hints.push(("Entrée", "restaurer"));
    }
    hints.push(("Échap", "retour"));
    hints
}

/// Draws the screen in `width` × `height` cells.
fn render(browser: &Browser, width: u16, height: u16, home: Option<&Path>) -> Drawn {
    let mut canvas = Canvas::new(width, height);
    let footer = height.checked_sub(1);
    // Title.
    if height > 0 {
        canvas.fill(0, 0, width, Style::default().on(BG1));
        canvas.text(1, 0, TITLE_ICON, Style::fg(YELLOW).on(BG1));
        let end = canvas.text(3, 0, "Récupérer une session", Style::fg(FG).on(BG1).bold());
        if let Ok(session) = &browser.session {
            let name = format!("session « {} » ", session.name);
            let x = width.saturating_sub(self::width(&name) as u16);
            if x > end + 2 {
                canvas.text(x, 0, &name, Style::fg(FG4).on(BG1));
            }
        }
    }
    let room = usize::from(width.saturating_sub(2));
    let text = |canvas: &mut Canvas, y: u16, text: &str, style: Style| {
        if Some(y) < footer {
            canvas.text(1, y, &ellipsize(text, room), style);
        }
    };
    // What the session is now.
    match (&browser.session, &browser.current) {
        (Err(e), _) => text(&mut canvas, 2, &format!("✗ {e}"), Style::fg(RED)),
        (Ok(_), Err(e)) => text(
            &mut canvas,
            2,
            &format!("Aujourd'hui : session.json {e}"),
            Style::fg(RED),
        ),
        (Ok(_), Ok(current)) => text(
            &mut canvas,
            2,
            &format!("Aujourd'hui : {}", recover::summary(current.counts())),
            Style::fg(FG4),
        ),
    }
    let mut drawn_rows = 0;
    let mut detail_top = 0;
    let mut top = browser.top;
    if browser.session.is_ok() && browser.snapshots.is_empty() {
        text(
            &mut canvas,
            LIST_TOP,
            "Aucun snapshot : Herdr n'en a pas encore écrit pour cette session.",
            Style::fg(FG4),
        );
    }
    // Under the title and the summary: the list, a rule, the details, the
    // error and the footer.
    let below = footer.unwrap_or(0).saturating_sub(LIST_TOP);
    if !browser.snapshots.is_empty() && below > 0 {
        let count = browser.snapshots.len();
        // A third of the room at most, for the details.
        let rows = count.min(usize::from(below / 3).max(1));
        top = crate::ui::scrolled(top.min(count.saturating_sub(rows)), browser.selected, rows);
        for (offset, snapshot) in browser.snapshots.iter().enumerate().skip(top).take(rows) {
            let y = LIST_TOP + (offset - top) as u16;
            let selected = offset == browser.selected;
            let bg = selected.then_some(BG2);
            let on = |style: Style| Style { bg, ..style };
            if selected {
                canvas.fill(0, y, width, on(Style::default()));
                canvas.text(0, y, "▌", on(Style::fg(YELLOW)));
            }
            let head = format!(
                "{}  {:<14}",
                recover::when(snapshot.saved),
                recover::ago(snapshot.saved, browser.now)
            );
            let mut x = canvas.text(2, y, &head, on(Style::fg(FG)));
            match &snapshot.layout {
                Ok(layout) => {
                    // What the session lacks first: what tells the snapshot
                    // to restore, even on a narrow screen.
                    let gone = browser.missing(layout).iter().filter(|&&m| m).count();
                    x += 2;
                    if gone > 0 {
                        let absent = if gone > 1 { "absents" } else { "absent" };
                        let text = format!("+{gone} {absent}");
                        x = canvas.text(x, y, &text, on(Style::fg(YELLOW).bold())) + 2;
                    }
                    canvas.text(x, y, &recover::summary(layout.counts()), on(Style::fg(FG4)));
                }
                Err(e) => {
                    canvas.text(x + 2, y, &format!("✗ {e}"), on(Style::fg(RED)));
                }
            }
        }
        drawn_rows = rows;
        // The details of the selected one.
        let rule_y = LIST_TOP + rows as u16;
        let error_y = footer.and_then(|f| f.checked_sub(1));
        let last = error_y.unwrap_or(0);
        if rule_y < last {
            canvas.text(0, rule_y, &"─".repeat(usize::from(width)), Style::fg(BG2));
            let first = rule_y + 1;
            let room = usize::from(last - first);
            if let Some(Ok(layout)) = browser.snapshots.get(browser.selected).map(|s| &s.layout) {
                let lines = details(layout, &browser.missing(layout), &browser.titles, home);
                let start = browser.detail_top.min(lines.len().saturating_sub(room));
                detail_top = start;
                for (i, line) in lines.iter().skip(start).take(room).enumerate() {
                    let mut x = 1;
                    for (part, style) in line {
                        x = canvas.text_to(
                            x,
                            first + i as u16,
                            part,
                            *style,
                            width.saturating_sub(1),
                        );
                    }
                }
                if lines.len() > start + room && room > 0 {
                    let more = format!(" … {} lignes de plus ", lines.len() - start - room);
                    let x = width.saturating_sub(self::width(&more) as u16 + 1);
                    canvas.text(x, last - 1, &more, Style::fg(GREY));
                }
            }
        }
        if let (Some(y), Some(e)) = (error_y.filter(|&y| y >= LIST_TOP), &browser.error) {
            canvas.text(1, y, &ellipsize(&format!("✗ {e}"), room), Style::fg(RED));
        }
    }
    if let Some(y) = footer.filter(|&y| y > 0) {
        canvas.fill(0, y, width, Style::default().on(BG1));
        let can_restore = browser
            .snapshots
            .get(browser.selected)
            .is_some_and(|s| s.layout.is_ok());
        let mut x = 1;
        for (key, what) in hints(can_restore) {
            let cells = (self::width(key) + 1 + self::width(what)) as u16;
            if x + cells >= width {
                break;
            }
            x = canvas.text(x, y, key, Style::fg(YELLOW).on(BG1).bold());
            x = canvas.text(x + 1, y, what, Style::fg(FG4).on(BG1)) + 3;
        }
    }
    Drawn {
        canvas,
        top,
        rows: drawn_rows,
        detail_top,
    }
}

/// The screen, until it is left.
pub fn browse() -> io::Result<Outcome> {
    let mut browser = Browser::new(herdr::current_session());
    let home = std::env::home_dir();
    let _screen = Screen::enter()?;
    let mut shown = Vec::new();
    loop {
        browser.find_titles();
        let (width, height) = terminal::size()?;
        let drawn = render(&browser, width, height, home.as_deref());
        browser.top = drawn.top;
        browser.detail_top = drawn.detail_top;
        let mut frame = Frame::new()?;
        drawn.canvas.write_to(&mut frame)?;
        frame.show_if_new(&mut shown)?;
        let count = browser.snapshots.len();
        let last = count.saturating_sub(1);
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press && is_control(key.modifiers) => {
                if key.code == KeyCode::Char('c') {
                    return Ok(Outcome::Back);
                }
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => return Ok(Outcome::Back),
                KeyCode::Up | KeyCode::Char('k') if count > 0 => {
                    browser.select((browser.selected + last) % count);
                }
                KeyCode::Down | KeyCode::Char('j') if count > 0 => {
                    browser.select((browser.selected + 1) % count);
                }
                KeyCode::Home | KeyCode::Char('g') => browser.select(0),
                KeyCode::End | KeyCode::Char('G') => browser.select(last),
                KeyCode::PageDown => browser.detail_top += 5,
                KeyCode::PageUp => browser.detail_top = browser.detail_top.saturating_sub(5),
                KeyCode::Enter => {
                    if let Some(snapshot) = browser.snapshots.get(browser.selected) {
                        let opened = match &snapshot.layout {
                            Ok(_) => recover::open_window(&snapshot.path),
                            Err(e) => Err(format!("ce snapshot ne peut pas être restauré : {e}")),
                        };
                        match opened {
                            Ok(()) => return Ok(Outcome::Restoring),
                            Err(e) => browser.error = Some(e),
                        }
                    }
                }
                _ => {}
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Moved | MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(i) = drawn.at(mouse.row, count) {
                        browser.select(i);
                    }
                }
                MouseEventKind::ScrollDown => browser.select((browser.selected + 1).min(last)),
                MouseEventKind::ScrollUp => browser.select(browser.selected.saturating_sub(1)),
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    const SNAPSHOT: &str = r#"{"version": 3, "workspaces": [{"custom_name": "app",
        "identity_cwd": "/home/u/app", "tabs": [
        {"custom_name": "agents", "layout": {"Split": {"first": {"Pane": 1}, "second": {"Pane": 2}}},
         "panes": {"1": {"cwd": "/home/u/app"},
                   "2": {"cwd": "/home/u/app/api", "agent_session":
                         {"agent": "claude", "kind": "id", "value": "c73a3ff4-0000"}}}}]}]}"#;
    const NOW: &str = r#"{"version": 3, "workspaces": [{"custom_name": "app", "tabs": [
        {"layout": {"Pane": 1}, "panes": {"1": {"cwd": "/home/u/app/"}}}]}]}"#;

    fn browser() -> Browser {
        let now = SystemTime::now();
        let parse = |text: &str| recover::layout::parse(text.as_bytes());
        let snapshot = |minutes: u64, layout| Snapshot {
            path: PathBuf::from(format!("/s/session-{minutes}.json")),
            saved: now - Duration::from_secs(minutes * 60),
            layout,
        };
        let session = Session {
            name: "work".into(),
            default: false,
            running: true,
            session_dir: "/s".into(),
            socket_path: "/s/herdr.sock".into(),
        };
        let mut browser = Browser::new(Err(String::new()));
        browser.session = Ok(session);
        browser.snapshots = vec![
            snapshot(12, parse(SNAPSHOT)),
            snapshot(130, Err("version 4 inconnue (connue : 3)".into())),
        ];
        browser.current = parse(NOW);
        browser.now = now;
        browser
            .titles
            .insert("c73a3ff4-0000".into(), Some("Réponse PING".into()));
        browser
    }

    #[test]
    fn shows_the_snapshots_and_what_the_session_lacks() {
        let browser = browser();
        let home = Path::new("/home/u");
        let text = render(&browser, 100, 24, Some(home)).canvas.to_text();
        for expected in [
            "Récupérer une session",
            "session « work »",
            "Aujourd'hui : 1 espace · 1 onglet · 1 pane · 0 agent",
            "il y a 12 min",
            "1 espace · 1 onglet · 2 panes · 1 agent",
            "+1 absent  1 espace",
            "il y a 2 h 10",
            "✗ version 4 inconnue",
            "app  ~/app",
            "  agents",
            "+ claude · Réponse PING (c73a3ff4)  ~/app/api",
            "shell  ~/app",
            "Entrée restaurer",
        ] {
            assert!(text.contains(expected), "{expected:?} missing:\n{text}");
        }
    }

    #[test]
    fn a_snapshot_it_cannot_read_is_not_offered() {
        let mut browser = browser();
        browser.select(1);
        let text = render(&browser, 100, 24, None).canvas.to_text();
        assert!(!text.contains("Entrée restaurer"), "{text}");
    }

    #[test]
    fn any_size_draws_without_panic() {
        let mut browser = browser();
        browser.error = Some("une erreur".into());
        for (w, h) in [(0, 0), (1, 1), (5, 3), (20, 6), (40, 10), (200, 60)] {
            for selected in 0..2 {
                browser.select(selected);
                browser.detail_top = 99;
                let drawn = render(&browser, w, h, None);
                browser.top = drawn.top;
            }
        }
        let mut lost = Browser::new(Err("session introuvable".into()));
        lost.now = SystemTime::now();
        let text = render(&lost, 60, 10, None).canvas.to_text();
        assert!(text.contains("✗ session introuvable"), "{text}");
    }

    #[test]
    fn only_a_drawn_row_answers_the_pointer() {
        let drawn = Drawn {
            canvas: Canvas::new(1, 1),
            top: 3,
            rows: 2,
            detail_top: 0,
        };
        assert_eq!(drawn.at(LIST_TOP, 10), Some(3));
        assert_eq!(drawn.at(LIST_TOP + 1, 10), Some(4));
        assert_eq!(drawn.at(LIST_TOP + 2, 10), None);
        assert_eq!(drawn.at(LIST_TOP - 1, 10), None);
    }

    #[test]
    fn a_folder_of_the_home_folder_starts_with_a_tilde() {
        let home = Some(Path::new("/home/u"));
        assert_eq!(short(Path::new("/home/u/app/"), home), "~/app");
        assert_eq!(short(Path::new("/home/u"), home), "~");
        assert_eq!(short(Path::new("/home/user2"), home), "/home/user2");
        // A cut inside a character is no prefix.
        assert_eq!(
            short(Path::new("/home/aé/x"), Some(Path::new("/home/ab"))),
            "/home/aé/x"
        );
        assert_eq!(short(Path::new("/srv"), None), "/srv");
    }
}
