//! Herdr plugin `imsike.links`: opens the path links the Claude Code plugin
//! dx draws (`dx-open:///C:/…#L12-L14`) on a ctrl+click.
//!
//! 1. `open` (the action of the `dx-open` link handler) reads the clicked URL
//!    (`url`), checks its path on disk (`target`), and decides from
//!    `links.toml` what to do (`plan`): select it in the File Explorer,
//!    preview it in herdr-explorer, open it with its program, or ask first.
//! 2. `confirm` (a popup `open` opens when a file outside `open_types` may
//!    run, `run_programs` and `confirm_programs` on) shows its real path and
//!    opens it on Enter (`confirm`).
//!
//! Windows only: elsewhere, both say so and do nothing.

mod confirm;
mod open;
mod plan;
mod target;
mod url;

use herdr_api::links::{Loaded, SETTINGS_FILE, Settings};
use plan::{Act, Step};
use std::process::ExitCode;

/// The clicked URL Herdr gives a link handler's action.
const CLICKED_URL_ENV: &str = "HERDR_PLUGIN_CLICKED_URL";
/// The notifications' title.
const NOT_OPENED: &str = "Lien non ouvert";
/// The title of the notification of a preview that did not answer in time.
const SLOW_PREVIEW: &str = "Aperçu lent";

fn main() -> ExitCode {
    let result = if !cfg!(windows) {
        Err("Windows seulement".into())
    } else {
        match std::env::args().nth(1).as_deref() {
            Some("open") => open(),
            Some("confirm") => confirm::run(),
            _ => Err("usage : herdr-links open|confirm".into()),
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("herdr-links : {message}");
            ExitCode::FAILURE
        }
    }
}

/// The settings a click goes by, and why `links.toml` is set aside when it
/// is (see `settings_from`).
pub(crate) fn settings() -> (Settings, Option<String>) {
    settings_from(Settings::load(herdr_api::config_dir()))
}

/// The settings read, a wrong type of `open_types` left out and logged. A
/// file that cannot be used at all (a syntax error, an unknown setting), or
/// found (Herdr gave no config folder), never falls back to defaults that
/// would open more than the user chose: files are only selected, nothing
/// runs, and the reason is returned for a notification.
fn settings_from(loaded: Result<Loaded, String>) -> (Settings, Option<String>) {
    match loaded {
        Ok(loaded) => {
            for skipped in &loaded.skipped {
                eprintln!("herdr-links : {SETTINGS_FILE} : {skipped} (type laissé de côté)");
            }
            (loaded.settings, None)
        }
        Err(e) => {
            eprintln!("herdr-links : {e}");
            (Settings::set_aside(), Some(e))
        }
    }
}

/// What a click says when `links.toml` is set aside (`why`): a file is only
/// selected; a folder is selected as always, so the notice speaks of files.
fn set_aside_notice(why: &str, kind: target::Kind) -> String {
    match kind {
        target::Kind::File => format!("{why} : le fichier est seulement sélectionné"),
        target::Kind::Folder => format!("{why} : les fichiers sont seulement sélectionnés"),
    }
}

/// What a click says when herdr-explorer did not answer in time about the
/// file `name`: the preview may still show, and nothing else is opened.
fn slow_notice(name: &str) -> String {
    format!(
        "{name} : herdr-explorer n'a pas répondu en {} s ; l'aperçu peut encore s'afficher, rien d'autre n'est ouvert",
        open::PEEK_TIMEOUT.as_secs()
    )
}

/// `herdr-links open`: the click.
fn open() -> Result<(), String> {
    let url = herdr_api::var(CLICKED_URL_ENV)?;
    let link = match url::parse(&url) {
        Ok(link) => link,
        Err(refused) => {
            open::notify(NOT_OPENED, &format!("Lien refusé : {}", refused.reason()));
            return Ok(());
        }
    };
    let target = match target::resolve(&link.path) {
        Ok(target) => target,
        Err(refused) => {
            open::notify(NOT_OPENED, &format!("{} : {}", link.path, refused.reason()));
            return Ok(());
        }
    };
    let (settings, unusable) = settings();
    if let Some(why) = unusable {
        open::notify(
            "Réglages des liens ignorés",
            &set_aside_notice(&why, target.kind),
        );
    }
    let act = match plan::plan(&settings, &target) {
        Step::Now(act) => act,
        Step::Preview { then } => match open::find_explorer() {
            None => then,
            Some((exe, root)) => match open::peek(&exe, &root, &target.path, link.lines) {
                Ok(()) => return Ok(()),
                Err(open::NotPeeked::Failed(e)) => {
                    eprintln!("herdr-links : {e}");
                    then
                }
                // The preview may still come: nothing else is opened.
                Err(open::NotPeeked::Slow) => {
                    open::notify(SLOW_PREVIEW, &slow_notice(target.name()));
                    return Ok(());
                }
            },
        },
    };
    // The File Explorer or the popup that did not start: said, as a
    // refusal is, and logged.
    if let Err(e) = run(act, &target) {
        open::notify(NOT_OPENED, &e);
    }
    Ok(())
}

