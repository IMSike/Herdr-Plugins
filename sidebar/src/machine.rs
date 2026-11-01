//! The whole machine's CPU use, from the system's own CPU times: processes
//! the plugin cannot read (protected ones, virtual machines) count too. They
//! also give the time between two runs, which a change of the wall clock
//! does not distort.

use serde::{Deserialize, Serialize};

/// CPU times of every core since the system started.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CpuTimes {
    /// Busy time, in the system's unit.
    pub busy: u64,
    /// Busy and idle time, in the system's unit.
    pub total: u64,
    /// Milliseconds of all cores together, busy or idle; 0 when unknown (a
    /// sample written by an older version).
    #[serde(default)]
    pub all_cores_ms: u64,
}

/// The machine's CPU share between two readings, in percent of all cores.
pub fn cpu_percent(before: CpuTimes, after: CpuTimes) -> Option<f64> {
    let total = after.total.checked_sub(before.total)?;
    let busy = after.busy.checked_sub(before.busy)?;
    (total > 0).then(|| (busy as f64 * 100.0 / total as f64).min(100.0))
}

/// Milliseconds of all cores together between two readings: what a share
/// of the whole machine is measured against.
pub fn all_cores_ms_between(before: CpuTimes, after: CpuTimes) -> Option<u64> {
    if before.all_cores_ms == 0 {
        return None;
    }
    after
        .all_cores_ms
        .checked_sub(before.all_cores_ms)
        .filter(|&ms| ms > 0)
}

#[cfg(windows)]
pub fn cpu_times() -> Option<CpuTimes> {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
    }
    let (mut idle, mut kernel, mut user) = Default::default();
    // SAFETY: three valid, writable FILETIME structures.
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return None;
    }
    let value = |t: FileTime| (u64::from(t.high) << 32) | u64::from(t.low);
    // Kernel time includes idle time; the unit is 100 nanoseconds.
    let total = value(kernel) + value(user);
    Some(CpuTimes {
        busy: total.saturating_sub(value(idle)),
        total,
        all_cores_ms: total / 10_000,
    })
}

#[cfg(target_os = "linux")]
pub fn cpu_times() -> Option<CpuTimes> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let uptime = std::fs::read_to_string("/proc/uptime").ok();
    parse_proc(&stat, uptime.as_deref())
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn cpu_times() -> Option<CpuTimes> {
    None
}

/// `/proc/stat`'s `cpu` line (user nice system idle iowait irq softirq
/// steal…, in clock ticks) and its `cpuN` lines, with `/proc/uptime`, a
/// clock that a change of the wall clock does not move.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_proc(stat: &str, uptime: Option<&str>) -> Option<CpuTimes> {
    let line = stat.lines().find(|l| l.starts_with("cpu "))?;
    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(|f| f.parse().ok())
        .collect::<Option<_>>()?;
    let total: u64 = fields.iter().sum();
    let idle = fields.get(3)? + fields.get(4).copied().unwrap_or(0);
    // Every core of the machine: a CPU quota or an affinity given to this
    // process does not change the share of the whole machine.
    let cores = stat
        .lines()
        .filter(|l| {
            l.strip_prefix("cpu")
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        })
        .count() as u64;
    let uptime_ms = uptime
        .and_then(|text| text.split_whitespace().next()?.parse::<f64>().ok())
        .map_or(0, |seconds| (seconds * 1000.0) as u64);
    Some(CpuTimes {
        busy: total.saturating_sub(idle),
        total,
        all_cores_ms: uptime_ms * cores,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(busy: u64, total: u64, all_cores_ms: u64) -> CpuTimes {
        CpuTimes {
            busy,
            total,
            all_cores_ms,
        }
    }

    #[test]
    fn reads_proc() {
        let stat = "cpu  100 0 50 800 50 0 0 0 0 0\ncpu0 1 2 3 4\ncpu1 1 2 3 4\n";
        assert_eq!(
            parse_proc(stat, Some("12.34 40.00\n")),
            Some(times(150, 1000, 24_680))
        );
        assert_eq!(parse_proc(stat, None), Some(times(150, 1000, 0)));
        assert_eq!(parse_proc("cpu  1 2 3\nintr 5\n", None), None);
    }

    #[test]
    fn share_between_two_readings() {
        let before = times(100, 1000, 10_000);
        let after = times(350, 2000, 18_000);
        assert_eq!(cpu_percent(before, after), Some(25.0));
        assert_eq!(cpu_percent(after, before), None);
        assert_eq!(cpu_percent(before, before), None);
        assert_eq!(all_cores_ms_between(before, after), Some(8_000));
        assert_eq!(all_cores_ms_between(after, before), None);
        assert_eq!(all_cores_ms_between(times(0, 0, 0), after), None);
    }

    #[test]
    fn reads_this_machine() {
        if let Some(times) = cpu_times() {
            assert!(times.busy <= times.total);
            assert!(times.all_cores_ms > 0);
        }
    }
}
