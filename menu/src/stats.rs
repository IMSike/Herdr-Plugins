//! What the bar shows under its icons: the size of the workspace folder, its
//! share of its disk, and its number of files.

use serde::{Deserialize, Serialize};

mod format;
pub use format::{age_label, files_line, size_line};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    /// Bytes of every file under the folder, `.git` included.
    pub bytes: u64,
    /// Files outside `.git`.
    pub files: u64,
    /// Size of the disk the folder is on, when known.
    pub disk: Option<u64>,
    /// `false` when the walk stopped at its limit: the figures are a floor.
    pub complete: bool,
    /// Files and folders read: what a reading costs.
    #[serde(default)]
    pub entries: u64,
}

/// Where a walk stops, so that a huge folder (a home folder, a disk root)
/// costs a bounded amount of time and disk reads.
#[derive(Debug, Clone, Copy)]
pub struct Limit {
    /// Files and folders read.
    pub entries: u64,
    pub time: Duration,
}

impl Default for Limit {
    /// A large project with its dependencies and build folders holds about
    /// 100 000 entries, read in about a second.
    fn default() -> Self {
        Limit {
            entries: 200_000,
            time: Duration::from_secs(2),
        }
    }
}

impl Limit {
    /// A reading asked for with a click: most folders are read in full.
    pub fn manual() -> Self {
        Limit {
            entries: 5_000_000,
            time: Duration::from_secs(60),
        }
    }
}

/// How long figures stay current: the larger the folder, the rarer the
/// reading. A folder read only in part waits a day, or a click.
pub fn lifetime(stats: &Stats) -> Duration {
    let minutes = match stats.entries {
        _ if !stats.complete => 24 * 60,
        0..10_000 => 2,
        10_000..50_000 => 15,
        50_000..200_000 => 2 * 60,
        _ => 24 * 60,
    };
    Duration::from_secs(minutes * 60)
}

/// Figures with the time they were measured at, kept in the plugin's state
/// folder: the bars of a folder share them, and a bar that opens again shows
/// them without reading the folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measured {
    pub root: PathBuf,
    pub stats: Stats,
    /// Seconds since the Unix epoch.
    pub at: u64,
}

impl Measured {
    pub fn now(root: &Path, stats: Stats) -> Self {
        Measured {
            root: root.to_path_buf(),
            stats,
            at: unix_now(),
        }
    }

    /// Seconds until the figures need a new reading; 0 when they do now.
    pub fn due_in(&self) -> u64 {
        let now = unix_now();
        // A time in the future (a clock set back) gives no lifetime.
        if self.at > now {
            return 0;
        }
        (self.at + lifetime(&self.stats).as_secs()).saturating_sub(now)
    }

    /// The more recent of two readings of the same folder.
    pub fn newest(a: Option<Self>, b: Option<Self>) -> Option<Self> {
        match (a, b) {
            (Some(a), Some(b)) => Some(if a.at >= b.at { a } else { b }),
            (a, b) => a.or(b),
        }
    }
}

/// Seconds since the Unix epoch.
pub fn unix_now() -> u64 {
    herdr_api::unix_millis() / 1000
}

/// The file of `root`'s figures in `dir`, named by a hash of its path.
pub fn cache_file(dir: &Path, root: &Path) -> PathBuf {
    // FNV-1a: stable from one build to the next, unlike the std hasher.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in normalized(root).to_string_lossy().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    dir.join(format!("stats-{hash:016x}.json"))
}

/// The figures of `root` kept in `file`, if any.
pub fn load(file: &Path, root: &Path) -> Option<Measured> {
    let text = std::fs::read_to_string(file).ok()?;
    let measured: Measured = serde_json::from_str(&text).ok()?;
    (normalized(&measured.root) == normalized(root)).then_some(measured)
}

pub fn save(file: &Path, measured: &Measured) {
    if let Ok(json) = serde_json::to_vec(measured) {
        let _ = herdr_api::write_atomic(file, &json);
    }
}

