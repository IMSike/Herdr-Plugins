//! What the sidebar shares with the other plugins: its id, the defaults of
//! its settings (the menu's settings screen shows them), and the totals its
//! runs leave in its state folder (the `Tools` pane shows them).

use crate::NumberFormat;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The sidebar plugin's id.
pub const ID: &str = "imsike.sidebar";
/// The sidebar's settings file, in its config folder.
pub const SETTINGS_FILE: &str = "sidebar.toml";
/// How long a value lasts when no run refreshes it, by default.
pub const TTL_SECONDS: u64 = 20;
/// Process names counted as shells, by default (without `.exe`, any case).
pub const SHELLS: &[&str] = &[
    "bash",
    "sh",
    "zsh",
    "fish",
    "dash",
    "nu",
    "pwsh",
    "powershell",
    "cmd",
];
/// The totals of the last run, in the sidebar's state folder.
const TOTALS_FILE: &str = "totals.json";
/// Nerd Font monitor glyph (nf-md-monitor), before the whole machine's figures.
const MACHINE_GLYPH: char = '\u{F0379}';

/// Every pane's total and the whole machine's, as one run measured them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Totals {
    /// When they were measured (Unix milliseconds).
    pub at_ms: u64,
    /// CPU share of every pane, in percent; `None` on a run without average.
    pub cpu: Option<f64>,
    pub memory: u64,
    pub machine_cpu: Option<f64>,
    pub machine_memory: u64,
}

impl Totals {
    /// `6% 2,4G · 󰍹 35% 21,3G`; a CPU share without average is left out.
    pub fn line(&self, numbers: &NumberFormat) -> String {
        let figures = |cpu: Option<f64>, memory: u64| {
            cpu.map(|p| format_cpu(p, numbers))
                .into_iter()
                .chain([numbers.size(memory)])
                .collect::<Vec<_>>()
                .join(" ")
        };
        format!(
            "{} · {MACHINE_GLYPH} {}",
            figures(self.cpu, self.memory),
            figures(self.machine_cpu, self.machine_memory)
        )
    }

    pub fn file(state_dir: &Path) -> PathBuf {
        state_dir.join(TOTALS_FILE)
    }

    pub fn save(&self, state_dir: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(self).map_err(std::io::Error::other)?;
        crate::write_atomic(&Self::file(state_dir), &bytes)
    }

    /// The totals of the sidebar's last run, when there are some.
    pub fn load(state_dir: &Path) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(Self::file(state_dir)).ok()?).ok()
    }
}

/// A CPU share rounded to the percent, to keep lines short: `0%`, `12%`.
pub fn format_cpu(percent: f64, numbers: &NumberFormat) -> String {
    numbers.percent(format!("{percent:.0}"))
}

/// The sidebar's state folder, beside this plugin's: Herdr gives each
/// plugin a folder named after its id, in the same place.
pub fn state_dir() -> Result<PathBuf, String> {
    let mine = crate::state_dir()?;
    let parent = mine.parent().ok_or("dossier d'état sans parent")?;
    Ok(parent.join(ID))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_read_like_the_tab_bar() {
        let gib = 1024 * 1024 * 1024;
        let totals = Totals {
            at_ms: 0,
            cpu: Some(6.4),
            memory: 2 * gib,
            machine_cpu: Some(35.0),
            machine_memory: 21 * gib,
        };
        let numbers = NumberFormat::default();
        assert_eq!(totals.line(&numbers), "6% 2,0G · \u{F0379} 35% 21,0G");
        let first = Totals {
            cpu: None,
            machine_cpu: None,
            ..totals
        };
        assert_eq!(first.line(&numbers), "2,0G · \u{F0379} 21,0G");
    }
}
