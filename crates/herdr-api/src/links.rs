//! The links plugin's id and its settings, `links.toml` in its config
//! folder, which it reads at each click. A missing file or setting keeps its
//! default, and a wrong type of `open_types` is left out; a file that cannot
//! be used at all, or found, is set aside: a click then goes by
//! `Settings::set_aside`, which only selects. Neither the defaults nor that
//! ever launch a program.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The links plugin's id.
pub const ID: &str = "imsike.links";
/// Its settings file, in its config folder.
pub const SETTINGS_FILE: &str = "links.toml";
/// What a new `links.toml` holds: every setting, commented out at its
/// default.
pub const TEMPLATE: &str = include_str!("../../../links/templates/links.toml");

/// The file types the default application may open: none runs code by
/// itself when Windows opens it with the program it associates with it, as
/// installed by default (that program may still have flaws), but a PDF,
/// whose reader (Acrobat, Edge's) may run the JavaScript it holds, in its
/// sandbox. Documents, images, sound and video, data and configuration, and
/// source code Windows does not run (not `.js`, `.py`, `.ps1`, `.sh`…).
/// Markup a browser opens (`.html`, `.svg`, `.xml`) is left out: its scripts
/// would run. So are OpenDocument files (LibreOffice Basic macros) and RTF.
#[rustfmt::skip]
pub const OPEN_TYPES: &[&str] = &[
    // Text and documents.
    "txt", "md", "markdown", "rst", "adoc", "log", "pdf", "docx",
    "xlsx", "pptx",
    // Data and configuration.
    "csv", "tsv", "json", "jsonc", "json5", "jsonl", "yaml", "yml", "toml", "ini", "cfg", "conf",
    "env", "properties", "lock", "sql", "diff", "patch",
    // Source code Windows does not run.
    "c", "h", "cc", "cpp", "cxx", "hpp", "cs", "go", "rs", "java", "kt", "swift", "ts", "tsx",
    "jsx", "vue", "svelte", "css", "scss", "sass", "less",
    // Images.
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tif", "tiff", "avif", "heic",
    // Sound and video.
    "mp3", "wav", "flac", "ogg", "m4a", "mp4", "mkv", "webm", "mov", "avi",
];

/// What a click on a file does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileAction {
    /// herdr-explorer's preview; without herdr-explorer, as `Application`.
    #[default]
    Preview,
    /// The file selected in the File Explorer, never opened.
    Select,
    /// The program Windows associates with the file, for the types of
    /// `open_types`; any other one is selected.
    Application,
}

impl FileAction {
    /// Every value.
    pub const ALL: [Self; 3] = [Self::Preview, Self::Select, Self::Application];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// What a click on a file does; a folder is always selected in its
    /// parent.
    pub file: FileAction,
    /// The file types (without the dot, any case) the default application
    /// may open; any other one is selected instead.
    pub open_types: Vec<String>,
    /// Danger: a file of another type opens with its program too, a program
    /// or a script runs.
    pub run_programs: bool,
    /// With `run_programs`: a popup shows the real path and waits for Enter
    /// before a file of another type opens.
    pub confirm_programs: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            file: FileAction::default(),
            open_types: OPEN_TYPES.iter().map(|t| t.to_string()).collect(),
            run_programs: false,
            confirm_programs: true,
        }
    }
}

/// Settings read from a file, and the types of `open_types` left out of
/// them, each with the reason.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Loaded {
    pub settings: Settings,
    pub skipped: Vec<String>,
}

impl Settings {
    /// The settings of the config folder `config_dir`, the defaults without
    /// a file; `Err` says why the file is ignored, or why it cannot be found
    /// (`config_dir` unknown, its error): the user may have written it, so
    /// that is no missing file.
    pub fn load(config_dir: Result<PathBuf, String>) -> Result<Loaded, String> {
        let dir = config_dir.map_err(|e| format!("{SETTINGS_FILE} introuvable : {e}"))?;
        Self::read(&dir.join(SETTINGS_FILE))
    }

