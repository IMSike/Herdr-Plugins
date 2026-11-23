//! The links plugin's part of the settings screen: its settings, written to
//! `links.toml` as soon as they change.

use super::{State, files, store, written};
use herdr_api::links::{self, FileAction};
use toml_edit::{Array, Value};

/// Why nothing is written to a `links.toml` the links plugin sets aside.
const SET_ASIDE: &str =
    "links.toml mis de côté : corrigez d'abord le fichier (Ouvrir les fichiers de configuration…)";
/// Why the open types are not emptied but by `*`.
const EMPTY_ON_PURPOSE: &str = "liste vide refusée : pour qu'aucun fichier ne s'ouvre avec son application (sauf avec run_programs), « * » dans « Retirer un type… »";

/// Where `action` is among the values of « Clic sur un fichier », as the
/// screen shows them (`FileAction::ALL`).
pub(super) fn file_index(action: FileAction) -> usize {
    FileAction::ALL
        .iter()
        .position(|&a| a == action)
        .unwrap_or(0)
}

/// How « Clic sur un fichier » names `action`.
pub(super) fn file_label(action: FileAction) -> &'static str {
    match action {
        FileAction::Preview => "aperçu",
        FileAction::Select => "sélection",
        FileAction::Application => "application",
    }
}

/// The open types the links plugin goes by when `links.toml` sets none, as
/// a list to write.
fn default_types() -> Array {
    links::Settings::default()
        .open_types
        .iter()
        .map(String::as_str)
        .collect()
}

