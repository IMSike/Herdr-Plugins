//! The runs a click leads to: the File Explorer, herdr-explorer's preview,
//! the confirmation popup, and Herdr's notification of a refusal.

use crate::url::Lines;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// herdr-explorer's plugin id.
const EXPLORER_ID: &str = "gcaudan.explorer";
/// How long the preview may take; past it, the click does nothing else.
pub const PEEK_TIMEOUT: Duration = Duration::from_secs(3);
/// How often a running preview is checked.
const PEEK_CHECK_EVERY: Duration = Duration::from_millis(20);
/// The path the confirmation popup shows, canonical.
pub const PATH_ENV: &str = "HERDR_LINKS_PATH";
/// How long the popup waits for another popup to close.
const BUSY_FOR_AT_MOST: Duration = Duration::from_secs(2);
const BUSY_RETRY_EVERY: Duration = Duration::from_millis(50);

/// Windows' folder (`%SystemRoot%`, else `%windir%`), where the File
/// Explorer is: never looked up on `PATH`, nor in the current folder.
fn system_root() -> Result<PathBuf, String> {
    ["SystemRoot", "windir"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|dir| dir.is_absolute())
        .ok_or_else(|| "SystemRoot absent".into())
}

/// `path` as the File Explorer reads it in its command line: in double
/// quotes, which a link's path never holds; a drive's root (`C:\`) bare, its
/// backslash would otherwise escape the closing quote.
fn quoted(path: &str) -> String {
    if path.ends_with('\\') {
        path.to_string()
    } else {
        format!("\"{path}\"")
    }
}

/// The File Explorer's argument that selects `path` in its parent folder.
pub fn select_arg(path: &str) -> String {
    format!("/select,{}", quoted(path))
}

/// The File Explorer's argument that opens `path` as a double click does:
/// with the program Windows associates with it.
pub fn open_arg(path: &str) -> String {
    quoted(path)
}

/// Runs the File Explorer with `arg`, as written: Explorer reads its command
/// line its own way, not as Rust would quote an argument. From Windows'
/// folder, without waiting for it, and without Herdr's variables: a
/// program it opens runs outside Herdr, and must not take itself for one of
/// its panes or of this plugin's commands. An Explorer already running may
/// take the request over and start the program with its own environment.
/// Built elsewhere only to be checked: the plugin runs on Windows alone.
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn explorer(arg: &str) -> Result<(), String> {
    let root = system_root()?;
    let mut command = Command::new(root.join("explorer.exe"));
    command
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    herdr_api::process::leave_out(&mut command, is_herdr_s);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::raw_arg(&mut command, arg);
    command
        .spawn()
        .map(drop)
        .map_err(|e| format!("impossible de lancer l'Explorateur : {e}"))
}

/// herdr-explorer's binary and its folder, when it is installed and
/// enabled; asked of Herdr, which knows where each plugin is.
pub fn find_explorer() -> Option<(PathBuf, PathBuf)> {
    let result = herdr_api::request("plugin.list", json!({ "plugin_id": EXPLORER_ID }))
        .inspect_err(|e| eprintln!("herdr-links : {e}"))
        .ok()?;
    explorer_in(&result).filter(|(exe, _)| exe.is_file())
}

/// herdr-explorer's binary (`bin/herdr-explorer.exe`) and its folder, in
/// `plugin.list`'s result.
fn explorer_in(result: &Value) -> Option<(PathBuf, PathBuf)> {
    let plugin = result["plugins"].as_array()?.iter().find(|p| {
        p["plugin_id"].as_str() == Some(EXPLORER_ID) && p["enabled"].as_bool() == Some(true)
    })?;
    let root = plugin["plugin_root"].as_str().filter(|r| !r.is_empty())?;
    let root = PathBuf::from(root.strip_prefix(r"\\?\").unwrap_or(root));
    Some((root.join("bin").join("herdr-explorer.exe"), root))
}

/// `peek`'s argument: the file, and its lines after a colon.
fn peek_arg(path: &str, lines: Option<Lines>) -> String {
    match lines {
        Some(lines) => format!("{path}:{}", lines.peek_places()),
        None => path.to_string(),
    }
}

/// Whether `name` is a variable of this plugin's that herdr-explorer would
/// read as its own (`HERDR_PLUGIN_ID`, its state and config folders, the
/// invocation's context): left out of its environment. `HERDR_PANE_ID`
/// stays, the pane whose tab shows the preview.
fn is_the_plugin_s(name: &str) -> bool {
    name.to_uppercase().starts_with("HERDR_PLUGIN_")
}

/// Whether `name` is one of the variables Herdr gives its panes and
/// plugins (`HERDR_ENV`, `HERDR_PANE_ID`, `HERDR_SOCKET_PATH`…).
fn is_herdr_s(name: &str) -> bool {
    name.to_uppercase().starts_with("HERDR_")
}

/// Why the preview did not show a file.
#[derive(Debug, PartialEq, Eq)]
pub enum NotPeeked {
    /// herdr-explorer could not show it: the click falls back.
    Failed(String),
    /// herdr-explorer did not answer in time. It is stopped, but what it
    /// started may still show the file (a `herdr` it runs outlives it): the
    /// click opens nothing else, so as not to open the file twice.
    Slow,
}

/// Shows `path` in herdr-explorer's preview of the clicked pane's tab.
pub fn peek(exe: &Path, root: &Path, path: &str, lines: Option<Lines>) -> Result<(), NotPeeked> {
    let mut command = Command::new(exe);
    command
        .arg("peek")
        .arg(peek_arg(path, lines))
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    herdr_api::process::leave_out(&mut command, is_the_plugin_s);
    herdr_api::process::no_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| NotPeeked::Failed(format!("impossible de lancer herdr-explorer : {e}")))?;
    let deadline = Instant::now() + PEEK_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(NotPeeked::Failed(format!(
                    "herdr-explorer peek a échoué ({status})"
                )));
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(PEEK_CHECK_EVERY),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(NotPeeked::Slow);
            }
            Err(e) => return Err(NotPeeked::Failed(e.to_string())),
        }
    }
}

