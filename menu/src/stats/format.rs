//! The figures as the bar writes them, in French.

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

/// `850M · 0,3 % du disque`: the size, and its share of the disk.
pub fn size_line(stats: &Stats) -> String {
    let more = at_least(stats);
    let size = herdr_api::format_size(stats.bytes);
    match stats.disk.filter(|&d| d > 0) {
        Some(disk) => {
            let share = stats.bytes as f64 * 100.0 / disk as f64;
            format!("{more}{size} · {more}{} du disque", format_share(share))
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

/// `0,3 %`, `12 %`, `< 0,1 %`.
fn format_share(percent: f64) -> String {
    if percent < 0.1 {
        "< 0,1 %".into()
    } else if percent < 10.0 {
        format!("{percent:.1} %").replace('.', ",")
    } else {
        format!("{percent:.0} %")
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
        assert_eq!(format_share(0.05), "< 0,1 %");
        assert_eq!(format_share(0.34), "0,3 %");
        assert_eq!(format_share(12.4), "12 %");
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
        assert_eq!(size_line(&stats), "850M");
        assert_eq!(files_line(&stats), "1 fichier");
        let partial = Stats {
            files: 200_000,
            disk: Some(100 * 1024 * 1024 * 1024),
            complete: false,
            ..stats
        };
        assert_eq!(size_line(&partial), "≥ 850M · ≥ 0,8 % du disque");
        assert_eq!(files_line(&partial), "≥ 200 000 fichiers");
    }
}
