//! A session's layout as Herdr saves it, in `session.json` and in its
//! snapshots (`session-snapshots/`): its workspaces, their tabs, their panes,
//! each pane's folder and the agent it holds, if any. Read only: the file is
//! restored as it is, byte for byte.

use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

/// The only version of the file this reading knows.
const VERSION: u64 = 3;

#[derive(Deserialize)]
struct File {
    version: u64,
    #[serde(default)]
    workspaces: Vec<FileWorkspace>,
}

#[derive(Deserialize)]
struct FileWorkspace {
    #[serde(default)]
    custom_name: Option<String>,
    #[serde(default)]
    identity_cwd: Option<PathBuf>,
    #[serde(default)]
    tabs: Vec<FileTab>,
}

#[derive(Deserialize)]
struct FileTab {
    #[serde(default)]
    custom_name: Option<String>,
    /// By the pane's number.
    #[serde(default)]
    panes: BTreeMap<String, FilePane>,
    /// The splits, down to `{"Pane": <number>}`: the panes' order on screen.
    #[serde(default)]
    layout: Value,
}

#[derive(Deserialize)]
struct FilePane {
    #[serde(default)]
    cwd: Option<PathBuf>,
    /// What an official integration reports: `{agent, kind, value}`.
    #[serde(default)]
    agent_session: Option<FileSession>,
    /// What an agent reports itself: the command that resumes it.
    #[serde(default)]
    agent_resume: Option<FileResume>,
}

#[derive(Deserialize)]
struct FileSession {
    agent: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Deserialize)]
struct FileResume {
    agent: String,
    #[serde(default)]
    argv: Vec<String>,
}

/// A session's layout.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    pub workspaces: Vec<Workspace>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    pub name: String,
    pub cwd: Option<PathBuf>,
    pub tabs: Vec<Tab>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    pub name: String,
    /// In their order on screen.
    pub panes: Vec<Pane>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pane {
    pub cwd: Option<PathBuf>,
    pub agent: Option<Agent>,
}

/// An agent Herdr resumes at its start.
#[derive(Debug, Clone, PartialEq)]
pub struct Agent {
    /// `claude`, `codex`…
    pub name: String,
    /// Its conversation's id, when its integration reports one.
    pub conversation: Option<String>,
    /// What tells it from another agent: its conversation, or the command
    /// that resumes it.
    key: String,
}

/// How many of each a layout holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub workspaces: usize,
    pub tabs: usize,
    pub panes: usize,
    pub agents: usize,
}

/// Reads a `session.json`, or one of its snapshots.
pub fn parse(bytes: &[u8]) -> Result<Layout, String> {
    let file: File = serde_json::from_slice(bytes).map_err(|e| format!("illisible : {e}"))?;
    if file.version != VERSION {
        return Err(format!(
            "version {} inconnue (connue : {VERSION})",
            file.version
        ));
    }
    let workspaces = file
        .workspaces
        .into_iter()
        .enumerate()
        .map(|(i, w)| Workspace {
            name: w
                .custom_name
                .filter(|n| !n.is_empty())
                .or_else(|| {
                    w.identity_cwd
                        .as_deref()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| format!("Espace {}", i + 1)),
            cwd: w.identity_cwd,
            tabs: w
                .tabs
                .into_iter()
                .enumerate()
                .map(|(j, t)| tab(j, t))
                .collect(),
        })
        .collect();
    Ok(Layout { workspaces })
}

fn tab(index: usize, file: FileTab) -> Tab {
    let mut order = Vec::new();
    numbers_in(&file.layout, &mut order);
    let mut panes = file.panes;
    let mut ordered: Vec<FilePane> = order
        .iter()
        .filter_map(|n| panes.remove(&n.to_string()))
        .collect();
    // A pane the splits do not name, last, by its number.
    let mut rest: Vec<(String, FilePane)> = panes.into_iter().collect();
    rest.sort_by_key(|(n, _)| n.parse::<u64>().unwrap_or(u64::MAX));
    ordered.extend(rest.into_iter().map(|(_, p)| p));
    Tab {
        name: file
            .custom_name
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("Onglet {}", index + 1)),
        panes: ordered.into_iter().map(pane).collect(),
    }
}

/// The pane numbers of a tab's splits, in their order on screen.
fn numbers_in(layout: &Value, into: &mut Vec<u64>) {
    if let Value::Object(map) = layout {
        if let Some(n) = map.get("Pane").and_then(Value::as_u64) {
            into.push(n);
        }
        if let Some(split) = map.get("Split") {
            for side in ["first", "second"] {
                if let Some(part) = split.get(side) {
                    numbers_in(part, into);
                }
            }
        }
    }
}

fn pane(file: FilePane) -> Pane {
    let agent = match (file.agent_session, file.agent_resume) {
        (Some(session), _) => {
            let conversation = session
                .value
                .filter(|_| session.kind.as_deref() == Some("id"));
            let key = format!(
                "{}\u{0}{}",
                session.agent,
                conversation.as_deref().unwrap_or_default()
            );
            Some(Agent {
                name: session.agent,
                conversation,
                key,
            })
        }
        (None, Some(resume)) => Some(Agent {
            key: format!("{}\u{0}{}", resume.agent, resume.argv.join("\u{0}")),
            name: resume.agent,
            conversation: None,
        }),
        (None, None) => None,
    };
    Pane {
        cwd: file.cwd,
        agent,
    }
}

impl Layout {
    pub fn counts(&self) -> Counts {
        let tabs = self.workspaces.iter().flat_map(|w| &w.tabs);
        let panes: Vec<&Pane> = tabs.clone().flat_map(|t| &t.panes).collect();
        Counts {
            workspaces: self.workspaces.len(),
            tabs: tabs.count(),
            panes: panes.len(),
            agents: panes.iter().filter(|p| p.agent.is_some()).count(),
        }
    }

