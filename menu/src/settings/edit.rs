//! Changing a setting part by part: an element of a list or a threshold
//! chosen with the arrows, then typed or removed; a setting put back to its
//! default; the last change undone; a change that waits for Enter.

use super::{Confirm, Files, Item, Shown, State, TTL_MAX, config, levels_unit, store};
use herdr_api::links;
use herdr_api::sidebar::Settings;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::PathBuf;

/// What a threshold is, as the field typing it says.
const LEVEL_NAMES: [&str; 3] = ["jaune dès", "orange dès", "rouge dès"];

/// The settings files as they are at a moment: each file's bytes, `None`
/// for one that does not exist. A file that cannot be read is left out:
/// nothing would put it back.
#[derive(Debug, PartialEq)]
pub struct Snapshot(Vec<(PathBuf, Option<Vec<u8>>)>);

/// A change made to the settings files: what they were before it, and
/// after it.
pub struct Change {
    before: Snapshot,
    after: Snapshot,
}

impl State {
    /// Moves the part chosen among `count` (`forward` or back), stopping at
    /// the first and the last.
    pub(super) fn move_part(&mut self, forward: bool, count: usize) {
        let last = count.saturating_sub(1);
        self.part = if forward {
            (self.part + 1).min(last)
        } else {
            self.part.min(last).saturating_sub(1)
        };
    }

    /// The part of `item` chosen, within its parts.
    pub(super) fn part_of(&self, item: Item) -> usize {
        match self.shown(item) {
            Shown::List(list) => self.part.min(list.len()),
            Shown::Levels(values, _) => self.part.min(values.len().saturating_sub(1)),
            _ => 0,
        }
    }

    /// Starts typing `item`'s chosen part: from nothing for an element to
    /// add, from its value otherwise, which the first character replaces.
    pub(super) fn start_typing(&mut self, item: Item) {
        if item == Item::OpenTypes
            && let Some(Err(e)) = self.with_links_file(|_, _| Ok(()))
        {
            self.report(Err(e));
            return;
        }
        let part = self.part_of(item);
        let (text, fresh) = match self.shown(item) {
            Shown::List(list) => match list.get(part) {
                Some(element) => (element.clone(), true),
                None => (String::new(), false),
            },
            Shown::Levels(values, _) => (values.get(part).cloned().unwrap_or_default(), true),
            Shown::Number(_) => match &self.sidebar {
                Some(s) => (s.ttl_seconds.to_string(), true),
                None => return,
            },
            _ => return,
        };
        self.part = part;
        self.typing = Some(text);
        self.fresh = fresh;
    }

    /// What the field typing `item`'s chosen part says before the text.
    pub(super) fn typing_prompt(&self, item: Item) -> String {
        let part = self.part_of(item);
        match (item, self.shown(item)) {
            (_, Shown::List(list)) => match list.get(part) {
                Some(element) => format!("remplacer {element} par "),
                None => "ajouter ".into(),
            },
            (Item::Levels(which), _) => {
                format!("{} ({}) ", LEVEL_NAMES[part.min(2)], levels_unit(which))
            }
            (_, Shown::Number(_)) => "secondes ".into(),
            _ => String::new(),
        }
    }

