//! The figures as the bar writes them, in French.

use herdr_api::NumberFormat;

use super::Stats;

/// `3 h`, `2 j`: how old the figures are, from an hour on.
pub fn age_label(seconds: u64) -> Option<String> {
    let hours = seconds / 3600;
    match hours {
        0 => None,
        1..48 => Some(format!("{hours} h")),
        _ => Some(format!("{} j", hours / 24)),
    }
}

/// `≥ ` before a figure the walk did not finish.
fn at_least(stats: &Stats) -> &'static str {
    if stats.complete { "" } else { "≥ " }
}

/// `850M · 0,3% du disque`: the size, and its share of the disk.
pub fn size_line(stats: &Stats, numbers: &NumberFormat) -> String {
    let more = at_least(stats);
    let size = numbers.size(stats.bytes);
    match stats.disk.filter(|&d| d > 0) {
        Some(disk) => {
            let share = stats.bytes as f64 * 100.0 / disk as f64;
            format!(
                "{more}{size} · {more}{} du disque",
                format_share(share, numbers)
            )
        }
        None => format!("{more}{size}"),
    }
}

/// `4 210 fichiers`.
pub fn files_line(stats: &Stats) -> String {
    let unit = if stats.files == 1 {
        "fichier"
    } else {
        "fichiers"
    };
    format!("{}{} {unit}", at_least(stats), group_thousands(stats.files))
}

/// `0,3%`, `12%`, `< 0,1%`.
fn format_share(percent: f64, numbers: &NumberFormat) -> String {
    if percent < 0.1 {
        format!("< {}", numbers.percent(numbers.decimal(0.1)))
    } else if percent < 10.0 {
        numbers.percent(numbers.decimal(percent))
    } else {
        numbers.percent(format!("{percent:.0}"))
    }
}

/// `4 210`, with a narrow no-break space as in French.
fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{202F}');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(age_label(3599), None);
        assert_eq!(age_label(3 * 3600), Some("3 h".into()));
        assert_eq!(age_label(50 * 3600), Some("2 j".into()));
        let numbers = NumberFormat::default();
        assert_eq!(format_share(0.05, &numbers), "< 0,1%");
        assert_eq!(format_share(0.34, &numbers), "0,3%");
        assert_eq!(format_share(12.4, &numbers), "12%");
        let spaced = NumberFormat {
            decimal_comma: false,
            percent_space: true,
            ..numbers
        };
        assert_eq!(format_share(0.34, &spaced), "0.3\u{202F}%");
        assert_eq!(group_thousands(4210), "4\u{202F}210");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1_234_567), "1\u{202F}234\u{202F}567");
        let stats = Stats {
            bytes: 850 * 1024 * 1024,
            files: 1,
            disk: None,
            complete: true,
            entries: 2,
        };
        assert_eq!(size_line(&stats, &numbers), "850M");
        assert_eq!(files_line(&stats), "1 fichier");
        let partial = Stats {
            files: 200_000,
            disk: Some(100 * 1024 * 1024 * 1024),
            complete: false,
            ..stats
        };
        assert_eq!(size_line(&partial, &numbers), "≥ 850M · ≥ 0,8% du disque");
        assert_eq!(files_line(&partial), "≥ 200 000 fichiers");
    }
}
