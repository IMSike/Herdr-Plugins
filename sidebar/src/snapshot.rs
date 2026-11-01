//! What the plugin reads of Herdr's session, in one `session.snapshot`
//! request: workspaces, tabs and panes.

use serde::Deserialize;

use crate::tree::{Agent, Space};

#[derive(Deserialize)]
pub struct Snapshot {
    pub panes: Vec<Pane>,
    pub tabs: Vec<Tab>,
    pub workspaces: Vec<Workspace>,
}

#[derive(Deserialize)]
pub struct Pane {
    pub pane_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    /// The terminal the pane shows: a pane given another one runs another shell.
    #[serde(default)]
    pub terminal_id: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub agent_status: String,
}

#[derive(Deserialize)]
pub struct Tab {
    pub tab_id: String,
    /// Place in its workspace's tab bar, from 1.
    pub number: u64,
    #[serde(default)]
    pub label: String,
}

#[derive(Deserialize)]
pub struct Workspace {
    pub workspace_id: String,
    /// Place in the sidebar, from 1.
    pub number: u64,
    #[serde(default)]
    pub label: String,
}

impl Snapshot {
    pub fn spaces(&self) -> Vec<Space> {
        self.workspaces
            .iter()
            .map(|w| Space {
                id: w.workspace_id.clone(),
                label: w.label.clone(),
                number: w.number,
            })
            .collect()
    }

    /// The agents among the panes, named after their tab. Herdr labels a
    /// tab never renamed with a number; an empty label, or a missing tab,
    /// shows the agent's name.
    pub fn agents(&self) -> Vec<Agent> {
        self.panes
            .iter()
            .filter_map(|pane| {
                let agent = pane.agent.as_ref()?;
                let tab = self.tabs.iter().find(|t| t.tab_id == pane.tab_id);
                let name = tab
                    .map(|t| t.label.clone())
                    .filter(|label| !label.is_empty())
                    .unwrap_or_else(|| agent.clone());
                Some(Agent {
                    pane_id: pane.pane_id.clone(),
                    workspace_id: pane.workspace_id.clone(),
                    tab_number: tab.map_or(0, |t| t.number),
                    name,
                    status: pane.agent_status.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agents_are_named_after_their_tab() {
        let snapshot: Snapshot = serde_json::from_str(
            r#"{
                "workspaces": [{"workspace_id": "w1", "number": 2, "label": "api"}],
                "tabs": [
                    {"tab_id": "w1:t1", "number": 2, "label": "Général"},
                    {"tab_id": "w1:t4", "number": 1, "label": ""}
                ],
                "panes": [
                    {"pane_id": "w1:p1", "workspace_id": "w1", "tab_id": "w1:t1", "agent": "claude", "agent_status": "idle"},
                    {"pane_id": "w1:p2", "workspace_id": "w1", "tab_id": "w1:t1"},
                    {"pane_id": "w1:pD", "workspace_id": "w1", "tab_id": "w1:t4", "agent": "codex", "agent_status": "done"}
                ]
            }"#,
        )
        .unwrap();
        let agents: Vec<_> = snapshot
            .agents()
            .into_iter()
            .map(|a| (a.pane_id, a.tab_number, a.name))
            .collect();
        assert_eq!(
            agents,
            [
                ("w1:p1".into(), 2, "Général".into()),
                ("w1:pD".into(), 1, "codex".into())
            ]
        );
        assert_eq!(snapshot.spaces()[0].number, 2);
    }
}
