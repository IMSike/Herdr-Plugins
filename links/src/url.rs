//! The links dx draws, read: `dx-open:///C:/folder/file.rs#L12-L14`, an
//! absolute path of a local drive, each segment percent-encoded, and an
//! optional range of lines. Nothing else: the URL says where, never what to
//! do. Whatever printed the link, a path that the File Explorer would read
//! otherwise than as one path is refused.

use herdr_api::links::is_refused_in_name;

/// What every link starts with.
pub const PREFIX: &str = "dx-open:///";
/// The longest link read, in bytes (`str::len`).
const MAX_LENGTH: usize = 2048;

/// A link read: an absolute Windows path (`C:\folder\file.rs`) and its lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub path: String,
    pub lines: Option<Lines>,
}

/// Lines `from` to `to` (one line when they are equal), from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lines {
    pub from: u32,
    pub to: u32,
}

impl Lines {
    /// As herdr-explorer's `peek` takes them after `:`: `12`, `12-14`.
    pub fn peek_places(self) -> String {
        if self.from == self.to {
            self.from.to_string()
        } else {
            format!("{}-{}", self.from, self.to)
        }
    }
}

/// Why a link is refused: said in the notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NotDxOpen,
    TooLong,
    Query,
    NoDrive,
    BadEscape,
    BadSegment,
    BadCharacter,
    Device,
    Clsid,
    BadLines,
}

impl Refused {
    pub fn reason(self) -> &'static str {
        match self {
            Self::NotDxOpen => "ce n'est pas un lien dx-open:///",
            Self::TooLong => "lien trop long",
            Self::Query => "lien avec des paramètres",
            Self::NoDrive => "pas un chemin absolu d'un lecteur local",
            Self::BadEscape => "encodage invalide",
            Self::BadSegment => "nom de fichier ou de dossier invalide",
            Self::BadCharacter => "caractère refusé dans le chemin",
            Self::Device => "périphérique refusé",
            Self::Clsid => "extension CLSID refusée",
            Self::BadLines => "lignes illisibles",
        }
    }
}

/// Reads a link; `Err` says why it is refused.
pub fn parse(url: &str) -> Result<Link, Refused> {
    if url.len() > MAX_LENGTH {
        return Err(Refused::TooLong);
    }
    let rest = url.strip_prefix(PREFIX).ok_or(Refused::NotDxOpen)?;
    let (rest, fragment) = match rest.split_once('#') {
        Some((rest, fragment)) => (rest, Some(fragment)),
        None => (rest, None),
    };
    if rest.contains('?') {
        return Err(Refused::Query);
    }
    let lines = fragment.map(parse_lines).transpose()?;
    let mut segments = rest.split('/');
    let drive = segments.next().unwrap_or_default();
    let letter = match drive.as_bytes() {
        [letter, b':'] if letter.is_ascii_alphabetic() => letter.to_ascii_uppercase() as char,
        _ => return Err(Refused::NoDrive),
    };
    let mut path = format!("{letter}:");
    let segments: Vec<&str> = segments.collect();
    // `C:/` is the drive's root; `C:` alone is no path.
    match segments.as_slice() {
        [] => return Err(Refused::NoDrive),
        [""] => path.push('\\'),
        _ => {
            for segment in segments {
                let name = decode(segment)?;
                check_name(&name)?;
                path.push('\\');
                path.push_str(&name);
            }
        }
    }
    Ok(Link { path, lines })
}

/// `L12` or `L12-L14`, lines from 1, the last one not before the first.
fn parse_lines(fragment: &str) -> Result<Lines, Refused> {
    let number = |text: &str| -> Result<u32, Refused> {
        let digits = text.strip_prefix('L').ok_or(Refused::BadLines)?;
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Refused::BadLines);
        }
        digits
            .parse::<u32>()
            .ok()
            .filter(|&n| n > 0)
            .ok_or(Refused::BadLines)
    };
    let (from, to) = match fragment.split_once('-') {
        Some((from, to)) => (number(from)?, number(to)?),
        None => {
            let line = number(fragment)?;
            (line, line)
        }
    };
    if to < from {
        return Err(Refused::BadLines);
    }
    Ok(Lines { from, to })
}