    /// The settings of the file `path`, the defaults when it does not exist;
    /// `Err` says why it is ignored.
    pub fn read(path: &Path) -> Result<Loaded, String> {
        crate::read_settings(path, SETTINGS_FILE, Self::parse)
    }

    /// What a click goes by when `links.toml` cannot be used at all (a
    /// syntax error, an unknown setting) or found (no config folder): never
    /// more than the user chose, so a file is only selected and nothing runs.
    pub fn set_aside() -> Self {
        Self {
            file: FileAction::Select,
            ..Self::default()
        }
    }

    /// A wrong type of `open_types` is left out, the others kept: one typo
    /// does not set the whole file aside.
    fn parse(text: &str) -> Result<Loaded, String> {
        let mut settings: Self = toml::from_str(text).map_err(|e| e.message().to_string())?;
        let mut skipped = Vec::new();
        let mut kept: Vec<String> = Vec::new();
        for written in &settings.open_types {
            match open_type(written) {
                Ok(t) if !kept.contains(&t) => kept.push(t),
                Ok(_) => {}
                Err(e) => skipped.push(format!("open_types : {e}")),
            }
        }
        settings.open_types = kept;
        Ok(Loaded { settings, skipped })
    }

    /// Whether the default application may open a file named `name`: its
    /// type is one of `open_types`. A name without a type never is.
    pub fn opens(&self, name: &str) -> bool {
        file_type(name).is_some_and(|t| self.open_types.contains(&t))
    }
}

/// A character no file or folder name of a link may hold: a separator, a
/// colon (an alternate data stream), what Windows refuses in a name or reads
/// as a wildcard, a comma, a semicolon or a double quote (the File Explorer
/// splits its command line at them), a percent sign (so that no `%NAME%`
/// can ever be read as a variable on the way to the File Explorer), a
/// control character (`char::is_control`, the category Cc), a character
/// that changes how a name looks without showing as itself (`is_invisible`),
/// the blank braille pattern U+2800, which shows as a blank, a character of
/// the category Co (private use, `PRIVATE_USE`), which a font may draw as
/// anything, or a Unicode noncharacter (`is_noncharacter`), which Unicode
/// keeps for a program's internal use.
pub fn is_refused_in_name(c: char) -> bool {
    c.is_control()
        || is_invisible(c)
        || c == '\u{2800}'
        || is_noncharacter(c)
        || PRIVATE_USE
            .iter()
            .any(|&(first, last)| (first..=last).contains(&c))
        || matches!(
            c,
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | ',' | ';' | '%'
        )
}

/// The characters of the Unicode General_Category Cf (format), Zl (line
/// separator) and Zp (paragraph separator), and those of the property
/// Default_Ignorable_Code_Point, as Unicode 16.0 lists them
/// (DerivedGeneralCategory.txt, DerivedCoreProperties.txt), adjacent ranges
/// merged. A terminal draws them as nothing, as no more than a mark, or as a
/// line break: zero-width characters, the soft hyphen, fillers, variation
/// selectors, tags, the bidirectional controls, which could show `txt.exe` as
/// `exe.txt`, the interlinear annotations, the line and paragraph separators,
/// and the format controls of other scripts (Arabic, Syriac, Kaithi, Egyptian
/// hieroglyphs, shorthand, musical notation).
const INVISIBLE: &[(char, char)] = &[
    ('\u{00AD}', '\u{00AD}'),
    ('\u{034F}', '\u{034F}'),
    ('\u{0600}', '\u{0605}'),
    ('\u{061C}', '\u{061C}'),
    ('\u{06DD}', '\u{06DD}'),
    ('\u{070F}', '\u{070F}'),
    ('\u{0890}', '\u{0891}'),
    ('\u{08E2}', '\u{08E2}'),
    ('\u{115F}', '\u{1160}'),
    ('\u{17B4}', '\u{17B5}'),
    ('\u{180B}', '\u{180F}'),
    ('\u{200B}', '\u{200F}'),
    // U+2028 is Zl, U+2029 Zp, U+202A–U+202E bidirectional controls.
    ('\u{2028}', '\u{202E}'),
    ('\u{2060}', '\u{206F}'),
    ('\u{3164}', '\u{3164}'),
    ('\u{FE00}', '\u{FE0F}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FFA0}', '\u{FFA0}'),
    // U+FFF0–U+FFF8 are default ignorable, U+FFF9–U+FFFB the annotations.
    ('\u{FFF0}', '\u{FFFB}'),
    ('\u{110BD}', '\u{110BD}'),
    ('\u{110CD}', '\u{110CD}'),
    ('\u{13430}', '\u{1343F}'),
    ('\u{1BCA0}', '\u{1BCA3}'),
    ('\u{1D173}', '\u{1D17A}'),
    ('\u{E0000}', '\u{E0FFF}'),
];

