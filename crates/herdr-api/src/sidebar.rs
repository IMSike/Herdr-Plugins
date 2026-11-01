//! What the sidebar shares with the other plugins: its id, the defaults of
//! its settings (the menu's settings screen shows them), the levels of its
//! threshold colors, and the totals its runs leave in its state folder (the
//! `Tools` pane shows them).

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
/// Default thresholds of an agent's, a workspace's and Herdr's CPU share
/// (percent of the machine).
pub const CPU_LEVELS: Thresholds = [10.0, 25.0, 50.0];
/// Default thresholds of their memory (GiB).
pub const MEMORY_LEVELS: Thresholds = [1.0, 2.0, 4.0];
/// Default thresholds of the machine's CPU share (percent).
pub const MACHINE_CPU_LEVELS: Thresholds = [50.0, 70.0, 90.0];
/// Default thresholds of the machine's memory in use (percent of its memory).
pub const MACHINE_MEMORY_LEVELS: Thresholds = [60.0, 75.0, 90.0];
/// Bytes in a GiB, the unit of the memory thresholds.
pub const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
/// The totals of the last run, in the sidebar's state folder.
const TOTALS_FILE: &str = "totals.json";
/// Nerd Font monitor glyph (nf-md-monitor), before the whole machine's figures.
pub const MACHINE_GLYPH: char = '\u{F0379}';
/// What a value carries for Herdr's color rules: one to four of them, by
/// level. It has no width: Herdr matches it but draws nothing.
pub const MARKER: char = '\u{200B}';

/// The three values between the four levels, in increasing order.
pub type Thresholds = [f64; 3];

/// How high a figure is against its thresholds: green, yellow, orange, red.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Green,
    Yellow,
    Orange,
    Red,
}

impl Level {
    /// The level of `value`: above a threshold (strictly, as Herdr's `gt`
    /// rules compare) is the next level.
    pub fn of(value: f64, thresholds: &Thresholds) -> Self {
        match thresholds.iter().filter(|&&t| value > t).count() {
            0 => Self::Green,
            1 => Self::Yellow,
            2 => Self::Orange,
            _ => Self::Red,
        }
    }

    /// The markers that tell it to Herdr's `contains` rules: one for green,
    /// four for red.
    pub fn marker(self) -> String {
        std::iter::repeat_n(MARKER, self as usize + 1).collect()
    }
}

/// Why `thresholds` cannot be used, if they cannot.
pub fn check_thresholds(thresholds: &Thresholds) -> Result<(), String> {
    if thresholds.iter().any(|t| !t.is_finite() || *t < 0.0) {
        return Err("seuils négatifs ou invalides".into());
    }
    if thresholds.windows(2).any(|w| w[0] >= w[1]) {
        return Err("seuils à donner en ordre croissant".into());
    }
    Ok(())
}

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
    /// The level of each figure, in their order, when the threshold colors
    /// are on; a CPU share without average has none.
    #[serde(default)]
    pub levels: Option<TotalsLevels>,
}

/// The levels of the totals' figures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotalsLevels {
    pub cpu: Option<Level>,
    pub memory: Level,
    pub machine_cpu: Option<Level>,
    pub machine_memory: Level,
}

impl Totals {
    /// `6% 2,4G · 󰍹 35% 21,3G`; a CPU share without average is left out.
    pub fn line(&self, numbers: &NumberFormat) -> String {
        self.parts(numbers)
            .into_iter()
            .map(|(text, _)| text)
            .collect()
    }

    /// The line in pieces, each figure with its level (when the colors are
    /// on), the separators with none.
    pub fn parts(&self, numbers: &NumberFormat) -> Vec<(String, Option<Level>)> {
        let levels = self.levels.as_ref();
        let figures = |cpu: Option<f64>, cpu_level, memory: u64, memory_level| {
            let mut parts = Vec::new();
            if let Some(cpu) = cpu {
                parts.push((numbers.cpu(cpu), cpu_level));
                parts.push((" ".to_string(), None));
            }
            parts.push((numbers.size(memory), memory_level));
            parts
        };
        let mut parts = figures(
            self.cpu,
            levels.and_then(|l| l.cpu),
            self.memory,
            levels.map(|l| l.memory),
        );
        parts.push((" · ".to_string(), None));
        parts.push((MACHINE_GLYPH.to_string(), None));
        parts.push((" ".to_string(), None));
        parts.extend(figures(
            self.machine_cpu,
            levels.and_then(|l| l.machine_cpu),
            self.machine_memory,
            levels.map(|l| l.machine_memory),
        ));
        parts
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

    const BYTES_IN_GIB: u64 = 1024 * 1024 * 1024;

    fn totals() -> Totals {
        Totals {
            at_ms: 0,
            cpu: Some(6.4),
            memory: 2 * BYTES_IN_GIB,
            machine_cpu: Some(35.0),
            machine_memory: 21 * BYTES_IN_GIB,
            levels: None,
        }
    }

    #[test]
    fn totals_read_like_the_tab_bar() {
        let numbers = NumberFormat::default();
        assert_eq!(totals().line(&numbers), "6% 2,0G · \u{F0379} 35% 21,0G");
        let first = Totals {
            cpu: None,
            machine_cpu: None,
            ..totals()
        };
        assert_eq!(first.line(&numbers), "2,0G · \u{F0379} 21,0G");
    }

    #[test]
    fn each_figure_keeps_its_level() {
        let levels = TotalsLevels {
            cpu: Some(Level::Green),
            memory: Level::Yellow,
            machine_cpu: None,
            machine_memory: Level::Red,
        };
        let colored = Totals {
            machine_cpu: None,
            levels: Some(levels),
            ..totals()
        };
        let parts = colored.parts(&NumberFormat::default());
        let figures: Vec<_> = parts.iter().filter(|(_, l)| l.is_some()).collect();
        assert_eq!(
            figures,
            [
                &("6%".to_string(), Some(Level::Green)),
                &("2,0G".to_string(), Some(Level::Yellow)),
                &("21,0G".to_string(), Some(Level::Red)),
            ]
        );
    }

    #[test]
    fn levels_follow_the_thresholds() {
        let thresholds = [10.0, 25.0, 50.0];
        assert_eq!(Level::of(10.0, &thresholds), Level::Green);
        assert_eq!(Level::of(10.5, &thresholds), Level::Yellow);
        assert_eq!(Level::of(30.0, &thresholds), Level::Orange);
        assert_eq!(Level::of(90.0, &thresholds), Level::Red);
        assert_eq!(Level::Green.marker(), "\u{200B}");
        assert_eq!(Level::Red.marker().chars().count(), 4);
        assert!(check_thresholds(&thresholds).is_ok());
        assert!(check_thresholds(&[10.0, 10.0, 50.0]).is_err());
        assert!(check_thresholds(&[-1.0, 10.0, 50.0]).is_err());
    }
}
