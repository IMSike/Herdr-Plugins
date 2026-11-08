# ADR 0004 — Plugin settings

- **Status**: accepted
- **Date**: 2026-11-01

## Context

The maintainer wanted to set what the plugins write (number format, how long a value lasts, which processes are shells) without changing their code, and to reach every setting from the menu.

Herdr gives each plugin a config folder (`HERDR_PLUGIN_CONFIG_DIR`, `herdr plugin config-dir <id>`), but no settings of its own for a plugin, and no screen to edit them. How the sidebar's values look (colors, order, rows) is already Herdr's: its `config.toml`, `[ui.sidebar.*]`. The sidebar's tab bar runs get no plugin environment, so no config folder.

Options studied:

| Option | Pros | Cons |
|---|---|---|
| A TOML file per plugin in its config folder, and a menu entry that opens the files in an editor | Plain text, commented, versionable (OS-Setup); the editor validates nothing but Herdr and the plugins do; little code | No form; a typo is only reported when the file is read |
| A settings screen (TUI) in the menu | Guided, each value checked as it is typed | Much code for few settings; to keep in step with each plugin's settings |
| Settings in Herdr's `config.toml` | One file | Herdr refuses unknown keys; a plugin must never write that file |

## Decision

Each plugin reads a TOML file of its own in its config folder: `menu.toml` (settings at the top, then the entries), `sidebar.toml`. A missing file or setting keeps its default; an unknown setting makes the file ignored, with the reason shown (menu) or logged (sidebar). The sidebar's `install` writes its config folder's path in its state folder for the tab bar runs.

The menu's built-in entry **Settings** opens a settings screen (TUI) in a popup, for the few settings of the plugins (number format, the `Tools` pane's totals, the sidebar's settings). Each change is written at once to the plugin's file with `toml_edit`, which keeps its comments and its other settings; a setting commented out in the file takes its value in its place. A plugin's file that does not exist is created from its commented template first.

The screen also opens Herdr's `config.toml`, `menu.toml` and, when the sidebar is installed, `sidebar.toml` in the user's editor, and closes: a popup holds Herdr while it is open, so the editing happens outside it. An editor with a window of its own starts detached; a terminal editor opens in a Herdr tab, which offers to reload Herdr's configuration once the editor is closed. The `Tools` pane the screen was opened from offers the reload as a button, and a way back to its icons. No plugin writes Herdr's `config.toml`.

*Revised on 2026-11-01: the entry first opened the files in an editor inside its popup, which held Herdr until the editor was closed; the settings screen was planned for later.*

## Consequences

- Colors and layout stay in Herdr's `config.toml`; the plugins' files only hold what Herdr cannot set.
- The files can be versioned with the rest of the configuration (OS-Setup's `dotfiles/herdr/`).
- A new setting means a field with its default, the template, a line of the settings screen, and the plugin's page, in both languages.
- The menu writes `menu.toml` and `sidebar.toml`, never Herdr's `config.toml`; a versioned copy of the plugins' files (OS-Setup) must be updated by hand after a change made on the screen.
- Until the sidebar's `install` has run once after an update, its tab bar runs use the defaults.
- The sidebar's settings are read by the sidebar and the menu with the same code, in `crates/herdr-api`; the template of `sidebar.toml` stays in `sidebar/templates/`, with the plugin whose file it is.
