//! What the sidebar shares with the other plugins: its id, its settings
//! (the menu's settings screen shows and changes them), the levels of its
//! threshold colors, and the totals its runs leave in its state folder (the
//! `Tools` pane shows them).

mod settings;
mod totals;

pub use settings::{Levels, Settings, TEMPLATE};
pub use totals::{Gauge, Level, Part, Thresholds, Totals, TotalsThresholds, check_thresholds};

use std::path::{Path, PathBuf};

/// The sidebar plugin's id.
pub const ID: &str = "imsike.sidebar";
/// The sidebar's settings file, in its config folder.
pub const SETTINGS_FILE: &str = "sidebar.toml";
/// How long a value lasts when no run refreshes it, by default.
pub(crate) const TTL_SECONDS: u64 = 20;
/// Process names counted as shells, by default (without `.exe`, any case).
pub(crate) const SHELLS: &[&str] = &[
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
pub(crate) const CPU_LEVELS: Thresholds = [10.0, 25.0, 50.0];
/// Default thresholds of its memory (GiB).
pub(crate) const MEMORY_LEVELS: Thresholds = [1.0, 2.0, 4.0];
/// Default thresholds of the machine's CPU share (percent).
pub(crate) const MACHINE_CPU_LEVELS: Thresholds = [50.0, 70.0, 90.0];
/// Default thresholds of the machine's memory in use (percent of its memory).
pub(crate) const MACHINE_MEMORY_LEVELS: Thresholds = [60.0, 75.0, 90.0];
/// Nerd Font monitor glyph (nf-md-monitor), before the whole machine's figures.
pub const MACHINE_GLYPH: char = '\u{F0379}';

/// The sidebar's state folder, beside this plugin's: Herdr gives each
/// plugin a folder named after its id, in the same place.
pub fn state_dir() -> Result<PathBuf, String> {
    beside(&crate::state_dir()?)
}

/// The folder of the running Herdr session in the sidebar's state folder,
/// where its totals are.
pub fn session_dir() -> Result<PathBuf, String> {
    crate::session_in(&state_dir()?).map_err(|e| e.to_string())
}

/// The sidebar's config folder, beside this plugin's, as its state folder.
pub fn config_dir() -> Result<PathBuf, String> {
    beside(&crate::config_dir()?)
}

/// The sidebar's folder beside `mine`, a folder of this plugin's.
fn beside(mine: &Path) -> Result<PathBuf, String> {
    let parent = mine.parent().ok_or("dossier du plugin sans parent")?;
    Ok(parent.join(ID))
}