/// Walks the folder without following links; unreadable folders are skipped.
pub fn measure(root: &Path, limit: Limit) -> Stats {
    let mut stats = Stats {
        disk: disk_size(root),
        complete: true,
        ..Stats::default()
    };
    let deadline = Instant::now() + limit.time;
    let mut entries_read = 0u64;
    let mut stack: Vec<(PathBuf, bool)> = vec![(root.to_path_buf(), false)];
    while let Some((dir, in_git)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            entries_read += 1;
            stats.entries = entries_read;
            if entries_read > limit.entries
                || (entries_read.is_multiple_of(1000) && Instant::now() >= deadline)
            {
                stats.complete = false;
                return stats;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                let git = in_git || entry.file_name() == ".git";
                stack.push((entry.path(), git));
            } else if kind.is_file() {
                stats.bytes += entry.metadata().map_or(0, |m| m.len());
                if !in_git {
                    stats.files += 1;
                }
            }
        }
    }
    stats
}

/// The disk whose mount point holds `path` the most closely.
fn disk_size(path: &Path) -> Option<u64> {
    let disks = sysinfo::Disks::new_with_refreshed_list().ok()?;
    disks
        .iter()
        .filter(|d| is_under(path, d.mount_point()))
        .max_by_key(|d| d.mount_point().components().count())
        .map(|d| d.total_space())
}

/// Whether `path` is `mount` or inside it, by whole components: `/data2` is
/// not inside `/data`. Windows paths differ in case only (`C:\` and `c:\`).
fn is_under(path: &Path, mount: &Path) -> bool {
    normalized(path).starts_with(normalized(mount))
}

/// `path` as compared: lowercased on Windows, where paths differ in case only.
fn normalized(path: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_files_outside_git_and_sizes_all() {
        let root = std::env::temp_dir().join(format!("herdr-menu-stats-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".git/objects")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/a.rs"), "12345").unwrap();
        std::fs::write(root.join(".git/objects/x"), "123").unwrap();
        let stats = measure(&root, Limit::default());
        let small = Limit {
            entries: 2,
            ..Limit::default()
        };
        let cut = measure(&root, small);
        let _ = std::fs::remove_dir_all(&root);
        // .git, objects, x, src, a.rs
        assert_eq!(
            (stats.bytes, stats.files, stats.complete, stats.entries),
            (8, 1, true, 5)
        );
        assert!(!cut.complete);
    }

    #[test]
    fn larger_folders_are_read_less_often() {
        let after = |entries, complete| {
            lifetime(&Stats {
                entries,
                complete,
                ..Stats::default()
            })
            .as_secs()
                / 60
        };
        assert_eq!(after(5_000, true), 2);
        assert_eq!(after(30_000, true), 15);
        assert_eq!(after(150_000, true), 120);
        assert_eq!(after(900_000, true), 1440);
        assert_eq!(after(5_000, false), 1440);
    }

    #[test]
    fn figures_are_kept_per_folder() {
        let dir = std::env::temp_dir().join(format!("herdr-menu-cache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (Path::new("/work/a"), Path::new("/work/b"));
        assert_ne!(cache_file(&dir, a), cache_file(&dir, b));
        let measured = Measured::now(a, Stats::default());
        save(&cache_file(&dir, a), &measured);
        let loaded = load(&cache_file(&dir, a), a);
        // Another folder never reads it, even under its file name.
        let other = load(&cache_file(&dir, a), b);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(loaded, Some(measured.clone()));
        assert_eq!(other, None);
        let older = Measured {
            at: 1,
            ..measured.clone()
        };
        assert_eq!(
            Measured::newest(Some(older), Some(measured.clone())),
            Some(measured)
        );
    }

    #[test]
    fn a_disk_holds_whole_components() {
        let (data, data2, inside) = if cfg!(windows) {
            (r"D:\data", r"d:\Data2\x", r"d:\DATA\x")
        } else {
            ("/data", "/data2/x", "/data/x")
        };
        assert!(is_under(Path::new(inside), Path::new(data)));
        assert!(!is_under(Path::new(data2), Path::new(data)));
    }
}
