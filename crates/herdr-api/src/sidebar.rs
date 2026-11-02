//! What the sidebar shares with the other plugins: its id, its settings
//! (the menu's settings screen shows and changes them), the levels of its
//! threshold colors, and the totals its runs leave in its state folder (the
//! `Tools` pane shows them).

mod settings;

pub use settings::{Levels, Settings, TEMPLATE};

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
/// Default thresholds of Herdr's total CPU share, all workspaces (percent of
/// the machine).
pub const CPU_LEVELS: Thresholds = [10.0, 25.0, 50.0];
/// Default thresholds of its memory (GiB).
pub const MEMORY_LEVELS: Thresholds = [1.0, 2.0, 4.0];
/// Default thresholds of the machine's CPU share (percent).
pub const MACHINE_CPU_LEVELS: Thresholds = [50.0, 70.0, 90.0];
/// Default thresholds of the machine's memory in use (percent of its memory).
pub const MACHINE_MEMORY_LEVELS: Thresholds = [60.0, 75.0, 90.0];
/// Bytes in a GiB, the unit of the memory thresholds.
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
/// The totals of the last run, in the sidebar's state folder.
const TOTALS_FILE: &str = "totals.json";
/// Nerd Font monitor glyph (nf-md-monitor), before the whole machine's figures.
pub const MACHINE_GLYPH: char = '\u{F0379}';

/// The three values between the four levels, in increasing order.
pub type Thresholds = [f64; 3];

/// How high a figure is against its thresholds: green, yellow, orange, red.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Green,
    Yellow,
    Orange,
    Red,
}

impl Level {
    /// The level of `value`: above a threshold (strictly) is the next level.
    pub fn of(value: f64, thresholds: &Thresholds) -> Self {
        match thresholds.iter().filter(|&&t| value > t).count() {
            0 => Self::Green,
            1 => Self::Yellow,
            2 => Self::Orange,
            _ => Self::Red,
        }
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

/// The thresholds of the totals, as `sidebar.toml` sets them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TotalsThresholds {
    /// Herdr's CPU, percent of the machine.
    pub cpu: Thresholds,
    /// Herdr's memory, GiB.
    pub memory: Thresholds,
    /// The machine's CPU, percent.
    pub machine_cpu: Thresholds,
    /// The machine's memory in use, percent of its memory.
    pub machine_memory: Thresholds,
}

impl Default for TotalsThresholds {
    fn default() -> Self {
        Self {
            cpu: CPU_LEVELS,
            memory: MEMORY_LEVELS,
            machine_cpu: MACHINE_CPU_LEVELS,
            machine_memory: MACHINE_MEMORY_LEVELS,
        }
    }
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
    /// The machine's memory, in use or not.
    #[serde(default)]
    pub machine_total: u64,
    /// Whether the figures take the color of their level.
    #[serde(default)]
    pub colors: bool,
    #[serde(default)]
    pub thresholds: TotalsThresholds,
}

/// One of the four figures of the totals, as a gauge: its text, how full
/// its bar is, and where its thresholds fall on the bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Gauge {
    /// CPU (rather than memory).
    pub cpu: bool,
    /// The whole machine's (rather than Herdr's).
    pub machine: bool,
    /// `6%`, `2,4G`; `None` for a CPU share without average.
    pub text: Option<String>,
    /// The share of the bar it fills, from 0 to 1.
    pub fill: Option<f64>,
    /// The value at the end of the bar, in the thresholds' unit.
    pub full: f64,
    /// The value, in the thresholds' unit.
    pub value: Option<f64>,
    pub thresholds: Thresholds,
}

impl Gauge {
    /// Its level, when the colors are on.
    pub fn level(&self, colors: bool) -> Option<Level> {
        self.value
            .filter(|_| colors)
            .map(|v| Level::of(v, &self.thresholds))
    }
}

impl Totals {
    /// Herdr's CPU and memory, then the machine's.
    pub fn gauges(&self, numbers: &NumberFormat) -> [Gauge; 4] {
        let total = self.machine_total as f64;
        let share = |bytes: u64| (total > 0.0).then(|| bytes as f64 / total);
        let cpu = |value: Option<f64>, machine, thresholds| Gauge {
            cpu: true,
            machine,
            text: value.map(|v| numbers.cpu(v)),
            fill: value.map(|v| v / 100.0),
            full: 100.0,
            value,
            thresholds,
        };
        let t = &self.thresholds;
        [
            cpu(self.cpu, false, t.cpu),
            Gauge {
                cpu: false,
                machine: false,
                text: Some(numbers.size(self.memory)),
                fill: share(self.memory),
                full: total / GIB,
                value: Some(self.memory as f64 / GIB),
                thresholds: t.memory,
            },
            cpu(self.machine_cpu, true, t.machine_cpu),
            Gauge {
                cpu: false,
                machine: true,
                text: Some(numbers.size(self.machine_memory)),
                fill: share(self.machine_memory),
                full: 100.0,
                value: share(self.machine_memory).map(|s| s * 100.0),
                thresholds: t.machine_memory,
            },
        ]
    }

