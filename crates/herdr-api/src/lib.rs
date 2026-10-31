//! What the plugins of this repository share: requests to the Herdr API
//! socket, and a few helpers.

mod socket;

pub use socket::{Error, request};

use std::path::Path;

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

/// A size in binary units, short: `512K`, `853M`, `2.4G`.
pub fn format_size(bytes: u64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_short() {
        assert_eq!(format_size(512 * 1024), "512K");
        assert_eq!(format_size(853 * 1024 * 1024), "853M");
        assert_eq!(format_size(2_576_980_378), "2.4G");
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
}
