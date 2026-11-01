//! What the plugins of this repository share: requests to the Herdr API
//! socket, and a few helpers.

mod socket;

pub use socket::{Error, request, request_field};

use serde::Deserialize;
use serde_json::json;
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

fn absent(name: &str) -> String {
    format!("{name} absent")
}

/// Milliseconds since the Unix epoch.
pub fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Whether the file was last written more than `age` ago.
pub fn is_older_than(metadata: &std::fs::Metadata, age: Duration) -> bool {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|elapsed| elapsed > age)
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
/// processes writing at once do not mix their content.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.partial", std::process::id()));
    let partial = path.with_file_name(name);
    std::fs::write(&partial, bytes)?;
    std::fs::rename(&partial, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&partial);
    })
}

/// A size in binary units, short, with a decimal comma as in French:
/// `512K`, `853M`, `2,4G`.
pub fn format_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KIB * KIB {
        format!("{:.0}K", b / KIB)
    } else if b < KIB * KIB * 1000.0 {
        format!("{:.0}M", b / (KIB * KIB))
    } else {
        format!("{:.1}G", b / (KIB * KIB * KIB)).replace('.', ",")
    }
}

/// A share in percent, French style: a narrow no-break space before `%`
/// (`4 %`).
pub fn format_percent(percent: impl std::fmt::Display) -> String {
    format!("{percent}\u{202F}%")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_short() {
        assert_eq!(format_size(512 * 1024), "512K");
        assert_eq!(format_size(853 * 1024 * 1024), "853M");
        assert_eq!(format_size(2_576_980_378), "2,4G");
        assert_eq!(format_percent(4), "4\u{202F}%");
    }

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