    /// Saves what was typed for `item`'s chosen part.
    pub(super) fn save_typed(&mut self, item: Item, text: &str) {
        let part = self.part_of(item);
        let result = match (item, self.shown(item)) {
            (Item::Shells, Shown::List(mut list)) => {
                let typed = split_list(text);
                let adding = part >= list.len();
                let (count, nothing) = (list.len(), typed.is_empty());
                if adding {
                    list.extend(typed);
                } else {
                    list.splice(part..=part, typed);
                }
                // The sidebar takes a shell's name in any case: once each.
                let mut seen = HashSet::new();
                list.retain(|shell| seen.insert(shell.to_lowercase()));
                if adding && list.len() == count {
                    if !nothing {
                        self.report(Ok(format!("{} : déjà dans la liste", text.trim())));
                    }
                    return;
                }
                self.save_shells(list)
            }
            (Item::OpenTypes, Shown::List(list)) => match list.get(part) {
                Some(old) => self.replace_type(old, text),
                None => {
                    let mut said = Ok(String::new());
                    for t in split_list(text) {
                        said = self.add_type(&t);
                        if said.is_err() {
                            break;
                        }
                    }
                    said
                }
            },
            (Item::Levels(which), _) => {
                let Some(sidebar) = &self.sidebar else {
                    return;
                };
                let mut thresholds = *which.of(sidebar);
                threshold(text).and_then(|value| {
                    thresholds[part.min(2)] = value;
                    self.save_levels(which, thresholds)
                })
            }
            (Item::Ttl, _) => self.save_ttl(text),
            _ => return,
        };
        // Nothing typed, nothing changed: the last message stays.
        if !matches!(&result, Ok(said) if said.is_empty()) {
            self.report(result);
        }
        // The element added stays chosen: the next one is added after it.
        if let Shown::List(list) = self.shown(item) {
            self.part = self.part.min(list.len());
        }
    }