/// The characters of the Unicode General_Category Co (private use), as
/// Unicode 16.0 lists them: the Private Use Area and the two supplementary
/// private use planes, their last two code points aside (noncharacters).
const PRIVATE_USE: &[(char, char)] = &[
    ('\u{E000}', '\u{F8FF}'),
    ('\u{F0000}', '\u{FFFFD}'),
    ('\u{100000}', '\u{10FFFD}'),
];

/// One of the 66 Unicode noncharacters: U+FDD0–U+FDEF, and the last two code
/// points of each plane, whose low 16 bits are FFFE or FFFF (U+FFFE,
/// U+1FFFF… U+10FFFF).
fn is_noncharacter(c: char) -> bool {
    let n = u32::from(c);
    (0xFDD0..=0xFDEF).contains(&n) || n & 0xFFFE == 0xFFFE
}

/// A character that changes how a name looks without showing as itself: one
/// of `INVISIBLE`.
fn is_invisible(c: char) -> bool {
    INVISIBLE
        .iter()
        .any(|&(first, last)| (first..=last).contains(&c))
}

/// A type as `open_types` holds it, from what the user wrote: without its
/// leading dot and the spaces around, lowercase. Any extension is taken
/// (`.$$$`, `.!ut`, `(x)`); refused, with the reason: an empty one, one
/// holding a dot (only the last extension counts: `gz` for `.tar.gz`), or a
/// character a link's name may not hold.
pub fn open_type(written: &str) -> Result<String, String> {
    let text = written.trim();
    let text = text.strip_prefix('.').unwrap_or(text);
    if text.is_empty() {
        return Err("type vide".into());
    }
    if text.contains('.') {
        // What to write instead: the last extension, trailing dots aside
        // (`md.` is `md`, as Windows drops them).
        let last = text.trim_end_matches('.').rsplit('.').next().unwrap_or("");
        return Err(if last.is_empty() {
            format!("« {text} » : pas d'extension")
        } else if text.ends_with('.') {
            format!("« {text} » : un type ne finit pas par un point, écrire « {last} »")
        } else {
            format!("« {text} » : seule la dernière extension compte, écrire « {last} »")
        });
    }
    if let Some(c) = text.chars().find(|&c| is_refused_in_name(c)) {
        return Err(format!(
            "« {text} » : caractère « {} » refusé",
            c.escape_debug()
        ));
    }
    Ok(text.to_lowercase())
}

/// The type of a file named `name`, lowercase: what follows its last dot,
/// as Windows reads it, a name that starts with its only dot included
/// (`.env` is an `env`, `.bat` a `bat`). `None` without a dot, or with
/// nothing after the last one.
pub fn file_type(name: &str) -> Option<String> {
    let (_, ext) = name.rsplit_once('.')?;
    (!ext.is_empty()).then(|| ext.to_lowercase())
}

