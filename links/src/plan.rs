//! What a click does, from the settings and the target checked on disk:
//! decided here without any I/O, run by `open`.

use crate::target::{Kind, Target};
use herdr_api::links::{FileAction, Settings};

/// What a click does: the preview first, or something at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Shown in herdr-explorer's preview; when it is missing or fails,
    /// `then`.
    Preview {
        then: Act,
    },
    Now(Act),
}

/// What is done with a target, other than the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Selected in the File Explorer, in its parent folder.
    Select,
    /// Opened with the program Windows associates with it.
    Open,
    /// The confirmation popup, which opens it on Enter.
    Confirm,
    /// Selected rather than opened: its type is not one the default
    /// application may open. Said in a notification.
    SelectNotAllowed,
}

/// What a click on `target` does. A folder is always selected: opening it
/// would run a program put in its place, and Explorer would show it anyway.
pub fn plan(settings: &Settings, target: &Target) -> Step {
    if target.kind == Kind::Folder {
        return Step::Now(Act::Select);
    }
    match settings.file {
        FileAction::Select => Step::Now(Act::Select),
        FileAction::Preview => Step::Preview {
            then: with_application(settings, target),
        },
        FileAction::Application => Step::Now(with_application(settings, target)),
    }
}

/// Opening `target` with its program: at once for a type of `open_types`;
/// for another one, selected, unless `run_programs` allows it, confirmed
/// first with `confirm_programs`.
fn with_application(settings: &Settings, target: &Target) -> Act {
    if settings.opens(target.name()) {
        Act::Open
    } else if !settings.run_programs {
        Act::SelectNotAllowed
    } else if settings.confirm_programs {
        Act::Confirm
    } else {
        Act::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> Target {
        Target {
            path: format!(r"C:\repo\{name}"),
            kind: Kind::File,
        }
    }

    fn with(file: FileAction, run_programs: bool, confirm_programs: bool) -> Settings {
        Settings {
            file,
            run_programs,
            confirm_programs,
            ..Settings::default()
        }
    }

    #[test]
    fn a_folder_is_always_selected() {
        let folder = Target {
            path: r"C:\repo".into(),
            kind: Kind::Folder,
        };
        for action in FileAction::ALL {
            for run in [false, true] {
                let settings = with(action, run, false);
                assert_eq!(plan(&settings, &folder), Step::Now(Act::Select));
            }
        }
    }

    #[test]
    fn by_default_a_file_is_previewed() {
        let settings = Settings::default();
        assert_eq!(
            plan(&settings, &file("a.rs")),
            Step::Preview { then: Act::Open }
        );
        // A program is previewed too: the preview runs nothing; should it
        // fail, the program is only selected.
        assert_eq!(
            plan(&settings, &file("setup.exe")),
            Step::Preview {
                then: Act::SelectNotAllowed
            }
        );
    }

    #[test]
    fn select_never_opens() {
        let settings = with(FileAction::Select, true, false);
        assert_eq!(plan(&settings, &file("a.md")), Step::Now(Act::Select));
        assert_eq!(plan(&settings, &file("a.exe")), Step::Now(Act::Select));
    }

    #[test]
    fn a_type_outside_the_list_runs_only_in_danger_and_after_a_confirmation() {
        let app =
            |run, confirm, name| plan(&with(FileAction::Application, run, confirm), &file(name));
        assert_eq!(app(false, true, "a.txt"), Step::Now(Act::Open));
        assert_eq!(app(false, true, "a.exe"), Step::Now(Act::SelectNotAllowed));
        assert_eq!(app(false, false, "a.exe"), Step::Now(Act::SelectNotAllowed));
        assert_eq!(app(true, true, "a.exe"), Step::Now(Act::Confirm));
        assert_eq!(app(true, false, "a.exe"), Step::Now(Act::Open));
        // A type of the list never asks.
        assert_eq!(app(true, true, "a.txt"), Step::Now(Act::Open));
        // A name without a type is outside the list: selected, or with
        // `run_programs` confirmed first, as a program is.
        assert_eq!(
            app(false, true, "Makefile"),
            Step::Now(Act::SelectNotAllowed)
        );
        assert_eq!(app(true, true, "Makefile"), Step::Now(Act::Confirm));
    }

    #[test]
    fn the_list_is_the_user_s() {
        let mut settings = with(FileAction::Application, false, true);
        settings.open_types = vec!["html".into()];
        assert_eq!(plan(&settings, &file("index.HTML")), Step::Now(Act::Open));
        assert_eq!(
            plan(&settings, &file("a.txt")),
            Step::Now(Act::SelectNotAllowed)
        );
    }
}
