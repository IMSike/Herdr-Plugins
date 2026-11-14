# ADR 0011 — An action per tool

- **Status**: accepted
- **Date**: 2026-11-14

## Context

The maintainer wanted one key per tool (`prefix+l` for lazygit, `prefix+d` for gh-dash…), without going through the menu. Herdr's own `[[keys.command]]` with `type = "popup"` can run a program, but in the focused pane's folder rather than the workspace's, without the menu's error message, and apart from `menu.toml`. A plugin action receives no argument: only its id (`HERDR_PLUGIN_ACTION_ID`), and the actions are fixed in the manifest, while the menu's entries, keys included, are set in `menu.toml`.

Options studied:

| Option | Pros | Cons |
|---|---|---|
| a. Herdr's `type = "popup"` / `type = "shell"` commands | No code | Focused pane's folder; no menu error message; `menu.toml` not followed |
| b. One action per menu key (`entry-a` … `entry-z`), starting the entry of that key | The key stays valid whatever tool `menu.toml` puts behind it | 26 actions declared; the binding names a letter, not a tool |
| c. One action per built-in tool (`lazygit`, `gh-dash`, `vscode`, `btop`) | Four readable actions, named after what they start | An entry renamed or added in `menu.toml` has no action |

## Decision

Option c. The four actions share one command, `herdr-menu tool`, which reads the action's id. It starts the entry of `menu.toml` with the tool's built-in label (`lazygit`, `gh-dash`, `VS Code`, `btop`, letter case aside), the built-in entry when there is none, the way the popup menu starts a chosen entry: the `run` popup, or on its own for a program with a window of its own, in the folder its `cwd` asks for. No key is bound by the plugin: Herdr's plugin guide gives the manifest no section for keys, which are bound in the user's `config.toml`.

## Consequences

- One key starts a tool, in the same popup and folder as from the menu.
- An entry's command or `cwd` changed in `menu.toml` is followed; an entry renamed is not: the action starts the built-in one.
- A tool added to `menu.toml` has no action of its own; adding one means a new action in the manifest.
- A `menu.toml` that cannot be read is logged and the built-in entry starts.
