//! The confirmation popup: shows the real path of a file outside
//! `open_types` that a click would open with its program (`run_programs` and
//! `confirm_programs` on), and its type, and opens it only on Enter. The path
//! shown is the canonical one, not the link's text: what will run. Checked
//! again on Enter, with the settings of that moment.
//!
//! A shortcut's path is not what it launches: for any shortcut (`.lnk`,
//! `.url`…), a red line says its target is not shown.
//!
//! Never confirmed by accident: only an Enter typed on its own, without Ctrl,
//! Alt or Shift, confirms. Any other key cancels at once: a modified Enter,
//! Esc, or text typed for another pane. An Enter is heard only once none has
//! come for longer than the keyboard waits before repeating a key held down (an
//! Enter already on its way, typed just after the click, or held down),
//! confirms when it is released, pressed once and not repeated, an Enter
//! followed at once by more input is a paste and cancels, and a path the
//! popup cannot show whole, or a shortcut's line, cannot be confirmed.

use crate::{open, target};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{cursor, execute, queue};
use std::io::{self, Write};
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthChar;

/// An Enter is heard only once no Enter has come for this long since the
/// popup showed: each one in that time is ignored and starts the wait again.
/// Any other key cancels, then too.
///
/// Longer than the longest wait the keyboard's repeat delay lets a key held
/// down make before its first repeat (250 ms to 1 s, 500 ms by default; a
/// longer delay set by FilterKeys, an accessibility setting, is not
/// covered): an Enter held down since the click, pressed before the popup
/// or in that time, is never heard. Confirming on the Enter's release, once
/// pressed and not repeated, does not do that alone: crossterm reports the
/// releases Windows' console gives it, but the popup's console is Herdr's
/// pseudoconsole, which may make up a release right after each press,
/// repeats included, so that a key held down looks like keys typed again
/// and again.
const DEAF_FOR: Duration = Duration::from_millis(1100);
/// Input this close after an Enter makes it part of a paste.
const PASTE_WITHIN: Duration = Duration::from_millis(30);

const RED: Color = Color::Rgb {
    r: 0xfb,
    g: 0x49,
    b: 0x34,
};
const YELLOW: Color = Color::Rgb {
    r: 0xfa,
    g: 0xbd,
    b: 0x2f,
};
const GREY: Color = Color::Rgb {
    r: 0x92,
    g: 0x83,
    b: 0x74,
};

const TITLE: &str = "Ouvrir ce fichier avec son programme ?";
const WHY: &str = "Son type n'est pas dans la liste des types ouverts : il peut lancer un programme ou un script.";
const KEYS: &str = "Entrée ouvrir · Échap annuler";
const TOO_SMALL: &str =
    "Fenêtre trop petite pour tout montrer : rien n'est ouvert. Une touche pour fermer.";
/// Said of a shortcut whose target the popup does not show.
const HIDDEN: &str = "Raccourci : ouvre une autre cible, non affichée";

/// The types of a shortcut: a file that, opened, launches or shows another
/// target, which its own path does not tell. None is read: the popup never
/// shows the target.
/// - `lnk`: a shell link, its target and arguments in binary fields;
/// - `pif`: a link to an MS-DOS program, which Windows runs;
/// - `url`, `website`: an Internet shortcut, a pinned site, an address
///   (`file:` included) in an INI text, which Windows reads in a way that is
///   not documented, passing over some characters a text would show;
/// - `application`, `appref-ms`: a ClickOnce deployment manifest, a
///   ClickOnce application reference, an application installed from an
///   address and run;
/// - `rdp`: a Remote Desktop connection, to the computer it names, which it
///   may give the local drives and the clipboard;
/// - `scf`: a Shell Command File, a command of the File Explorer;
/// - `settingcontent-ms`: a Settings shortcut, whose `DeepLink` is any
///   command;
/// - `shb`, `xnk`: other types Windows opens as a shortcut to another
///   target;
/// - `library-ms`, `searchconnector-ms`, `search-ms`: a library, a search
///   connector, a saved search, opened in the File Explorer on the folders
///   they name, on a network share or a WebDAV server as well;
/// - `wsh`, `jnlp`, `ica`: files whose program opens another target they
///   name, a script, an application, a remote session.
const SHORTCUTS: &[&str] = &[
    "application",
    "appref-ms",
    "ica",
    "jnlp",
    "library-ms",
    "lnk",
    "pif",
    "rdp",
    "scf",
    "search-ms",
    "searchconnector-ms",
    "settingcontent-ms",
    "shb",
    "url",
    "website",
    "wsh",
    "xnk",
];

