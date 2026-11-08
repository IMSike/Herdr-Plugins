//! Starting an entry's command: in the `run` popup, or on its own for the
//! programs that open their own window (`popup = false`).

use crate::config::Entry;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The command of `argv`, without a shell. On Windows a bare name is looked
/// up on `PATH` here: Rust's own search tries the system folders first (`bash`
/// would be WSL's, not the Git Bash found on `PATH`), and a batch file (`code`
/// is `code.cmd`) runs with the right escaping when given its full path.
///
/// On Unix too: `execvp` would read an empty `PATH` entry (`PATH=:/usr/bin`)
/// as the folder the program starts in, and run a program of that name a
/// cloned repository holds.
pub fn command(argv: &[String]) -> Result<Command, String> {
    let (name, args) = argv.split_first().ok_or("commande vide")?;
    let mut command = Command::new(program(name)?);
    command.args(args);
    Ok(command)
}

/// A name with its extension (`code.cmd`) is looked up on `PATH` too.
#[cfg(windows)]
fn program(name: &str) -> Result<std::ffi::OsString, String> {
    if Path::new(name).components().count() > 1 {
        return Ok(name.into());
    }
    // Rust's own search does not look in the current folder.
    Ok(find_on_path(name).map_or_else(|| name.into(), PathBuf::into_os_string))
}

/// A name with a `/` is a path, used as it is.
#[cfg(not(windows))]
fn program(name: &str) -> Result<std::ffi::OsString, String> {
    if name.contains('/') {
        return Ok(name.into());
    }
    find_on_path(name)
        .map(PathBuf::into_os_string)
        .ok_or_else(|| not_found(name))
}

/// The error of a program `name` that is not on `PATH`.
pub fn not_found(name: &str) -> String {
    format!("« {name} » introuvable dans le PATH")
}

/// The extensions of the programs Windows runs, in the order it tries them.
const WINDOWS_PROGRAMS: [&str; 4] = ["exe", "cmd", "bat", "com"];

/// Where the program `name` is on `PATH`, if it is there.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let files = file_names(name, cfg!(windows));
    // An empty or relative entry would be looked up from the current folder.
    std::env::split_paths(&std::env::var_os("PATH")?)
        .filter(|dir| dir.is_absolute())
        .find_map(|dir| {
            files
                .iter()
                .map(|file| dir.join(file))
                .find(|file| is_program(file))
        })
}

/// The file names the program `name` may have: on Windows, with each
/// extension of a program, unless it has one already.
fn file_names(name: &str, windows: bool) -> Vec<String> {
    let has_one = Path::new(name).extension().is_some_and(|ext| {
        WINDOWS_PROGRAMS.contains(&ext.to_string_lossy().to_lowercase().as_str())
    });
    if !windows || has_one {
        return vec![name.to_string()];
    }
    WINDOWS_PROGRAMS
        .iter()
        .map(|ext| format!("{name}.{ext}"))
        .collect()
}

/// A file that can run: on Unix, one with an execute bit.
#[cfg(unix)]
fn is_program(file: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(file).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_program(file: &Path) -> bool {
    file.is_file()
}

/// Starts the entry without waiting for it: its output goes nowhere.
pub fn detach(entry: &Entry, cwd: Option<&Path>) -> Result<(), String> {
    spawn_detached(&entry.command, cwd)
        .map_err(|e| format!("impossible de lancer « {} » : {e}", entry.label))
}

/// Starts `argv` without waiting for it, outside the plugin's pane (see
/// `leave_the_pane`).
pub fn spawn_detached(argv: &[String], cwd: Option<&Path>) -> Result<(), String> {
    let mut command = command(argv)?;
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    leave_the_pane(&mut command);
    detach_command(command)
}

/// Leaves out the variables that only hold in the plugin's pane (the
/// `Tools` pane, a popup): which pane it is, its workspace, its tab and the
/// folder it started in, the plugin's own (`HERDR_PLUGIN_*`,
/// `HERDR_MENU_*`), the binary that runs this Herdr, and the one by which
/// Herdr tells its panes. A program started from the plugin runs outside
/// that pane: it would otherwise act on it, or take itself for one of the
/// plugin's commands.
pub fn leave_the_pane(command: &mut Command) {
    for (name, _) in std::env::vars_os() {
        if is_the_panes(&name.to_string_lossy()) {
            command.env_remove(&name);
        }
    }
}

fn is_the_panes(name: &str) -> bool {
    use herdr_api::env::{BIN_PATH, INSIDE, PANE_ID, TAB_ID, WORKSPACE_ID};
    let name = name.to_uppercase();
    name.starts_with("HERDR_PLUGIN_")
        || name.starts_with("HERDR_MENU_")
        || [INSIDE, PANE_ID, WORKSPACE_ID, TAB_ID, BIN_PATH].contains(&name.as_str())
}

/// Starts `command` without waiting for it: its output goes nowhere, and it
/// keeps running when the pane that started it closes.
pub fn detach_command(mut command: Command) -> Result<(), String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // A group of its own: closing the plugin's pane hangs up the pane's group,
    // not the program it started.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    // Nor does it end with the console of the plugin's pane on Windows.
    #[cfg(windows)]
    {
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        std::os::windows::process::CommandExt::creation_flags(
            &mut command,
            DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
        );
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    // Reaped in the background, so that it leaves no zombie while the pane runs.
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_windows_program_keeps_its_own_extension() {
        assert_eq!(file_names("code", false), ["code"]);
        assert_eq!(
            file_names("code", true),
            ["code.exe", "code.cmd", "code.bat", "code.com"]
        );
        assert_eq!(file_names("code.CMD", true), ["code.CMD"]);
        assert_eq!(file_names("v1.2", true)[0], "v1.2.exe");
    }

    #[test]
    fn leaves_out_only_the_panes_variables() {
        for name in [
            "HERDR_PANE_ID",
            "HERDR_ENV",
            "HERDR_PLUGIN_STATE_DIR",
            "HERDR_MENU_BAR",
            "HERDR_TAB_ID",
            "herdr_workspace_id",
        ] {
            assert!(is_the_panes(name), "{name}");
        }
        for name in [
            "HERDR_SOCKET_PATH",
            "HERDR_CONFIG_PATH",
            "PATH",
            "HERDR_ENVX",
        ] {
            assert!(!is_the_panes(name), "{name}");
        }
    }
}
