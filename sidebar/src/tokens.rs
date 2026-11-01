//! The values the plugin reports as sidebar tokens.

use serde_json::{Map, Value};

use herdr_api::NumberFormat;
use herdr_api::sidebar::{GIB, Level, Thresholds};

use crate::tree::Block;
use crate::usage::Usage;

/// Nerd Font terminal glyph (nf-oct-terminal). Some terminals draw it two
/// cells wide: the space after it keeps the count clear.
const SHELL_GLYPH: char = '\u{F489}';

/// How the figures are written: their format, and their thresholds when
/// the colors are on.
pub struct Look {
    pub numbers: NumberFormat,
    /// CPU (percent) and memory (GiB) thresholds.
    pub levels: Option<(Thresholds, Thresholds)>,
}

impl Look {
    /// The level of a CPU share, of a memory size, and of both: the higher.
    fn level(&self, cpu: Option<f64>, memory: Option<u64>) -> Option<Level> {
        let (cpu_levels, memory_levels) = self.levels.as_ref()?;
        let cpu = cpu.map(|c| Level::of(c, cpu_levels));
        let memory = memory.map(|m| Level::of(m as f64 / GIB, memory_levels));
        cpu.max(memory)
    }

    /// `text` with the markers of `level`, for Herdr's color rules.
    fn marked(&self, text: String, level: Option<Level>) -> String {
        match level {
            Some(level) => text + &level.marker(),
            None => text,
        }
    }
}

/// The tokens that place a block in the tree.
pub fn layout(block: &Block) -> Map<String, Value> {
    let mut tokens = Map::new();
    tokens.insert("order".into(), block.order.clone().into());
    tokens.insert("line".into(), block.line.clone().into());
    tokens.insert("ws".into(), optional(block.header.clone()));
    tokens
}

/// The tokens of an agent's block. `usage` is `None` when its processes
/// could not be read: its figures stay as the last run reported them.
/// `workspace` is the workspace's figures, on a header with several agents.
pub fn agent(
    block: &Block,
    usage: Option<(Option<f64>, &Usage)>,
    workspace: Option<String>,
    look: &Look,
) -> Map<String, Value> {
    let mut tokens = layout(block);
    if let Some((cpu, usage)) = usage {
        if let Some(cpu) = cpu {
            // Left out (not cleared) on the first run, which has no average yet.
            let text = look.marked(look.numbers.cpu(cpu), look.level(Some(cpu), None));
            tokens.insert("cpu".into(), text.into());
        }
        let ram = look.marked(
            look.numbers.size(usage.memory),
            look.level(None, Some(usage.memory)),
        );
        tokens.insert("ram".into(), ram.into());
        let shells = match usage.shells {
            0 => None,
            1 => Some("1 shell".into()),
            n => Some(format!("{n} shells")),
        };
        tokens.insert("shells".into(), optional(shells));
        let count = (usage.shells > 0).then(|| format!("{SHELL_GLYPH} {}", usage.shells));
        tokens.insert("shell_count".into(), optional(count));
        tokens.insert("usage".into(), figures(cpu, usage, false, look).into());
        tokens.insert("stats".into(), figures(cpu, usage, true, look).into());
    }
    tokens.insert("ws_stats".into(), optional(workspace));
    tokens
}

/// `4% 853M  1`; the CPU share is missing on the first run. With the
/// colors, it carries the higher level of its CPU and memory.
pub fn figures(cpu: Option<f64>, usage: &Usage, with_shells: bool, look: &Look) -> String {
    let mut parts: Vec<String> = cpu.map(|c| look.numbers.cpu(c)).into_iter().collect();
    parts.push(look.numbers.size(usage.memory));
    if with_shells && usage.shells > 0 {
        parts.push(format!("{SHELL_GLYPH} {}", usage.shells));
    }
    look.marked(parts.join(" "), look.level(cpu, Some(usage.memory)))
}

/// Whether reporting `tokens` would change nothing to the `current` ones
/// (a cleared token is an absent one).
pub fn unchanged(tokens: &Map<String, Value>, current: &Map<String, Value>) -> bool {
    tokens
        .iter()
        .all(|(key, value)| current.get(key).unwrap_or(&Value::Null) == value)
}

/// A token set to `value`, or cleared.
fn optional(value: Option<String>) -> Value {
    value.map_or(Value::Null, Value::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    fn block(header: Option<&str>) -> Block {
        Block {
            pane_id: "w1:p1".into(),
            workspace_id: "w1".into(),
            order: "0001".into(),
            workspace_figures: header.is_some(),
            header: header.map(str::to_string),
            line: "├ ○ api".into(),
        }
    }

    #[test]
    fn an_agent_reports_its_figures_or_keeps_the_last_ones() {
        let numbers = Look {
            numbers: NumberFormat::default(),
            levels: None,
        };
        let usage = Usage {
            cpu_ms: 0,
            memory: 853 * MB,
            shells: 2,
        };
        let tokens = agent(
            &block(Some("api")),
            Some((Some(4.2), &usage)),
            Some("6% 1,2G".into()),
            &numbers,
        );
        assert_eq!(tokens["ws"], "api");
        assert_eq!(tokens["cpu"], "4%");
        assert_eq!(tokens["ram"], "853M");
        assert_eq!(tokens["shells"], "2 shells");
        assert_eq!(tokens["shell_count"], "\u{F489} 2");
        assert_eq!(tokens["usage"], "4% 853M");
        assert_eq!(tokens["stats"], "4% 853M \u{F489} 2");
        assert_eq!(tokens["ws_stats"], "6% 1,2G");

        let unread = agent(&block(None), None, None, &numbers);
        assert_eq!(unread["ws"], Value::Null);
        assert_eq!(unread["ws_stats"], Value::Null);
        assert!(!unread.contains_key("stats") && !unread.contains_key("ram"));

        // First run: no CPU share, and the token is left as it was.
        let first = agent(&block(None), Some((None, &usage)), None, &numbers);
        assert!(!first.contains_key("cpu"));
        assert_eq!(first["stats"], "853M \u{F489} 2");

        let idle = Usage { shells: 0, ..usage };
        let none = agent(&block(None), Some((None, &idle)), None, &numbers);
        assert_eq!(none["shell_count"], Value::Null);
    }

    #[test]
    fn colored_figures_carry_their_level() {
        let look = Look {
            numbers: NumberFormat::default(),
            levels: Some(([10.0, 25.0, 50.0], [1.0, 2.0, 4.0])),
        };
        let usage = Usage {
            cpu_ms: 0,
            memory: 3 * 1024 * MB,
            shells: 0,
        };
        let tokens = agent(&block(None), Some((Some(4.0), &usage)), None, &look);
        // CPU green (one marker), memory orange (three): the pair is orange.
        assert_eq!(tokens["cpu"], "4%\u{200B}");
        assert_eq!(tokens["ram"], "3,0G\u{200B}\u{200B}\u{200B}");
        assert_eq!(tokens["usage"], "4% 3,0G\u{200B}\u{200B}\u{200B}");
        assert_eq!(tokens["shell_count"], Value::Null);
    }

    #[test]
    fn tokens_already_shown_need_no_report() {
        let tokens = layout(&block(None));
        let mut current = tokens.clone();
        current.remove("ws");
        current.insert("ram".into(), "853M".into());
        assert!(unchanged(&tokens, &current));
        current.insert("line".into(), "└ ○ api".into());
        assert!(!unchanged(&tokens, &current));
    }
}