/// `herdr-links confirm`, in its popup.
pub fn run() -> Result<(), String> {
    let path = herdr_api::var(open::PATH_ENV)?;
    let confirmed = ask(&path).map_err(|e| e.to_string())?;
    if !confirmed {
        return Ok(());
    }
    if let Err(e) = launch(&path) {
        // The user just asked for it: the popup says why, until a key.
        let _ = tell(&e);
        return Err(e);
    }
    Ok(())
}

/// Opens `path` once checked again: still the same file, and `run_programs`
/// still on.
fn launch(path: &str) -> Result<(), String> {
    if !crate::settings().0.run_programs {
        return Err("run_programs est désactivé : rien n'est lancé".into());
    }
    let found = target::resolve(path).map_err(|e| format!("{path} : {}", e.reason()))?;
    if found.path != path || found.kind != target::Kind::File {
        return Err(format!("{path} a changé : rien n'est lancé"));
    }
    open::explorer(&open::open_arg(&found.path))
}

/// Whether the file named `name` is a shortcut: its type one of `SHORTCUTS`.
fn is_shortcut(name: &str) -> bool {
    herdr_api::links::file_type(name).is_some_and(|t| SHORTCUTS.contains(&t.as_str()))
}

/// `text` cut in lines of at most `width` terminal cells, a wide character
/// never split.
fn wrapped(text: &str, width: usize) -> Vec<String> {
    let width = width.max(2);
    let mut lines = Vec::new();
    let (mut line, mut used) = (String::new(), 0);
    for c in text.chars() {
        let cells = c.width().unwrap_or(0);
        if used + cells > width && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
            used = 0;
        }
        line.push(c);
        used += cells;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// What a line of the popup is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Title,
    Text,
    Type,
    Path,
    /// A shortcut's target not shown.
    Hidden,
    Keys,
}

/// The popup's lines, from its size: everything when it fits, else the
/// title, the type, the whole path, a shortcut's line and the keys;
/// `confirmable` false when even that does not fit, the lines then saying
/// so.
#[derive(Debug, PartialEq)]
struct Layout {
    lines: Vec<(Kind, String)>,
    confirmable: bool,
}

fn layout(path: &str, width: u16, height: u16) -> Layout {
    let width = usize::from(width.saturating_sub(2));
    let height = usize::from(height);
    let part = |kind: Kind, text: &str| -> Vec<(Kind, String)> {
        wrapped(text, width)
            .into_iter()
            .map(|l| (kind, l))
            .collect()
    };
    let blank = || vec![(Kind::Text, String::new())];
    let kind = part(Kind::Type, &type_line(path));
    let hidden = if is_shortcut(target::name_of(path)) {
        part(Kind::Hidden, HIDDEN)
    } else {
        Vec::new()
    };
    let full = [
        part(Kind::Title, TITLE),
        part(Kind::Text, WHY),
        blank(),
        kind.clone(),
        part(Kind::Path, path),
        hidden.clone(),
        blank(),
        part(Kind::Keys, KEYS),
    ]
    .concat();
    if width >= 10 && full.len() <= height {
        return Layout {
            lines: full,
            confirmable: true,
        };
    }
    let short = [
        part(Kind::Title, TITLE),
        kind,
        part(Kind::Path, path),
        hidden,
        part(Kind::Keys, KEYS),
    ]
    .concat();
    if width >= 10 && short.len() <= height {
        return Layout {
            lines: short,
            confirmable: true,
        };
    }
    let mut lines = part(Kind::Title, TOO_SMALL);
    lines.truncate(height.max(1));
    Layout {
        lines,
        confirmable: false,
    }
}

/// The type of the file at `path`, said: what the program that opens it
/// is chosen by, at the start of its line: `Type .exe`, `Sans type`.
fn type_line(path: &str) -> String {
    let label = herdr_api::links::type_label(target::name_of(path));
    let mut chars = label.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Raw mode for the popup's lifetime.
struct Raw;

impl Raw {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), cursor::Hide)?;
        Ok(Self)
    }
}

