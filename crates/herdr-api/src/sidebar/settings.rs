//! The sidebar's settings: `sidebar.toml` in its config folder. A missing
//! file or setting keeps its default; a file that cannot be used is ignored
//! as a whole. The sidebar reads them at each run, the menu's settings
//! screen shows and changes them.

use std::path::Path;

use super::{
    CPU_LEVELS, MACHINE_CPU_LEVELS, MACHINE_MEMORY_LEVELS, MEMORY_LEVELS, SETTINGS_FILE, SHELLS,
    TTL_SECONDS, Thresholds, TotalsThresholds, check_thresholds,
};
use crate::{MemoryUnit, NumberFormat};
use serde::{Deserialize, Serialize};

/// What a new `sidebar.toml` holds: every setting, commented out at its
/// default.
pub const TEMPLATE: &str = include_str!("../../../../sidebar/templates/sidebar.toml");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// `2,4G` rather than `2.4G`.
    pub decimal_comma: bool,
    /// `4 %` rather than `4%`.
    pub percent_space: bool,
    /// `4,2%` rather than `4%`.
    pub cpu_decimal: bool,
    /// `853M`, `853Mo` or `853MB`.
    pub memory_unit: MemoryUnit,
    /// `2,4 Go` rather than `2,4Go`.
    pub memory_space: bool,
    /// How long a value lasts when no run refreshes it (Herdr closed the tab
    /// bar entry, or the plugin was removed).
    pub ttl_seconds: u64,
    /// Process names counted as shells (without `.exe`, any case).
    pub shells: Vec<String>,
    /// The totals in the tab bar too; the `Tools` pane shows them anyway.
    pub tab_bar: bool,
    /// Colors the totals of the `Tools` pane by their level.
    pub colors: bool,
    /// Thresholds of Herdr's total CPU, all workspaces (percent).
    pub cpu_levels: Thresholds,
    /// Thresholds of its memory (GiB).
    pub memory_levels: Thresholds,
    /// Thresholds of the machine's CPU (percent).
    pub machine_cpu_levels: Thresholds,
    /// Thresholds of the machine's memory in use (percent of its memory).
    pub machine_memory_levels: Thresholds,
}

impl Default for Settings {
    fn default() -> Self {
        let numbers = NumberFormat::default();
        Self {
            decimal_comma: numbers.decimal_comma,
            percent_space: numbers.percent_space,
            cpu_decimal: numbers.cpu_decimal,
            memory_unit: numbers.memory_unit,
            memory_space: numbers.memory_space,
            ttl_seconds: TTL_SECONDS,
            shells: SHELLS.iter().map(|s| s.to_string()).collect(),
            tab_bar: false,
            colors: false,
            cpu_levels: CPU_LEVELS,
            memory_levels: MEMORY_LEVELS,
            machine_cpu_levels: MACHINE_CPU_LEVELS,
            machine_memory_levels: MACHINE_MEMORY_LEVELS,
        }
    }
}

impl Settings {
    pub fn numbers(&self) -> NumberFormat {
        NumberFormat {
            decimal_comma: self.decimal_comma,
            percent_space: self.percent_space,
            cpu_decimal: self.cpu_decimal,
            memory_unit: self.memory_unit,
            memory_space: self.memory_space,
        }
    }

    pub fn thresholds(&self) -> TotalsThresholds {
        TotalsThresholds {
            cpu: *Levels::Cpu.of(self),
            memory: *Levels::Memory.of(self),
            machine_cpu: *Levels::MachineCpu.of(self),
            machine_memory: *Levels::MachineMemory.of(self),
        }
    }

    /// The settings of `config_dir`, the defaults without a file; `Err` says
    /// why the file is ignored.
    pub fn load(config_dir: Option<&Path>) -> Result<Self, String> {
        match config_dir {
            Some(dir) => Self::read(&dir.join(SETTINGS_FILE)),
            None => Ok(Self::default()),
        }
    }

    /// The settings of the file `path`, the defaults when it does not exist;
    /// `Err` says why it is ignored.
    pub fn read(path: &Path) -> Result<Self, String> {
        crate::read_settings(path, SETTINGS_FILE, Self::parse)
    }

