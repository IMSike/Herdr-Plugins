//! What the plugins of this repository share: requests to the Herdr API
//! socket, and a few helpers.

pub mod sidebar;
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

/// How the plugins write their figures, as their settings choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberFormat {
    /// `2,4G` rather than `2.4G`.
    pub decimal_comma: bool,
    /// `4 %` (a narrow no-break space) rather than `4%`.
    pub percent_space: bool,
    /// CPU shares with one decimal: `4,2%` rather than `4%`.
    pub cpu_decimal: bool,
    /// The units of a size: `853M`, `853Mo` or `853MB`.
    pub memory_unit: MemoryUnit,
    /// `2,4 Go` (a narrow no-break space) rather than `2,4Go`.
    pub memory_space: bool,
}

/// How a size is written; `K`, `M` or `G` as the size needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum MemoryUnit {
    /// `853M`, `2,4G`.
    #[default]
    M,
    /// `853Mo`, `2,4Go`.
    Mo,
    /// `853MB`, `2.4GB`.
    MB,
}

impl MemoryUnit {
    /// As the settings files write it.
    pub fn name(self) -> &'static str {
        match self {
            Self::M => "M",
            Self::Mo => "Mo",
            Self::MB => "MB",
        }
    }

    /// The next one, for a settings screen.
    pub fn next(self) -> Self {
        match self {
            Self::M => Self::Mo,
            Self::Mo => Self::MB,
            Self::MB => Self::M,
        }
    }

    /// `prefix` (`K`, `M`, `G`) with its unit.
    fn suffix(self, prefix: char) -> String {
        match self {
            Self::M => prefix.to_string(),
            Self::Mo => format!("{prefix}o"),
            Self::MB => format!("{prefix}B"),
        }
    }
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self {
            decimal_comma: true,
            percent_space: false,
            cpu_decimal: false,
            memory_unit: MemoryUnit::M,
            memory_space: false,
        }
    }
}

impl NumberFormat {
    /// A number written with one decimal: `2,4`.
    pub fn decimal(&self, value: f64) -> String {
        let text = format!("{value:.1}");
        if self.decimal_comma {
            text.replace('.', ",")
        } else {
            text
        }
    }

    /// A size in binary units: `512K`, `853M`, `2,4G` (or `Ko`, `Mo`, `Go`;
    /// `KB`, `MB`, `GB`), after a narrow no-break space with `memory_space`.
    pub fn size(&self, bytes: u64) -> String {
        const KIB: f64 = 1024.0;
        let b = bytes as f64;
        let space = if self.memory_space { "\u{202F}" } else { "" };
        let unit = |prefix| format!("{space}{}", self.memory_unit.suffix(prefix));
        if b < KIB * KIB {
            format!("{:.0}{}", b / KIB, unit('K'))
        } else if b < KIB * KIB * 1000.0 {
            format!("{:.0}{}", b / (KIB * KIB), unit('M'))
        } else {
            format!("{}{}", self.decimal(b / (KIB * KIB * KIB)), unit('G'))
        }
    }

    /// A share in percent: `4%`, or `4 %`.
    pub fn percent(&self, value: impl std::fmt::Display) -> String {
        let space = if self.percent_space { "\u{202F}" } else { "" };
        format!("{value}{space}%")
    }

    /// A CPU share: `4%`, or `4,2%` with `cpu_decimal`.
    pub fn cpu(&self, percent: f64) -> String {
        if self.cpu_decimal {
            self.percent(self.decimal(percent))
        } else {
            self.percent(format!("{percent:.0}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_short() {
        let french = NumberFormat::default();
        assert_eq!(french.size(512 * 1024), "512K");
        assert_eq!(french.size(853 * 1024 * 1024), "853M");
        assert_eq!(french.size(2_576_980_378), "2,4G");
        assert_eq!(french.percent(4), "4%");
        let other = NumberFormat {
            decimal_comma: false,
            percent_space: true,
            ..french
        };
        assert_eq!(other.size(2_576_980_378), "2.4G");
        assert_eq!(other.percent(4), "4\u{202F}%");
    }

    #[test]
    fn sizes_take_their_unit() {
        let octets = NumberFormat {
            memory_unit: MemoryUnit::Mo,
            ..NumberFormat::default()
        };
        assert_eq!(octets.size(512 * 1024), "512Ko");
        assert_eq!(octets.size(853 * 1024 * 1024), "853Mo");
        assert_eq!(octets.size(2_576_980_378), "2,4Go");
        let bytes = NumberFormat {
            memory_unit: MemoryUnit::MB,
            memory_space: true,
            decimal_comma: false,
            ..NumberFormat::default()
        };
        assert_eq!(bytes.size(2_576_980_378), "2.4\u{202F}GB");
    }

    #[test]
    fn cpu_shares_round_or_keep_a_decimal() {
        let round = NumberFormat::default();
        assert_eq!(round.cpu(0.42), "0%");
        assert_eq!(round.cpu(12.5), "12%");
        let decimal = NumberFormat {
            cpu_decimal: true,
            ..round
        };
        assert_eq!(decimal.cpu(0.42), "0,4%");
        assert_eq!(decimal.cpu(12.46), "12,5%");
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
