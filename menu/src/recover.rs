//! Getting a session back as one of Herdr's snapshots saved it: when the
//! shells of the panes are killed (an update of PowerShell from the
//! Microsoft Store closes every `pwsh.exe`), Herdr takes their panes out of
//! the layout, and their agents stop with them; before that, it copies the
//! layout to `session-snapshots/`, at most once every 15 minutes.
//!
//! The settings screen lists the session's snapshots (see
//! `settings::sessions`); the one chosen is restored in a terminal window
//! of its own, outside Herdr, since stopping the server closes every pane:
//! `herdr-menu recover` asks for a yes, copies `session.json` aside, stops
//! the server, puts the snapshot in its place, and puts the command that
//! starts the session again in the clipboard. At its start, Herdr recreates
//! the panes in their folders and resumes their agents (ADR 0010).

pub mod claude;
pub mod layout;

use crate::herdr::{self, Session};
use crate::ui::plural;
use crate::window::{self, herdr_shown};
use chrono::{DateTime, Local};
use layout::{Counts, Layout};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// The snapshot to restore, handed to the window.
const SNAPSHOT_ENV: &str = "HERDR_MENU_SNAPSHOT";
/// Herdr's files in a session's folder.
const SESSION_FILE: &str = "session.json";
const SNAPSHOTS_DIR: &str = "session-snapshots";
/// How long the window waits for the server to be gone.
const STOP_WITHIN: Duration = Duration::from_secs(10);

/// A snapshot of the session's layout.
pub struct Snapshot {
    pub path: PathBuf,
    /// When Herdr wrote it.
    pub saved: SystemTime,
    /// Its layout, or why it cannot be restored.
    pub layout: Result<Layout, String>,
}

/// The snapshots of `session`, the latest first.
pub fn snapshots(session: &Session) -> Vec<Snapshot> {
    let Ok(entries) = std::fs::read_dir(session.session_dir.join(SNAPSHOTS_DIR)) else {
        return Vec::new();
    };
    let mut snapshots: Vec<Snapshot> = entries
        .flatten()
        .filter(|e| is_snapshot_name(&e.file_name()))
        .filter_map(|e| {
            let saved = e.metadata().ok()?.modified().ok()?;
            let layout = std::fs::read(e.path())
                .map_err(|e| format!("illisible : {e}"))
                .and_then(|bytes| layout::parse(&bytes));
            Some(Snapshot {
                path: e.path(),
                saved,
                layout,
            })
        })
        .collect();
    snapshots.sort_by_key(|s| std::cmp::Reverse(s.saved));
    snapshots
}

/// `session-<time>-<pid>-<n>.json`, as Herdr names its snapshots.
fn is_snapshot_name(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    name.starts_with("session-") && name.ends_with(".json")
}

/// The session's layout now, as Herdr last saved it.
pub fn current(session: &Session) -> Result<Layout, String> {
    match std::fs::read(session.session_dir.join(SESSION_FILE)) {
        Ok(bytes) => layout::parse(&bytes),
        // No file: no layout saved.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Layout::default()),
        Err(e) => Err(format!("illisible : {e}")),
    }
}

/// `11/11 15:05`: when, in local time.
pub fn when(time: SystemTime) -> String {
    DateTime::<Local>::from(time)
        .format("%d/%m %H:%M")
        .to_string()
}

/// `il y a 2 h 10`: how long ago.
pub fn ago(time: SystemTime, now: SystemTime) -> String {
    age(now.duration_since(time).unwrap_or_default())
}

fn age(elapsed: Duration) -> String {
    let minutes = elapsed.as_secs() / 60;
    match minutes {
        0 => "à l'instant".into(),
        1..60 => format!("il y a {minutes} min"),
        60..2880 => format!("il y a {} h {:02}", minutes / 60, minutes % 60),
        _ => format!("il y a {} j", minutes / 1440),
    }
}

/// `1 espace · 2 onglets · 4 panes · 3 agents`.
pub fn summary(counts: Counts) -> String {
    [
        plural(counts.workspaces, "espace"),
        plural(counts.tabs, "onglet"),
        plural(counts.panes, "pane"),
        plural(counts.agents, "agent"),
    ]
    .join(" · ")
}

/// Opens the window that restores `snapshot`, and returns at once.
pub fn open_window(snapshot: &Path) -> Result<(), String> {
    window::open(
        "recover",
        &[(SNAPSHOT_ENV, snapshot.as_os_str())],
        "voir « Récupérer une session » dans la documentation du menu",
    )
}

/// `herdr-menu recover`, in the window `open_window` opened. The window
/// closes with it, so it ends on a key press.
pub fn run() -> Result<(), String> {
    let result = recover();
    let message = match &result {
        Ok(message) => message.clone(),
        Err(e) => format!("Erreur : {e}"),
    };
    crate::ui::pause(&format!("\n{message}"));
    result.map(|_| ())
}

