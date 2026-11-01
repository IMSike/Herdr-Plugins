//! The plugin's settings: `sidebar.toml` in its config folder. A missing file
//! or setting keeps its default; a file that cannot be used is ignored, and
//! the run says why in Herdr's log.

use std::path::Path;

use herdr_api::NumberFormat;
use serde::Deserialize;

pub const FILE_NAME: &str = "sidebar.toml";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// `2,4G` rather than `2.4G`.
    pub decimal_comma: bool,
    /// `4 %` rather than `4%`.
    pub percent_space: bool,
    /// How long a value lasts when no run refreshes it (Herdr closed the tab
    /// bar entry, or the plugin was removed).
    pub ttl_seconds: u64,
    /// Process names counted as shells (without `.exe`, any case).
    pub shells: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        let numbers = NumberFormat::default();
        Self {
            decimal_comma: numbers.decimal_comma,
            percent_space: numbers.percent_space,
            ttl_seconds: 20,
            shells: [
                "bash",
                "sh",
                "zsh",
                "fish",
                "dash",
                "nu",
                "pwsh",
                "powershell",
                "cmd",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

impl Settings {
    pub fn numbers(&self) -> NumberFormat {
        NumberFormat {
            decimal_comma: self.decimal_comma,
            percent_space: self.percent_space,
        }
    }

    /// The settings of `config_dir`, the defaults without a file; `Err` says
    /// why the file is ignored.
    pub fn load(config_dir: Option<&Path>) -> Result<Self, String> {
        let Some(dir) = config_dir else {
            return Ok(Self::default());
        };
        match std::fs::read_to_string(dir.join(FILE_NAME)) {
            Ok(text) => Self::parse(&text).map_err(|e| format!("{FILE_NAME} ignoré : {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{FILE_NAME} illisible : {e}")),
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let settings: Self = toml::from_str(text).map_err(|e| e.message().to_string())?;
        if settings.ttl_seconds == 0 {
            return Err("ttl_seconds doit valoir au moins 1".into());
        }
        Ok(settings)
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
    }

    /// The menu's `Réglages` entry writes this template: uncommented, it
    /// gives the defaults.
    #[test]
    fn the_menu_template_gives_the_defaults() {
        let template = include_str!("../../menu/templates/sidebar.toml");
        let uncommented: String = template
            .lines()
            .filter_map(|line| line.strip_prefix("# "))
            .filter(|line| line.contains(" = "))
            .map(|line| format!("{line}\n"))
            .collect();
        assert_eq!(Settings::parse(&uncommented).unwrap(), Settings::default());
    }
}