    /// `6% 2,4G · 󰍹 35% 21,3G`; a CPU share without average is left out.
    pub fn line(&self, numbers: &NumberFormat) -> String {
        self.parts(numbers)
            .into_iter()
            .map(|(text, _)| text)
            .collect()
    }

    /// The line in pieces, each figure with its level (when the colors are
    /// on), the separators with none. The glyph keeps the space after it:
    /// a terminal may draw it over two cells.
    pub fn parts(&self, numbers: &NumberFormat) -> Vec<(String, Option<Level>)> {
        let mut parts = Vec::new();
        for gauge in self.gauges(numbers) {
            if gauge.machine && gauge.cpu {
                parts.push((" · ".to_string(), None));
                parts.push((format!("{MACHINE_GLYPH} "), None));
            }
            if let Some(text) = &gauge.text {
                parts.push((text.clone(), gauge.level(self.colors)));
                if gauge.cpu {
                    parts.push((" ".to_string(), None));
                }
            }
        }
        parts
    }

    fn file(state_dir: &Path) -> PathBuf {
        state_dir.join(TOTALS_FILE)
    }

    pub fn save(&self, state_dir: &Path) -> std::io::Result<()> {
        crate::write_json(&Self::file(state_dir), self)
    }

    /// The totals of the sidebar's last run, when there are some.
    pub fn load(state_dir: &Path) -> Option<Self> {
        crate::read_json(&Self::file(state_dir))
    }
}

/// The sidebar's state folder, beside this plugin's: Herdr gives each
/// plugin a folder named after its id, in the same place.
pub fn state_dir() -> Result<PathBuf, String> {
    beside(&crate::state_dir()?)
}

/// The sidebar's config folder, beside this plugin's, as its state folder.
pub fn config_dir() -> Result<PathBuf, String> {
    let mine = std::env::var_os(crate::env::CONFIG_DIR)
        .ok_or_else(|| crate::absent(crate::env::CONFIG_DIR))?;
    beside(Path::new(&mine))
}

/// The sidebar's folder beside `mine`, a folder of this plugin's.
fn beside(mine: &Path) -> Result<PathBuf, String> {
    let parent = mine.parent().ok_or("dossier du plugin sans parent")?;
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
            machine_total: 32 * BYTES_IN_GIB,
            colors: false,
            thresholds: TotalsThresholds::default(),
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
    fn each_figure_takes_its_level_with_the_colors() {
        let numbers = NumberFormat::default();
        let levels = |totals: &Totals| -> Vec<Level> {
            totals
                .parts(&numbers)
                .into_iter()
                .filter_map(|(_, level)| level)
                .collect()
        };
        assert!(levels(&totals()).is_empty());
        let colored = Totals {
            colors: true,
            ..totals()
        };
        // 6% green, 2 GiB yellow (above 1, not above 2), 35% green, 21 of 32
        // GiB (66%) yellow.
        assert_eq!(
            levels(&colored),
            [Level::Green, Level::Yellow, Level::Green, Level::Yellow]
        );
    }

    #[test]
    fn gauges_fill_their_bar() {
        let gauges = totals().gauges(&NumberFormat::default());
        assert!((gauges[0].fill.unwrap() - 0.064).abs() < 1e-9);
        assert_eq!(gauges[1].fill, Some(2.0 / 32.0));
        assert_eq!(gauges[1].full, 32.0);
        assert_eq!(gauges[3].value, Some(21.0 / 32.0 * 100.0));
        assert!(gauges[2].machine && gauges[2].cpu);
        let unknown = Totals {
            machine_total: 0,
            ..totals()
        };
        assert_eq!(unknown.gauges(&NumberFormat::default())[3].fill, None);
    }

    #[test]
    fn levels_follow_the_thresholds() {
        let thresholds = [10.0, 25.0, 50.0];
        assert_eq!(Level::of(10.0, &thresholds), Level::Green);
        assert_eq!(Level::of(10.5, &thresholds), Level::Yellow);
        assert_eq!(Level::of(30.0, &thresholds), Level::Orange);
        assert_eq!(Level::of(90.0, &thresholds), Level::Red);
        assert!(check_thresholds(&thresholds).is_ok());
        assert!(check_thresholds(&[10.0, 10.0, 50.0]).is_err());
        assert!(check_thresholds(&[-1.0, 10.0, 50.0]).is_err());
    }
}