/// Opens the confirmation popup on `path`, retrying a moment while another
/// popup closes.
pub fn confirm(path: &str) -> Result<(), String> {
    let plugin = herdr_api::var(herdr_api::env::PLUGIN_ID)?;
    let params = json!({
        "plugin_id": plugin,
        "entrypoint": "confirm",
        "focus": true,
        "env": { PATH_ENV: path },
    });
    let deadline = Instant::now() + BUSY_FOR_AT_MOST;
    loop {
        match herdr_api::request("plugin.pane.open", params.clone()) {
            Err(e) if e.code.as_deref() == Some("ui_busy") && Instant::now() < deadline => {
                std::thread::sleep(BUSY_RETRY_EVERY);
            }
            result => return result.map(drop).map_err(String::from),
        }
    }
}

/// Says in a Herdr notification why a click did not do what it would
/// have; a notification that cannot be shown is only logged.
pub fn notify(title: &str, body: &str) {
    eprintln!("herdr-links : {title} : {body}");
    // Said without a sound: a refusal answers a click the user just made.
    let params = json!({ "title": title, "body": body, "sound": "none" });
    if let Err(e) = herdr_api::request("notification.show", params) {
        eprintln!("herdr-links : {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_explorer_s_arguments_keep_the_path_whole() {
        assert_eq!(
            select_arg(r"C:\My Docs\a (1).txt"),
            r#"/select,"C:\My Docs\a (1).txt""#
        );
        assert_eq!(open_arg(r"C:\a b\c.md"), r#""C:\a b\c.md""#);
        assert_eq!(select_arg(r"C:\"), r"/select,C:\");
    }

    #[test]
    fn peek_takes_the_lines_after_a_colon() {
        assert_eq!(peek_arg(r"C:\a.rs", None), r"C:\a.rs");
        assert_eq!(
            peek_arg(r"C:\a.rs", Some(Lines { from: 3, to: 3 })),
            r"C:\a.rs:3"
        );
        assert_eq!(
            peek_arg(r"C:\a.rs", Some(Lines { from: 3, to: 9 })),
            r"C:\a.rs:3-9"
        );
    }

    #[test]
    fn herdr_explorer_keeps_its_own_variables_and_the_pane() {
        assert!(is_the_plugin_s("HERDR_PLUGIN_ID"));
        assert!(is_the_plugin_s("HERDR_PLUGIN_CONTEXT_JSON"));
        assert!(is_the_plugin_s("herdr_plugin_state_dir"));
        assert!(!is_the_plugin_s("HERDR_PANE_ID"));
        assert!(!is_the_plugin_s("HERDR_SOCKET_PATH"));
        assert!(!is_the_plugin_s("HERDR_BIN_PATH"));
        assert!(is_herdr_s("HERDR_PANE_ID") && is_herdr_s("herdr_env"));
        assert!(!is_herdr_s("PATH") && !is_herdr_s("SystemRoot"));
    }

    #[test]
    fn finds_herdr_explorer_in_the_plugin_list() {
        let list: Value = serde_json::from_str(
            r#"{"plugins":[
                {"plugin_id":"imsike.menu","enabled":true,"plugin_root":"C:\\p\\menu"},
                {"plugin_id":"gcaudan.explorer","enabled":true,"plugin_root":"\\\\?\\C:\\p\\explorer"}]}"#,
        )
        .unwrap();
        let (exe, root) = explorer_in(&list).unwrap();
        assert_eq!(root, PathBuf::from(r"C:\p\explorer"));
        assert_eq!(
            exe,
            PathBuf::from(r"C:\p\explorer")
                .join("bin")
                .join("herdr-explorer.exe")
        );
        let disabled: Value = serde_json::from_str(
            r#"{"plugins":[{"plugin_id":"gcaudan.explorer","enabled":false,"plugin_root":"C:\\p"}]}"#,
        )
        .unwrap();
        assert_eq!(explorer_in(&disabled), None);
        assert_eq!(explorer_in(&json!({ "plugins": [] })), None);
        assert_eq!(explorer_in(&Value::Null), None);
    }
}