    /// Its panes, in order: workspace by workspace, tab by tab.
    pub fn panes(&self) -> impl Iterator<Item = &Pane> {
        self.workspaces
            .iter()
            .flat_map(|w| &w.tabs)
            .flat_map(|t| &t.panes)
    }

    /// For each of its panes (in the order of `panes`), whether `current`
    /// lacks it: an agent by its conversation (or the command that resumes
    /// it), a shell by its folder. Each pane of `current` stands for one
    /// pane only.
    pub fn missing_from(&self, current: &Layout) -> Vec<bool> {
        let mut left: HashMap<String, usize> = HashMap::new();
        for pane in current.panes() {
            *left.entry(pane.key()).or_default() += 1;
        }
        self.panes()
            .map(|pane| match left.get_mut(&pane.key()) {
                Some(n) if *n > 0 => {
                    *n -= 1;
                    false
                }
                _ => true,
            })
            .collect()
    }
}

impl Pane {
    fn key(&self) -> String {
        match &self.agent {
            Some(agent) => format!("agent\u{0}{}", agent.key),
            None => format!(
                "shell\u{0}{}",
                self.cwd
                    .as_deref()
                    .map(|p| p
                        .to_string_lossy()
                        .trim_end_matches(['/', '\\'])
                        .to_string())
                    .unwrap_or_default()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up session: one workspace, a tab of three panes (two agents,
    /// one reported by its integration, one by itself) and a tab of one.
    const SNAPSHOT: &str = r#"{
      "version": 3,
      "workspaces": [{
        "custom_name": "app",
        "identity_cwd": "/home/u/app/",
        "tabs": [
          {
            "custom_name": null,
            "layout": {"Split": {"direction": "Horizontal", "ratio": 0.5,
              "first": {"Split": {"direction": "Vertical", "ratio": 0.5,
                "first": {"Pane": 3}, "second": {"Pane": 1}}},
              "second": {"Pane": 2}}},
            "panes": {
              "1": {"cwd": "/home/u/app/"},
              "2": {"cwd": "/home/u/app/api/",
                    "agent_session": {"source": "herdr:claude", "agent": "claude", "kind": "id", "value": "0000-aaaa"}},
              "3": {"cwd": "/home/u/app/web/",
                    "agent_resume": {"source": "x", "agent": "x", "argv": ["x", "--resume"]}}
            },
            "focused": 1, "root_pane": 1
          },
          {"custom_name": "docs", "layout": {"Pane": 4},
           "panes": {"4": {"cwd": "/home/u/app/docs"}}}
        ]
      }]
    }"#;

    /// The same session once the agents' panes are gone.
    const NOW: &str = r#"{"version": 3, "workspaces": [{"custom_name": "app", "tabs": [
        {"layout": {"Pane": 1}, "panes": {"1": {"cwd": "/home/u/app"}}},
        {"layout": {"Pane": 4}, "panes": {"4": {"cwd": "/home/u/app/docs/"}}}]}]}"#;

    #[test]
    fn reads_the_tree_in_its_order_on_screen() {
        let layout = parse(SNAPSHOT.as_bytes()).unwrap();
        let w = &layout.workspaces[0];
        assert_eq!(w.name, "app");
        assert_eq!(w.tabs[0].name, "Onglet 1");
        assert_eq!(w.tabs[1].name, "docs");
        let cwds: Vec<_> = w.tabs[0]
            .panes
            .iter()
            .map(|p| p.cwd.clone().unwrap())
            .collect();
        assert_eq!(
            cwds,
            ["/home/u/app/web/", "/home/u/app/", "/home/u/app/api/"].map(PathBuf::from)
        );
        let claude = w.tabs[0].panes[2].agent.as_ref().unwrap();
        assert_eq!(claude.name, "claude");
        assert_eq!(claude.conversation.as_deref(), Some("0000-aaaa"));
        let resumed = w.tabs[0].panes[0].agent.as_ref().unwrap();
        assert_eq!(
            (resumed.name.as_str(), resumed.conversation.as_ref()),
            ("x", None)
        );
        assert_eq!(
            layout.counts(),
            Counts {
                workspaces: 1,
                tabs: 2,
                panes: 4,
                agents: 2
            }
        );
    }

    #[test]
    fn says_what_the_session_lacks_now() {
        let snapshot = parse(SNAPSHOT.as_bytes()).unwrap();
        let now = parse(NOW.as_bytes()).unwrap();
        // The agents are gone; the shells are there, a final slash aside.
        assert_eq!(snapshot.missing_from(&now), [true, false, true, false]);
        assert_eq!(snapshot.missing_from(&snapshot), [false; 4]);
        assert_eq!(snapshot.missing_from(&Layout::default()), [true; 4]);
    }

    #[test]
    fn refuses_another_version_or_another_file() {
        assert!(
            parse(br#"{"version": 4, "workspaces": []}"#)
                .unwrap_err()
                .contains("version 4")
        );
        assert!(parse(b"not json").is_err());
        assert_eq!(parse(br#"{"version": 3}"#).unwrap(), Layout::default());
    }

    #[test]
    fn names_what_has_no_name() {
        let layout = parse(
            br#"{"version": 3, "workspaces": [{"identity_cwd": "/src/site/", "tabs": [{}]}, {"tabs": []}]}"#,
        )
        .unwrap();
        assert_eq!(layout.workspaces[0].name, "site");
        assert_eq!(layout.workspaces[1].name, "Espace 2");
    }
}
