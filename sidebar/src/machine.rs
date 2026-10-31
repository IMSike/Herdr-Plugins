//! The whole machine's CPU use, from the system's own CPU times: processes
//! the plugin cannot read (protected ones, virtual machines) count too.

use serde::{Deserialize, Serialize};

/// CPU times of every core since the system started, in the system's unit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CpuTimes {
    pub busy: u64,
    pub total: u64,
}

/// The machine's CPU share between two readings, in percent of all cores.
pub fn cpu_percent(before: CpuTimes, after: CpuTimes) -> Option<f64> {
    let total = after.total.checked_sub(before.total)?;
    let busy = after.busy.checked_sub(before.busy)?;
    (total > 0).then(|| (busy as f64 * 100.0 / total as f64).min(100.0))
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
    // Kernel time includes idle time.
    let total = value(kernel) + value(user);
    Some(CpuTimes {
        busy: total.saturating_sub(value(idle)),
        total,
    })
}

#[cfg(target_os = "linux")]
pub fn cpu_times() -> Option<CpuTimes> {
    parse_proc_stat(&std::fs::read_to_string("/proc/stat").ok()?)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn cpu_times() -> Option<CpuTimes> {
    None
}

/// The `cpu` line of `/proc/stat`: user nice system idle iowait irq softirq steal…
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_proc_stat(text: &str) -> Option<CpuTimes> {
    let line = text.lines().find(|l| l.starts_with("cpu "))?;
    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(|f| f.parse().ok())
        .collect::<Option<_>>()?;
    let total: u64 = fields.iter().sum();
    let idle = fields.get(3)? + fields.get(4).copied().unwrap_or(0);
    Some(CpuTimes {
        busy: total.saturating_sub(idle),
        total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_proc_stat() {
        let text = "cpu  100 0 50 800 50 0 0 0 0 0\ncpu0 1 2 3 4\n";
        assert_eq!(
            parse_proc_stat(text),
            Some(CpuTimes {
                busy: 150,
                total: 1000
            })
        );
    }

    #[test]
    fn share_between_two_readings() {
        let before = CpuTimes {
            busy: 100,
            total: 1000,
        };
        let after = CpuTimes {
            busy: 350,
            total: 2000,
        };
        assert_eq!(cpu_percent(before, after), Some(25.0));
        assert_eq!(cpu_percent(after, before), None);
        assert_eq!(cpu_percent(before, before), None);
    }

    #[test]
    fn reads_this_machine() {
        if let Some(times) = cpu_times() {
            assert!(times.busy <= times.total);
        }
    }
}
