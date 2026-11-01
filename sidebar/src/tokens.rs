//! The values the plugin reports as sidebar tokens, and the line it prints
//! for the tab bar.

use serde_json::{Map, Value};

use herdr_api::format_size;

use crate::tree::Block;
use crate::usage::Usage;

/// Nerd Font terminal glyph (nf-oct-terminal). Some terminals draw it two
/// cells wide: the space after it keeps the count clear.
const SHELL_GLYPH: char = '\u{F489}';
/// Nerd Font monitor glyph (nf-md-monitor), before the whole machine's figures.
const MACHINE_GLYPH: char = '\u{F0379}';

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
    usage: Option<(Option<String>, &Usage)>,
    workspace: Option<String>,
) -> Map<String, Value> {
    let mut tokens = layout(block);
    if let Some((cpu, usage)) = usage {
        tokens.extend(figure_tokens(cpu.clone(), usage));
        let shells = match usage.shells {
            0 => None,
            1 => Some("1 shell".into()),
            n => Some(format!("{n} shells")),
        };
        tokens.insert("shells".into(), optional(shells));
        tokens.insert("stats".into(), figures(cpu, usage, true).into());
    }
    tokens.insert("ws_stats".into(), optional(workspace));
    tokens
}

/// `cpu` is left out (not cleared) on the first run, which has no average yet.
fn figure_tokens(cpu: Option<String>, usage: &Usage) -> Map<String, Value> {
    let mut tokens = Map::new();
    if let Some(cpu) = cpu {
        tokens.insert("cpu".into(), cpu.into());
    }
    tokens.insert("ram".into(), format_size(usage.memory).into());
    tokens
}

/// `4% 853M  1`; the CPU share is missing on the first run.
pub fn figures(cpu: Option<String>, usage: &Usage, with_shells: bool) -> String {
    let mut parts: Vec<String> = cpu.into_iter().collect();
    parts.push(format_size(usage.memory));
    if with_shells && usage.shells > 0 {
        parts.push(format!("{SHELL_GLYPH} {}", usage.shells));
    }
    parts.join(" ")
}

/// Every pane's total, then the whole machine's: `6% 2.4G · 󰍹 35% 21.3G`.
pub fn tab_bar(
    cpu: Option<String>,
    total: &Usage,
    machine_cpu: Option<String>,
    used_memory: u64,
) -> String {
    let machine: Vec<String> = machine_cpu
        .into_iter()
        .chain([format_size(used_memory)])
        .collect();
    format!(
        "{} · {MACHINE_GLYPH} {}",
        figures(cpu, total, false),
        machine.join(" ")
    )
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
            order: "0001".into(),
            header: header.map(str::to_string),
            line: "├ ○ api".into(),
        }
    }

    #[test]
    fn an_agent_reports_its_figures_or_keeps_the_last_ones() {
        let usage = Usage {
            cpu_ms: 0,
            memory: 853 * MB,
            shells: 2,
        };
        let tokens = agent(
            &block(Some("api")),
            Some((Some("4%".into()), &usage)),
            Some("6% 1.2G".into()),
        );
        assert_eq!(tokens["ws"], "api");
        assert_eq!(tokens["cpu"], "4%");
        assert_eq!(tokens["ram"], "853M");
        assert_eq!(tokens["shells"], "2 shells");
        assert_eq!(tokens["stats"], "4% 853M \u{F489} 2");
        assert_eq!(tokens["ws_stats"], "6% 1.2G");

        let unread = agent(&block(None), None, None);
        assert_eq!(unread["ws"], Value::Null);
        assert_eq!(unread["ws_stats"], Value::Null);
        assert!(!unread.contains_key("stats") && !unread.contains_key("ram"));

        // First run: no CPU share, and the token is left as it was.
        let first = agent(&block(None), Some((None, &usage)), None);
        assert!(!first.contains_key("cpu"));
        assert_eq!(first["stats"], "853M \u{F489} 2");
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

    #[test]
    fn the_tab_bar_shows_both_totals() {
        let total = Usage {
            cpu_ms: 0,
            memory: 2 * 1024 * MB,
            shells: 3,
        };
        assert_eq!(
            tab_bar(
                Some("6%".into()),
                &total,
                Some("35%".into()),
                21 * 1024 * MB
            ),
            "6% 2.0G · \u{F0379} 35% 21.0G"
        );
        assert_eq!(
            tab_bar(None, &total, None, 512 * MB),
            "2.0G · \u{F0379} 512M"
        );
    }
}
