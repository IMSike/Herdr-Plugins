//! What the plugin reads from and asks of Herdr: invocation context, panes and plugin panes.

use crate::config::StartIn;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long a popup waits for the previous one to be torn down.
const BUSY_FOR_AT_MOST: Duration = Duration::from_secs(3);
/// How often it tries again meanwhile.
const BUSY_RETRY_EVERY: Duration = Duration::from_millis(50);

/// The `herdr` to run: the release Herdr calls current (an update moves it
/// to the new one), else the one that runs this session, else `PATH`'s.
pub fn binary() -> PathBuf {
    let Some(running) = std::env::var_os(herdr_api::env::BIN_PATH).map(PathBuf::from) else {
        return "herdr".into();
    };
    current_release(&running)
        .filter(|c| c.is_file())
        .unwrap_or(running)
}

/// `.../current/herdr.exe` for `.../releases/<version>/herdr.exe`; `None`
/// for a binary laid out otherwise.
fn current_release(running: &std::path::Path) -> Option<PathBuf> {
    let releases = running.parent()?.parent()?;
    if releases.file_name()? != "releases" {
        return None;
    }
    Some(
        releases
            .parent()?
            .join("current")
            .join(running.file_name()?),
    )
}

/// `herdr <args>`, without input.
pub fn command(args: &[&str]) -> Command {
    let mut command = Command::new(binary());
    command.args(args).stdin(Stdio::null());
    command
}

/// A Herdr session, as `herdr session list --json` gives it.
#[derive(Debug, Clone, Deserialize)]
pub struct Session {
    pub name: String,
    /// The session `herdr` runs without `--session`.
    #[serde(default)]
    pub default: bool,
    /// Whether its server runs.
    #[serde(default)]
    pub running: bool,
    /// Where its `session.json` and `session-snapshots/` are.
    pub session_dir: PathBuf,
    pub socket_path: PathBuf,
}

impl Session {
    /// The command that starts it again: `herdr`, or `herdr --session <name>`.
    pub fn start_command(&self) -> String {
        if self.default {
            return "herdr".into();
        }
        let name = if self.name.contains(char::is_whitespace) {
            format!("\"{}\"", self.name)
        } else {
            self.name.clone()
        };
        format!("herdr --session {name}")
    }

    /// The arguments of `start_command`, after `herdr`.
    pub fn start_args(&self) -> Vec<&str> {
        if self.default {
            Vec::new()
        } else {
            vec!["--session", &self.name]
        }
    }
}

/// The session this process belongs to: the one of its socket
/// (`HERDR_SOCKET_PATH`), the default one without.
pub fn current_session() -> Result<Session, String> {
    let socket = std::env::var_os(herdr_api::env::SOCKET_PATH).map(PathBuf::from);
    session_of(sessions()?, socket.as_deref()).ok_or_else(|| "session introuvable".into())
}

/// Every Herdr session, running or not.
pub fn sessions() -> Result<Vec<Session>, String> {
    let output = command(&["session", "list", "--json"])
        .output()
        .map_err(|e| format!("impossible de lancer « herdr session list » : {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "« herdr session list » a échoué ({})",
            output.status
        ));
    }
    #[derive(Deserialize)]
    struct Sessions {
        sessions: Vec<Session>,
    }
    let sessions: Sessions = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("liste des sessions illisible : {e}"))?;
    Ok(sessions.sessions)
}

/// The session of `socket` among `sessions`; the default one without.
fn session_of(sessions: Vec<Session>, socket: Option<&std::path::Path>) -> Option<Session> {
    sessions.into_iter().find(|s| match socket {
        Some(socket) => same_path(&s.socket_path, socket),
        None => s.default,
    })
}

/// Whether two paths name the same file: Windows ignores case and takes
/// either slash.
pub fn same_path(a: &std::path::Path, b: &std::path::Path) -> bool {
    if cfg!(windows) {
        let plain = |p: &std::path::Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
        plain(a) == plain(b)
    } else {
        a == b
    }
}

/// The part of `HERDR_PLUGIN_CONTEXT_JSON` the menu uses.
#[derive(Debug, Default, Deserialize)]
pub struct Context {
    pub workspace_id: Option<String>,
    pub tab_id: Option<String>,
    pub workspace_cwd: Option<PathBuf>,
    focused_pane_cwd: Option<PathBuf>,
}

impl Context {
    pub fn from_env() -> Self {
        std::env::var(herdr_api::env::CONTEXT_JSON)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    /// Folder a tool starts in, falling back to the other folder when one is unknown.
    pub fn cwd(&self, start_in: StartIn) -> Option<PathBuf> {
        start_folder(
            start_in,
            self.workspace_cwd.clone(),
            self.focused_pane_cwd.clone(),
        )
    }
}

/// The workspace's folder or the pane's, as the entry asks, or else the other one.
pub fn start_folder(
    start_in: StartIn,
    workspace: Option<PathBuf>,
    pane: Option<PathBuf>,
) -> Option<PathBuf> {
    match start_in {
        StartIn::Workspace => workspace.or(pane),
        StartIn::Pane => pane.or(workspace),
    }
}

/// A tiled pane, as `pane.list` and `pane.get` give it.
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
    /// The agent Herdr detected in the pane.
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub agent_status: Option<String>,
}