/// A segment percent-decoded, as strict UTF-8.
fn decode(segment: &str) -> Result<String, Refused> {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let digit = |at: usize| {
                bytes
                    .get(at)
                    .and_then(|&b| char::from(b).to_digit(16))
                    .ok_or(Refused::BadEscape)
            };
            out.push((digit(i + 1)? * 16 + digit(i + 2)?) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| Refused::BadEscape)
}

/// A file or folder name the File Explorer takes whole, of the file it
/// names: not empty, not `.` or `..`, none of the characters of
/// `is_refused_in_name`, no trailing dot or space (Windows drops them,
/// opening another file than the one checked), not a device's name, and no
/// CLSID as its extension (the File Explorer opens the object it names).
pub fn check_name(name: &str) -> Result<(), Refused> {
    if name.is_empty() || name == "." || name == ".." {
        return Err(Refused::BadSegment);
    }
    if name.ends_with(['.', ' ']) {
        return Err(Refused::BadSegment);
    }
    if name.chars().any(is_refused_in_name) {
        return Err(Refused::BadCharacter);
    }
    if is_device(name) {
        return Err(Refused::Device);
    }
    if name.rsplit_once('.').is_some_and(|(_, ext)| is_clsid(ext)) {
        return Err(Refused::Clsid);
    }
    Ok(())
}

/// Whether Windows reads `name` as a device: one of its reserved names, in
/// any case, alone or before an extension, spaces before the dot included
/// (`nul`, `COM1.txt`, `aux .md`, `lpt¹`).
fn is_device(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ');
    let stem = stem.to_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    let mut chars = stem.chars();
    let prefix: String = chars.by_ref().take(3).collect();
    let digit = chars.next();
    (prefix == "COM" || prefix == "LPT")
        && chars.next().is_none()
        && digit.is_some_and(|d| d.is_ascii_digit() || matches!(d, '¹' | '²' | '³'))
}

