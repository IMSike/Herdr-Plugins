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

The menu's built-in entry **Réglages** opens Herdr's `config.toml`, `menu.toml` and, when the sidebar is installed, `sidebar.toml` in the user's editor, creating a plugin's file from a commented template first; when the editor closes, it checks the files and reloads Herdr's configuration. No plugin writes Herdr's `config.toml`.

A settings screen may come later, writing the same files.

## Consequences

- Colors and layout stay in Herdr's `config.toml`; the plugins' files only hold what Herdr cannot set.
- The files can be versioned with the rest of the configuration (OS-Setup's `dotfiles/herdr/`).
- A new setting means a field with its default, the template, and the plugin's page, in both languages.
- Until the sidebar's `install` has run once after an update, its tab bar runs use the defaults.