impl State {
    /// Writes the keys `change` gives, in that order and at once, from the
    /// settings the file holds now (see `write_links`). `None` when the
    /// links plugin is not installed.
    fn set_in_links(
        &mut self,
        change: impl FnOnce(&links::Settings) -> Vec<(&'static str, Value)>,
    ) -> Option<Result<String, String>> {
        let saved = self.write_links(|now, path| {
            store::set_all(path, files::LINKS_TEMPLATE, change(now)).map(|()| true)
        });
        saved.map(|result| result.map(Option::unwrap_or_default))
    }

    /// Writes `links.toml` as `write` does, the one way the screen writes
    /// it: on the file read again first (see `with_links_file`), `write`
    /// getting the settings it holds now, a value changed by hand since the
    /// screen opened included, and saying whether it wrote anything. The
    /// screen then shows the file as the links plugin reads it, written or
    /// not. `Ok(None)` when nothing was written; `None` when the links
    /// plugin is not installed.
    fn write_links(
        &mut self,
        write: impl FnOnce(&links::Settings, &std::path::Path) -> Result<bool, String>,
    ) -> Option<Result<Option<String>, String>> {
        self.with_links_file(|state, path| {
            let now = state.links.clone().unwrap_or_default();
            let wrote = write(&now, path);
            let ignored = state.reread_links(path);
            match wrote? {
                true => written(ignored, &[links::SETTINGS_FILE]).map(Some),
                false => Ok(None),
            }
        })
    }

    /// Runs `change` on `links.toml`, read again first: a file set aside
    /// now is not written, one fixed since the screen opened is. `None` when
    /// the links plugin is not installed.
    fn with_links_file<T>(
        &mut self,
        change: impl FnOnce(&mut Self, &std::path::Path) -> Result<T, String>,
    ) -> Option<Result<T, String>> {
        self.links.as_ref()?;
        let path = self.files.links.clone()?;
        self.reread_links(&path);
        if self.links_set_aside {
            return Some(Err(SET_ASIDE.into()));
        }
        Some(change(self, &path))
    }

    /// Turns `run_programs` over, as the file holds it now. Turned on, it
    /// turns `confirm_programs` on first, each time: letting programs run
    /// always starts with confirming them, whatever was chosen before.
    pub(super) fn switch_run_programs(&mut self) -> Option<Result<String, String>> {
        self.set_in_links(|l| {
            let run = !l.run_programs;
            let mut keys = Vec::new();
            if run {
                keys.push(("confirm_programs", true.into()));
            }
            keys.push(("run_programs", run.into()));
            keys
        })
    }

    /// Turns a yes-or-no setting of `links.toml` over, as the file holds it
    /// now.
    pub(super) fn toggle_links(
        &mut self,
        key: &'static str,
        field: fn(&links::Settings) -> bool,
    ) -> Option<Result<String, String>> {
        self.set_in_links(|l| vec![(key, (!field(l)).into())])
    }

    /// Sets `file` to the value after the one the file holds now
    /// (`forward`), or the one before, round the values.
    pub(super) fn step_link_file(&mut self, forward: bool) -> Option<Result<String, String>> {
        self.set_in_links(|l| {
            let at = file_index(l.file);
            let count = FileAction::ALL.len();
            let next = if forward { at + 1 } else { at + count - 1 };
            let file = FileAction::ALL[next % count];
            vec![("file", herdr_api::name_of(&file).into())]
        })
    }

    /// Starts typing the open types, from the list the file holds now;
    /// `None`, nothing to show yet.
    pub(super) fn type_open_types(&mut self) -> Option<Result<String, String>> {
        if let Err(e) = self.with_links_file(|_, _| Ok(()))? {
            return Some(Err(e));
        }
        self.typing = Some(self.links.as_ref()?.open_types.join(", "));
        None
    }

    /// Starts typing one type to add or remove, from nothing; `None`,
    /// nothing to show yet.
    pub(super) fn type_one_type(&mut self) -> Option<Result<String, String>> {
        if let Err(e) = self.with_links_file(|_, _| Ok(()))? {
            return Some(Err(e));
        }
        self.typing = Some(String::new());
        None
    }

    /// Types typed with commas or spaces between them, with or without their
    /// dot. An empty list is only set on purpose, by `*` in « Retirer un
    /// type… ».
    pub(super) fn save_open_types(&mut self, text: &str) -> Result<String, String> {
        let mut types: Vec<String> = Vec::new();
        for written in text.split([',', ' ']).filter(|t| !t.trim().is_empty()) {
            let t = links::open_type(written)?;
            if !types.contains(&t) {
                types.push(t);
            }
        }
        if types.is_empty() {
            return Err(EMPTY_ON_PURPOSE.into());
        }
        self.write_types(types).unwrap_or_else(|| Ok(String::new()))
    }

    /// Adds one type, written with or without its dot, to the list the file
    /// holds now: a type written there by hand since the screen opened stays.
    pub(super) fn add_type(&mut self, text: &str) -> Result<String, String> {
        let t = links::open_type(text)?;
        match self.edit_types(|array| Ok(add_to(array, &t))) {
            None => Ok(String::new()),
            Some(Ok(None)) => Ok(format!("{t} est déjà dans la liste")),
            Some(result) => {
                result.map(|saved| format!("{t} ajouté, {}", saved.unwrap_or_default()))
            }
        }
    }

    /// Removes one type from the list the file holds now; `*` empties the
    /// list, said as the warning it is. Removing the last one is refused:
    /// emptying it is only done with `*`.
    pub(super) fn remove_type(&mut self, text: &str) -> Result<String, String> {
        if self.links.is_none() {
            return Ok(String::new());
        }
        if text.trim() == "*" {
            self.write_types(Vec::new())
                .unwrap_or_else(|| Ok(String::new()))?;
            return Err(
                "liste vidée : plus aucun fichier ne s'ouvre avec son application, sauf avec run_programs (enregistré)"
                    .into(),
            );
        }
        let t = links::open_type(text)?;
        match self.edit_types(|array| remove_from(array, &t)) {
            None => Ok(String::new()),
            Some(result) => {
                result.map(|saved| format!("{t} retiré, {}", saved.unwrap_or_default()))
            }
        }
    }

    /// Changes `open_types` as `edit` does, on the list `links.toml` holds
    /// now, not the one read when the screen opened (see `write_links`).
    /// `Ok(None)` when `edit` changed nothing; `None` when the links plugin
    /// is not installed.
    fn edit_types(
        &mut self,
        edit: impl FnOnce(&mut Array) -> Result<bool, String>,
    ) -> Option<Result<Option<String>, String>> {
        self.write_links(|_, path| {
            store::edit_array(
                path,
                files::LINKS_TEMPLATE,
                "open_types",
                default_types(),
                edit,
            )
        })
    }

    /// Shows `links.toml` at `path` as the links plugin reads it now: set
    /// aside when it cannot use it, usable again once it can. Why it does
    /// not take the file as it is, if it does not: the file set aside, or
    /// types of `open_types` it leaves out.
    pub(super) fn reread_links(&mut self, path: &std::path::Path) -> Option<String> {
        let (settings, ignored, set_aside) = match links::Settings::read(path) {
            Ok(loaded) => (loaded.settings, skipped_types(&loaded.skipped), false),
            Err(e) => (links::Settings::set_aside(), Some(e), true),
        };
        self.links = Some(settings);
        self.links_set_aside = set_aside;
        ignored
    }

    /// Makes `open_types` hold `types`, the file's list changed in place
    /// (see `keep_only_types`).
    fn write_types(&mut self, types: Vec<String>) -> Option<Result<String, String>> {
        let saved = self.write_links(|_, path| {
            let unset = Array::new();
            store::edit_array(path, files::LINKS_TEMPLATE, "open_types", unset, |array| {
                keep_only_types(array, &types);
                Ok(true)
            })
        });
        saved.map(|result| result.map(Option::unwrap_or_default))
    }

    /// Writes the default types as `open_types`, the whole list replaced:
    /// what was written inside it, comments and types the plugin leaves out
    /// included, goes; what is written around it stays.
    pub(super) fn reset_types(&mut self) -> Option<Result<String, String>> {
        self.set_in_links(|_| vec![("open_types", default_types().into())])
    }
}

/// Makes `array`, `links.toml`'s `open_types`, hold `types`, in place: an
/// entry of a type no longer in `types` is removed, a type missing is added
/// at the end. The comments and the entries the links plugin does not take
/// (an empty or wrong type, which it leaves out and logs) stay as written.
fn keep_only_types(array: &mut Array, types: &[String]) {
    store::remove_where(array, |value| {
        type_of(value).is_some_and(|t| !types.contains(&t))
    });
    for t in types {
        if !array.iter().any(|value| type_of(value).as_ref() == Some(t)) {
            store::push_like_last(array, t.as_str());
        }
    }
}

/// The type an entry of `open_types` is, as the links plugin reads it;
/// `None` for one it leaves out.
fn type_of(value: &Value) -> Option<String> {
    value.as_str().and_then(|t| links::open_type(t).ok())
}

/// Adds `t` at the end of `array`, `open_types`, unless one of its entries
/// is that type already; whether it was added.
fn add_to(array: &mut Array, t: &str) -> bool {
    if array
        .iter()
        .any(|value| type_of(value).as_deref() == Some(t))
    {
        return false;
    }
    store::push_like_last(array, t);
    true
}

/// Removes the entries of `array`, `open_types`, that are `t`, the others
/// left as written. Refused when none is, or when `t` is the only type the
/// links plugin would keep: emptying the list is only done on purpose.
fn remove_from(array: &mut Array, t: &str) -> Result<bool, String> {
    let is_t = |value: &Value| type_of(value).as_deref() == Some(t);
    if !array.iter().any(is_t) {
        return Err(format!("{t} n'est pas dans la liste"));
    }
    if array.iter().filter_map(type_of).all(|other| other == t) {
        return Err(EMPTY_ON_PURPOSE.into());
    }
    store::remove_where(array, is_t);
    Ok(true)
}

/// The types `links.toml` holds that the links plugin leaves out, said.
fn skipped_types(skipped: &[String]) -> Option<String> {
    (!skipped.is_empty()).then(|| {
        format!(
            "{} : types laissés de côté : {}",
            links::SETTINGS_FILE,
            skipped.join(" ; ")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::super::Item;
    use super::super::tests::{rendered, state, state_in};
    use super::*;

    #[test]
    fn letting_programs_run_turns_their_confirmation_on() {
        let dir = herdr_api::TempDir::new("menu-run-programs").unwrap();
        let path = dir.join(links::SETTINGS_FILE);
        std::fs::write(&path, "confirm_programs = false\n").unwrap();
        let mut state = state_in(&dir);
        if let Some(l) = &mut state.links {
            l.confirm_programs = false;
        }
        let read = || links::Settings::read(&path).unwrap().settings;
        assert!(state.step(Item::RunPrograms, true).unwrap().is_ok());
        assert!(read().run_programs && read().confirm_programs);
        let shown = state.links.as_ref().unwrap();
        assert!(shown.run_programs && shown.confirm_programs);
        // Turned off on purpose, it stays off until programs are let run again.
        assert!(state.step(Item::ConfirmPrograms, true).unwrap().is_ok());
        assert!(state.step(Item::RunPrograms, true).unwrap().is_ok());
        assert!(!read().run_programs && !read().confirm_programs);
        assert!(state.step(Item::RunPrograms, true).unwrap().is_ok());
        assert!(read().run_programs && read().confirm_programs);
    }

    #[test]
    fn the_links_sections_show_when_the_plugin_is_installed() {
        let mut state = state();
        let text = rendered(&state, 92, 60);
        for expected in ["Liens", "Clic sur un fichier", "aperçu", "Danger"] {
            assert!(text.contains(expected), "{expected:?} missing:\n{text}");
        }
        // Confirming only means something once programs may run.
        assert!(!state.items().contains(&Item::ConfirmPrograms));
        if let Some(links) = &mut state.links {
            links.run_programs = true;
        }
        let items = state.items();
        let run = items.iter().position(|&i| i == Item::RunPrograms).unwrap();
        assert_eq!(items[run + 1], Item::ConfirmPrograms);
        state.links = None;
        let text = rendered(&state, 92, 60);
        assert!(
            !text.contains("Liens") && !text.contains("Danger"),
            "{text}"
        );
    }

    #[test]
    fn a_wrong_type_is_not_saved() {
        let dir = herdr_api::TempDir::new("menu-open-types").unwrap();
        let mut state = state_in(&dir);
        assert!(state.save_open_types("md, a/b").is_err());
        assert_eq!(
            state.links.as_ref().unwrap().open_types,
            links::Settings::default().open_types
        );
        assert!(!dir.join(links::SETTINGS_FILE).exists());
        let tar = state.add_type("tar.gz").unwrap_err();
        assert!(tar.contains("« gz »"), "{tar}");
        assert!(state.save_open_types(" , ").is_err());
    }

    #[test]
    fn types_are_typed_with_commas_or_spaces_between_them() {
        let dir = herdr_api::TempDir::new("menu-types-typed").unwrap();
        let mut state = state_in(&dir);
        let said = state.save_open_types(".MD, txt  rs,,MD");
        assert!(said.is_ok(), "{said:?}");
        let open_types = &state.links.as_ref().unwrap().open_types;
        assert_eq!(open_types, &["md", "txt", "rs"]);
    }

    #[test]
    fn types_are_added_and_removed_one_at_a_time() {
        let dir = herdr_api::TempDir::new("menu-one-type").unwrap();
        let mut state = state_in(&dir);
        let types = |s: &State| s.links.as_ref().unwrap().open_types.clone();
        let file = |dir: &std::path::Path| {
            links::Settings::read(&dir.join(links::SETTINGS_FILE))
                .unwrap()
                .settings
                .open_types
        };
        assert!(state.add_type(".!UT").unwrap().starts_with("!ut ajouté"));
        assert_eq!(types(&state).last().map(String::as_str), Some("!ut"));
        assert!(state.add_type("md").unwrap().contains("déjà"));
        assert!(state.remove_type("pdf").unwrap().starts_with("pdf retiré"));
        assert!(!file(&dir).contains(&"pdf".to_string()));
        assert!(state.remove_type("pdf").is_err());
        // Back to the list it ships with.
        assert!(state.step(Item::ResetTypes, true).unwrap().is_ok());
        assert_eq!(file(&dir), links::Settings::default().open_types);
    }

    #[test]
    fn a_type_written_by_hand_since_the_screen_opened_stays() {
        let dir = herdr_api::TempDir::new("menu-types-meanwhile").unwrap();
        let path = dir.join(links::SETTINGS_FILE);
        std::fs::write(&path, "open_types = [\"md\", \"log\"]\n").unwrap();
        let mut state = state_in(&dir);
        // Written in an editor while the screen is open.
        std::fs::write(&path, "open_types = [\"md\", \"log\", \"rs\"]\n").unwrap();
        assert!(state.add_type("pdf").unwrap().starts_with("pdf ajouté"));
        let file = || std::fs::read_to_string(&path).unwrap();
        assert_eq!(file(), "open_types = [\"md\", \"log\", \"rs\", \"pdf\"]\n");
        // The screen shows the file, the type added by hand included.
        let shown = |s: &State| s.links.as_ref().unwrap().open_types.clone();
        assert_eq!(shown(&state), ["md", "log", "rs", "pdf"]);
        std::fs::write(
            &path,
            "open_types = [\"md\", \"log\", \"rs\", \"pdf\", \"c\"]\n",
        )
        .unwrap();
        assert!(state.remove_type("log").unwrap().starts_with("log retiré"));
        assert_eq!(file(), "open_types = [\"md\", \"rs\", \"pdf\", \"c\"]\n");
        assert_eq!(shown(&state), ["md", "rs", "pdf", "c"]);
        // Already there, or removed by hand meanwhile: said, nothing written.
        std::fs::write(&path, "open_types = [\"md\", \"c\"]\n").unwrap();
        assert!(state.add_type("C").unwrap().contains("déjà"));
        assert_eq!(
            state.remove_type("rs").unwrap_err(),
            "rs n'est pas dans la liste"
        );
        assert_eq!(file(), "open_types = [\"md\", \"c\"]\n");
        assert_eq!(shown(&state), ["md", "c"]);
    }

    #[test]
    fn a_file_broken_since_the_screen_opened_is_not_written() {
        let dir = herdr_api::TempDir::new("menu-types-broken").unwrap();
        let path = dir.join(links::SETTINGS_FILE);
        let mut state = state_in(&dir);
        let broken = "open_types = [\"md\"]\nbogus = 1\n";
        std::fs::write(&path, broken).unwrap();
        let said = state.add_type("pdf").unwrap_err();
        assert!(said.starts_with("links.toml mis de côté"), "{said}");
        assert!(state.links_set_aside);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
    }

    /// Every other setting of `links.toml` too: read again before it is
    /// written, and a file set aside meanwhile left as it is.
    #[test]
    fn a_setting_is_not_written_to_a_file_broken_since_the_screen_opened() {
        for item in [
            Item::LinkFile,
            Item::RunPrograms,
            Item::ConfirmPrograms,
            Item::ResetTypes,
            Item::OpenTypes,
        ] {
            let dir = herdr_api::TempDir::new("menu-links-broken").unwrap();
            let path = dir.join(links::SETTINGS_FILE);
            let mut state = state_in(&dir);
            assert!(!state.links_set_aside);
            let broken = "file = \"select\"\nbogus = 1\n";
            std::fs::write(&path, broken).unwrap();
            let said = state.step(item, true).unwrap().unwrap_err();
            assert!(
                said.starts_with("links.toml mis de côté"),
                "{item:?}: {said}"
            );
            assert!(state.links_set_aside, "{item:?}");
            assert!(state.typing.is_none(), "{item:?}");
            assert_eq!(state.links, Some(links::Settings::set_aside()), "{item:?}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), broken, "{item:?}");
        }
        // A whole list typed, likewise.
        let dir = herdr_api::TempDir::new("menu-links-broken-list").unwrap();
        let path = dir.join(links::SETTINGS_FILE);
        let mut state = state_in(&dir);
        std::fs::write(&path, "bogus = 1\n").unwrap();
        assert!(state.save_open_types("md, pdf").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "bogus = 1\n");
    }

    /// A setting changed on the screen is written on the file as it is now:
    /// another one changed by hand meanwhile stays.
    #[test]
    fn a_setting_changed_by_hand_since_the_screen_opened_stays() {
        let dir = herdr_api::TempDir::new("menu-links-meanwhile").unwrap();
        let path = dir.join(links::SETTINGS_FILE);
        let mut state = state_in(&dir);
        std::fs::write(&path, "open_types = [\"md\"]\n").unwrap();
        assert!(state.step(Item::LinkFile, true).unwrap().is_ok());
        let read = links::Settings::read(&path).unwrap().settings;
        assert_eq!(read.file, FileAction::Select);
        assert_eq!(read.open_types, ["md"]);
        assert_eq!(state.links, Some(read));
    }

    /// The next value is the one after what the file holds now, not after
    /// what the screen showed: a value changed by hand meanwhile counts.
    #[test]
    fn a_setting_is_stepped_from_the_file_as_it_is_now() {
        let dir = herdr_api::TempDir::new("menu-links-step").unwrap();
        let path = dir.join(links::SETTINGS_FILE);
        let read = || links::Settings::read(&path).unwrap().settings;
        let mut state = state_in(&dir);
        // The screen shows `aperçu`; `application` was written by hand.
        std::fs::write(&path, "file = \"application\"\n").unwrap();
        assert!(state.step(Item::LinkFile, true).unwrap().is_ok());
        assert_eq!(read().file, FileAction::Preview);
        std::fs::write(&path, "file = \"select\"\n").unwrap();
        assert!(state.step(Item::LinkFile, false).unwrap().is_ok());
        assert_eq!(read().file, FileAction::Preview);
        // The screen shows `confirm_programs` on; turned off by hand.
        std::fs::write(&path, "confirm_programs = false\n").unwrap();
        assert!(state.step(Item::ConfirmPrograms, true).unwrap().is_ok());
        assert!(read().confirm_programs);
        // Programs let run by hand, confirmation off: turned off again,
        // the confirmation left as it was.
        std::fs::write(&path, "run_programs = true\nconfirm_programs = false\n").unwrap();
        assert!(!state.links.as_ref().unwrap().run_programs);
        assert!(state.step(Item::RunPrograms, true).unwrap().is_ok());
        assert!(!read().run_programs && !read().confirm_programs);
        assert_eq!(state.links, Some(read()));
    }

    /// « Types ouverts » starts from the list the file holds now, a type
    /// added by hand since the screen opened included.
    #[test]
    fn the_open_types_typed_start_from_the_file() {
        let dir = herdr_api::TempDir::new("menu-types-prefill").unwrap();
        let path = dir.join(links::SETTINGS_FILE);
        let mut state = state_in(&dir);
        std::fs::write(&path, "open_types = [\"md\", \"log\"]\n").unwrap();
        assert!(state.step(Item::OpenTypes, true).is_none());
        assert_eq!(state.typing.as_deref(), Some("md, log"));
    }

    #[test]
    fn one_type_is_added_or_removed_in_the_file_s_list() {
        let list = |text: &str| -> Array {
            let doc: toml_edit::DocumentMut = format!("t = {text}").parse().unwrap();
            doc["t"].as_array().unwrap().clone()
        };
        let mut array = list(r#"["md", "tar.gz", ".PDF"]"#);
        assert!(!add_to(&mut array, "pdf"));
        assert!(add_to(&mut array, "rs"));
        assert_eq!(
            array.to_string().trim(),
            r#"["md", "tar.gz", ".PDF", "rs"]"#
        );
        // Every entry of that type goes, the wrong one stays.
        let mut array = list(r#"["md", "tar.gz", ".PDF", "pdf"]"#);
        assert_eq!(remove_from(&mut array, "pdf"), Ok(true));
        assert_eq!(array.to_string().trim(), r#"["md", "tar.gz"]"#);
        assert!(remove_from(&mut array, "rs").is_err());
        // The last type the plugin would keep is not removed by mistake.
        let refused = remove_from(&mut array, "md").unwrap_err();
        assert!(refused.contains("« * »"), "{refused}");
        assert_eq!(array.to_string().trim(), r#"["md", "tar.gz"]"#);
    }

    #[test]
    fn a_hand_written_list_keeps_its_comments_and_entries() {
        let dir = herdr_api::TempDir::new("menu-hand-types").unwrap();
        let path = dir.join(links::SETTINGS_FILE);
        let text =
            "open_types = [\n    # Mine.\n    \"md\",\n    \"tar.gz\", # wrong\n    \"pdf\",\n]\n";
        std::fs::write(&path, text).unwrap();
        let mut state = state_in(&dir);
        if let Some(l) = &mut state.links {
            l.open_types = vec!["md".into(), "pdf".into()];
        }
        // Saved, but the wrong entry is still said.
        assert!(state.add_type("txt").unwrap_err().contains("tar.gz"));
        assert!(state.remove_type("md").is_err());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "open_types = [\n    # Mine.\n    \"tar.gz\", # wrong\n    \"pdf\",\n    \"txt\",\n]\n"
        );
        assert!(state.save_open_types("pdf, rs").is_err());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "open_types = [\n    # Mine.\n    \"tar.gz\", # wrong\n    \"pdf\",\n    \"rs\",\n]\n"
        );
        // Resetting replaces the whole list.
        assert!(state.step(Item::ResetTypes, true).unwrap().is_ok());
        let reset = std::fs::read_to_string(&path).unwrap();
        assert!(
            !reset.contains("Mine") && !reset.contains("tar.gz"),
            "{reset}"
        );
    }

    #[test]
    fn a_file_set_aside_is_not_written() {
        let dir = herdr_api::TempDir::new("menu-set-aside").unwrap();
        let path = dir.join(links::SETTINGS_FILE);
        let broken = "open_types = [\"md\", \"log\"]\nbogus = 1\n";
        std::fs::write(&path, broken).unwrap();
        let mut state = state_in(&dir);
        state.links = Some(links::Settings::set_aside());
        state.links_set_aside = true;
        for item in [Item::AddType, Item::RemoveType, Item::OpenTypes] {
            let said = state.step(item, true).unwrap();
            assert!(said.unwrap_err().starts_with("links.toml mis de côté"));
            assert!(state.typing.is_none(), "{item:?}");
        }
        for item in [Item::ResetTypes, Item::LinkFile, Item::RunPrograms] {
            assert!(state.step(item, true).unwrap().is_err(), "{item:?}");
        }
        assert!(state.add_type("txt").is_err() && state.remove_type("*").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
    }

    /// A file set aside, fixed in an editor while the screen is open, is
    /// written again from the next change, and shown as it is.
    #[test]
    fn a_file_fixed_since_the_screen_opened_is_written_again() {
        let dir = herdr_api::TempDir::new("menu-set-aside-fixed").unwrap();
        let path = dir.join(links::SETTINGS_FILE);
        std::fs::write(&path, "open_types = [\"md\"]\nbogus = 1\n").unwrap();
        let mut state = state_in(&dir);
        assert!(state.reread_links(&path).is_some());
        assert!(state.links_set_aside);
        assert!(state.add_type("pdf").is_err());
        std::fs::write(&path, "open_types = [\"md\"]\n").unwrap();
        let said = state.add_type("pdf");
        assert!(
            said.as_deref().is_ok_and(|s| s.starts_with("pdf ajouté")),
            "{said:?}"
        );
        assert!(!state.links_set_aside);
        assert_eq!(state.links.as_ref().unwrap().open_types, ["md", "pdf"]);
        assert!(state.step(Item::LinkFile, true).unwrap().is_ok());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "open_types = [\"md\", \"pdf\"]\nfile = \"select\"\n"
        );
    }

    #[test]
    fn an_empty_list_is_only_set_on_purpose() {
        let dir = herdr_api::TempDir::new("menu-empty-types").unwrap();
        let mut state = state_in(&dir);
        assert!(state.save_open_types("md").is_ok());
        // The last one cannot go by mistake.
        let refused = state.remove_type("md").unwrap_err();
        assert!(refused.contains("« * »"), "{refused}");
        assert_eq!(state.links.as_ref().unwrap().open_types, ["md"]);
        // `*` empties it, and says so as a warning.
        let warned = state.remove_type("*").unwrap_err();
        assert!(warned.starts_with("liste vidée"), "{warned}");
        let written = links::Settings::read(&dir.join(links::SETTINGS_FILE)).unwrap();
        assert!(written.settings.open_types.is_empty());
    }

    /// A list written one type per line, emptied by `*`, then given a type:
    /// written on the list's line, not before a line break of its own.
    #[test]
    fn a_type_added_after_star_is_written_on_the_list_s_line() {
        let dir = herdr_api::TempDir::new("menu-refill-types").unwrap();
        let path = dir.join(links::SETTINGS_FILE);
        std::fs::write(&path, "open_types = [\n    \"md\",\n    \"pdf\",\n]\n").unwrap();
        let mut state = state_in(&dir);
        assert!(state.remove_type("*").is_err());
        assert!(state.add_type("md").unwrap().starts_with("md ajouté"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "open_types = [\"md\"]\n"
        );
    }
}
