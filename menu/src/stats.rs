//! What the bar shows under its icons: the size of the workspace folder, its
//! share of its disk, and its number of files.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Bytes of every file under the folder, `.git` included.
    pub bytes: u64,
    /// Files outside `.git`.
    pub files: u64,
    /// Size of the disk the folder is on, when known.
    pub disk: Option<u64>,
    /// `false` when the walk stopped at its limit: the figures are a floor.
    pub complete: bool,
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
    if cfg!(windows) {
        let lower = |p: &Path| PathBuf::from(p.to_string_lossy().to_lowercase());
        lower(path).starts_with(lower(mount))
    } else {
        path.starts_with(mount)
    }
}

/// `≥ ` before a figure the walk did not finish.
fn at_least(stats: &Stats) -> &'static str {
    if stats.complete { "" } else { "≥ " }
}

/// `850M · 0.3 %`: the size, and its share of the disk.
pub fn size_line(stats: &Stats) -> String {
    let more = at_least(stats);
    let size = herdr_api::format_size(stats.bytes);
    match stats.disk.filter(|&d| d > 0) {
        Some(disk) => {
            let share = stats.bytes as f64 * 100.0 / disk as f64;
            format!("{more}{size} · {more}{} du disque", format_share(share))
        }
        None => format!("{more}{size}"),
    }
}

/// `4 210 fichiers`.
pub fn files_line(stats: &Stats) -> String {
    let unit = if stats.files == 1 {
        "fichier"
    } else {
        "fichiers"
    };
    format!("{}{} {unit}", at_least(stats), group_thousands(stats.files))
}

/// `0.3 %`, `12 %`, `< 0.1 %`.
fn format_share(percent: f64) -> String {
    if percent < 0.1 {
        "< 0,1 %".into()
    } else if percent < 10.0 {
        format!("{percent:.1} %").replace('.', ",")
    } else {
        format!("{percent:.0} %")
    }
}

/// `4 210`, with a narrow no-break space as in French.
fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{202F}');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(format_share(0.05), "< 0,1 %");
        assert_eq!(format_share(0.34), "0,3 %");
        assert_eq!(format_share(12.4), "12 %");
        assert_eq!(group_thousands(4210), "4\u{202F}210");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1_234_567), "1\u{202F}234\u{202F}567");
        let stats = Stats {
            bytes: 850 * 1024 * 1024,
            files: 1,
            disk: None,
            complete: true,
        };
        assert_eq!(size_line(&stats), "850M");
        assert_eq!(files_line(&stats), "1 fichier");
        let partial = Stats {
            files: 200_000,
            disk: Some(100 * 1024 * 1024 * 1024),
            complete: false,
            ..stats
        };
        assert_eq!(size_line(&partial), "≥ 850M · ≥ 0,8 % du disque");
        assert_eq!(files_line(&partial), "≥ 200 000 fichiers");
    }

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
        assert_eq!((stats.bytes, stats.files, stats.complete), (8, 1, true));
        assert!(!cut.complete);
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
