//! The colors of the threshold levels (gruvbox), the gradient of the bars
//! through them, and the zones of the levels on the empty part of a bar;
//! the settings screen draws with them too, and the `Update` entry is green
//! with them when a new version is out.

use crossterm::style::Color;
use herdr_api::sidebar::{Level, Thresholds};

/// The empty part of a bar (gruvbox's dark grey).
const TRACK: (u8, u8, u8) = (0x3c, 0x38, 0x36);
/// The same, as a color.
pub const TRACK_COLOR: Color = rgb(TRACK);
/// The `Update` entry when a new version is out.
pub const NEW_VERSION_COLOR: Color = level_color(Level::Green);
/// How much of its level's color the empty part of a bar takes.
const ZONE_TINT: f64 = 0.45;

/// The color of `value` between the thresholds: green at 0, yellow at the
/// first, orange at the second, red from the third.
pub fn gradient(value: f64, thresholds: &Thresholds) -> Color {
    let stops = [
        (0.0, Level::Green),
        (thresholds[0], Level::Yellow),
        (thresholds[1], Level::Orange),
        (thresholds[2], Level::Red),
    ];
    for pair in stops.windows(2) {
        let ((from, low), (to, high)) = (pair[0], pair[1]);
        if value <= to {
            let t = if to > from {
                ((value - from) / (to - from)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            return mix(level_rgb(low), level_rgb(high), t);
        }
    }
    level_color(Level::Red)
}

/// The color of the empty part of a bar at `value`: its level's, dimmed
/// toward the track, so the bar shows where each threshold falls.
pub fn zone(value: f64, thresholds: &Thresholds) -> Color {
    mix(TRACK, level_rgb(Level::of(value, thresholds)), ZONE_TINT)
}

/// The color `t` of the way from `a` to `b` (0 to 1).
fn mix(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> Color {
    let channel = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
    Color::Rgb {
        r: channel(a.0, b.0),
        g: channel(a.1, b.1),
        b: channel(a.2, b.2),
    }
}

/// The colors of the levels.
const fn level_rgb(level: Level) -> (u8, u8, u8) {
    match level {
        Level::Green => (0xb8, 0xbb, 0x26),
        Level::Yellow => (0xfa, 0xbd, 0x2f),
        Level::Orange => (0xfe, 0x80, 0x19),
        Level::Red => (0xfb, 0x49, 0x34),
    }
}

pub const fn level_color(level: Level) -> Color {
    rgb(level_rgb(level))
}

const fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::Rgb { r, g, b }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gradient_goes_through_the_levels() {
        let thresholds = [10.0, 25.0, 50.0];
        assert_eq!(gradient(0.0, &thresholds), level_color(Level::Green));
        assert_eq!(gradient(10.0, &thresholds), level_color(Level::Yellow));
        assert_eq!(gradient(25.0, &thresholds), level_color(Level::Orange));
        assert_eq!(gradient(80.0, &thresholds), level_color(Level::Red));
        // The empty part takes its level's color, dimmed.
        assert_eq!(zone(5.0, &thresholds), zone(0.0, &thresholds));
        assert_ne!(zone(5.0, &thresholds), zone(30.0, &thresholds));
        // Halfway between green and yellow.
        assert_eq!(
            gradient(5.0, &thresholds),
            Color::Rgb {
                r: 0xd9,
                g: 0xbc,
                b: 0x2b
            }
        );
    }
}