    /// `ttl_seconds` as typed: a whole number of seconds, 1 to 600.
    fn save_ttl(&mut self, text: &str) -> Result<String, String> {
        let value = text
            .trim()
            .trim_end_matches('s')
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|v| (1..=TTL_MAX).contains(v))
            .ok_or_else(|| format!("durée : un nombre entier de secondes, de 1 à {TTL_MAX}"))?;
        self.set_in_sidebar("ttl_seconds", (value as i64).into(), |s| {
            s.ttl_seconds = value
        })
        .unwrap_or_else(|| Ok(String::new()))
    }

    /// Removes the chosen element of the list `item` (Delete, its `✕`);
    /// the last open type only after Enter: emptying that list is done on
    /// purpose.
    pub(super) fn remove_part(&mut self, item: Item) {
        let part = self.part_of(item);
        let Shown::List(mut list) = self.shown(item) else {
            return;
        };
        let Some(element) = list.get(part).cloned() else {
            return;
        };
        let result = match item {
            Item::Shells => {
                list.remove(part);
                self.save_shells(list)
                    .map(|saved| format!("{element} retiré, {saved}"))
            }
            Item::OpenTypes if list.len() == 1 => {
                self.confirm = Some(Confirm::EmptyTypes);
                return;
            }
            Item::OpenTypes => self.remove_type(&element),
            _ => return,
        };
        self.report(result);
        if let Shown::List(list) = self.shown(item) {
            self.part = part.min(list.len());
        }
    }

    /// The screen as it would be with every setting at its default.
    fn defaults(&self) -> Self {
        let files = Files {
            menu: None,
            sidebar: None,
            links: None,
        };
        Self::new(
            files,
            config::Menu::default(),
            self.sidebar.as_ref().map(|_| Settings::default()),
            self.links.as_ref().map(|_| links::Settings::default()),
        )
    }

    /// Whether `item` holds its default value; an action has none.
    pub(super) fn is_default(&self, item: Item) -> bool {
        match (self.shown(item), self.defaults().shown(item)) {
            (Shown::Action, _) => true,
            // A number format's values are written in the current format.
            (
                Shown::Choice { current, .. },
                Shown::Choice {
                    current: default, ..
                },
            ) => current == default,
            (shown, default) => shown == default,
        }
    }

    /// Puts `item` back to its default value.
    pub(super) fn reset(&mut self, item: Item) {
        let result = match (item, self.defaults().shown(item)) {
            (_, Shown::Choice { current, .. }) => return self.choose(item, current),
            (Item::Ttl, _) => self.save_ttl(&Settings::default().ttl_seconds.to_string()),
            (Item::Shells, _) => self.save_shells(Settings::default().shells),
            (Item::OpenTypes, _) => match self.reset_types() {
                Some(result) => result.map(|saved| format!("types par défaut rétablis, {saved}")),
                None => return,
            },
            (Item::Levels(which), _) => self.save_levels(which, *which.of(&Settings::default())),
            _ => return,
        };
        self.report(result);
        self.part = 0;
    }

    /// The settings files as they are now.
    fn snapshot(&self) -> Snapshot {
        let paths = [&self.files.menu, &self.files.sidebar, &self.files.links];
        Snapshot(
            paths
                .into_iter()
                .flatten()
                .filter_map(|path| match std::fs::read(path) {
                    Ok(bytes) => Some((path.clone(), Some(bytes))),
                    Err(e) if e.kind() == ErrorKind::NotFound => Some((path.clone(), None)),
                    Err(_) => None,
                })
                .collect(),
        )
    }

    /// Runs `change`; when it changed a file, what the files were before
    /// it becomes the change to undo.
    pub(super) fn tracked<T>(&mut self, change: impl FnOnce(&mut Self) -> T) -> T {
        let before = self.snapshot();
        let result = change(self);
        let after = self.snapshot();
        if after != before {
            self.undo = Some(Change { before, after });
            self.undone = false;
        }
        result
    }

    /// Puts the files the last change wrote back as they were, and shows
    /// them; done again, it redoes the change. A file changed since, by
    /// hand or by a plugin, is not overwritten: nothing is undone then.
    pub(super) fn undo(&mut self) {
        let Some(Change { before, after }) = self.undo.take() else {
            return;
        };
        let bytes_in = |snapshot: &Snapshot, path: &PathBuf| {
            snapshot
                .0
                .iter()
                .find(|(p, _)| p == path)
                .map(|(_, b)| b.clone())
        };
        let written: Vec<_> = before
            .0
            .iter()
            .filter(|(path, bytes)| bytes_in(&after, path).as_ref() != Some(bytes))
            .collect();
        let now = self.snapshot();
        if let Some((path, _)) = written
            .iter()
            .find(|(path, _)| bytes_in(&now, path) != bytes_in(&after, path))
        {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            return self.report(Err(format!("{name} a changé depuis : rien n'est annulé")));
        }
        let mut failed = None;
        for (path, bytes) in written {
            let put_back = match bytes {
                Some(bytes) => store::write_bytes(path, bytes),
                None => std::fs::remove_file(path).map_err(|e| format!("{} : {e}", path.display())),
            };
            failed = failed.or(put_back.err());
        }
        let ignored = self.reread();
        self.undo = Some(Change {
            before: after,
            after: before,
        });
        self.undone = !self.undone;
        self.report(match failed.or(ignored) {
            Some(e) => Err(e),
            None if self.undone => Ok("modification annulée".into()),
            None => Ok("modification refaite".into()),
        });
    }

    /// Shows the settings as the files hold them now; why a plugin ignores
    /// its file, if one does, as the screen says when it opens.
    fn reread(&mut self) -> Option<String> {
        let mut ignored = None;
        if let Some(path) = &self.files.menu {
            let (menu, error) = config::load(path.parent());
            self.menu = menu;
            ignored = ignored.or(error);
        }
        if let Some(path) = &self.files.sidebar {
            let (settings, error) = match Settings::read(path) {
                Ok(settings) => (settings, None),
                Err(e) => (Settings::default(), Some(e)),
            };
            self.sidebar = Some(settings);
            ignored = ignored.or(error);
        }
        if let Some(path) = self.files.links.clone() {
            ignored = ignored.or(self.reread_links(&path));
        }
        ignored
    }

    /// Answers the change waiting for Enter: done (`yes`) or dropped.
    pub(super) fn answer(&mut self, yes: bool) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        if !yes {
            return self.report(Ok("rien n'a changé".into()));
        }
        let result = match confirm {
            Confirm::RunPrograms => self.set_run_programs(true),
            Confirm::EmptyTypes => self.empty_types(),
        };
        if let Some(result) = result {
            self.report(result);
        }
        self.part = 0;
    }
}

