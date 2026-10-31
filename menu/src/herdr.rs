//! What the plugin reads from and asks of Herdr: invocation context and plugin panes.

use crate::config::StartIn;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

/// The part of `HERDR_PLUGIN_CONTEXT_JSON` the menu uses.
#[derive(Debug, Default, Deserialize)]
pub struct Context {
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

/// Opens one of this plugin's panes, with extra environment variables.
pub fn open_pane(entrypoint: &str, env: &[(&str, &str)]) -> Result<(), OpenError> {
    open_pane_once(entrypoint, env).map_err(|message| {
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

fn open_pane_once(entrypoint: &str, env: &[(&str, &str)]) -> Result<(), String> {
    let bin = std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".into());
    let plugin =
        std::env::var("HERDR_PLUGIN_ID").map_err(|_| "HERDR_PLUGIN_ID absent".to_string())?;
    let mut command = Command::new(bin);
    command.args([
        "plugin",
        "pane",
        "open",
        "--plugin",
        &plugin,
        "--entrypoint",
        entrypoint,
    ]);
    for (key, value) in env {
        command.arg("--env").arg(format!("{key}={value}"));
    }
    let output = command
        .output()
        .map_err(|e| format!("herdr introuvable : {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let mut message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if message.is_empty() {
            message = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
        Err(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_folders_and_falls_back() {
        let context: Context = serde_json::from_str(
            r#"{"workspace_id":"w3","workspace_cwd":"/repo","focused_pane_cwd":"/repo/src"}"#,
        )
        .unwrap();
        assert_eq!(
            context.cwd(StartIn::Workspace),
            Some(PathBuf::from("/repo"))
        );
        assert_eq!(context.cwd(StartIn::Pane), Some(PathBuf::from("/repo/src")));

        let only_pane: Context = serde_json::from_str(r#"{"focused_pane_cwd":"/x"}"#).unwrap();
        assert_eq!(only_pane.cwd(StartIn::Workspace), Some(PathBuf::from("/x")));
    }
}
