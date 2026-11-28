//! Changing a setting part by part: an element of a list or a threshold
//! chosen with the arrows, then typed or removed; a setting put back to its
//! default; the last change undone; a change that waits for Enter.

use super::{Confirm, Files, Item, Shown, State, TTL_MAX, config, levels_unit, number, store};
use herdr_api::links;
use herdr_api::sidebar::Settings;
use std::path::PathBuf;

/// What a threshold is, as the field typing it says.
const LEVEL_NAMES: [&str; 3] = ["jaune dès", "orange dès", "rouge dès"];

/// The settings files as they are at a moment: each file's bytes, `None`
/// for one that does not exist.
#[derive(Debug, PartialEq)]
pub struct Snapshot(Vec<(PathBuf, Option<Vec<u8>>)>);

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
        match self.shown(item) {
            Shown::List(list) => match list.get(part) {
                Some(element) => format!("remplacer {element} par "),
                None => "ajouter ".into(),
            },
            Shown::Levels(..) => {
                let unit = match item {
                    Item::Levels(which) => levels_unit(which),
                    _ => "",
                };
                format!("{} ({unit}) ", LEVEL_NAMES[part.min(2)])
            }
            Shown::Number(_) => "secondes ".into(),
            _ => String::new(),
        }
    }

    /// Saves what was typed for `item`'s chosen part.
    pub(super) fn save_typed(&mut self, item: Item, text: &str) {
        let part = self.part_of(item);
        let result = match (item, self.shown(item)) {
            (Item::Shells, Shown::List(mut list)) => {
                let typed: Vec<String> = split_list(text);
                if part < list.len() {
                    list.splice(part..=part, typed);
                } else if let Some(there) = typed.iter().find(|t| list.contains(t)) {
                    return self.report(Ok(format!("{there} est déjà dans la liste")));
                } else {
                    list.extend(typed);
                }
                self.save_shells(&list.join(", "))
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
            (Item::Levels(which), Shown::Levels(mut values, _)) => {
                if let Some(value) = values.get_mut(part) {
                    *value = text.trim().replace(',', ".");
                }
                self.save_levels(which, &values.join(" "))
            }
            (Item::Ttl, _) => self.save_ttl(text),
            _ => return,
        };
        self.report(result);
        // The element added stays chosen: the next one is added after it.
        if let Shown::List(list) = self.shown(item) {
            self.part = self.part.min(list.len());
        }
    }

    /// Replaces the open type `old` with the one typed: added first, so
    /// that the list is never empty; nothing typed removes it.
    fn replace_type(&mut self, old: &str, text: &str) -> Result<String, String> {
        if text.trim().is_empty() {
            return self.remove_type(old);
        }
        let new = links::open_type(text)?;
        if new == old {
            return Ok(String::new());
        }
        self.add_type(&new)?;
        let saved = self.remove_type(old)?;
        Ok(saved.replacen(
            &format!("{old} retiré"),
            &format!("{old} remplacé par {new}"),
            1,
        ))
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
                self.save_shells(&list.join(", "))
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
        let defaults = self.defaults();
        let result = match (item, defaults.shown(item)) {
            (_, Shown::Choice { current, .. }) => return self.choose(item, current),
            (Item::Ttl, _) => self.save_ttl(&Settings::default().ttl_seconds.to_string()),
            (Item::Shells, Shown::List(list)) => self.save_shells(&list.join(", ")),
            (Item::OpenTypes, _) => match self.reset_types() {
                Some(result) => result.map(|saved| format!("types par défaut rétablis, {saved}")),
                None => return,
            },
            (Item::Levels(which), _) => {
                let values: Vec<String> = which
                    .of(&Settings::default())
                    .iter()
                    .map(|v| number(*v))
                    .collect();
                self.save_levels(which, &values.join(" "))
            }
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
                .map(|path| (path.clone(), std::fs::read(path).ok()))
                .collect(),
        )
    }

    /// Runs `change`; when it changed a file, what the files were before
    /// it becomes the change to undo.
    pub(super) fn tracked<T>(&mut self, change: impl FnOnce(&mut Self) -> T) -> T {
        let before = self.snapshot();
        let result = change(self);
        if self.snapshot() != before {
            self.undo = Some(before);
            self.undone = false;
        }
        result
    }

    /// Puts the files back as they were before the last change, and shows
    /// them; done again, it redoes the change.
    pub(super) fn undo(&mut self) {
        let Some(before) = self.undo.take() else {
            return;
        };
        let now = self.snapshot();
        let mut failed = None;
        for (path, bytes) in before.0 {
            let put_back = match bytes {
                Some(bytes) => store::write(&path, &String::from_utf8_lossy(&bytes)),
                None => {
                    std::fs::remove_file(&path).map_err(|e| format!("{} : {e}", path.display()))
                }
            };
            failed = failed.or(put_back.err());
        }
        self.reread();
        self.undo = Some(now);
        self.undone = !self.undone;
        self.report(match failed {
            Some(e) => Err(e),
            None if self.undone => Ok("modification annulée".into()),
            None => Ok("modification refaite".into()),
        });
    }

    /// Shows the settings as the files hold them now.
    fn reread(&mut self) {
        if let Some(path) = &self.files.menu {
            self.menu = config::load(path.parent()).0;
        }
        if let Some(path) = &self.files.sidebar {
            self.sidebar = Some(Settings::read(path).unwrap_or_default());
        }
        if let Some(path) = self.files.links.clone() {
            self.reread_links(&path);
        }
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
            Confirm::RunPrograms => self.switch_run_programs(),
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
        state.save_typed(Item::OpenTypes, ".RS");
        assert!(types(&state).contains(&"rs".to_string()));
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
        state.save_levels(Levels::Memory, "1 5 9").unwrap();
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
