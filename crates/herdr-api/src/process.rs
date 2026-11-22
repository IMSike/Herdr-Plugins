//! Running the programs a plugin starts.

use std::process::Command;

/// Opens no console window for `command` on Windows; nothing elsewhere. A
/// plugin's command has no console of its own: a console program it starts
/// would otherwise flash a window.
pub fn no_window(command: &mut Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::os::windows::process::CommandExt::creation_flags(command, CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// Leaves out of `command`'s environment the variables of this process that
/// `which` names (any case, as Windows reads them).
pub fn leave_out(command: &mut Command, which: impl Fn(&str) -> bool) {
    for (name, _) in std::env::vars_os() {
        if which(&name.to_string_lossy()) {
            command.env_remove(&name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_out_the_variables_named() {
        let mut command = Command::new("x");
        leave_out(&mut command, |name| name.eq_ignore_ascii_case("PATH"));
        let removed: Vec<_> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().to_uppercase())
            .collect();
        assert_eq!(removed, ["PATH"]);
    }
}
