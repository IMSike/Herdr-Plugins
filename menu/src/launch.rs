//! Starting an entry's command: in the `run` popup, or on its own for the
//! programs that open their own window (`popup = false`).

use crate::config::Entry;
use std::path::Path;
use std::process::{Command, Stdio};

/// The command of `argv`, without a shell. On Windows a bare name is looked
/// up on `PATH` here: Rust's own search tries the system folders first (`bash`
/// would be WSL's, not the Git Bash found on `PATH`), and a batch file (`code`
/// is `code.cmd`) runs with the right escaping when given its full path.
pub fn command(argv: &[String]) -> Result<Command, String> {
    let (name, args) = argv.split_first().ok_or("commande vide")?;
    let mut command = Command::new(program(name));
    command.args(args);
    Ok(command)
}

#[cfg(windows)]
fn program(name: &str) -> std::ffi::OsString {
    let bare = Path::new(name);
    if bare.extension().is_some() || bare.components().count() > 1 {
        return name.into();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return name.into();
    };
    for dir in std::env::split_paths(&path) {
        for ext in ["exe", "cmd", "bat"] {
            let file = dir.join(format!("{name}.{ext}"));
            if file.is_file() {
                return file.into_os_string();
            }
        }
    }
    name.into()
}

#[cfg(not(windows))]
fn program(name: &str) -> std::ffi::OsString {
    name.into()
}

/// Starts the entry without waiting for it: its output goes nowhere.
pub fn detach(entry: &Entry, cwd: Option<&Path>) -> Result<(), String> {
    let mut command = command(&entry.command)?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    // A group of its own: closing the bar's pane hangs up the pane's group,
    // not the program it started.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    // Nor does it end with the console of the bar's pane on Windows.
    #[cfg(windows)]
    {
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        std::os::windows::process::CommandExt::creation_flags(
            &mut command,
            DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
        );
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("impossible de lancer « {} » : {e}", entry.label))?;
    // Reaped in the background, so that it leaves no zombie while the bar runs.
    std::thread::spawn(move || child.wait());
    Ok(())
}
