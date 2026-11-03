//! The bars this plugin opened: one empty file per pane id, in the `bars`
//! folder of the plugin's state folder. A bar is a pane labelled `Tools`
//! that has its file: a pane the user renamed `Tools` is not one, and is
//! never closed. Herdr keeps pane ids across a restart, so a bar it restored
//! as a plain shell still has its file.

use super::LABEL;
use crate::herdr::Pane;
use herdr_api::{Lock, Taken};
use std::collections::HashSet;
use std::path::PathBuf;

/// The folder of the files, in the state folder.
const DIR: &str = "bars";
/// What starts the lock of a tab's bars, in the state folder.
const TAB_LOCK_PREFIX: &str = "opening-";

/// Removes the locks of the tabs that are no longer among `live`, every
/// pane Herdr has.
pub fn forget_tab_locks(live: &[Pane]) {
    let Ok(dir) = herdr_api::state_dir() else {
        return;
    };
    let live: HashSet<String> = live
        .iter()
        .map(|p| format!("{TAB_LOCK_PREFIX}{}.lock", Bars::file_name(&p.tab_id)))
        .collect();
    herdr_api::remove_stale(&dir, std::time::Duration::ZERO, |name| {
        name.starts_with(TAB_LOCK_PREFIX) && !live.contains(name)
    });
}

/// The lock of a tab's bars, held from listing them to opening or closing
/// one: `prefix+space` and the explorer's `pane.created` hook never both
/// open one. `Busy` while another run lists or changes the tab's bars.
pub fn lock_tab(tab_id: &str) -> Taken {
    let Ok(dir) = herdr_api::state_dir() else {
        return Taken::Unavailable;
    };
    Lock::try_take(&dir.join(format!("{TAB_LOCK_PREFIX}{}.lock", Bars::file_name(tab_id))))
}

pub struct Bars {
    /// `None` without a state folder: a bar is then known by its label alone.
    dir: Option<PathBuf>,
}

impl Bars {
    pub fn new() -> Self {
        Self {
            dir: herdr_api::state_dir().ok().map(|dir| dir.join(DIR)),
        }
    }

    /// Known by their label alone, as without a state folder.
    #[cfg(test)]
    pub fn by_label() -> Self {
        Self { dir: None }
    }

    fn file_name(pane_id: &str) -> String {
        pane_id.replace(':', "-")
    }

    fn file(&self, pane_id: &str) -> Option<PathBuf> {
        self.dir
            .as_ref()
            .map(|dir| dir.join(Self::file_name(pane_id)))
    }

    /// Records `pane_id` as a bar. A failure is logged: the bar then is not
    /// known as one, and `prefix+space` or the events leave it open.
    pub fn add(&self, pane_id: &str) {
        let (Some(dir), Some(file)) = (&self.dir, self.file(pane_id)) else {
            return;
        };
        let written =
            std::fs::create_dir_all(dir).and_then(|()| herdr_api::write_atomic(&file, b""));
        if let Err(e) = written {
            eprintln!("herdr-menu : {} : {e}", file.display());
        }
    }

    /// Whether no bar is open, as far as the files tell: never without a
    /// state folder, where bars are known by their label alone.
    pub fn none(&self) -> bool {
        self.dir.as_ref().is_some_and(|dir| {
            std::fs::read_dir(dir).map_or(true, |mut files| files.next().is_none())
        })
    }

    /// Whether `pane` is one of the bars.
    pub fn is(&self, pane: &Pane) -> bool {
        pane.label.as_deref() == Some(LABEL)
            && self.file(&pane.pane_id).is_none_or(|file| file.exists())
    }

    pub fn forget(&self, pane_id: &str) {
        if let Some(file) = self.file(pane_id) {
            let _ = std::fs::remove_file(file);
        }
    }

    /// Forgets the bars that are no longer among `live`, every pane Herdr has:
    /// a closed pane's id is never taken for a bar again.
    pub fn keep_only(&self, live: &[Pane]) {
        let Some(dir) = &self.dir else {
            return;
        };
        let Ok(files) = std::fs::read_dir(dir) else {
            return;
        };
        let live: HashSet<String> = live.iter().map(|p| Self::file_name(&p.pane_id)).collect();
        for file in files.flatten() {
            if !live.contains(file.file_name().to_string_lossy().as_ref()) {
                let _ = std::fs::remove_file(file.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: &str, label: &str) -> Pane {
        Pane {
            pane_id: id.into(),
            tab_id: "t".into(),
            workspace_id: "w".into(),
            label: Some(label.into()),
            cwd: None,
            agent: None,
            agent_status: None,
        }
    }

    #[test]
    fn a_bar_is_a_tools_pane_it_recorded() {
        let dir = std::env::temp_dir().join(format!("herdr-menu-bars-{}", std::process::id()));
        let bars = Bars {
            dir: Some(dir.clone()),
        };
        let (bar, renamed, other) = (
            pane("w1:p1", LABEL),
            pane("w1:p2", LABEL),
            pane("w1:p3", "x"),
        );
        assert!(bars.none());
        bars.add(&bar.pane_id);
        assert!(bars.is(&bar) && !bars.none());
        // Renamed `Tools` by the user: not a bar.
        assert!(!bars.is(&renamed));
        assert!(!bars.is(&other));
        // Gone from Herdr: forgotten.
        bars.keep_only(std::slice::from_ref(&renamed));
        assert!(!bars.is(&bar));
        bars.add(&bar.pane_id);
        bars.forget(&bar.pane_id);
        assert!(!bars.is(&bar) && bars.none());
        let _ = std::fs::remove_dir_all(dir);
        // Without a state folder, the label tells.
        assert!(Bars::by_label().is(&renamed) && !Bars::by_label().none());
    }
}