impl Drop for Raw {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), cursor::Show);
        let _ = terminal::disable_raw_mode();
    }
}

fn draw(layout: &Layout) -> io::Result<()> {
    let mut out = io::stdout();
    queue!(out, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
    for (row, (kind, text)) in layout.lines.iter().enumerate() {
        let (color, bold) = match kind {
            Kind::Title => (Some(RED), true),
            Kind::Text => (None, false),
            Kind::Type => (Some(YELLOW), true),
            Kind::Path => (Some(YELLOW), false),
            Kind::Hidden => (Some(RED), false),
            Kind::Keys => (Some(GREY), false),
        };
        queue!(
            out,
            cursor::MoveTo(1, u16::try_from(row).unwrap_or(u16::MAX))
        )?;
        if let Some(color) = color {
            queue!(out, SetForegroundColor(color))?;
        }
        if bold {
            queue!(out, SetAttribute(Attribute::Bold))?;
        }
        queue!(out, Print(text), SetAttribute(Attribute::Reset), ResetColor)?;
    }
    out.flush()
}

/// What the popup does after a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Heard {
    Nothing,
    Confirm,
    Cancel,
}

/// When the popup hears an Enter, and which one confirms. Any key other than
/// a plain Enter (an Enter with Ctrl, Alt or Shift included), pressed or
/// repeated, cancels at once, whenever it comes: Esc, or text typed for
/// another pane the popup took the focus from. An Enter is
/// heard once no Enter has come for `DEAF_FOR`, the popup's opening counting
/// as one, from an Enter pressed then; it confirms on its release, when it
/// was pressed once, without any repeat.
struct Ears {
    last_input: Instant,
    listening: bool,
    /// The presses of the Enter down now, repeats included, once heard.
    enter_presses: u32,
}

impl Ears {
    fn new(now: Instant) -> Self {
        Self {
            last_input: now,
            listening: false,
            enter_presses: 0,
        }
    }

    /// What `key`, with `modifiers` held, received at `now` does. An Enter
    /// with Ctrl, Alt or Shift (or any other modifier) is another key. A key
    /// held down, pressed again by Windows, comes as a press on Windows, as a
    /// repeat where the terminal says so.
    fn hear(
        &mut self,
        now: Instant,
        key: KeyCode,
        modifiers: KeyModifiers,
        kind: KeyEventKind,
    ) -> Heard {
        if key != KeyCode::Enter || !modifiers.is_empty() {
            // The release of a key pressed before the popup: nothing to hear.
            return match kind {
                KeyEventKind::Release => Heard::Nothing,
                KeyEventKind::Press | KeyEventKind::Repeat => Heard::Cancel,
            };
        }
        if !self.listening {
            let quiet = now.saturating_duration_since(self.last_input) >= DEAF_FOR;
            match kind {
                KeyEventKind::Press if quiet => self.listening = true,
                // The release of a key pressed before: nothing to hear.
                KeyEventKind::Release if quiet => return Heard::Nothing,
                // Too soon, or an Enter held down: the wait starts again.
                _ => {
                    self.last_input = now;
                    return Heard::Nothing;
                }
            }
        }
        match kind {
            KeyEventKind::Press | KeyEventKind::Repeat => {
                self.enter_presses += 1;
                Heard::Nothing
            }
            KeyEventKind::Release => match self.enter_presses {
                0 => Heard::Nothing,
                1 => Heard::Confirm,
                // Held down: not a key typed on purpose.
                _ => Heard::Cancel,
            },
        }
    }
}