    fn parse(text: &str) -> Result<Self, String> {
        let settings: Self = toml::from_str(text).map_err(|e| e.message().to_string())?;
        if settings.ttl_seconds == 0 {
            return Err("ttl_seconds doit valoir au moins 1".into());
        }
        for levels in Levels::ALL {
            check_thresholds(levels.of(&settings))
                .map_err(|e| format!("{} : {e}", levels.key()))?;
        }
        Ok(settings)
    }
}

/// One of the sidebar's sets of thresholds: the list of them, for whatever
/// goes through all four.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Levels {
    Cpu,
    Memory,
    MachineCpu,
    MachineMemory,
}

impl Levels {
    pub const ALL: [Self; 4] = [
        Self::Cpu,
        Self::Memory,
        Self::MachineCpu,
        Self::MachineMemory,
    ];

    /// Its setting in `sidebar.toml`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Cpu => "cpu_levels",
            Self::Memory => "memory_levels",
            Self::MachineCpu => "machine_cpu_levels",
            Self::MachineMemory => "machine_memory_levels",
        }
    }

    pub fn of(self, settings: &Settings) -> &Thresholds {
        match self {
            Self::Cpu => &settings.cpu_levels,
            Self::Memory => &settings.memory_levels,
            Self::MachineCpu => &settings.machine_cpu_levels,
            Self::MachineMemory => &settings.machine_memory_levels,
        }
    }

    pub fn of_mut(self, settings: &mut Settings) -> &mut Thresholds {
        match self {
            Self::Cpu => &mut settings.cpu_levels,
            Self::Memory => &mut settings.memory_levels,
            Self::MachineCpu => &mut settings.machine_cpu_levels,
            Self::MachineMemory => &mut settings.machine_memory_levels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_keep_their_default() {
        let settings = Settings::parse("percent_space = true\nshells = [\"bash\"]\n").unwrap();
        assert!(settings.percent_space && settings.decimal_comma);
        assert_eq!(settings.ttl_seconds, 20);
        assert_eq!(settings.shells, ["bash"]);
        assert_eq!(Settings::parse("").unwrap(), Settings::default());
        assert!(Settings::parse("colour = 1").is_err());
        assert!(Settings::parse("ttl_seconds = 0").is_err());
        let colored = Settings::parse(
            "memory_unit = \"Mo\"
cpu_levels = [5, 20.5, 60]
",
        )
        .unwrap();
        assert_eq!(colored.memory_unit, MemoryUnit::Mo);
        assert_eq!(colored.cpu_levels, [5.0, 20.5, 60.0]);
        assert!(Settings::parse("cpu_levels = [50, 25, 10]").is_err());
        assert!(Settings::parse("memory_unit = \"Gb\"").is_err());
    }

    #[test]
    fn a_file_that_cannot_be_used_says_why() {
        let dir = std::env::temp_dir().join(format!("herdr-api-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(Settings::load(Some(&dir)), Ok(Settings::default()));
        std::fs::write(dir.join(SETTINGS_FILE), "unknown = 1\n").unwrap();
        let error = Settings::load(Some(&dir)).unwrap_err();
        assert!(error.starts_with("sidebar.toml ignoré"), "{error}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Uncommented, the template gives the defaults, and it has a line for
    /// every setting.
    #[test]
    fn the_template_gives_every_setting_its_default() {
        let lines: Vec<&str> = TEMPLATE
            .lines()
            .filter_map(|line| line.strip_prefix("# "))
            .filter(|line| line.contains(" = "))
            .collect();
        let uncommented: String = lines.iter().map(|line| format!("{line}\n")).collect();
        assert_eq!(Settings::parse(&uncommented).unwrap(), Settings::default());
        let mut in_template: Vec<&str> = lines
            .iter()
            .filter_map(|line| line.split(" = ").next())
            .collect();
        in_template.sort_unstable();
        let defaults = serde_json::to_value(Settings::default()).unwrap();
        let mut settings: Vec<&str> = defaults
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        settings.sort_unstable();
        assert_eq!(in_template, settings);
    }
}
