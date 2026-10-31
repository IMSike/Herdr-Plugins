//! What the plugin reads from and asks of Herdr: invocation context, panes and plugin panes.

use crate::config::StartIn;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

/// The part of `HERDR_PLUGIN_CONTEXT_JSON` the menu uses.
#[derive(Debug, Default, Deserialize)]
pub struct Context {
    pub workspace_id: Option<String>,
    pub tab_id: Option<String>,
    pub workspace_cwd: Option<PathBuf>,
    pub focused_pane_cwd: Option<PathBuf>,
}

impl Context {
    pub fn from_env() -> Self {
        std::env::var("HERDR_PLUGIN_CONTEXT_JSON")
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    /// Folder a tool starts in, falling back to the other folder when one is unknown.
    pub fn cwd(&self, start_in: StartIn) -> Option<PathBuf> {
        let (first, second) = match start_in {
            StartIn::Workspace => (&self.workspace_cwd, &self.focused_pane_cwd),
            StartIn::Pane => (&self.focused_pane_cwd, &self.workspace_cwd),
        };
        first.clone().or_else(|| second.clone())
    }
}

/// A tiled pane, as `pane list` and `pane get` give it.
#[derive(Debug, Clone, Deserialize)]
pub struct Pane {
    pub pane_id: String,
    pub tab_id: String,
    pub workspace_id: String,
    /// Set on plugin panes: their manifest title.
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
}

/// Runs a Herdr command and returns its `result`.
pub fn call(args: &[&str]) -> Result<Value, String> {
    let bin = std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".into());
    let output = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("herdr introuvable : {e}"))?;
    if !output.status.success() {
        let mut message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if message.is_empty() {
            message = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
        return Err(message);
    }
    let value: Value = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
    Ok(value.get("result").cloned().unwrap_or(Value::Null))
}

/// The panes of a workspace.
pub fn workspace_panes(workspace_id: &str) -> Result<Vec<Pane>, String> {
    let result = call(&["pane", "list", "--workspace", workspace_id])?;
    serde_json::from_value(result["panes"].clone())
        .map_err(|e| format!("pane list illisible : {e}"))
}

/// The panes of a tab.
pub fn tab_panes(workspace_id: &str, tab_id: &str) -> Result<Vec<Pane>, String> {
    let panes = workspace_panes(workspace_id)?;
    Ok(panes.into_iter().filter(|p| p.tab_id == tab_id).collect())
}

/// Every pane of every workspace.
pub fn all_panes() -> Result<Vec<Pane>, String> {
    let result = call(&["pane", "list"])?;
    serde_json::from_value(result["panes"].clone())
        .map_err(|e| format!("pane list illisible : {e}"))
}

pub fn pane(pane_id: &str) -> Result<Pane, String> {
    let result = call(&["pane", "get", pane_id])?;
    serde_json::from_value(result["pane"].clone()).map_err(|e| format!("pane get illisible : {e}"))
}

/// Why a pane did not open.
#[derive(Debug)]
pub enum OpenError {
    /// Another popup (or a Herdr modal) is still on screen.
    Busy,
    Other(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::Busy => f.write_str("un autre popup est ouvert"),
            OpenError::Other(message) => f.write_str(message),
        }
    }
}

/// Opens one of this plugin's panes as its manifest places it, with extra
/// environment variables.
pub fn open_pane(entrypoint: &str, env: &[(&str, &str)]) -> Result<(), OpenError> {
    open_plugin_pane(entrypoint, &[], env)
        .map(|_| ())
        .map_err(|message| {
            if message.contains("\"ui_busy\"") {
                OpenError::Busy
            } else {
                OpenError::Other(message)
            }
        })
}

/// Opens a pane, retrying while the previous popup is being torn down.
pub fn open_pane_when_free(entrypoint: &str, env: &[(&str, &str)]) -> Result<(), OpenError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match open_pane(entrypoint, env) {
            Err(OpenError::Busy) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            result => return result,
        }
    }
}

/// Opens one of this plugin's panes with extra `plugin pane open` options;
/// returns the new pane's id, when it has one (a popup has none).
pub fn open_plugin_pane(
    entrypoint: &str,
    options: &[&str],
    env: &[(&str, &str)],
) -> Result<Option<String>, String> {
    let plugin =
        std::env::var("HERDR_PLUGIN_ID").map_err(|_| "HERDR_PLUGIN_ID absent".to_string())?;
    let env: Vec<String> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let mut args = vec![
        "plugin",
        "pane",
        "open",
        "--plugin",
        &plugin,
        "--entrypoint",
        entrypoint,
    ];
    args.extend_from_slice(options);
    for pair in &env {
        args.extend(["--env", pair]);
    }
    let result = call(&args)?;
    Ok(result["plugin_pane"]["pane"]["pane_id"]
        .as_str()
        .map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_folders_and_falls_back() {
        let context: Context = serde_json::from_str(
            r#"{"workspace_id":"w3","tab_id":"w3:t1","workspace_cwd":"/repo","focused_pane_cwd":"/repo/src"}"#,
        )
        .unwrap();
        assert_eq!(context.tab_id.as_deref(), Some("w3:t1"));
        assert_eq!(
            context.cwd(StartIn::Workspace),
            Some(PathBuf::from("/repo"))
        );
        assert_eq!(context.cwd(StartIn::Pane), Some(PathBuf::from("/repo/src")));

        let only_pane: Context = serde_json::from_str(r#"{"focused_pane_cwd":"/x"}"#).unwrap();
        assert_eq!(only_pane.cwd(StartIn::Workspace), Some(PathBuf::from("/x")));
    }
}
