# ADR 0010 — Recovering a session from Herdr's snapshots

- **Status**: accepted
- **Date**: 2026-11-11

## Context

On Windows, Herdr's panes run `pwsh.exe`. PowerShell 7 from the Microsoft Store updates by closing every `pwsh.exe` of its package, with no way to avoid it short of not using that package. Herdr's server survives, but every pane whose shell dies is taken out of the layout, its agent (a child of the shell) stops with it, and `session.json` is written again without them. The conversations themselves are kept by the agents (`claude --resume <id>`), but nothing in Herdr points to them any more.

What Herdr offers (measured on Herdr 0.9.5, and in its documentation, *Session state and restore*):

- Up to 48 copies of the layout in `session-snapshots/`, beside `session.json`, at most one every 15 minutes: when the layout changes, the previous one is copied if the last copy is older than that. In a test, killing three panes' shells more than 15 minutes after the last copy left a snapshot with the 4 panes and 3 agents, and a `session.json` with 2 panes and no agent.
- A documented recovery, by hand: stop the server, keep a copy of `session.json`, copy the chosen snapshot over it, start the session again. At its start, Herdr recreates the panes in their folders and resumes their agents (`[session] resume_agents_on_restore`, on by default): measured with an agent that reports its own resume command, and with a real `claude --resume` that came back to its conversation in its folder.
- No screen to choose a snapshot: they are files named after a time in nanoseconds, all alike.
- `herdr session list --json` gives each session's folder and socket; `herdr server stop` stops the session of `HERDR_SOCKET_PATH`, and has written `session.json` for the last time when it returns. A window opened by `cmd /c start` from a pane keeps running once the server stops.
- A `session.json` of an unknown version is not loaded: Herdr keeps a copy in `session-backups/`.

Questions decided:

| Question | Options | Choice |
|---|---|---|
| Which copies | a. Herdr's snapshots only. b. Copies the plugin writes too, at each agent event | a: the store closes `pwsh` while the session is locked, usually long after the last change, when Herdr's copy is exact. Only what changed in the 15 minutes before is lost. b would write files while agents work, for a rare case |
| What is restored | a. The whole snapshot, server stopped. b. Only the missing panes, recreated through the API, server running | a: the documented, measured way. b is much more code, never measured, and would have to rebuild splits |
| Where | a. A line of the settings screen. b. An entry of the menu too | a: a rare action, not worth a place in the bar |
| Which sessions | a. The current one. b. Any | a: restoring another session would stop a server from a session the user is not looking at |
| Start again | a. In the window. b. The command in the clipboard | b, chosen by the maintainer: the window says so and shows the command |
| Claude Code's titles | a. The conversation's id only. b. Its title read from Claude Code's transcript | b, chosen by the maintainer, with a as the fallback |

## Decision

A line **Récupérer une session…** in the settings screen, section **Session**, opens a screen in the same popup:

1. The current session's snapshots (found through `herdr session list --json` and `HERDR_SOCKET_PATH`), the latest first: their local date and age, what the session lacks now compared with each (`+3 absents`: an agent by its conversation, or by its resume command; a shell by its folder), and their workspaces, tabs, panes and agents. A snapshot of a version other than 3, or unreadable, is listed and cannot be restored.
2. The selected snapshot in detail: workspaces, tabs, panes with their folder, agents with their kind and the start of their conversation's id, the panes the session lacks marked `+`. For Claude Code, the conversation's last title (`ai-title`), else its last prompt, read from `~/.claude/projects/*/<id>.jsonl` (`CLAUDE_CONFIG_DIR` when set); an internal file of Claude Code, which only gives the id when it changes.
3. Enter opens a terminal window of its own, outside Herdr, as Update does ([ADR 0005](0005-herdr-update-entry.md)), running `herdr-menu recover`. It checks that the snapshot is one of the session's, says what it holds and what closes, and asks `[o/N]`; once confirmed, it copies `session.json` to `session-before-restore-<date>-<time>.json` beside it, stops the server (`herdr server stop`) and waits until Herdr says it no longer runs, writes the snapshot over `session.json` at once (a file renamed over it), and puts `herdr` (`herdr --session <name>` for a named session) in the clipboard, which it shows.

## Consequences

- Getting lost agents back takes the settings, a line, Enter, `o`, and a paste in a terminal; each agent comes back on its conversation, in its folder, after its last saved message: the turn under way when it stopped is lost.
- Every pane closes, including those that survived: Herdr recreates them from the snapshot. What is not an agent (a dev server, a watcher) comes back as a shell in its folder.
- Changes made in the 15 minutes before the panes died may be missing from every snapshot.
- The previous `session.json` stays beside it until deleted: copied back over `session.json`, server stopped, it brings the session back as it was.
- The titles depend on a file of Claude Code that is not documented; when it changes, the screen shows the ids alone.
- The dates use `chrono` (local time zone), a new dependency of the menu.
