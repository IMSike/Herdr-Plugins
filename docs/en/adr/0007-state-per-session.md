# ADR 0007 — State per Herdr session

- **Status**: accepted
- **Date**: 2026-11-04

## Context

Herdr runs named sessions side by side (`herdr --session <name>`), each with its own server and socket (`…/sessions/<name>/herdr.sock`; the default session's socket sits in Herdr's config folder). Plugins are installed for the user, in every session, and Herdr gives each plugin one state folder for all of them (`HERDR_PLUGIN_STATE_DIR`). Pane and tab ids start over at `w1` in each session.

The menu kept its bars (`bars/<pane id>`) and the tabs' locks there, and every `pane.closed` removes the bars Herdr no longer lists: a second session removed the first one's, which then opened a second bar on `prefix+space`. The sidebar's `totals.json` and `sample.json` were overwritten by whichever session ran last.

Options studied:

| Option | Pros | Cons |
|---|---|---|
| a. One folder per session in the state folder, named after the session's socket | Sessions no longer see each other's files; no setting | One more folder level; a socket set elsewhere by `HERDR_SOCKET_PATH` counts as the default session |
| b. Document "one Herdr session at a time" | No code | Named sessions break the bar and mix the totals |

## Decision

Option a. `herdr_api::session_dir` is `sessions/<session>/` in the plugin's state folder: `<session>` is the folder of a socket at `…/sessions/<name>/herdr.sock`, `default` otherwise. It holds what names a pane or a tab, or describes the session:

- menu: the `bars` folder, the tabs' locks (`opening-*.lock`), the flags of the files open from a `Tools` pane (`files-open-*`);
- sidebar: `sample.json` and `totals.json`, which the `Tools` pane reads from the sidebar's folder of the same session.

What is not tied to a session stays at the top of the state folder: the figures of a folder (`stats-*`), the update's lock (Herdr's binary is the same for every session), the menu's choice files, the sidebar's binary and `config-dir`, and `seq`, which only has to grow.

## Consequences

- Two sessions each have their bars and their totals.
- The files written before this change at the top of the state folder (`bars`, `opening-*.lock`, `totals.json`, `sample.json`) are no longer read; they can be deleted.
- A deleted session leaves its folder behind, as the state folder's other files.
