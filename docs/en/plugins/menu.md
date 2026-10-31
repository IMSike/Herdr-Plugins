# menu

**English** · [Français](../../fr/plugins/menu.md)

A small popup menu, opened by one key, that starts a tool (lazygit, gh-dash…) in a large popup over the current workspace. Tabs and panes stay as they are behind it; quitting the tool closes the popup.

## Installation

```sh
herdr plugin install IMSike/Herdr-Plugins/menu
```

Then bind a key to its action in Herdr's `config.toml`:

```toml
[[keys.command]]
key = "prefix+space"
type = "plugin_action"
command = "imsike.menu.open"
description = "tools menu"
```

## Use

`prefix+space` opens the menu, titled with the workspace's folder name. Choose an entry with its key, with the arrows (or `j`/`k`) and Enter, or with a click. Escape, `q` or Ctrl+C closes the menu. An entry's own key wins over `j`, `k` and `q`.

The tool starts in the workspace's folder (or the focused pane's folder, see below) in a popup of 95% of the terminal. When the tool cannot start or ends with an error, the popup shows why and waits for Enter.

## Entries

Without configuration the menu offers:

| Key | Entry | Command |
|---|---|---|
| `l` | lazygit | `lazygit` |
| `g` | gh-dash | `gh dash` (the [gh-dash](https://github.com/dlvhdr/gh-dash) extension of the GitHub CLI) |

To choose your own, write `menu.toml` in the plugin's config folder (`herdr plugin config-dir imsike.menu` prints it). The file replaces the built-in entries:

```toml
[[entries]]
key = "l"                 # one character
label = "lazygit"
command = ["lazygit"]     # program and arguments, run without a shell

[[entries]]
key = "t"
label = "btop"
command = ["btop"]
cwd = "pane"              # "workspace" (default) or "pane": the focused pane's folder
```

A file that cannot be read or is invalid (unknown field, empty command, a key used twice) is ignored: the menu shows the built-in entries and the error on its bottom line. The file is read each time the menu opens; no reload is needed.

## How it works, implications

- Herdr shows one popup at a time. The action therefore stays running while the menu is open: it waits for the menu to write the choice into a file of the plugin's state folder, then opens the tool's popup once the menu's is gone. While waiting it uses about 4 MB of memory and checks the file 20 times a second.
- If the menu popup is closed by something else than the menu itself, the action keeps waiting, invisibly, for at most one hour.
- Commands run as your user, with your environment, like any program you would start in a pane.

## Turn it off

Remove the key from `config.toml`, then `herdr plugin disable imsike.menu` (or `herdr plugin uninstall imsike.menu`). `menu.toml` stays in the config folder until you delete it.
