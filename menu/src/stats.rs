//! What the bar shows under its icons: the size of the workspace folder, its
//! share of its disk, and its number of files.

use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Bytes of every file under the folder, `.git` included.
    pub bytes: u64,
    /// Files outside `.git`.
    pub files: u64,
    /// Size of the disk the folder is on, when known.
    pub disk: Option<u64>,
}

/// Walks the folder without following links; unreadable folders are skipped.
pub fn measure(root: &Path) -> Stats {
    let mut stats = Stats {
        disk: disk_size(root),
        ..Stats::default()
    };
    let mut stack: Vec<(PathBuf, bool)> = vec![(root.to_path_buf(), false)];
    while let Some((dir, in_git)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
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
        .filter(|d| starts_with_ignoring_case(path, d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.total_space())
}

/// Windows paths differ in case only (`C:\` and `c:\`).
fn starts_with_ignoring_case(path: &Path, prefix: &Path) -> bool {
    let path = path.to_string_lossy().to_lowercase();
    let prefix = prefix.to_string_lossy().to_lowercase();
    path.starts_with(&prefix)
}

/// `850M · 0.3 %`: the size, and its share of the disk.
pub fn size_line(stats: &Stats) -> String {
    let size = format_bytes(stats.bytes);
    match stats.disk.filter(|&d| d > 0) {
        Some(disk) => {
            let share = stats.bytes as f64 * 100.0 / disk as f64;
            format!("{size} · {} du disque", format_share(share))
        }
        None => size,
    }
}

/// `4 210 fichiers`.
pub fn files_line(stats: &Stats) -> String {
    let unit = if stats.files == 1 {
        "fichier"
    } else {
        "fichiers"
    };
    format!("{} {unit}", group_thousands(stats.files))
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KIB * KIB {
        format!("{:.0}K", b / KIB)
    } else if b < KIB * KIB * 1000.0 {
        format!("{:.0}M", b / (KIB * KIB))
    } else {
        format!("{:.1}G", b / (KIB * KIB * KIB))
    }
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
        assert_eq!(format_bytes(850 * 1024 * 1024), "850M");
        assert_eq!(format_bytes(1229 * 1024 * 1024), "1.2G");
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
        };
        assert_eq!(size_line(&stats), "850M");
        assert_eq!(files_line(&stats), "1 fichier");
    }

    #[test]
    fn counts_files_outside_git_and_sizes_all() {
        let root = std::env::temp_dir().join(format!("herdr-menu-stats-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".git/objects")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/a.rs"), "12345").unwrap();
        std::fs::write(root.join(".git/objects/x"), "123").unwrap();
        let stats = measure(&root);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!((stats.bytes, stats.files), (8, 1));
    }
}
