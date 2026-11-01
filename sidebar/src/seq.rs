//! The `seq` of a run's reports: Herdr drops a report whose `seq` is not
//! above the last one it applied for the same source.

/// Low bits of a `seq` taken by the process id: two runs started in the same
/// millisecond, or after the same last `seq`, do not get the same one.
const PID_BITS: u32 = 10;

/// This run's `seq`: the time `now` in milliseconds, or the millisecond
/// after the `last` run's when the clock was set back since, with `pid` in
/// the low bits.
pub fn next(now: u64, last: Option<u64>, pid: u32) -> u64 {
    let after_last = last.map_or(0, |last| (last >> PID_BITS).saturating_add(1));
    let millis = now.max(after_last).min(u64::MAX >> PID_BITS);
    (millis << PID_BITS) | u64::from(pid & ((1 << PID_BITS) - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goes_on_when_the_clock_goes_back() {
        assert_eq!(next(5_000, None, 3), (5_000 << 10) | 3);
        assert_eq!(next(5_000, Some(4_000 << 10), 3), (5_000 << 10) | 3);
        assert_eq!(next(5_000, Some(9_000 << 10), 3), (9_001 << 10) | 3);
        // Two runs after the same last one still differ.
        let last = Some(9_000 << 10);
        assert_ne!(next(5_000, last, 3), next(5_000, last, 4));
        // A corrupt file does not overflow.
        assert!(next(5_000, Some(u64::MAX), 3) > 0);
    }
}