/// Whether `ext` is a CLSID, `{8-4-4-4-12}` hexadecimal digits in braces.
fn is_clsid(ext: &str) -> bool {
    let Some(inner) = ext.strip_prefix('{').and_then(|e| e.strip_suffix('}')) else {
        return false;
    };
    let groups: Vec<&str> = inner.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, len)| g.len() == len && g.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(url: &str) -> Result<String, Refused> {
        parse(url).map(|link| link.path)
    }

    #[test]
    fn reads_a_drive_path() {
        assert_eq!(
            parse("dx-open:///C:/repo/src/app.ts"),
            Ok(Link {
                path: r"C:\repo\src\app.ts".into(),
                lines: None
            })
        );
        assert_eq!(path("dx-open:///d:/x"), Ok(r"D:\x".into()));
        assert_eq!(path("dx-open:///C:/"), Ok(r"C:\".into()));
        assert_eq!(path("dx-open:///C:"), Err(Refused::NoDrive));
        assert_eq!(path("dx-open:///repo/x"), Err(Refused::NoDrive));
        assert_eq!(path("dx-open:///CC:/x"), Err(Refused::NoDrive));
        assert_eq!(path("dx-open:///1:/x"), Err(Refused::NoDrive));
    }

    #[test]
    fn decodes_each_segment() {
        assert_eq!(
            path("dx-open:///C:/My%20Docs/a%20(1).md"),
            Ok(r"C:\My Docs\a (1).md".into())
        );
        assert_eq!(
            path("dx-open:///C:/caf%C3%A9/%E6%97%A5.txt"),
            Ok(r"C:\café\日.txt".into())
        );
        assert_eq!(path("dx-open:///C:/a%21%27%2A"), Err(Refused::BadCharacter));
        assert_eq!(path("dx-open:///C:/a%21%27"), Ok(r"C:\a!'".into()));
        assert_eq!(path("dx-open:///C:/a b"), Ok(r"C:\a b".into()));
        assert_eq!(path("dx-open:///C:/%23x"), Ok(r"C:\#x".into()));
    }

    #[test]
    fn refuses_broken_escapes() {
        for url in [
            "dx-open:///C:/a%2",
            "dx-open:///C:/a%zz",
            "dx-open:///C:/a%C3",
            "dx-open:///C:/a%FF",
            "dx-open:///C:/a%",
        ] {
            assert_eq!(path(url), Err(Refused::BadEscape), "{url}");
        }
    }

    #[test]
    fn refuses_what_would_name_another_file() {
        for (url, why) in [
            ("dx-open:///C:/a%00b", Refused::BadCharacter),
            ("dx-open:///C:/a%0Ab", Refused::BadCharacter),
            ("dx-open:///C:/a%1Fb", Refused::BadCharacter),
            ("dx-open:///C:/a%7Fb", Refused::BadCharacter),
            ("dx-open:///C:/a%E2%80%8Bb.txt", Refused::BadCharacter),
            ("dx-open:///C:/a%E2%80%8F.txt", Refused::BadCharacter),
            ("dx-open:///C:/photo%E2%80%AEtxt.exe", Refused::BadCharacter),
            ("dx-open:///C:/a%E2%81%A6b%E2%81%A9", Refused::BadCharacter),
            ("dx-open:///C:/a%E2%81%A0b", Refused::BadCharacter),
            ("dx-open:///C:/soft%C2%ADhyphen.txt", Refused::BadCharacter),
            ("dx-open:///C:/a%D8%9Cb.txt", Refused::BadCharacter),
            ("dx-open:///C:/a%E1%A0%8Eb.txt", Refused::BadCharacter),
            ("dx-open:///C:/a%EF%B8%8F.txt", Refused::BadCharacter),
            ("dx-open:///C:/a%F3%A0%81%81b.txt", Refused::BadCharacter),
            ("dx-open:///C:/%EF%BB%BFa.txt", Refused::BadCharacter),
            ("dx-open:///C:/a%E3%85%A4b.txt", Refused::BadCharacter),
            ("dx-open:///C:/a%CD%8Fb.txt", Refused::BadCharacter),
            ("dx-open:///C:/a%EF%BF%B9b.txt", Refused::BadCharacter),
            ("dx-open:///C:/a%EF%BF%BBb.txt", Refused::BadCharacter),
            ("dx-open:///C:/a%2Fb", Refused::BadCharacter),
            ("dx-open:///C:/a%5Cb", Refused::BadCharacter),
            ("dx-open:///C:/a.txt%3Aevil.exe", Refused::BadCharacter),
            ("dx-open:///C:/a:b", Refused::BadCharacter),
            ("dx-open:///C:/a,b.exe", Refused::BadCharacter),
            ("dx-open:///C:/a%2Cb", Refused::BadCharacter),
            ("dx-open:///C:/a%25b.txt", Refused::BadCharacter),
            ("dx-open:///C:/%25windir%25/x.txt", Refused::BadCharacter),
            ("dx-open:///C:/a;b", Refused::BadCharacter),
            ("dx-open:///C:/a%22b", Refused::BadCharacter),
            ("dx-open:///C:/a*", Refused::BadCharacter),
            ("dx-open:///C:/a%3F", Refused::BadCharacter),
            ("dx-open:///C:/a%7Cb", Refused::BadCharacter),
            ("dx-open:///C:/a%3Cb", Refused::BadCharacter),
            ("dx-open:///C:/../Windows", Refused::BadSegment),
            ("dx-open:///C:/x/./y", Refused::BadSegment),
            ("dx-open:///C:/x/%2E%2E/y", Refused::BadSegment),
            ("dx-open:///C:/x//y", Refused::BadSegment),
            ("dx-open:///C:/x/", Refused::BadSegment),
            ("dx-open:///C:/a.exe.", Refused::BadSegment),
            ("dx-open:///C:/a.exe%20", Refused::BadSegment),
            ("dx-open:///C:/x?y=1", Refused::Query),
        ] {
            assert_eq!(path(url), Err(why), "{url}");
        }
    }

    #[test]
    fn refuses_a_device_or_a_clsid() {
        for device in [
            "CON",
            "con",
            "Con.txt",
            "prn.tar.gz",
            "AUX",
            "nul .md",
            "COM0",
            "com9.log",
            "LPT1",
            "lpt5.txt",
            "COM¹",
            "lpt³.txt",
            "CONIN$",
            "conout$.x",
        ] {
            assert_eq!(check_name(device), Err(Refused::Device), "{device}");
        }
        for clsid in [
            "a.{645FF040-5081-101B-9F08-00AA002F954E}",
            "x.txt.{20d04fe0-3aea-1069-a2d8-08002b30309d}",
        ] {
            assert_eq!(check_name(clsid), Err(Refused::Clsid), "{clsid}");
        }
        for kept in [
            "CONSOLE",
            "console.txt",
            "a.con",
            "COM10",
            "COM",
            "LPT¹0",
            "nul_",
            "x.{645FF040}",
            "x.{645FF040-5081-101B-9F08-00AA002F954}",
            "{645FF040-5081-101B-9F08-00AA002F954E}",
        ] {
            assert_eq!(check_name(kept), Ok(()), "{kept}");
        }
        assert_eq!(path("dx-open:///C:/x/NUL.txt"), Err(Refused::Device));
        assert_eq!(path("dx-open:///C:/x/COM%C2%B9"), Err(Refused::Device));
    }

    #[test]
    fn refuses_another_scheme_or_a_host() {
        for url in [
            "file:///C:/a.txt",
            "dx-open:C:/a.txt",
            "dx-open://C:/a.txt",
            "dx-open://server/share/a.txt",
            "DX-OPEN:///C:/a.txt",
            " dx-open:///C:/a.txt",
        ] {
            assert!(parse(url).is_err(), "{url}");
        }
        // A host is not a drive.
        assert_eq!(path("dx-open:////server/share"), Err(Refused::NoDrive));
    }

    #[test]
    fn refuses_a_link_too_long() {
        let long = format!("{PREFIX}C:/{}", "a/".repeat(MAX_LENGTH));
        assert_eq!(path(&long), Err(Refused::TooLong));
        let fits = format!("{PREFIX}C:/{}", "a".repeat(MAX_LENGTH - PREFIX.len() - 3));
        assert!(parse(&fits).is_ok());
    }

    #[test]
    fn reads_lines() {
        let lines = |url: &str| parse(url).map(|link| link.lines);
        assert_eq!(
            lines("dx-open:///C:/a.rs#L12"),
            Ok(Some(Lines { from: 12, to: 12 }))
        );
        assert_eq!(
            lines("dx-open:///C:/a.rs#L12-L14"),
            Ok(Some(Lines { from: 12, to: 14 }))
        );
        assert_eq!(
            lines("dx-open:///C:/a.rs#L12-L12"),
            Ok(Some(Lines { from: 12, to: 12 }))
        );
        for junk in [
            "#L14-L12",
            "#",
            "#L",
            "#L0",
            "#12",
            "#L12-14",
            "#L1x",
            "#L12,L14",
            "#L-1",
            "#x",
            "#L99999999999",
            "#L12-L14-L16",
        ] {
            assert_eq!(
                lines(&format!("dx-open:///C:/a.rs{junk}")),
                Err(Refused::BadLines),
                "{junk}"
            );
        }
        assert_eq!(Lines { from: 12, to: 12 }.peek_places(), "12");
        assert_eq!(Lines { from: 12, to: 14 }.peek_places(), "12-14");
    }
}