/// The restoring; what to tell once done.
fn recover() -> Result<String, String> {
    // Stopping the server would close this program's own pane first.
    if std::env::var_os(herdr_api::env::INSIDE).is_some() {
        return Err("à lancer hors de Herdr : la ligne « Récupérer une session… » des réglages ouvre sa fenêtre".into());
    }
    let snapshot = PathBuf::from(herdr_api::var(SNAPSHOT_ENV)?);
    let session = herdr::current_session()?;
    let dir = &session.session_dir;
    // Only a snapshot of this session, as the screen listed them.
    if !snapshot
        .parent()
        .is_some_and(|parent| herdr::same_path(parent, &dir.join(SNAPSHOTS_DIR)))
        || !snapshot.file_name().is_some_and(is_snapshot_name)
    {
        return Err(format!(
            "{} n'est pas un snapshot de la session « {} »",
            snapshot.display(),
            session.name
        ));
    }
    let bytes = std::fs::read(&snapshot)
        .map_err(|e| format!("snapshot illisible ({}) : {e}", snapshot.display()))?;
    let layout = layout::parse(&bytes).map_err(|e| format!("snapshot {e}"))?;
    let saved = std::fs::metadata(&snapshot)
        .and_then(|m| m.modified())
        .map_err(|e| e.to_string())?;
    println!(
        "Session « {} » : restaurer le snapshot {} ({}).\n\
         Il contient {}.\n",
        session.name,
        DateTime::<Local>::from(saved).format("du %d/%m à %H:%M"),
        ago(saved, SystemTime::now()),
        summary(layout.counts())
    );
    if session.running {
        println!(
            "Le serveur Herdr s'arrête : tous les panes se ferment ({}).",
            window::stopping()?
        );
    }
    println!(
        "Au prochain démarrage, Herdr recrée les panes du snapshot dans leurs \
         dossiers et relance ses agents sur leur conversation."
    );
    if !crate::ui::ask("Restaurer ? [o/N] ", false) {
        return Ok("Rien n'a été restauré.".into());
    }
    // Asked again: the server may have stopped while the question waited.
    if running(&session.name)? {
        herdr_shown(&session.stop_args())?;
        wait_until_stopped(&session)?;
    }
    let session_file = dir.join(SESSION_FILE);
    // The session as its server left it, to go back to.
    let kept = if session_file.exists() {
        let kept = dir.join(format!(
            "session-before-restore-{}.json",
            Local::now().format("%Y%m%d-%H%M%S")
        ));
        std::fs::copy(&session_file, &kept)
            .map_err(|e| format!("copie de {SESSION_FILE} impossible : {e}"))?;
        Some(kept)
    } else {
        None
    };
    herdr_api::write_atomic(&session_file, &bytes)
        .map_err(|e| format!("écriture de {} impossible : {e}", session_file.display()))?;
    let mut message = String::from("Session restaurée.");
    if let Some(kept) = kept {
        message.push_str(&format!(
            "\nL'ancienne est copiée dans {} : la remettre à la place de {SESSION_FILE}, \
             serveur arrêté, la rétablit.",
            kept.display()
        ));
    }
    message.push('\n');
    message.push_str(&window::start_from_clipboard(&session.start_command()));
    Ok(message)
}

/// Whether Herdr says the session `name` runs.
fn running(name: &str) -> Result<bool, String> {
    Ok(herdr::sessions()?
        .into_iter()
        .any(|s| s.name == name && s.running))
}

/// Waits until Herdr says `session` no longer runs: its last save of
/// `session.json` is done.
fn wait_until_stopped(session: &Session) -> Result<(), String> {
    let since = Instant::now();
    while running(&session.name)? {
        if since.elapsed() >= STOP_WITHIN {
            return Err(format!(
                "le serveur ne s'est pas arrêté : rien n'a été restauré. \
                 S'il s'arrête plus tard, relancez Herdr avec « {} ».",
                session.start_command()
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_how_long_ago() {
        let min = |m: u64| age(Duration::from_secs(m * 60 + 5));
        assert_eq!(min(0), "à l'instant");
        assert_eq!(min(12), "il y a 12 min");
        assert_eq!(min(130), "il y a 2 h 10");
        assert_eq!(min(60 * 49), "il y a 2 j");
        // A date in the future (a clock set back) is now.
        let now = SystemTime::now();
        assert_eq!(ago(now + Duration::from_secs(600), now), "à l'instant");
    }

    #[test]
    fn sums_a_layout_up() {
        let counts = Counts {
            workspaces: 1,
            tabs: 2,
            panes: 4,
            agents: 0,
        };
        assert_eq!(summary(counts), "1 espace · 2 onglets · 4 panes · 0 agent");
    }

    #[test]
    fn lists_the_snapshots_latest_first() {
        let dir = herdr_api::TempDir::new("menu-snapshots").unwrap();
        let snapshots_dir = dir.join(SNAPSHOTS_DIR);
        std::fs::create_dir_all(&snapshots_dir).unwrap();
        let old = snapshots_dir.join("session-1-1-0.json");
        std::fs::write(&old, r#"{"version": 3, "workspaces": []}"#).unwrap();
        std::fs::write(snapshots_dir.join("session-2-1-0.json"), "{").unwrap();
        std::fs::write(snapshots_dir.join("notes.txt"), "").unwrap();
        let file = std::fs::File::options().write(true).open(&old).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(3600))
            .unwrap();
        let session = Session {
            name: "work".into(),
            default: false,
            running: true,
            session_dir: dir.to_path_buf(),
            socket_path: dir.join("herdr.sock"),
        };
        let found = snapshots(&session);
        assert_eq!(found.len(), 2);
        assert!(found[0].layout.is_err());
        assert_eq!(found[1].path, old);
        assert_eq!(current(&session), Ok(Layout::default()));
    }
}