/// Does what a click comes to, the preview aside.
fn run(act: Act, target: &target::Target) -> Result<(), String> {
    match act {
        Act::Select => open::explorer(&open::select_arg(&target.path)),
        Act::Open => open::explorer(&open::open_arg(&target.path)),
        Act::Confirm => open::confirm(&target.path),
        Act::SelectNotAllowed => {
            let kind = herdr_api::links::type_label(target.name());
            open::notify(
                NOT_OPENED,
                &format!(
                    "{} : {kind} hors de la liste des types ouverts, sélectionné dans l'Explorateur",
                    target.name()
                ),
            );
            open::explorer(&open::select_arg(&target.path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use herdr_api::links::FileAction;

    #[test]
    fn an_unusable_file_only_selects() {
        let (settings, why) = settings_from(Err("links.toml ignoré : x".into()));
        assert_eq!(settings.file, FileAction::Select);
        assert!(!settings.run_programs);
        assert_eq!(why.as_deref(), Some("links.toml ignoré : x"));
        let chosen = Settings {
            file: FileAction::Application,
            ..Settings::default()
        };
        let loaded = Loaded {
            settings: chosen.clone(),
            skipped: vec!["open_types : type vide".into()],
        };
        assert_eq!(settings_from(Ok(loaded)), (chosen, None));
    }

    /// Without its config folder, the file the user may have written is not
    /// found: as with a file that cannot be used, files are only selected,
    /// and the notification says why.
    #[test]
    fn no_config_folder_only_selects() {
        let missing = Settings::load(Err("HERDR_PLUGIN_CONFIG_DIR absent".into()));
        let (settings, why) = settings_from(missing);
        assert_eq!(settings, Settings::set_aside());
        assert_eq!(
            why.as_deref(),
            Some("links.toml introuvable : HERDR_PLUGIN_CONFIG_DIR absent")
        );
    }

    #[test]
    fn a_folder_is_not_said_to_be_a_file() {
        let file = set_aside_notice("x", target::Kind::File);
        assert_eq!(file, "x : le fichier est seulement sélectionné");
        let folder = set_aside_notice("x", target::Kind::Folder);
        assert_eq!(folder, "x : les fichiers sont seulement sélectionnés");
    }

    #[test]
    fn a_slow_preview_says_nothing_else_opens() {
        assert_eq!(
            slow_notice("a.rs"),
            "a.rs : herdr-explorer n'a pas répondu en 3 s ; l'aperçu peut encore s'afficher, rien d'autre n'est ouvert"
        );
    }

    /// The manifest routes dx's links to the `open` action, and its id is
    /// the one the menu and dx look for.
    #[test]
    fn the_manifest_handles_dx_s_links() {
        let manifest: toml::Table = include_str!("../herdr-plugin.toml").parse().unwrap();
        assert_eq!(manifest["id"].as_str(), Some(herdr_api::links::ID));
        let handlers = manifest["link_handlers"].as_array().unwrap();
        let handler = handlers
            .iter()
            .find(|h| h["id"].as_str() == Some("dx-open"))
            .unwrap();
        let pattern = handler["pattern"].as_str().unwrap();
        assert_eq!(pattern, "^dx-open:");
        assert!(crate::url::PREFIX.starts_with(pattern.trim_start_matches('^')));
        let action = handler["action"].as_str().unwrap();
        let actions = manifest["actions"].as_array().unwrap();
        assert!(actions.iter().any(|a| a["id"].as_str() == Some(action)));
        let panes = manifest["panes"].as_array().unwrap();
        assert!(panes.iter().any(|p| p["id"].as_str() == Some("confirm")));
    }
}
