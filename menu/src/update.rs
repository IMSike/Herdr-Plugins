//! The `Update` entry. Herdr refuses `herdr update` inside one of its panes
//! (`HERDR_ENV`), so the entry opens a terminal window of its own, outside
//! Herdr, which runs `herdr-menu update`: what has a new version (see
//! `check`), ticked, to untick; the update of what stays ticked; then, when
//! the server must restart to run Herdr's new version, a confirmation that
//! says what stops.

pub mod check;

use crate::herdr;
use crate::window::{self, herdr_shown};
use herdr_api::Taken;
use serde_json::Value;

/// Held by the update that runs, in the plugin's state folder: a second
/// click opens no second update.
const LOCK: &str = "update.lock";
const ALREADY_RUNNING: &str = "une mise à jour est déjà en cours dans une autre fenêtre";

/// The update's lock: the update runs without it when there is none to take.
fn lock() -> Taken {
    herdr_api::state_dir().map_or(Taken::Unavailable, |dir| {
        herdr_api::Lock::try_take(&dir.join(LOCK))
    })
}

/// Opens the window that runs the update, and returns at once.
pub fn open_window() -> Result<(), String> {
    if matches!(lock(), Taken::Busy) {
        return Err(ALREADY_RUNNING.into());
    }
    window::open("update", &[], "lancez « herdr update » hors de Herdr")
}

/// Whether the server runs an older binary than the one installed.
fn restart_needed(status: &Value) -> bool {
    ["restart_needed", "server_binary_stale"]
        .iter()
        .any(|key| status["update"][key] == true || status["server"][key] == true)
}

/// `herdr-menu update`, in the window `open_window` opened. The window
/// closes with it, so it ends on a key press.
pub fn run() -> Result<(), String> {
    let result = match lock() {
        Taken::Busy => Err(ALREADY_RUNNING.to_string()),
        Taken::Held(lock) => update(Some(lock)),
        Taken::Unavailable => update(None),
    };
    let message = match &result {
        Ok(message) => message.clone(),
        Err(e) => format!("Erreur : {e}"),
    };
    crate::ui::pause(&format!("\n{message}"));
    result.map(|_| ())
}

/// The update, holding `lock`; what to tell once done.
fn update(lock: Option<herdr_api::Lock>) -> Result<String, String> {
    println!("Recherche des mises à jour…");
    let found = check::now_and_keep();
    let offered = offers(&found);
    if offered.is_empty() {
        return Ok("Herdr et les plugins installés depuis GitHub sont à jour.".into());
    }
    let labels: Vec<String> = offered.iter().map(|(_, label)| label.clone()).collect();
    let ticked = crate::ui::pick("Mises à jour", &labels, "mettre à jour")
        .map_err(|e| format!("liste impossible : {e}"))?;
    let chosen: Vec<&Offer> = offered
        .iter()
        .zip(ticked.unwrap_or_default())
        .filter(|(_, ticked)| *ticked)
        .map(|((offer, _), _)| offer)
        .collect();
    if chosen.is_empty() {
        return Ok("Rien n'a été mis à jour.".into());
    }
    let herdr = chosen.contains(&&Offer::Herdr);
    let plugins: Vec<String> = chosen
        .iter()
        .filter_map(|offer| match offer {
            Offer::Plugin(id) => Some(id.clone()),
            Offer::Herdr => None,
        })
        .collect();
    let installed = install(herdr, &plugins);
    // What is still new once done, even when a step failed midway.
    println!("\nVérification des versions…");
    check::now_and_keep();
    installed?;
    if !herdr {
        return Ok(
            "Plugins à jour : leurs panes et commandes prennent la nouvelle \
                   version dès leur prochain lancement."
                .into(),
        );
    }
    if !restart_needed(&check::status()?) {
        return Ok(
            "Herdr est à jour. Le client déjà ouvert garde son ancienne version : \
                   détachez-vous (prefix+q) puis relancez « herdr » pour passer à la nouvelle."
                .into(),
        );
    }
    println!(
        "\nLa nouvelle version demande de redémarrer le serveur Herdr.\n\
         Tous les panes se ferment : {}.",
        window::stopping()?
    );
    if !crate::ui::ask("Redémarrer maintenant ? [o/N] ", false) {
        return Ok(
            "Redémarrage annulé : la nouvelle version servira au prochain \
                   démarrage du serveur Herdr."
                .into(),
        );
    }
    // Started again as it was: a named session, by its name.
    let session = herdr::current_session().ok();
    herdr_shown(
        &session
            .as_ref()
            .map_or_else(|| vec!["server", "stop"], herdr::Session::stop_args),
    )?;
    if crate::ui::ask(
        "\nServeur arrêté. Relancer Herdr dans cette fenêtre ? [o/N] ",
        false,
    ) {
        // Herdr runs in this window for as long as it is used: the update
        // is over.
        drop(lock);
        herdr_shown(
            &session
                .as_ref()
                .map(herdr::Session::start_args)
                .unwrap_or_default(),
        )?;
        return Ok("Herdr s'est fermé.".into());
    }
    let start = session.map_or_else(|| "herdr".into(), |s| s.start_command());
    Ok(window::start_from_clipboard(&start))
}

