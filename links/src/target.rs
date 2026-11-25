//! What a link's path really names, checked on disk: the path made
//! canonical (symbolic links and junctions followed, short names made long,
//! a `subst` drive replaced by its folder), on a local drive, a folder or a
//! file. The command that opens it is built from this canonical path, the
//! same string that was checked.

use crate::url;

/// A path checked on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Canonical, without the `\\?\` prefix: `C:\folder\file.rs`.
    pub path: String,
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Folder,
    File,
}

impl Target {
    /// The name of the file or folder, its last segment.
    pub fn name(&self) -> &str {
        name_of(&self.path)
    }
}

/// The name of the file or folder at the canonical `path`, its last segment.
pub fn name_of(path: &str) -> &str {
    path.rsplit('\\').next().unwrap_or_default()
}

/// Why a path is not opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// Nothing there (or it cannot be read).
    Missing,
    /// Not on a local drive: on a network share, or a path of a volume or
    /// device namespace (`\\?\Volume{…}`, `\\.\COM1`); a device's name on a
    /// drive is a `BadName`.
    NotLocal,
    /// Its real name holds what a link may not (see `url::check_name`).
    BadName,
    /// Its real path is `MAX_PATH` UTF-16 code units long or more.
    TooLong,
    /// Neither a folder nor a file.
    Other,
}

/// Windows' classic limit on a path, in UTF-16 code units, the final null
/// included: a path of as many units or more is past it.
const MAX_PATH: usize = 260;

impl Refused {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Missing => "introuvable",
            Self::NotLocal => "chemin réseau ou hors d'un lecteur local refusé",
            Self::BadName => "nom réel refusé",
            Self::TooLong => "chemin trop long",
            Self::Other => "ni un fichier ni un dossier",
        }
    }
}

/// Checks the path `path` on disk.
pub fn resolve(path: &str) -> Result<Target, Refused> {
    let canonical = std::fs::canonicalize(path).map_err(|_| Refused::Missing)?;
    let canonical = canonical.to_str().ok_or(Refused::BadName)?;
    let path = local_path(canonical)?;
    let metadata = std::fs::metadata(&path).map_err(|_| Refused::Missing)?;
    let kind = if metadata.is_dir() {
        Kind::Folder
    } else if metadata.is_file() {
        Kind::File
    } else {
        return Err(Refused::Other);
    };
    Ok(Target { path, kind })
}

/// A canonical path as Windows gives it (`\\?\C:\x`), as a plain local path
/// (`C:\x`) whose every name passes the link's rules; refused when it is on
/// a network share (`\\?\UNC\…`), or is anything but a drive's: a path of a
/// volume or device namespace (`\\?\GLOBALROOT\…`, `\\.\COM1`); refused too
/// when that plain path is past `MAX_PATH`.
fn local_path(canonical: &str) -> Result<String, Refused> {
    let plain = canonical.strip_prefix(r"\\?\").unwrap_or(canonical);
    let bytes = plain.as_bytes();
    let on_drive =
        bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\';
    if !on_drive {
        return Err(Refused::NotLocal);
    }
    if plain.encode_utf16().count() >= MAX_PATH {
        return Err(Refused::TooLong);
    }
    let rest = &plain[3..];
    if !rest.is_empty() {
        for name in rest.split('\\') {
            url::check_name(name).map_err(|_| Refused::BadName)?;
        }
    }
    Ok(plain.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_canonical_path_is_a_drive_s() {
        assert_eq!(local_path(r"\\?\C:\repo\a.rs"), Ok(r"C:\repo\a.rs".into()));
        assert_eq!(local_path(r"\\?\C:\"), Ok(r"C:\".into()));
        assert_eq!(local_path(r"D:\x"), Ok(r"D:\x".into()));
        for not_local in [
            r"\\?\UNC\server\share\a.txt",
            r"\\server\share\a.txt",
            r"\\?\GLOBALROOT\Device\HarddiskVolume1\a",
            r"\\?\Volume{1234}\a",
            r"\\.\COM1",
            "/tmp/a",
            "C:",
        ] {
            assert_eq!(local_path(not_local), Err(Refused::NotLocal), "{not_local}");
        }
    }

    #[test]
    fn a_real_name_passes_the_link_s_rules() {
        for bad in [
            r"\\?\C:\a,b\c.txt",
            r"\\?\C:\x\a.exe.",
            r"\\?\C:\x\a ",
            r"\\?\C:\x\a;b",
            r"\\?\C:\x\%windir%.txt",
            r"\\?\C:\x\\y",
            r"\\?\C:\x\con.txt",
            r"\\?\C:\x\a.{645FF040-5081-101B-9F08-00AA002F954E}",
        ] {
            assert_eq!(local_path(bad), Err(Refused::BadName), "{bad}");
        }
    }

    /// 260 UTF-16 code units or more, counted without the `\\?\` prefix: a
    /// character outside the Basic Multilingual Plane counts as two.
    #[test]
    fn a_path_past_max_path_is_refused() {
        let path = |len: usize| format!(r"\\?\C:\{}", "a".repeat(len - 3));
        assert_eq!(local_path(&path(259)).map(|p| p.len()), Ok(259));
        assert_eq!(local_path(&path(260)), Err(Refused::TooLong));
        assert_eq!(local_path(&path(400)), Err(Refused::TooLong));
        // 3 + 128 × 2 = 259 units, then one more.
        let wide = format!(r"C:\{}", "\u{1F600}".repeat(128));
        assert!(local_path(&wide).is_ok());
        assert_eq!(local_path(&format!("{wide}a")), Err(Refused::TooLong));
        assert_eq!(Refused::TooLong.reason(), "chemin trop long");
    }

    #[test]
    fn names_its_last_segment() {
        let target = Target {
            path: r"C:\repo\Cargo.toml".into(),
            kind: Kind::File,
        };
        assert_eq!(target.name(), "Cargo.toml");
    }

    #[test]
    fn a_missing_path_is_refused() {
        let dir = herdr_api::TempDir::new("links-missing").unwrap();
        let missing = dir.join("none.txt");
        assert_eq!(resolve(missing.to_str().unwrap()), Err(Refused::Missing));
    }

    #[cfg(windows)]
    #[test]
    fn tells_a_folder_from_a_file() {
        let dir = herdr_api::TempDir::new("links-target").unwrap();
        let file = dir.join("a b.txt");
        std::fs::write(&file, "x").unwrap();
        let folder = resolve(dir.to_str().unwrap()).unwrap();
        assert_eq!(folder.kind, Kind::Folder);
        assert!(!folder.path.starts_with(r"\\"), "{}", folder.path);
        let found = resolve(file.to_str().unwrap()).unwrap();
        assert_eq!(found.kind, Kind::File);
        assert_eq!(found.name(), "a b.txt");
    }

    /// A link followed: the type checked is the one of the file it leads to.
    /// Creating one needs the developer mode or the right to; skipped
    /// without.
    #[cfg(windows)]
    #[test]
    fn follows_a_symbolic_link() {
        let dir = herdr_api::TempDir::new("links-symlink").unwrap();
        let real = dir.join("tool.exe");
        std::fs::write(&real, "x").unwrap();
        let link = dir.join("notes.txt");
        if std::os::windows::fs::symlink_file(&real, &link).is_err() {
            return;
        }
        let found = resolve(link.to_str().unwrap()).unwrap();
        assert_eq!(found.name(), "tool.exe");
    }
}