/// The elements typed, between commas or spaces.
fn split_list(text: &str) -> Vec<String> {
    text.split([',', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// A threshold as typed, a comma or a point before its decimals.
fn threshold(text: &str) -> Result<f64, String> {
    text.trim()
        .replace(',', ".")
        .parse()
        .map_err(|_| "seuil : un nombre, une virgule ou un point avant les décimales".into())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{state, state_in};
    use super::*;
    use crate::settings::Levels;

    #[test]
    fn the_arrows_choose_a_part_and_stop_at_the_ends() {
        let mut state = state();
        state.move_part(false, 3);
        assert_eq!(state.part, 0);
        state.move_part(true, 3);
        state.move_part(true, 3);
        state.move_part(true, 3);
        assert_eq!(state.part, 2);
    }

    #[test]
    fn a_shell_is_added_replaced_and_removed() {
        let dir = herdr_api::TempDir::new("menu-edit-shells").unwrap();
        let mut state = state_in(&dir);
        state.focus(Item::Shells);
        let shells = |s: &State| s.sidebar.as_ref().unwrap().shells.clone();
        let count = shells(&state).len();
        // After the last: added.
        state.part = count;
        state.start_typing(Item::Shells);
        assert_eq!(state.typing.as_deref(), Some(""));
        state.save_typed(Item::Shells, "elvish");
        assert_eq!(shells(&state).last().unwrap(), "elvish");
        // Already there: said, not added.
        state.part = usize::MAX;
        state.save_typed(Item::Shells, "bash");
        assert_eq!(shells(&state).len(), count + 1);
        // On an element: replaced, the value first, replaced as typed.
        state.part = 0;
        state.start_typing(Item::Shells);
        assert!(state.fresh);
        state.save_typed(Item::Shells, "xonsh");
        assert_eq!(shells(&state)[0], "xonsh");
        // Typed again, in another case: once.
        state.part = 1;
        state.save_typed(Item::Shells, "XONSH");
        assert_eq!(
            shells(&state)
                .iter()
                .filter(|s| s.eq_ignore_ascii_case("xonsh"))
                .count(),
            1
        );
        state.part = 0;
        state.remove_part(Item::Shells);
        assert!(!shells(&state).contains(&"xonsh".to_string()));
    }

    #[test]
    fn an_open_type_is_replaced_in_place_and_the_last_waits_for_enter() {
        let dir = herdr_api::TempDir::new("menu-edit-types").unwrap();
        let mut state = state_in(&dir);
        let types = |s: &State| s.links.as_ref().unwrap().open_types.clone();
        let first = types(&state)[0].clone();
        state.part = 0;
        state.save_typed(Item::OpenTypes, ".ZIG");
        assert_eq!(types(&state)[0], "zig");
        assert!(!types(&state).contains(&first));
        // Down to one type: removing it asks first.
        state.write_types(vec!["md".into()]).unwrap().unwrap();
        state.part = 0;
        state.remove_part(Item::OpenTypes);
        assert_eq!(state.confirm, Some(Confirm::EmptyTypes));
        assert_eq!(types(&state), ["md"]);
        state.answer(false);
        assert_eq!(types(&state), ["md"]);
        state.remove_part(Item::OpenTypes);
        state.answer(true);
        assert!(types(&state).is_empty());
    }

    #[test]
    fn a_threshold_is_typed_on_its_own() {
        let dir = herdr_api::TempDir::new("menu-edit-levels").unwrap();
        let mut state = state_in(&dir);
        let item = Item::Levels(Levels::Cpu);
        state.focus(item);
        state.part = 1;
        state.start_typing(item);
        assert_eq!(state.typing.as_deref(), Some("25"));
        assert_eq!(state.typing_prompt(item), "orange dès (%) ");
        state.save_typed(item, "30");
        assert_eq!(
            state.sidebar.as_ref().unwrap().cpu_levels,
            [10.0, 30.0, 50.0]
        );
        // Out of order: refused, nothing written.
        state.save_typed(item, "60");
        assert!(state.status.as_ref().unwrap().1);
        assert_eq!(
            state.sidebar.as_ref().unwrap().cpu_levels,
            [10.0, 30.0, 50.0]
        );
    }

    #[test]
    fn a_threshold_is_typed_with_a_comma_or_a_point() {
        assert_eq!(threshold(" 2,5 "), Ok(2.5));
        assert_eq!(threshold("10"), Ok(10.0));
        assert!(threshold("dix").is_err());
        assert!(threshold("1 2").is_err());
    }

    #[test]
    fn the_duration_is_typed_in_seconds() {
        let dir = herdr_api::TempDir::new("menu-edit-ttl").unwrap();
        let mut state = state_in(&dir);
        assert!(state.save_ttl("42 s").is_ok());
        assert_eq!(state.sidebar.as_ref().unwrap().ttl_seconds, 42);
        assert!(state.save_ttl("0").is_err());
        assert!(state.save_ttl("601").is_err());
        assert!(state.save_ttl("deux").is_err());
    }

    #[test]
    fn a_setting_goes_back_to_its_default() {
        let dir = herdr_api::TempDir::new("menu-edit-reset").unwrap();
        let mut state = state_in(&dir);
        assert!(state.is_default(Item::Totals));
        state.change(Item::Totals, true, true);
        assert!(!state.is_default(Item::Totals));
        state.reset(Item::Totals);
        assert!(state.is_default(Item::Totals));
        let item = Item::Levels(Levels::Memory);
        state.save_levels(Levels::Memory, [1.0, 5.0, 9.0]).unwrap();
        assert!(!state.is_default(item));
        state.reset(item);
        assert!(state.is_default(item));
        assert!(state.is_default(Item::Reload));
    }

    #[test]
    fn the_last_change_is_undone_then_redone() {
        let dir = herdr_api::TempDir::new("menu-edit-undo").unwrap();
        let mut state = state_in(&dir);
        let before = state.menu.totals;
        state.tracked(|s| s.change(Item::Totals, true, true));
        assert_ne!(state.menu.totals, before);
        state.undo();
        assert_eq!(state.menu.totals, before);
        assert!(state.undone);
        state.undo();
        assert_ne!(state.menu.totals, before);
        assert!(!state.undone);
        // Nothing written: nothing to undo.
        let mut state = state_in(&dir);
        state.tracked(|s| s.move_part(true, 3));
        assert!(state.undo.is_none());
        // A file the change did not write stays as it is now.
        let dir = herdr_api::TempDir::new("menu-edit-undo-by-hand").unwrap();
        let mut state = state_in(&dir);
        let sidebar = dir.join(herdr_api::sidebar::SETTINGS_FILE);
        std::fs::write(&sidebar, "colors = false\n").unwrap();
        state.tracked(|s| s.change(Item::Totals, true, true));
        std::fs::write(&sidebar, "colors = true\n").unwrap();
        state.undo();
        assert_eq!(
            std::fs::read_to_string(&sidebar).unwrap(),
            "colors = true\n"
        );
        // A file the change wrote, changed since: nothing undone.
        let menu = dir.join(config::FILE_NAME);
        state.tracked(|s| s.change(Item::Totals, true, true));
        std::fs::write(&menu, "totals = \"line\"\n").unwrap();
        state.undo();
        assert_eq!(
            std::fs::read_to_string(&menu).unwrap(),
            "totals = \"line\"\n"
        );
        assert!(state.status.as_ref().unwrap().1);
    }

    #[test]
    fn letting_programs_run_waits_for_enter() {
        let dir = herdr_api::TempDir::new("menu-edit-danger").unwrap();
        let mut state = state_in(&dir);
        let run = |s: &State| s.links.as_ref().unwrap().run_programs;
        state.change(Item::RunPrograms, true, true);
        assert_eq!(state.confirm, Some(Confirm::RunPrograms));
        assert!(!run(&state));
        state.answer(true);
        assert!(run(&state));
        // Turned off at once.
        state.change(Item::RunPrograms, true, true);
        assert!(!run(&state) && state.confirm.is_none());
    }
}