/// Shows the question; `true` on an Enter typed on its own, after a moment
/// without any key, with the whole path, and a shortcut's line, on screen.
fn ask(path: &str) -> io::Result<bool> {
    let _raw = Raw::enter()?;
    let (width, height) = terminal::size()?;
    let mut shown = layout(path, width, height);
    draw(&shown)?;
    let mut ears = Ears::new(Instant::now());
    loop {
        match event::read()? {
            Event::Resize(width, height) => {
                shown = layout(path, width, height);
                draw(&shown)?;
            }
            Event::Key(key) => {
                match ears.hear(Instant::now(), key.code, key.modifiers, key.kind) {
                    Heard::Nothing => {}
                    Heard::Cancel => return Ok(false),
                    // More input right behind it: a paste, not a key.
                    Heard::Confirm => {
                        return Ok(shown.confirmable && !more_input_within(PASTE_WITHIN)?);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Whether another key or a paste comes within `window`; a key's release
/// (which Windows reports after each press), a repeat reported as such, a
/// resize or a change of focus is not one. On Windows, crossterm reports a
/// repeat as a press: a key held down there counts as more input.
fn more_input_within(window: Duration) -> io::Result<bool> {
    let deadline = Instant::now() + window;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || !event::poll(left)? {
            return Ok(false);
        }
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Press => {}
            Event::Resize(..) | Event::FocusGained | Event::FocusLost => {}
            _ => return Ok(true),
        }
    }
}

/// Shows why the file was not opened, until a key the question's `Ears`
/// would act on, confirming or cancelling: any key but a plain Enter at
/// once; an Enter only once none has come for `DEAF_FOR`, on its release; a
/// release alone never. The Enter that confirmed, held or typed again, does
/// not close it unread.
fn tell(message: &str) -> io::Result<()> {
    let _raw = Raw::enter()?;
    let (width, height) = terminal::size()?;
    let mut lines: Vec<(Kind, String)> = wrapped(message, usize::from(width.saturating_sub(2)))
        .into_iter()
        .map(|l| (Kind::Title, l))
        .collect();
    lines.push((Kind::Keys, "Une touche pour fermer".into()));
    lines.truncate(usize::from(height.max(1)));
    draw(&Layout {
        lines,
        confirmable: false,
    })?;
    let mut ears = Ears::new(Instant::now());
    loop {
        if let Event::Key(key) = event::read()?
            && ears.hear(Instant::now(), key.code, key.modifiers, key.kind) != Heard::Nothing
        {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use KeyEventKind::{Press, Release, Repeat};

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    const ENTER: KeyCode = KeyCode::Enter;
    const OTHER: KeyCode = KeyCode::Char('a');

    /// What the popup makes of keys, each `(ms after it opened, the key,
    /// kind)`; the first verdict other than `Nothing`.
    fn heard(keys: &[(u64, KeyCode, KeyEventKind)]) -> Heard {
        let open = Instant::now();
        let mut ears = Ears::new(open);
        keys.iter()
            .map(|&(at, key, kind)| ears.hear(open + ms(at), key, KeyModifiers::NONE, kind))
            .find(|&h| h != Heard::Nothing)
            .unwrap_or(Heard::Nothing)
    }

    /// An Enter pressed `at` ms after the popup opened and held down until
    /// `up`: Windows presses it again after `delay`, then every `every`.
    /// `each` when every press comes with a release of its own (a console
    /// fed text, as a pseudoconsole may be), else one release when it goes
    /// up.
    fn held(
        at: u64,
        delay: u64,
        every: u64,
        up: u64,
        each: bool,
    ) -> Vec<(u64, KeyCode, KeyEventKind)> {
        let mut presses = vec![at];
        let mut next = at + delay;
        while next < up {
            presses.push(next);
            next += every;
        }
        let mut keys = Vec::new();
        for press in presses {
            keys.push((press, ENTER, Press));
            if each {
                keys.push((press + 1, ENTER, Release));
            }
        }
        if !each {
            keys.push((up, ENTER, Release));
        }
        keys
    }

    /// The keyboard's shortest, default and longest repeat delays, and its
    /// fastest and slowest repeat rates.
    const DELAYS: [u64; 3] = [250, 500, 1000];
    const EVERY: [u64; 2] = [33, 400];

    #[test]
    fn an_enter_held_down_from_the_click_never_confirms() {
        for delay in DELAYS {
            for every in EVERY {
                for at in [0, 50, 300, 1000] {
                    // Let go just before its first repeat, or seconds later.
                    for up in [at + delay - 1, 5000] {
                        for each in [false, true] {
                            let keys = held(at, delay, every, up, each);
                            assert_ne!(
                                heard(&keys),
                                Heard::Confirm,
                                "pressed at {at} ms, delay {delay}, every {every}, up at {up}, each {each}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn an_enter_held_since_before_the_popup_never_confirms() {
        // Its press came before the popup: only its repeats reach it, the
        // first one at most a whole repeat delay later.
        for delay in DELAYS {
            for every in EVERY {
                for first in [1, delay / 2, delay] {
                    for each in [false, true] {
                        let mut keys = held(first, every, every, 5000, each);
                        // The first press seen is a repeat.
                        keys[0].2 = Repeat;
                        assert_ne!(
                            heard(&keys),
                            Heard::Confirm,
                            "first at {first} ms, delay {delay}, every {every}, each {each}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn keys_are_heard_only_after_a_quiet_moment() {
        let wait = DEAF_FOR.as_millis() as u64;
        // That key started the wait again: from it, not from the opening.
        let keys = [
            (100, ENTER, Press),
            (100 + wait - 1, ENTER, Release),
            (100 + 2 * wait - 2, ENTER, Press),
            (100 + 2 * wait, ENTER, Release),
        ];
        assert_eq!(heard(&keys), Heard::Nothing);
        let keys = [
            (100, ENTER, Press),
            (100 + wait, ENTER, Press),
            (100 + wait + 80, ENTER, Release),
        ];
        assert_eq!(heard(&keys), Heard::Confirm);
    }

    /// Ctrl, Alt or Shift held with an Enter: another key, which cancels,
    /// even after the quiet moment; its release alone does nothing.
    #[test]
    fn a_modified_enter_cancels() {
        let wait = DEAF_FOR.as_millis() as u64;
        for modifiers in [
            KeyModifiers::CONTROL,
            KeyModifiers::ALT,
            KeyModifiers::SHIFT,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ] {
            let open = Instant::now();
            let mut ears = Ears::new(open);
            let at = open + ms(wait + 100);
            assert_eq!(
                ears.hear(at, ENTER, modifiers, Press),
                Heard::Cancel,
                "{modifiers:?}"
            );
            let mut ears = Ears::new(open);
            assert_eq!(
                ears.hear(at, ENTER, modifiers, Release),
                Heard::Nothing,
                "{modifiers:?}"
            );
            // A plain Enter pressed, then released with a modifier down: not
            // the Enter typed on its own that confirms.
            let mut ears = Ears::new(open);
            let none = KeyModifiers::NONE;
            assert_eq!(ears.hear(at, ENTER, none, Press), Heard::Nothing);
            let up = at + ms(80);
            assert_ne!(
                ears.hear(up, ENTER, modifiers, Release),
                Heard::Confirm,
                "{modifiers:?}"
            );
        }
    }

    #[test]
    fn an_enter_typed_after_a_quiet_moment_confirms_on_its_release() {
        let wait = DEAF_FOR.as_millis() as u64;
        let pressed = [(wait, ENTER, Press)];
        assert_eq!(heard(&pressed), Heard::Nothing);
        for up in [wait + 1, wait + 120] {
            let keys = [(wait, ENTER, Press), (up, ENTER, Release)];
            assert_eq!(heard(&keys), Heard::Confirm, "up at {up} ms");
        }
        // Held until Windows repeats it: not typed on purpose.
        let keys = held(wait + 500, 500, 33, wait + 2000, false);
        assert_eq!(heard(&keys), Heard::Cancel);
    }

    #[test]
    fn another_key_cancels_and_a_stray_release_is_not_heard() {
        let wait = DEAF_FOR.as_millis() as u64;
        let esc = [(wait, KeyCode::Esc, Press)];
        assert_eq!(heard(&esc), Heard::Cancel);
        let other = [(wait, OTHER, Press)];
        assert_eq!(heard(&other), Heard::Cancel);
        // A key pressed just after the click, released after the wait.
        let keys = [
            (50, ENTER, Press),
            (wait + 200, ENTER, Release),
            (wait + 201, OTHER, Release),
        ];
        assert_eq!(heard(&keys), Heard::Nothing);
        // A key pressed while Enter is down.
        let keys = [
            (wait, ENTER, Press),
            (wait + 20, OTHER, Press),
            (wait + 40, ENTER, Release),
        ];
        assert_eq!(heard(&keys), Heard::Cancel);
    }

    /// Esc, or any key but Enter, cancels at once, even while an Enter
    /// would not be heard yet, pressed or held down.
    #[test]
    fn esc_or_another_key_cancels_even_before_an_enter_is_heard() {
        let wait = DEAF_FOR.as_millis() as u64;
        for at in [0, 1, 300, wait - 1] {
            for key in [KeyCode::Esc, OTHER, KeyCode::Char(' '), KeyCode::Tab] {
                for kind in [Press, Repeat] {
                    let keys = [(at, key, kind)];
                    assert_eq!(heard(&keys), Heard::Cancel, "{key:?} at {at} ms");
                }
            }
        }
        // After an Enter that started the wait again.
        let keys = [
            (100, ENTER, Press),
            (150, ENTER, Release),
            (900, KeyCode::Esc, Press),
        ];
        assert_eq!(heard(&keys), Heard::Cancel);
        // The release of a key pressed before the popup cancels nothing.
        let keys = [(10, KeyCode::Esc, Release)];
        assert_eq!(heard(&keys), Heard::Nothing);
    }

    /// A prompt typed for another pane when the popup took the focus, then
    /// sent with Enter after a pause: the letters cancel, the Enter
    /// confirms nothing.
    #[test]
    fn a_prompt_typed_for_another_pane_cancels() {
        let mut keys = Vec::new();
        let mut at = 0;
        for c in "fix this".chars() {
            keys.push((at, KeyCode::Char(c), Press));
            keys.push((at + 40, KeyCode::Char(c), Release));
            at += 90;
        }
        at += 1200;
        keys.push((at, ENTER, Press));
        keys.push((at + 80, ENTER, Release));
        assert_eq!(heard(&keys), Heard::Cancel);
    }

    /// Only an Enter typed on its own, after the wait, confirms; an Enter
    /// in the wait, its release too, starts it again.
    #[test]
    fn only_a_lone_enter_after_the_wait_confirms() {
        let wait = DEAF_FOR.as_millis() as u64;
        let keys = [
            (200, ENTER, Press),
            (260, ENTER, Release),
            (260 + wait, ENTER, Press),
            (320 + wait, ENTER, Release),
        ];
        assert_eq!(heard(&keys), Heard::Confirm);
        let keys = [
            (200, ENTER, Press),
            (260, ENTER, Release),
            (259 + wait, ENTER, Press),
            (320 + wait, ENTER, Release),
        ];
        assert_eq!(heard(&keys), Heard::Nothing);
    }

    /// Whether keys, as `heard` takes them, close `tell`'s message.
    fn closed(keys: &[(u64, KeyCode, KeyEventKind)]) -> bool {
        heard(keys) != Heard::Nothing
    }

    #[test]
    fn the_message_closes_on_the_keys_the_question_acts_on() {
        let wait = DEAF_FOR.as_millis() as u64;
        // The Enter that confirmed, held down or typed again at once.
        for delay in DELAYS {
            for every in EVERY {
                for each in [false, true] {
                    let keys = held(0, delay, every, 5000, each);
                    assert!(!closed(&keys), "delay {delay}, every {every}, each {each}");
                }
            }
        }
        let again = [(100, ENTER, Press), (160, ENTER, Release)];
        assert!(!closed(&again));
        // Any other key closes it at once.
        assert!(closed(&[(10, KeyCode::Esc, Press)]));
        assert!(closed(&[(10, OTHER, Press)]));
        // An Enter after the wait, on its release.
        assert!(!closed(&[(wait, ENTER, Press)]));
        let enter = [(wait, ENTER, Press), (wait + 60, ENTER, Release)];
        assert!(closed(&enter));
        // A release after the wait, of a key pressed before the message,
        // neither closes it nor starts the wait again.
        let keys = [
            (wait + 100, OTHER, Release),
            (wait + 150, ENTER, Release),
            (wait + 200, ENTER, Press),
            (wait + 260, ENTER, Release),
        ];
        assert!(!closed(&keys[..2]));
        assert!(closed(&keys));
    }

    #[test]
    fn the_type_is_shown_with_the_path() {
        assert_eq!(type_line(r"C:\outils.v2\setup.EXE"), "Type .exe");
        assert_eq!(type_line(r"C:\outils.v2\Makefile"), "Sans type");
        let shown = layout(r"C:\a\tool.ps1", 80, 16);
        assert!(shown.lines.contains(&(Kind::Type, "Type .ps1".into())));
        // Kept in a popup too short for the explanation.
        let path = format!(r"C:\{}\a.exe", "x".repeat(150));
        let short = layout(&path, 60, 7);
        assert!(short.confirmable);
        assert!(short.lines.contains(&(Kind::Type, "Type .exe".into())));
    }

    #[test]
    fn lines_are_cut_by_display_width() {
        assert_eq!(wrapped("abcdef", 4), ["abcd", "ef"]);
        // 日 takes two cells: never split, never past the width.
        assert_eq!(wrapped("a日日", 4), ["a日", "日"]);
        assert_eq!(wrapped("é日x", 3), ["é日", "x"]);
        assert!(wrapped("", 4).is_empty());
        for line in wrapped(&"日".repeat(9), 5) {
            let cells: usize = line.chars().map(|c| c.width().unwrap_or(0)).sum();
            assert!(cells <= 5, "{line}");
        }
    }

    /// The path drawn, its lines joined back.
    fn path_shown(layout: &Layout) -> String {
        layout
            .lines
            .iter()
            .filter(|(kind, _)| *kind == Kind::Path)
            .map(|(_, text)| text.as_str())
            .collect()
    }

    #[test]
    fn a_long_path_and_the_title_are_shown_whole() {
        let path = format!(r"C:\{}\setup.exe", "dossier très long\\".repeat(12));
        let shown = layout(&path, 80, 16);
        assert!(shown.confirmable);
        assert_eq!(path_shown(&shown), path);
        let title: String = shown
            .lines
            .iter()
            .filter(|(kind, _)| *kind == Kind::Title)
            .map(|(_, t)| t.as_str())
            .collect();
        assert_eq!(title, TITLE);
        assert!(shown.lines.len() <= 16);
    }

    #[test]
    fn a_short_popup_keeps_the_path_and_drops_the_rest() {
        let path = format!(r"C:\{}\a.exe", "x".repeat(150));
        let shown = layout(&path, 60, 6);
        assert!(shown.confirmable);
        assert_eq!(path_shown(&shown), path);
        assert!(!shown.lines.iter().any(|(_, t)| t.contains("Son type")));
    }

    #[test]
    fn a_path_that_does_not_fit_cannot_be_confirmed() {
        let path = format!(r"C:\{}\a.exe", "x".repeat(400));
        let shown = layout(&path, 40, 8);
        assert!(!shown.confirmable);
        assert!(shown.lines.len() <= 8);
        assert!(!layout(r"C:\a.exe", 8, 20).confirmable);
        assert!(!layout(r"C:\a.exe", 80, 0).confirmable);
    }

    /// The lines of `kind`, joined back.
    fn shown_as(layout: &Layout, kind: Kind) -> String {
        layout
            .lines
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, text)| text.as_str())
            .collect()
    }

    #[test]
    fn a_shortcut_is_told_by_its_type() {
        assert!(!is_shortcut("setup.exe"));
        assert!(!is_shortcut("Makefile"));
        for kind in SHORTCUTS {
            assert!(is_shortcut(&format!("a.{kind}")), "{kind}");
        }
        for name in [
            "Outil.LNK",
            "site.URL",
            "app.Application",
            "poste.RDP",
            "script.WSH",
            "app.jnlp",
            "session.ICA",
        ] {
            assert!(is_shortcut(name), "{name}");
        }
        // Only the last extension counts.
        assert!(!is_shortcut("a.lnk.txt"));
    }

    #[test]
    fn a_shortcut_says_its_target_is_not_shown() {
        for path in [r"C:\a\outil.lnk", r"C:\a\site.url", r"C:\a\poste.rdp"] {
            let shown = layout(path, 80, 16);
            assert!(shown.confirmable, "{path}");
            assert_eq!(shown_as(&shown, Kind::Hidden), HIDDEN, "{path}");
        }
        let shown = layout(r"C:\a\setup.exe", 80, 16);
        assert_eq!(shown_as(&shown, Kind::Hidden), "");
    }

    #[test]
    fn a_shortcut_whose_target_line_does_not_fit_cannot_be_confirmed() {
        // The path, the title, the type and the keys fit in 6 lines; the
        // shortcut's line makes 7.
        let dir = "x".repeat(150);
        assert!(layout(&format!(r"C:\{dir}\a.exe"), 60, 6).confirmable);
        let path = format!(r"C:\{dir}\a.lnk");
        let shown = layout(&path, 60, 6);
        assert!(!shown.confirmable);
        assert_eq!(shown_as(&shown, Kind::Hidden), "");
        let shown = layout(&path, 60, 7);
        assert!(shown.confirmable);
        assert_eq!(shown_as(&shown, Kind::Hidden), HIDDEN);
    }
}
