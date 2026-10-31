//! Resource usage of process trees: CPU between two samples, memory, shells.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Process names counted as shells (lowercase, without `.exe`).
const SHELLS: &[&str] = &[
    "bash",
    "sh",
    "zsh",
    "fish",
    "dash",
    "nu",
    "pwsh",
    "powershell",
    "cmd",
];

/// One running process, as read from the system.
#[derive(Debug, Clone)]
pub struct Proc {
    pub pid: u32,
    pub parent: Option<u32>,
    pub start_time: u64,
    /// CPU time used since the process started, in milliseconds (all cores).
    pub cpu_ms: u64,
    pub memory: u64,
    pub name: String,
}

/// CPU times of every process at one moment, kept until the next run.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Sample {
    pub at_ms: u64,
    /// pid -> (start time, CPU milliseconds)
    pub cpu: HashMap<u32, (u64, u64)>,
}

impl Sample {
    pub fn of(at_ms: u64, procs: &[Proc]) -> Self {
        let cpu = procs
            .iter()
            .map(|p| (p.pid, (p.start_time, p.cpu_ms)))
            .collect();
        Self { at_ms, cpu }
    }
}

/// Usage of a process tree.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Usage {
    /// CPU time used since the previous sample, in milliseconds.
    pub cpu_ms: u64,
    pub memory: u64,
    /// Shells started by a non-shell process below the root: a shell and
    /// the shells it starts (Git Bash chains several) count once.
    pub shells: u32,
}

impl std::ops::AddAssign for Usage {
    fn add_assign(&mut self, other: Self) {
        self.cpu_ms += other.cpu_ms;
        self.memory += other.memory;
        self.shells += other.shells;
    }
}

pub struct Table<'a> {
    by_pid: HashMap<u32, &'a Proc>,
    children: HashMap<u32, Vec<u32>>,
    previous: &'a Sample,
}

impl<'a> Table<'a> {
    pub fn new(procs: &'a [Proc], previous: &'a Sample) -> Self {
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for p in procs {
            if let Some(parent) = p.parent.filter(|&parent| parent != p.pid) {
                children.entry(parent).or_default().push(p.pid);
            }
        }
        let by_pid = procs.iter().map(|p| (p.pid, p)).collect();
        Self {
            by_pid,
            children,
            previous,
        }
    }

    /// Usage of `root` and all its descendants.
    pub fn tree(&self, root: u32) -> Usage {
        let mut usage = Usage::default();
        let mut seen = HashSet::new();
        let mut stack = vec![root];
        while let Some(pid) = stack.pop() {
            if !seen.insert(pid) {
                continue;
            }
            let Some(proc) = self.by_pid.get(&pid) else {
                continue;
            };
            // A child started before its parent is a reused parent pid.
            if pid != root
                && let Some(parent) = proc.parent.and_then(|p| self.by_pid.get(&p))
                && proc.start_time < parent.start_time
            {
                continue;
            }
            usage.cpu_ms += self.cpu_since_previous(proc);
            usage.memory += proc.memory;
            if pid != root && is_shell(&proc.name) && !self.parent_is_shell(proc) {
                usage.shells += 1;
            }
            if let Some(kids) = self.children.get(&pid) {
                stack.extend(kids);
            }
        }
        usage
    }

    fn parent_is_shell(&self, proc: &Proc) -> bool {
        proc.parent
            .and_then(|p| self.by_pid.get(&p))
            .is_some_and(|parent| is_shell(&parent.name))
    }

    fn cpu_since_previous(&self, proc: &Proc) -> u64 {
        match self.previous.cpu.get(&proc.pid) {
            Some(&(start, cpu)) if start == proc.start_time => proc.cpu_ms.saturating_sub(cpu),
            // Started since the previous sample: all of its CPU time is new.
            _ => proc.cpu_ms,
        }
    }
}

fn is_shell(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    SHELLS.contains(&name)
}

/// CPU share of the whole machine, in percent.
pub fn cpu_percent(cpu_ms: u64, elapsed_ms: u64, cores: usize) -> f64 {
    if elapsed_ms == 0 || cores == 0 {
        return 0.0;
    }
    (cpu_ms as f64 * 100.0 / (elapsed_ms as f64 * cores as f64)).min(100.0)
}

/// Rounded to the percent, to keep lines short: `0%`, `12%`.
pub fn format_cpu(percent: f64) -> String {
    format!("{percent:.0}%")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, parent: Option<u32>, start: u64, cpu: u64, name: &str) -> Proc {
        Proc {
            pid,
            parent,
            start_time: start,
            cpu_ms: cpu,
            memory: 100,
            name: name.into(),
        }
    }

    #[test]
    fn sums_the_tree_and_counts_shells_below_the_root() {
        let procs = vec![
            proc(1, None, 0, 50, "pwsh.exe"),
            proc(2, Some(1), 1, 400, "claude.exe"),
            proc(3, Some(2), 2, 30, "bash.exe"),
            proc(4, Some(3), 3, 10, "git.exe"),
            proc(5, Some(3), 4, 0, "bash.exe"),
            proc(6, Some(4), 5, 0, "sh.exe"),
            proc(9, None, 0, 999, "other.exe"),
        ];
        let previous = Sample {
            at_ms: 0,
            cpu: HashMap::from([(1, (0, 40)), (2, (1, 300))]),
        };
        let usage = Table::new(&procs, &previous).tree(1);
        assert_eq!(
            usage,
            Usage {
                cpu_ms: 10 + 100 + 30 + 10,
                memory: 600,
                shells: 2
            }
        );
    }

    #[test]
    fn a_reused_pid_restarts_its_cpu_count() {
        let procs = vec![proc(1, None, 5, 20, "x")];
        let previous = Sample {
            at_ms: 0,
            cpu: HashMap::from([(1, (0, 500))]),
        };
        assert_eq!(Table::new(&procs, &previous).tree(1).cpu_ms, 20);
    }

    #[test]
    fn ignores_children_of_a_reused_parent_pid_and_cycles() {
        let procs = vec![proc(1, Some(2), 10, 0, "a"), proc(2, Some(1), 5, 0, "b")];
        let usage = Table::new(&procs, &Sample::default()).tree(1);
        assert_eq!(usage.memory, 100);
    }

    #[test]
    fn formats() {
        assert_eq!(cpu_percent(500, 1000, 4), 12.5);
        assert_eq!(cpu_percent(1, 0, 4), 0.0);
        assert_eq!(format_cpu(0.42), "0%");
        assert_eq!(format_cpu(0.6), "1%");
        assert_eq!(format_cpu(12.5), "12%");
        assert!(is_shell("PowerShell.exe") && is_shell("bash") && !is_shell("node.exe"));
    }
}
