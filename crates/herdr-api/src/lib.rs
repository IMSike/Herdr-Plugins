//! What the plugins of this repository share: requests to the Herdr API
//! socket, and a few helpers.

mod format;
pub mod sidebar;
mod socket;

pub use format::{MemoryUnit, NumberFormat};
pub use socket::{Error, request, request_field, set_deadline};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// What `pane.process_info` tells of a pane's processes.
#[derive(Debug, Default, Deserialize)]
pub struct ProcessInfo {
    /// The pane's shell.
    #[serde(default)]
    pub shell_pid: Option<u32>,
    /// What runs in the foreground of the pane; `None` when Herdr does not say.
    #[serde(default)]
    pub foreground_processes: Option<Vec<Process>>,
}

/// A process of a pane.
#[derive(Debug, Deserialize)]
pub struct Process {
    #[serde(default)]
    pub name: String,
}

/// Asks Herdr about the processes of a pane.
pub fn process_info(pane_id: &str) -> Result<ProcessInfo, Error> {
    request_field(
        "pane.process_info",
        json!({ "pane_id": pane_id }),
        "process_info",
    )
}

/// Environment variables Herdr gives a plugin's commands.
pub mod env {
    pub const SOCKET_PATH: &str = "HERDR_SOCKET_PATH";
    pub const PLUGIN_ID: &str = "HERDR_PLUGIN_ID";
    pub const STATE_DIR: &str = "HERDR_PLUGIN_STATE_DIR";
    pub const CONFIG_DIR: &str = "HERDR_PLUGIN_CONFIG_DIR";
    pub const CONTEXT_JSON: &str = "HERDR_PLUGIN_CONTEXT_JSON";
    pub const EVENT_JSON: &str = "HERDR_PLUGIN_EVENT_JSON";
    /// The pane a plugin pane's program runs in.
    pub const PANE_ID: &str = "HERDR_PANE_ID";
    /// The workspace and the tab of a pane, and the folder it started in.
    pub const WORKSPACE_ID: &str = "HERDR_WORKSPACE_ID";
    pub const TAB_ID: &str = "HERDR_TAB_ID";
    pub const STARTUP_CWD: &str = "HERDR_STARTUP_CWD";
    /// The `herdr` binary that runs this session, in Herdr's panes.
    pub const BIN_PATH: &str = "HERDR_BIN_PATH";
    /// Herdr's `config.toml`, when set.
    pub const CONFIG_PATH: &str = "HERDR_CONFIG_PATH";
    /// Set in Herdr's panes; `herdr update` refuses to run where it is.
    pub const INSIDE: &str = "HERDR_ENV";
}

/// An environment variable Herdr or the plugin sets: its absence is an error.
pub fn var(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| absent(name))
}

/// The plugin's state folder.
pub fn state_dir() -> Result<PathBuf, String> {
    std::env::var_os(env::STATE_DIR)
        .map(PathBuf::from)
        .ok_or_else(|| absent(env::STATE_DIR))
}

/// The plugin's config folder.
pub fn config_dir() -> Result<PathBuf, String> {
    std::env::var_os(env::CONFIG_DIR)
        .map(PathBuf::from)
        .ok_or_else(|| absent(env::CONFIG_DIR))
}

fn absent(name: &str) -> String {
    format!("{name} absent")
}

/// Milliseconds since the Unix epoch.
pub fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Whether the file was last written more than `age` ago. A date in the
/// future (written before the clock was set back) counts as old: otherwise
/// the file would stay young until the clock caught up.
fn is_older_than(metadata: &std::fs::Metadata, age: Duration) -> bool {
    metadata
        .modified()
        .is_ok_and(|modified| modified.elapsed().ok().is_none_or(|elapsed| elapsed > age))
}

