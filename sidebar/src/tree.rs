//! The workspace → agents tree drawn in Herdr's Agents panel.
//!
//! Herdr draws one block per agent. Each block gets a `line` token
//! (`├ ◐ Général`, with the agent's tab name), the first block of a workspace
//! also gets a `ws` header token (`api`), and an `order` token by which
//! the panel is sorted so that a workspace's agents follow its header.
//! Figures go in tokens of their own, so they can be styled apart.

/// Two blank cells Herdr does not trim: it strips leading whitespace from
/// token values, and U+2800 (blank braille pattern) is not whitespace.
const INDENT: &str = "\u{2800}\u{2800}";

/// A workspace, in sidebar order.
pub struct Space {
    pub id: String,
    pub label: String,
    pub number: u64,
}

pub struct Agent {
    pub pane_id: String,
    pub workspace_id: String,
    /// Place of its tab in the tab bar: the tree follows it.
    pub tab_number: u64,
    /// The agent's tab name.
    pub name: String,
    pub status: String,
}

/// Tokens of one agent block.
#[derive(Debug, PartialEq)]
pub struct Block {
    pub pane_id: String,
    pub order: String,
    /// Only on the first agent of a workspace.
    pub header: Option<String>,
    pub line: String,
}

pub fn blocks(spaces: &[Space], agents: &[Agent]) -> Vec<Block> {
    let mut sorted: Vec<(String, &Agent)> = agents
        .iter()
        .filter_map(|agent| {
            let space = spaces.iter().find(|s| s.id == agent.workspace_id)?;
            Some((
                order_key(space.number, agent.tab_number, &agent.pane_id),
                agent,
            ))
        })
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut blocks = Vec::with_capacity(sorted.len());
    for (index, (order, agent)) in sorted.iter().enumerate() {
        let first = index == 0 || sorted[index - 1].1.workspace_id != agent.workspace_id;
        let last = sorted
            .get(index + 1)
            .is_none_or(|(_, next)| next.workspace_id != agent.workspace_id);
        let header = first.then(|| {
            spaces
                .iter()
                .find(|s| s.id == agent.workspace_id)
                .map_or_else(String::new, |s| s.label.clone())
        });
        let branch = if last { '└' } else { '├' };
        // Herdr indents a block's second row by two more cells than its
        // first: a line without a header above it makes up for it.
        let pad = if header.is_some() { "" } else { INDENT };
        blocks.push(Block {
            pane_id: agent.pane_id.clone(),
            order: order.clone(),
            header,
            line: format!(
                "{pad}{branch} {} {}",
                status_glyph(&agent.status),
                agent.name
            ),
        });
    }
    blocks
}

/// The glyph also lets sidebar rules colour the line by state.
pub fn status_glyph(status: &str) -> char {
    match status {
        "working" => '◐',
        "blocked" => '◆',
        "done" => '●',
        "idle" => '○',
        _ => '·',
    }
}

/// Sorts by workspace and tab, as the sidebar and the tab bar show them, then
/// by pane. A pane id ends with a counter (`w1:pB`) whose longer forms come later.
fn order_key(workspace: u64, tab: u64, pane_id: &str) -> String {
    let counter = pane_id.rsplit(':').next().unwrap_or(pane_id);
    format!("{workspace:04}{tab:04}{counter:0>8}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(id: &str, label: &str, number: u64) -> Space {
        Space {
            id: id.into(),
            label: label.into(),
            number,
        }
    }

    fn agent(pane: &str, tab: u64, status: &str) -> Agent {
        Agent {
            pane_id: pane.into(),
            workspace_id: pane.split(':').next().unwrap().into(),
            tab_number: tab,
            name: "tab".into(),
            status: status.into(),
        }
    }

    #[test]
    fn groups_agents_under_their_workspace_in_sidebar_order() {
        let spaces = [space("w1", "api", 2), space("w3", "CC", 1)];
        let agents = [
            agent("w1:pD", 4, "idle"),
            agent("w3:p1", 1, "done"),
            agent("w1:p1", 1, "working"),
            agent("w9:p1", 1, "idle"),
        ];
        let blocks = blocks(&spaces, &agents);
        let shown: Vec<_> = blocks
            .iter()
            .map(|b| (b.pane_id.as_str(), b.header.as_deref(), b.line.as_str()))
            .collect();
        assert_eq!(
            shown,
            [
                ("w3:p1", Some("CC"), "└ ● tab"),
                ("w1:p1", Some("api"), "├ ◐ tab"),
                ("w1:pD", None, "\u{2800}\u{2800}└ ○ tab"),
            ]
        );
        assert!(blocks.windows(2).all(|w| w[0].order < w[1].order));
    }

    #[test]
    fn sorts_by_place_then_pane_counter() {
        // A tab moved before another one comes first, whatever its id.
        assert!(order_key(1, 2, "w1:p1") < order_key(1, 10, "w1:p1"));
        assert!(order_key(1, 1, "w1:pZ") < order_key(1, 2, "w1:p1"));
        assert!(order_key(1, 1, "w1:p9") < order_key(1, 1, "w1:pA"));
        assert!(order_key(1, 1, "w1:pZ") < order_key(1, 1, "w1:p10"));
        assert!(order_key(2, 1, "w1:p1") < order_key(10, 1, "w1:p1"));
    }
}