/// Updates Herdr when `herdr`, then `plugins`: Herdr first, since a
/// plugin's new version may need Herdr's.
fn install(herdr: bool, plugins: &[String]) -> Result<(), String> {
    if herdr {
        // Where the server can hand its sessions over, nothing stops.
        let handoff = check::status()?["server"]["capabilities"]["live_handoff"] == true;
        println!("Mise à jour de Herdr…\n");
        if handoff {
            herdr_shown(&["update", "--handoff"])?;
        } else {
            herdr_shown(&["update"])?;
        }
    }
    if !plugins.is_empty() {
        println!("\nMise à jour des plugins…\n");
        let mut args = vec!["plugin", "update"];
        args.extend(plugins.iter().map(String::as_str));
        herdr_shown(&args)?;
    }
    Ok(())
}

/// Something the window offers to update.
#[derive(Debug, PartialEq, Eq)]
enum Offer {
    Herdr,
    /// A plugin installed from GitHub, by its id.
    Plugin(String),
}

/// What the window offers, and how it names each: Herdr also when its new
/// version is not known (the preview channel, no answer), since
/// `herdr update` finds it on its own.
fn offers(found: &check::Found) -> Vec<(Offer, String)> {
    let herdr = match &found.herdr {
        check::Herdr::Newer { installed, latest } => Some(format!("Herdr {installed} → {latest}")),
        check::Herdr::Unknown(why) => Some(format!("Herdr (nouvelle version inconnue : {why})")),
        check::Herdr::UpToDate => None,
    };
    herdr
        .map(|label| (Offer::Herdr, label))
        .into_iter()
        .chain(found.plugins.iter().map(|p| {
            (
                Offer::Plugin(p.id.clone()),
                format!("Plugin {} ({})", p.name, p.id),
            )
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn offers_herdr_unless_up_to_date_and_each_new_plugin() {
        let plugin = check::Plugin {
            id: "gcaudan.explorer".into(),
            name: "Explorer".into(),
        };
        let found = |herdr| check::Found {
            herdr,
            plugins: vec![plugin.clone()],
        };
        let newer = check::Herdr::Newer {
            installed: "0.9.5".into(),
            latest: "0.9.6".into(),
        };
        assert_eq!(
            offers(&found(newer)),
            [
                (Offer::Herdr, "Herdr 0.9.5 → 0.9.6".to_string()),
                (
                    Offer::Plugin("gcaudan.explorer".into()),
                    "Plugin Explorer (gcaudan.explorer)".to_string()
                ),
            ]
        );
        let unknown = offers(&found(check::Herdr::Unknown("canal preview".into())));
        assert_eq!(unknown[0].0, Offer::Herdr);
        assert_eq!(offers(&found(check::Herdr::UpToDate)).len(), 1);
    }

    #[test]
    fn reads_whether_a_restart_is_needed() {
        assert!(!restart_needed(
            &json!({"update": {"restart_needed": false, "server_binary_stale": false}})
        ));
        assert!(restart_needed(&json!({"update": {"restart_needed": true}})));
        assert!(restart_needed(
            &json!({"server": {"server_binary_stale": true}})
        ));
        assert!(!restart_needed(&json!({})));
    }
}
