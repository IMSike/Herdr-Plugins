# menu

**English** · [Français](../../fr/plugins/menu.md)

Starts a tool (lazygit, gh-dash, VS Code…) from one click or one key. In a tab that shows the file explorer of [herdr-explorer](https://github.com/gcaudan/herdr-explorer), the menu is a short `Tools` pane above it, in its column: icons, names, and figures on the workspace folder. Elsewhere it is a small popup. Tools that run in the terminal open in a large popup over the current workspace; tabs and panes stay as they are behind it, and quitting the tool closes the popup.

```
      󰊢        󰊤        󰨞
   lazygit  gh-dash  VS Code

     1.5G · 0,2 % du disque
         3 552 fichiers
```

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

The icons are [Nerd Font](https://www.nerdfonts.com) glyphs: without a Nerd Font in the terminal, they show as blanks or boxes.

## Use

**With the explorer.** The `Tools` pane opens with the explorer, however it is opened (`prefix+f`, opening on its own in a new tab), and closes with it. Click an icon or its name, or type the entry's key while the pane has the focus; hovering shows the entry in yellow. `prefix+space` closes the pane and opens it again; Escape, `q` or Ctrl+C in the pane closes it.

Under the icons, the figures on the workspace folder (the explorer's folder):

- its size on disk, every file included (`.git` and build folders too), and its share of the disk it is on;
- its number of files, those of `.git` left out, how old the figures are from an hour on (`· 3 h`), and an icon: a click on the figures measures the folder again (`…` while it is read).

**Without the explorer.** `prefix+space` opens the popup menu, titled with the workspace's folder name. Choose an entry with its key, with the arrows (or `j`/`k`) and Enter, or with a click. Escape, `q` or Ctrl+C closes the menu. An entry's own key wins over `j`, `k` and `q`.

A terminal tool starts in the workspace's folder (or a terminal's folder, see below) in a popup of 95% of the terminal. When it cannot start or ends with an error, the popup shows why and waits for Enter. A program with a window of its own (VS Code) starts directly, without the popup.

## Entries

Without configuration the menu offers:

| Key | Entry | Command |
|---|---|---|
| `l` | lazygit | `lazygit` |
| `g` | gh-dash | `gh dash` (the [gh-dash](https://github.com/dlvhdr/gh-dash) extension of the GitHub CLI) |
| `c` | VS Code | `code .`: the workspace's folder in VS Code |

To choose your own, write `menu.toml` in the plugin's config folder (`herdr plugin config-dir imsike.menu` prints it). The file replaces the built-in entries:

```toml
[[entries]]
key = "l"                 # one character
label = "lazygit"
command = ["lazygit"]     # program and arguments, run without a shell
icon = "󰊢"                # optional: a Nerd Font glyph; the key without it

[[entries]]
key = "t"
label = "btop"
command = ["btop"]
cwd = "pane"              # "workspace" (default) or "pane", see below

[[entries]]
key = "c"
label = "VS Code"
command = ["code", "."]
popup = false             # a program with a window of its own: no popup
```

The folder a tool starts in:

| `cwd` | From the `Tools` pane | From the popup menu |
|---|---|---|
| `workspace` | the explorer's folder | the workspace's folder |
| `pane` | the folder of the tab's first terminal | the focused pane's folder |

On Windows, a command found on `PATH` as a batch file (`code` is `code.cmd`) runs without naming its extension.

A file that cannot be read or is invalid (unknown field, empty command, a key used twice) is ignored: the menu shows the built-in entries, and the popup menu shows the error on its bottom line. The `Tools` pane reads the file when it opens, the popup menu each time it opens; no reload is needed. A tool starts as the menu showed it, even when the file changed in between.

## How it works, implications

**The `Tools` pane**

- It is a normal Herdr pane. Herdr only splits to the right or downwards: the pane opens below the explorer, then swaps places with it, and the focus goes back to the pane that had it.
- It opens on Herdr's `pane.created` event of the explorer, once the explorer stops moving (it places and sizes itself right after it opens: about one second). It closes on the `pane.closed` event, as soon as its tab has no explorer, or no pane besides it and the explorer, so that the explorer can close its tab as it does on its own.
- It asks for 5 rows, but Herdr never gives a side of a split less than a tenth of it: on a screen of fewer than 60 rows, it is taller. It takes its height back when the window is resized.
- **While it is open, the explorer no longer keeps its width**: `<` and `>` do nothing and its width follows the window's in proportion. The explorer only looks for its width at the full height of its own pane; a change to it is proposed upstream.
- The figures are measured in the background: the whole folder is read, which takes a moment on a large one (`node_modules`, build folders). The reading stops after 200,000 files and folders or 2 seconds, whichever comes first: the figures then start with `≥` (at least). A reading asked for with a click goes up to 5 million files and folders or 60 seconds.
- The figures are kept in a file of the plugin's state folder, one per folder (its path and its figures): the panes of the same folder share them, and a pane that opens again shows them without reading the folder. They are measured again once they are older than:

  | Files and folders read | Measured again after |
  | --- | --- |
  | fewer than 10,000 | 2 minutes |
  | fewer than 50,000 | 15 minutes |
  | fewer than 200,000 | 2 hours |
  | more, or a reading stopped at its limit | 1 day |

- It finds the explorer by its pane's label, `Explorer`; a later version of the explorer that renamed it would leave the menu as a popup only.
- After a Herdr restart, Herdr restores the pane as a plain shell; the plugin closes it at startup, and the explorer, when it reopens, gets a new one.

**The popup menu**

- Herdr shows one popup at a time. The action therefore stays running while the menu is open: it waits for the menu to write the choice into a file of the plugin's state folder, then opens the tool's popup once the menu's is gone. While waiting it uses about 4 MB of memory, checks the file 20 times a second, and checks once a second that the menu still runs.
- If the menu popup is closed by something else than the menu itself, the action sees within a second that the menu is gone, and stops.

Commands run as your user, with your environment, like any program you would start in a pane.

## Turn it off

Remove the key from `config.toml`, then `herdr plugin disable imsike.menu` (or `herdr plugin uninstall imsike.menu`): the `Tools` panes close with their explorer or at the next restart. `menu.toml` stays in the config folder, and the figures' files in the state folder, until you delete them.