/// Removes the files of `dir` that `matches` and that were last written
/// more than `age` ago: what a process killed midway left behind.
pub fn remove_stale(dir: &Path, age: Duration, matches: impl Fn(&str) -> bool) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let stale = entry.metadata().is_ok_and(|m| is_older_than(&m, age));
        if stale && matches(&name.to_string_lossy()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Writes `bytes` to `path` through a file of this process, renamed over it:
/// a reader sees the old content or the new one, never half of it, and two
/// processes writing at once do not mix their content. A link (a settings
/// file deployed by stow or chezmoi) stays a link: the file it points to is
/// written, keeping its permissions.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let linked = std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink());
    // A link to nothing is replaced, as a missing file would be created.
    let target = linked
        .then(|| std::fs::canonicalize(path).ok())
        .flatten()
        .unwrap_or_else(|| path.to_path_buf());
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.partial", std::process::id()));
    let partial = target.with_file_name(name);
    std::fs::write(&partial, bytes)?;
    // On Windows, the only permission is read-only, which would keep the
    // partial file from being removed.
    let kept = if cfg!(unix) {
        std::fs::metadata(&target).map_or(Ok(()), |m| {
            std::fs::set_permissions(&partial, m.permissions())
        })
    } else {
        Ok(())
    };
    kept.and_then(|()| std::fs::rename(&partial, &target))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&partial);
        })
}

/// The name a settings file gives `value`, a choice among names (`"M"`,
/// `"double"`): the one serde writes, so that the two never differ.
pub fn name_of<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        _ => String::new(),
    }
}

/// A JSON file's content; `None` when it is missing or unreadable.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Writes `value` as JSON to `path`, all at once (see `write_atomic`).
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    write_atomic(path, &bytes)
}

/// A lock held by one process at a time, through the system: released when
/// dropped, or when its process ends, killed midway included. Its file stays:
/// removed, it could be locked by one process while another one creates and
/// locks a new one.
#[derive(Debug)]
pub struct Lock {
    _file: std::fs::File,
}

impl Lock {
    /// Takes the lock at `path`; `None` while another process holds it.
    pub fn take(path: &Path) -> io::Result<Option<Self>> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }

    /// `take`, for a lock the caller can do without when there is none to
    /// take (no state folder, one that cannot be written).
    pub fn try_take(path: &Path) -> Taken {
        match Self::take(path) {
            Ok(Some(lock)) => Taken::Held(lock),
            Ok(None) => Taken::Busy,
            Err(_) => Taken::Unavailable,
        }
    }
}

/// What trying a lock gave.
#[derive(Debug)]
pub enum Taken {
    /// Held until dropped.
    Held(Lock),
    /// Another process holds it.
    Busy,
    /// No lock to take: go on without.
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_over_the_old_content() {
        let dir = std::env::temp_dir().join(format!("herdr-api-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.json");
        write_atomic(&path, b"old").unwrap();
        write_atomic(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_link_stays_a_link() {
        let dir = std::env::temp_dir().join(format!("herdr-api-link-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (target, link) = (dir.join("dotfile.toml"), dir.join("menu.toml"));
        std::fs::write(&target, b"old").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        write_atomic(&link, b"new").unwrap();
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn one_process_holds_the_lock_at_a_time() {
        let dir = std::env::temp_dir().join(format!("herdr-api-lock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.lock");
        // A file left behind does not hold the lock.
        std::fs::write(&path, "left").unwrap();
        let held = Lock::take(&path).unwrap();
        assert!(held.is_some());
        assert!(Lock::take(&path).unwrap().is_none());
        drop(held);
        assert!(Lock::take(&path).unwrap().is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn names_are_those_of_the_files() {
        assert_eq!(name_of(&MemoryUnit::Mo), "Mo");
        assert_eq!(name_of(&MemoryUnit::MB), "MB");
        assert_eq!(name_of(&1), "");
    }

    #[test]
    fn reads_back_what_it_wrote_as_json() {
        let dir = std::env::temp_dir().join(format!("herdr-api-json-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.json");
        write_json(&path, &vec![1, 2]).unwrap();
        assert_eq!(read_json::<Vec<u32>>(&path), Some(vec![1, 2]));
        assert_eq!(read_json::<Vec<u32>>(&dir.join("none.json")), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn removes_only_old_matching_files() {
        let dir = std::env::temp_dir().join(format!("herdr-api-stale-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.partial"), "").unwrap();
        std::fs::write(dir.join("keep.json"), "").unwrap();
        // Nothing is old enough yet.
        remove_stale(&dir, Duration::from_secs(60), |n| n.ends_with(".partial"));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        std::thread::sleep(Duration::from_millis(20));
        remove_stale(&dir, Duration::ZERO, |n| n.ends_with(".partial"));
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(left, ["keep.json"]);
    }
}
