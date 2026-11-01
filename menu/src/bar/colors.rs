//! The colors of the threshold levels, and the gradient of the bars through
//! them.

use crossterm::style::Color;
use herdr_api::sidebar::{Level, Thresholds};

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
            let (a, b) = (level_rgb(low), level_rgb(high));
            let mix =
                |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
            return Color::Rgb {
                r: mix(a.0, b.0),
                g: mix(a.1, b.1),
                b: mix(a.2, b.2),
            };
        }
    }
    level_color(Level::Red)
}

/// The colors of the levels (gruvbox).
fn level_rgb(level: Level) -> (u8, u8, u8) {
    match level {
        Level::Green => (0xb8, 0xbb, 0x26),
        Level::Yellow => (0xfa, 0xbd, 0x2f),
        Level::Orange => (0xfe, 0x80, 0x19),
        Level::Red => (0xfb, 0x49, 0x34),
    }
}

pub fn level_color(level: Level) -> Color {
    let (r, g, b) = level_rgb(level);
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