/// The type of a file named `name` as a message says it: `type .exe`, or
/// `sans type` (see `file_type`).
pub fn type_label(name: &str) -> String {
    file_type(name).map_or_else(|| "sans type".into(), |t| format!("type .{t}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Settings, String> {
        Settings::parse(text).map(|loaded| loaded.settings)
    }

    #[test]
    fn missing_settings_keep_their_default() {
        assert_eq!(parse("").unwrap(), Settings::default());
        let settings = parse("file = \"select\"\nrun_programs = true\n").unwrap();
        assert_eq!(settings.file, FileAction::Select);
        assert!(settings.run_programs && settings.confirm_programs);
        assert_eq!(settings.open_types, Settings::default().open_types);
        assert!(parse("file = \"run\"").is_err());
        assert!(parse("danger = true").is_err());
    }

    #[test]
    fn the_defaults_launch_nothing() {
        let settings = Settings::default();
        assert_eq!(settings.file, FileAction::Preview);
        assert!(!settings.run_programs);
        for runs in [
            "a.exe",
            "a.bat",
            "a.cmd",
            "a.com",
            "a.js",
            "a.vbs",
            "a.ps1",
            "a.py",
            "a.sh",
            "a.lnk",
            "a.url",
            "a.msi",
            "a.reg",
            "a.hta",
            "a.html",
            "a.svg",
            "a.xml",
            "a.docm",
            "a.xlsm",
            "a.odt",
            "a.ods",
            "a.rtf",
            "a.jar",
            "a.scr",
            "a.cpl",
            "a.chm",
            "README",
            ".bat",
            ".gitignore",
        ] {
            assert!(!settings.opens(runs), "{runs}");
        }
        for shows in [
            "notes.TXT",
            "a.b.md",
            "photo.jpeg",
            "Cargo.lock",
            "x.rs",
            ".env",
        ] {
            assert!(settings.opens(shows), "{shows}");
        }
    }

    #[test]
    fn types_are_read_without_their_dot_in_any_case() {
        let settings = parse("open_types = [\".HTML\", \" txt \", \"TXT\"]").unwrap();
        assert_eq!(settings.open_types, ["html", "txt"]);
        assert!(settings.opens("index.html") && !settings.opens("a.md"));
        assert_eq!(open_type(".Md"), Ok("md".into()));
    }

    #[test]
    fn any_extension_a_name_holds_is_a_type() {
        for (written, kept) in [
            (".!ut", "!ut"),
            (".$$$", "$$$"),
            ("(x)", "(x)"),
            ("#", "#"),
            ("&", "&"),
            ("'", "'"),
            ("=", "="),
            ("^", "^"),
            ("{}", "{}"),
            ("[]", "[]"),
            ("Ünï", "ünï"),
        ] {
            assert_eq!(open_type(written), Ok(kept.to_string()), "{written}");
        }
        let settings = parse("open_types = [\".$$$\", \"!ut\"]").unwrap();
        assert!(settings.opens("setup.$$$") && settings.opens("big.iso.!UT"));
    }

    #[test]
    fn a_type_that_cannot_be_one_says_why() {
        assert_eq!(open_type(" "), Err("type vide".into()));
        assert_eq!(open_type("."), Err("type vide".into()));
        let tar = open_type("tar.gz").unwrap_err();
        assert!(
            tar.contains("seule la dernière extension") && tar.contains("« gz »"),
            "{tar}"
        );
        assert!(open_type("..gz").is_err());
        let md = open_type("md.").unwrap_err();
        assert!(
            md.contains("ne finit pas par un point") && md.ends_with("« md »"),
            "{md}"
        );
        assert!(open_type("tar.gz.").unwrap_err().ends_with("« gz »"));
        assert_eq!(open_type("...").unwrap_err(), "« .. » : pas d'extension");
        for refused in [
            "a/b",
            "a\\b",
            "a:b",
            "*",
            "a?",
            "\"",
            "<",
            ">",
            "|",
            ",",
            ";",
            "a%b",
            "%windir%",
            "a\tb",
            "e\u{202E}xe",
            "a\u{200B}",
            "\u{FEFF}md",
            "a\u{2066}b",
            "soft\u{00AD}hyphen",
            "a\u{061C}b",
            "a\u{180E}b",
            "md\u{FE0F}",
            "a\u{E0041}b",
            "a\u{E007F}",
        ] {
            assert!(open_type(refused).is_err(), "{refused}");
        }
    }

    /// Spot checks, written apart from `INVISIBLE`: the first and the last
    /// character of each range refused, and one inside the long ones; the
    /// characters on either side of a range kept.
    #[test]
    fn invisible_characters_are_refused_and_their_neighbours_kept() {
        for n in [
            0x00AD, 0x034F, 0x0600, 0x0605, 0x061C, 0x06DD, 0x070F, 0x0890, 0x0891, 0x08E2, 0x115F,
            0x1160, 0x17B4, 0x17B5, 0x180B, 0x180F, 0x200B, 0x200F, 0x2028, 0x202E, 0x2060, 0x2064,
            0x206F, 0x3164, 0xFE00, 0xFE0F, 0xFEFF, 0xFFA0, 0xFFF0, 0xFFFB, 0x110BD, 0x110CD,
            0x13430, 0x1343F, 0x1BCA0, 0x1BCA3, 0x1D173, 0x1D17A, 0xE0000, 0xE0080, 0xE0FFF,
        ] {
            let c = char::from_u32(n).unwrap();
            assert!(is_refused_in_name(c), "U+{n:04X}");
        }
        for n in [
            0x00AC, 0x00AE, 0x034E, 0x0350, 0x05FF, 0x0606, 0x061B, 0x061D, 0x06DC, 0x06DE, 0x070E,
            0x0710, 0x088F, 0x0892, 0x08E1, 0x08E3, 0x115E, 0x1161, 0x17B3, 0x17B6, 0x180A, 0x1810,
            0x200A, 0x2010, 0x2027, 0x202F, 0x205F, 0x2070, 0x3163, 0x3165, 0xFDFF, 0xFE10, 0xFEFE,
            0xFF00, 0xFF9F, 0xFFA1, 0xFFEF, 0xFFFC, 0x110BC, 0x110BE, 0x110CC, 0x110CE, 0x1342F,
            0x13440, 0x1BC9F, 0x1BCA4, 0x1D172, 0x1D17B, 0xDFFFD, 0xE1000,
        ] {
            // Below U+E0000, U+DFFFE and U+DFFFF are noncharacters: U+DFFFD
            // stands for the tags' neighbour.
            let c = char::from_u32(n).unwrap();
            assert!(!is_refused_in_name(c), "U+{n:04X}");
            assert!(open_type(&format!("a{c}")).is_ok(), "U+{n:04X}");
        }
        // Named ones: the soft hyphen, the combining grapheme joiner, the
        // Arabic letter mark, the zero-width space, the left-to-right mark,
        // the line and paragraph separators, the right-to-left override, the
        // left-to-right isolate, the byte order mark, an interlinear
        // annotation, an Egyptian hieroglyph joiner, a shorthand overlap, a
        // language tag and two variation selectors.
        for c in [
            '\u{00AD}',
            '\u{034F}',
            '\u{061C}',
            '\u{200B}',
            '\u{200E}',
            '\u{2028}',
            '\u{2029}',
            '\u{202E}',
            '\u{2066}',
            '\u{FEFF}',
            '\u{FFF9}',
            '\u{13430}',
            '\u{1BCA0}',
            '\u{E0001}',
            '\u{FE0F}',
            '\u{E0100}',
        ] {
            assert!(is_refused_in_name(c), "U+{:04X}", c as u32);
        }
        // The no-break space, the hyphen, the replacement character and an
        // emoji show as themselves.
        for c in ['\u{00A0}', '\u{2010}', '\u{FFFD}', '\u{1F600}'] {
            assert!(!is_refused_in_name(c), "U+{:04X}", c as u32);
            assert!(open_type(&format!("a{c}")).is_ok(), "U+{:04X}", c as u32);
        }
    }

    /// Private use characters, whose look a font chooses, and the blank
    /// braille pattern, drawn as a blank: the first and the last of each
    /// range refused; the assigned characters around the blank pattern and
    /// the Private Use Area kept (the planes' neighbours are unassigned).
    #[test]
    fn private_use_and_blank_braille_are_refused() {
        for n in [
            0x2800, 0xE000, 0xF000, 0xF8FF, 0xF0000, 0xFFFFD, 0x100000, 0x10FFFD,
        ] {
            let c = char::from_u32(n).unwrap();
            assert!(is_refused_in_name(c), "U+{n:04X}");
            assert!(open_type(&format!("a{c}")).is_err(), "U+{n:04X}");
        }
        // A long squiggle arrow, the braille pattern of dot 1, the last
        // Hangul jamo before the surrogates, a CJK compatibility ideograph.
        for n in [0x27FF, 0x2801, 0xD7FB, 0xF900] {
            let c = char::from_u32(n).unwrap();
            assert!(!is_refused_in_name(c), "U+{n:04X}");
            assert!(open_type(&format!("a{c}")).is_ok(), "U+{n:04X}");
        }
    }

    /// The 66 noncharacters, counted apart from `is_noncharacter`, are
    /// refused; the characters on either side of U+FDD0–U+FDEF and below
    /// U+FFFE kept.
    #[test]
    fn noncharacters_are_refused() {
        for n in [0xFDD0, 0xFDEF, 0xFFFE, 0xFFFF, 0x1FFFE, 0x10FFFF] {
            let c = char::from_u32(n).unwrap();
            assert!(is_refused_in_name(c), "U+{n:04X}");
            assert!(open_type(&format!("a{c}")).is_err(), "U+{n:04X}");
        }
        let all: Vec<u32> = (0xFDD0..=0xFDEF)
            .chain((0..=0x10).flat_map(|plane| [plane << 16 | 0xFFFE, plane << 16 | 0xFFFF]))
            .collect();
        assert_eq!(all.len(), 66);
        assert!(
            all.iter()
                .all(|&n| is_noncharacter(char::from_u32(n).unwrap()))
        );
        let refused = (0..=0x10FFFF)
            .filter_map(char::from_u32)
            .filter(|&c| is_noncharacter(c))
            .count();
        assert_eq!(refused, 66);
        for n in [0xFDCF, 0xFDF0, 0xFFFD] {
            let c = char::from_u32(n).unwrap();
            assert!(!is_refused_in_name(c), "U+{n:04X}");
            assert!(open_type(&format!("a{c}")).is_ok(), "U+{n:04X}");
        }
    }

    /// Every character dx may write in a name of a path it links is taken:
    /// this mirrors dx's own list (CC-Plugins, `plugins/dx/hooks/paths/
    /// paths-model.ts`, `WRITTEN`), the visible Unicode letters, combining
    /// marks and decimal digits and the signs `. _ - + ~ @ ( ) '`, and the
    /// space where dx reads a delimited path (`WRITTEN_SPACED`), by a sample
    /// of each. Should dx's list grow, this sample grows with it.
    #[test]
    fn every_character_dx_may_link_is_taken() {
        let sample = [
            // ASCII letters and digits, then each sign.
            "azAZ09",
            "._-+~@()'",
            // The space, in a delimited path.
            "a b",
            // Accented Latin, sharp s, Cyrillic, Greek, CJK, Arabic.
            "éÀçÿ",
            "ß",
            "Жж",
            "λΩ",
            "漢字",
            "عرب",
            // A Devanagari letter and digit, a fullwidth digit.
            "क५",
            "５",
            // A combining acute accent after a letter.
            "e\u{0301}",
        ];
        for text in sample {
            for c in text.chars() {
                assert!(!is_refused_in_name(c), "U+{:04X}", u32::from(c));
            }
        }
    }

    #[test]
    fn a_type_is_said_with_its_dot() {
        assert_eq!(type_label(r"setup.EXE"), "type .exe");
        assert_eq!(type_label(".env"), "type .env");
        assert_eq!(type_label("Makefile"), "sans type");
        assert_eq!(type_label("a."), "sans type");
    }

    #[test]
    fn a_wrong_type_is_left_out_the_rest_kept() {
        let loaded = Settings::parse(
            "file = \"application\"\nopen_types = [\"md\", \"tar.gz\", \"a/b\", \"\"]",
        )
        .unwrap();
        assert_eq!(loaded.settings.open_types, ["md"]);
        assert_eq!(loaded.settings.file, FileAction::Application);
        assert_eq!(loaded.skipped.len(), 3, "{:?}", loaded.skipped);
        assert!(loaded.skipped[0].starts_with("open_types : « tar.gz »"));
    }

    #[test]
    fn a_type_is_what_follows_the_last_dot() {
        assert_eq!(file_type("a.tar.GZ"), Some("gz".into()));
        assert_eq!(file_type("Makefile"), None);
        // A name that starts with its dot has a type: Windows opens `.bat`
        // as a batch file.
        assert_eq!(file_type(".env"), Some("env".into()));
        assert_eq!(file_type(".BAT"), Some("bat".into()));
        assert_eq!(file_type(".gitignore"), Some("gitignore".into()));
        assert_eq!(file_type("a."), None);
        assert_eq!(file_type("x.env"), Some("env".into()));
    }

    #[test]
    fn every_default_type_is_one_the_file_can_hold() {
        for t in OPEN_TYPES {
            assert_eq!(open_type(t).as_deref(), Ok(*t));
        }
    }

    /// A PDF stays a default type, its script said in the docs; an EPUB is
    /// not one.
    #[test]
    fn the_defaults_keep_pdf_and_leave_epub_out() {
        assert!(OPEN_TYPES.contains(&"pdf"));
        assert!(!OPEN_TYPES.contains(&"epub"));
    }

    #[test]
    fn a_file_that_cannot_be_used_says_why() {
        let dir = crate::TempDir::new("api-links").unwrap();
        let load = || Settings::load(Ok(dir.to_path_buf()));
        assert_eq!(load(), Ok(Loaded::default()));
        std::fs::write(dir.join(SETTINGS_FILE), "unknown = 1\n").unwrap();
        let error = load().unwrap_err();
        assert!(error.starts_with("links.toml ignoré"), "{error}");
        // Without its folder, the file the user may have written is not
        // found: no defaults.
        let unknown = Settings::load(Err("HERDR_PLUGIN_CONFIG_DIR absent".into()));
        assert_eq!(
            unknown,
            Err("links.toml introuvable : HERDR_PLUGIN_CONFIG_DIR absent".into())
        );
    }

    /// Uncommented, the template gives the defaults, and it has a line for
    /// every setting.
    #[test]
    fn the_template_gives_every_setting_its_default() {
        let lines: Vec<&str> = TEMPLATE
            .lines()
            .filter_map(|line| line.strip_prefix("# "))
            .filter(|line| line.contains(" = "))
            .collect();
        let uncommented: String = lines.iter().map(|line| format!("{line}\n")).collect();
        assert_eq!(parse(&uncommented).unwrap(), Settings::default());
        let mut in_template: Vec<&str> = lines
            .iter()
            .filter_map(|line| line.split(" = ").next())
            .collect();
        in_template.sort_unstable();
        let defaults = serde_json::to_value(Settings::default()).unwrap();
        let mut settings: Vec<&str> = defaults
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        settings.sort_unstable();
        assert_eq!(in_template, settings);
    }
}
