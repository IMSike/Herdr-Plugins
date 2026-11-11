# ADR 0009 — Showing new versions, choosing what to update

- **Status**: accepted
- **Date**: 2026-11-11

## Context

Nothing updates on its own: Herdr through `herdr update`, its plugins installed from GitHub through `herdr plugin update` ([ADR 0002](0002-plugin-versions.md)). The menu's **Update** entry only updated Herdr ([ADR 0005](0005-herdr-update-entry.md)), and nothing said a new version was out: Herdr announces its own in its interface, not the plugins'.

What Herdr offers to know whether something is new:

- `herdr status --json` gives the installed version and the channel (`stable` or `preview`), but not the latest published version. Its documentation gives `https://herdr.dev/latest.json`, the latest stable version, to download the binary on a remote machine; its binary shows that Herdr's own announcement reads it too (`preview.json` for the preview channel). That announcement can be turned off (`update.version_check`), and only concerns Herdr.
- `plugin.list` (the API) gives, for a plugin installed from GitHub, its repository, the ref asked at install (`requested_ref`, missing for the default branch) and the installed commit (`resolved_commit`). `herdr plugin update` fetches that ref again and skips the plugin when the commit did not change; it has no "check only" mode.
- On Windows, the server cannot hand its sessions over to a new version: running the new Herdr stops every agent.

Options considered:

| Option | For | Against |
|---|---|---|
| a. Automatic update, in the background | Nothing to do | Code changes without anyone deciding it (a release's `SHA256SUMS` vouch for the file, not for its author); for Herdr, a restart would stop agents at work; every GitHub plugin, others' included |
| b. Show new versions, update in one click, choosing what | Nothing changes without a click; the restart stays confirmed | A request to herdr.dev and one per GitHub plugin every few hours; more code |
| c. Change nothing | Nothing to write | New plugin versions go unnoticed |

## Decision

Option b, chosen by the maintainer:

1. **Detection**, on by default, can be turned off (`check_updates = false` in `menu.toml`, the "Signaler les mises à jour" line of the settings). It runs at Herdr's start (the plugin's `[[startup]]` command) then every 4 hours (by a `Tools` pane, on a thread of its own), one at a time (a lock in the state folder); its result is kept in the state folder (`updates.json`) for every pane.
   - Herdr: the version of `herdr status --json` against the one of `latest.json`, on the stable channel only (`curl`, 20 seconds at most).
   - Plugins installed from GitHub: `git ls-remote` of their ref (`HEAD` without `--ref`, otherwise the branch, or the tag's commit, of that exact name) against the installed commit, without prompt nor credentials window, 20 seconds at most. A linked plugin (`herdr plugin link`) is left out: Herdr does not update it.
2. **Display**: the Update icon of the `Tools` pane in green, its name on hover followed by what has a new version; the Update line of the popup menu likewise. The popup menu checks nothing itself.
3. **Update**: the Update entry's window checks again, then lists what has a new version, all ticked, each line can be unticked. Herdr is also listed when its new version is not known (the preview channel, no answer), as before. It runs `herdr update` first (a plugin's new version may need Herdr's), then `herdr plugin update <id>…` for the ticked plugins, without `--yes`: Herdr keeps its own confirmation. It then checks again, even when a step failed. The rest of ADR 0005 (a confirmed restart) does not change, and only happens when Herdr was updated.

## Consequences

- The plugin only compares versions: no code is installed without a click, and the server's restart is still asked, with what it stops.
- Network requests go out without the user doing anything: one to herdr.dev, one to GitHub per plugin installed from GitHub, at start then every 4 hours while a `Tools` pane is open. Without a `Tools` pane, only the check at start runs.
- What does not answer keeps what the last check found, and no error is shown: offline, a new version already found stays shown; when nothing answered, nothing is kept, and a `Tools` pane checks again 10 minutes later.
- Herdr's own announcement and this check are independent: each can be turned off without the other.
- `latest.json` is documented to download the binary, not as a way to compare versions: should its shape change, Herdr is no longer shown (its version stays "unknown"), but the Update entry still offers it and `herdr update` works as before.
- A plugin's commit is compared, not its release: right after a push to `main`, before its release is published, a plugin of this repository can turn green, and its update build with `cargo` or fail ([ADR 0008](0008-prebuilt-binaries.md)).
- In a repository with several plugins, a commit to one of them shows them all: Herdr records the repository's commit, not the plugin's.
- A Herdr installed by a package manager (Homebrew, mise, Nix) is shown like the others, whereas `herdr update` is only for Herdr's own installer: it updates through its manager.
- It needs `curl`, which Windows ships since Windows 10 1803, and `git`, which the plugins' install already requires.
