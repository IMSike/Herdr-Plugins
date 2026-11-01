# ADR 0005 — Updating Herdr from the menu

- **Status**: accepted
- **Date**: 2026-11-01

## Context

The maintainer wanted to update Herdr from the `Tools` pane. `herdr update` installs the new version, but the running server keeps the old binary until it restarts. Live handoff (`herdr update --handoff`) keeps the sessions, but the server does not offer it on Windows (`herdr status --json`: `server.capabilities.live_handoff = false`). There, the server must stop: every pane closes and every agent stops, possibly in the middle of its work. A process that runs inside Herdr cannot start Herdr again after stopping it.

Options studied:

| Option | Pros | Cons |
|---|---|---|
| a. Update only; the new version runs at the next start | Nothing stops | The update waits for the user to quit Herdr, without being told |
| b. Update, then ask before stopping the server, saying which agents stop | One key; nothing stops without a yes that names the cost | The user starts Herdr again by hand |
| c. Update and restart without asking | One key | Stops agents at work without warning |

## Decision

Option b, as the menu's built-in entry **Herdr** (`update-herdr`), run in the large popup:

1. `herdr update`, with `--handoff` when the server offers it;
2. `herdr status --json`: when neither `restart_needed` nor `server_binary_stale` is set, the popup says Herdr is up to date;
3. otherwise it gives the number of panes that close and of agents that stop (and how many are at work), and asks `[o/N]`; anything but yes cancels;
4. once confirmed, it puts `herdr` in the clipboard (clip, pbcopy, wl-copy, xclip or xsel), waits for Enter, then runs `herdr server stop`.

## Consequences

- No agent stops without a confirmation that says so.
- Where handoff works, the confirmation is not asked: nothing stops.
- Starting Herdr again stays a manual step, eased by the clipboard.
- The entry depends on `herdr status --json` keeping its `update` fields; if they go, the entry says Herdr is up to date and the new version runs at the next start.
