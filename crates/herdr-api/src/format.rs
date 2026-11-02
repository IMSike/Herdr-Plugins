//! How the plugins write their figures: sizes, shares and decimals, as the
//! settings choose.

use serde::{Deserialize, Serialize};

/// How the plugins write their figures, as their settings choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberFormat {
    /// `2,4G` rather than `2.4G`.
    pub decimal_comma: bool,
    /// `4 %` (a narrow no-break space) rather than `4%`.
    pub percent_space: bool,
    /// CPU shares with one decimal: `4,2%` rather than `4%`.
    pub cpu_decimal: bool,
    /// The units of a size: `853M`, `853Mo` or `853MB`.
    pub memory_unit: MemoryUnit,
    /// `2,4 Go` (a narrow no-break space) rather than `2,4Go`.
    pub memory_space: bool,
}

/// How a size is written; `K`, `M` or `G` as the size needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MemoryUnit {
    /// `853M`, `2,4G`.
    #[default]
    M,
    /// `853Mo`, `2,4Go`.
    Mo,
    /// `853MB`, `2,4GB`.
    MB,
}

impl MemoryUnit {
    /// The next one, for a settings screen.
    pub fn next(self) -> Self {
        match self {
            Self::M => Self::Mo,
            Self::Mo => Self::MB,
            Self::MB => Self::M,
        }
    }

    /// `prefix` (`K`, `M`, `G`) with its unit.
    fn suffix(self, prefix: char) -> String {
        match self {
            Self::M => prefix.to_string(),
            Self::Mo => format!("{prefix}o"),
            Self::MB => format!("{prefix}B"),
        }
    }
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self {
            decimal_comma: true,
            percent_space: false,
            cpu_decimal: false,
            memory_unit: MemoryUnit::M,
            memory_space: false,
        }
    }
}

impl NumberFormat {
    /// A number written with one decimal: `2,4`.
    pub fn decimal(&self, value: f64) -> String {
        let text = format!("{value:.1}");
        if self.decimal_comma {
            text.replace('.', ",")
        } else {
            text
        }
    }

    /// A size in binary units: `512K`, `853M`, `2,4G` (or `Ko`, `Mo`, `Go`;
    /// `KB`, `MB`, `GB`), after a narrow no-break space with `memory_space`.
    pub fn size(&self, bytes: u64) -> String {
        const KIB: f64 = 1024.0;
        let b = bytes as f64;
        let space = if self.memory_space { "\u{202F}" } else { "" };
        let unit = |prefix| format!("{space}{}", self.memory_unit.suffix(prefix));
        if b < KIB * KIB {
            format!("{:.0}{}", b / KIB, unit('K'))
        } else if b < KIB * KIB * 1000.0 {
            format!("{:.0}{}", b / (KIB * KIB), unit('M'))
        } else {
            format!("{}{}", self.decimal(b / (KIB * KIB * KIB)), unit('G'))
        }
    }

    /// A share in percent: `4%`, or `4 %`.
    pub fn percent(&self, value: impl std::fmt::Display) -> String {
        let space = if self.percent_space { "\u{202F}" } else { "" };
        format!("{value}{space}%")
    }

    /// A CPU share: `4%`, or `4,2%` with `cpu_decimal`.
    pub fn cpu(&self, percent: f64) -> String {
        if self.cpu_decimal {
            self.percent(self.decimal(percent))
        } else {
            self.percent(format!("{percent:.0}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_short() {
        let french = NumberFormat::default();
        assert_eq!(french.size(512 * 1024), "512K");
        assert_eq!(french.size(853 * 1024 * 1024), "853M");
        assert_eq!(french.size(2_576_980_378), "2,4G");
        assert_eq!(french.percent(4), "4%");
        let other = NumberFormat {
            decimal_comma: false,
            percent_space: true,
            ..french
        };
        assert_eq!(other.size(2_576_980_378), "2.4G");
        assert_eq!(other.percent(4), "4\u{202F}%");
    }

    #[test]
    fn sizes_take_their_unit() {
        let octets = NumberFormat {
            memory_unit: MemoryUnit::Mo,
            ..NumberFormat::default()
        };
        assert_eq!(octets.size(512 * 1024), "512Ko");
        assert_eq!(octets.size(853 * 1024 * 1024), "853Mo");
        assert_eq!(octets.size(2_576_980_378), "2,4Go");
        let bytes = NumberFormat {
            memory_unit: MemoryUnit::MB,
            memory_space: true,
            decimal_comma: false,
            ..NumberFormat::default()
        };
        assert_eq!(bytes.size(2_576_980_378), "2.4\u{202F}GB");
    }

    #[test]
    fn cpu_shares_round_or_keep_a_decimal() {
        let round = NumberFormat::default();
        assert_eq!(round.cpu(0.42), "0%");
        assert_eq!(round.cpu(12.5), "12%");
        let decimal = NumberFormat {
            cpu_decimal: true,
            ..round
        };
        assert_eq!(decimal.cpu(0.42), "0,4%");
        assert_eq!(decimal.cpu(12.46), "12,5%");
    }
}