fn panes(params: Value) -> Result<Vec<Pane>, String> {
    Ok(herdr_api::request_field("pane.list", params, "panes")?)
}

/// The panes of a workspace.
fn workspace_panes(workspace_id: &str) -> Result<Vec<Pane>, String> {
    panes(json!({ "workspace_id": workspace_id }))
}

/// The panes of a tab.
pub fn tab_panes(workspace_id: &str, tab_id: &str) -> Result<Vec<Pane>, String> {
    let panes = workspace_panes(workspace_id)?;
    Ok(panes.into_iter().filter(|p| p.tab_id == tab_id).collect())
}

/// Every pane of every workspace.
pub fn all_panes() -> Result<Vec<Pane>, String> {
    panes(json!({}))
}

/// Whether the plugin `id` is installed.
pub fn has_plugin(id: &str) -> Result<bool, String> {
    #[derive(Deserialize)]
    struct Plugin {
        plugin_id: String,
    }
    let plugins: Vec<Plugin> = herdr_api::request_field("plugin.list", json!({}), "plugins")?;
    Ok(plugins.iter().any(|p| p.plugin_id == id))
}

pub fn close(pane_id: &str) -> Result<(), String> {
    herdr_api::request("pane.close", json!({ "pane_id": pane_id }))?;
    Ok(())
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

/// Opens one of this plugin's panes as its manifest places it, focused, with
/// extra environment variables.
pub fn open_pane(entrypoint: &str, env: &[(&str, &str)]) -> Result<(), OpenError> {
    open_plugin_pane(entrypoint, json!({ "focus": true }), env)
        .map(|_| ())
        .map_err(|error| match error.code.as_deref() {
            Some("ui_busy") => OpenError::Busy,
            _ => OpenError::Other(error.into()),
        })
}

/// Opens a pane, retrying while the previous popup is being torn down.
pub fn open_pane_when_free(entrypoint: &str, env: &[(&str, &str)]) -> Result<(), OpenError> {
    let deadline = Instant::now() + BUSY_FOR_AT_MOST;
    loop {
        match open_pane(entrypoint, env) {
            Err(OpenError::Busy) if Instant::now() < deadline => {
                std::thread::sleep(BUSY_RETRY_EVERY);
            }
            result => return result,
        }
    }
}

/// Opens one of this plugin's panes with extra `plugin.pane.open` parameters
/// (an object); returns the new pane's id, when it has one (a popup has none).
pub fn open_plugin_pane(
    entrypoint: &str,
    options: Value,
    env: &[(&str, &str)],
) -> Result<Option<String>, herdr_api::Error> {
    let plugin = herdr_api::var(herdr_api::env::PLUGIN_ID)?;
    let env: Map<String, Value> = env
        .iter()
        .map(|(k, v)| (k.to_string(), Value::from(*v)))
        .collect();
    let mut params = json!({ "plugin_id": plugin, "entrypoint": entrypoint, "env": env });
    if let (Some(params), Value::Object(options)) = (params.as_object_mut(), options) {
        params.extend(options);
    }
    let result = herdr_api::request("plugin.pane.open", params)?;
    Ok(result["plugin_pane"]["pane"]["pane_id"]
        .as_str()
        .map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_current_release_beside_the_running_one() {
        let running = std::path::Path::new("/opt/herdr/releases/0.9.5/herdr");
        assert_eq!(
            current_release(running),
            Some(PathBuf::from("/opt/herdr/current/herdr"))
        );
        assert_eq!(
            current_release(std::path::Path::new("/home/u/shims/bin/herdr")),
            None
        );
    }

    #[test]
    fn finds_the_session_of_its_socket() {
        let sessions: Vec<Session> = serde_json::from_str(
            r#"[{"default":true,"name":"default","running":true,"session_dir":"/c/herdr","socket_path":"/c/herdr/herdr.sock"},
                {"default":false,"name":"work","running":true,"session_dir":"/c/herdr/sessions/work","socket_path":"/c/herdr/sessions/work/herdr.sock"}]"#,
        )
        .unwrap();
        let of = |socket: Option<&str>| {
            session_of(sessions.clone(), socket.map(std::path::Path::new)).map(|s| s.name)
        };
        assert_eq!(
            of(Some("/c/herdr/sessions/work/herdr.sock")).unwrap(),
            "work"
        );
        assert_eq!(of(None).unwrap(), "default");
        assert_eq!(of(Some("/elsewhere/herdr.sock")), None);
        assert_eq!(sessions[0].start_command(), "herdr");
        assert!(sessions[0].start_args().is_empty());
        assert_eq!(sessions[1].start_command(), "herdr --session work");
        assert_eq!(sessions[1].start_args(), ["--session", "work"]);
    }

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
