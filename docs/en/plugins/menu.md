# menu

**English** · [Français](../../fr/plugins/menu.md)

Starts a tool (lazygit, gh-dash, VS Code…) from one click or one key, updates Herdr, and opens the settings files. In a tab that shows the file explorer of [herdr-explorer](https://github.com/gcaudan/herdr-explorer), the menu is a short `Tools` pane above it, in its column: icons, names, and figures on the workspace folder. Elsewhere it is a small popup. Tools that run in the terminal open in a large popup over the current workspace; tabs and panes stay as they are behind it, and quitting the tool closes the popup.

```
   󰊢        󰊤        󰨞       󰚰       󰒓
lazygit  gh-dash  VS Code   Herdr  Réglages

     1,5G · 0,2% du disque
         3 552 fichiers
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

**With the explorer.** The `Tools` pane opens with the explorer, however it is opened (`prefix+f`, opening on its own in a new tab), and closes with it. Click an icon or its name, or type the entry's key while the pane has the focus; hovering shows the entry in yellow. A tool that cannot start shows why in red, in place of the figures, for 5 seconds. `prefix+space` closes the pane and opens it again; Escape, `q` or Ctrl+C in the pane closes it. An entry's own key wins over `q`.

Under the icons, the figures on the workspace folder (the explorer's folder):

- the total size of its files, every file included (`.git` and build folders too), and its share of the disk it is on. A file linked several times (hard links, as pnpm makes) counts each time, and a cloud file not downloaded (OneDrive) counts at its full size;
- its number of files, those of `.git` left out, how old the figures are from an hour on (`· 3 h`), and an icon: a click on the figures measures the folder again (`…` while it is read).

**Without the explorer.** `prefix+space` opens the popup menu, titled with the workspace's folder name. Choose an entry with its key, with the arrows (or `j`/`k`) and Enter, or with a click. Escape, `q` or Ctrl+C closes the menu. An entry's own key wins over `j`, `k` and `q`.

A terminal tool starts in the workspace's folder (or a terminal's folder, see below) in a popup of 95% of the terminal. When it cannot start or ends with an error, the popup shows why and waits for Enter. A program with a window of its own (VS Code) starts directly, without the popup, and keeps running when the menu or the `Tools` pane closes.

## Entries

Without configuration the menu offers:

| Key | Entry | Command |
|---|---|---|
| `l` | lazygit | `lazygit` |
| `g` | gh-dash | `gh dash` (the [gh-dash](https://github.com/dlvhdr/gh-dash) extension of the GitHub CLI) |
| `c` | VS Code | `code .`: the workspace's folder in VS Code |
| `u` | Herdr | built-in action `update-herdr`: updates Herdr, see [below](#built-in-entries) |
| `s` | Réglages | built-in action `settings`: opens the settings files, see [below](#built-in-entries) |

To choose your own, write `menu.toml` in the plugin's config folder (`herdr plugin config-dir imsike.menu` prints it; the **Réglages** entry opens it, and first creates it with every setting commented out). Its entries replace the built-in ones, all of them; a file without entries keeps them:

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

[[entries]]
key = "u"
label = "Herdr"
action = "update-herdr"   # or "settings": a built-in entry, in place of command
```

At the top of the file, before the entries, the settings of the figures under the icons:

| Setting | Default | Effect |
|---|---|---|
| `decimal_comma` | `true` | `1,5G` (`true`) or `1.5G` (`false`) |
| `percent_space` | `false` | `0,2%` (`false`) or `0,2 %` with a narrow no-break space (`true`) |

The folder a tool starts in:

| `cwd` | From the `Tools` pane | From the popup menu |
|---|---|---|
| `workspace` | the explorer's folder | the workspace's folder |
| `pane` | the folder of the tab's first terminal | the focused pane's folder |

On Windows, a command name is looked up on `PATH`, before the system folders (`bash` is the first one on `PATH`, Git Bash for instance, not WSL's), and a batch file (`code` is `code.cmd`) runs without naming its extension.

A file that cannot be read or is invalid (unknown field, empty command, both a command and an action, a key used twice) is ignored: the menu shows the built-in entries, and shows the error: the popup menu above its help line, the `Tools` pane in red in place of the figures for 5 seconds. The `Tools` pane reads the file when it opens, the popup menu each time it opens; no reload is needed. A tool starts as the menu showed it, even when the file changed in between.

## Built-in entries

Both run in the large popup, which shows what they do and asks before anything that cannot be undone.

**Herdr** (`u`) installs the latest Herdr with `herdr update` (with `--handoff` where the server can hand its sessions over to the new version, which it cannot on Windows). Then `herdr status --json` tells whether the running server is still the old version:

- if not, the popup says Herdr is up to date and waits for Enter;
- if so, it says how many panes close and how many agents stop (`5 panes, dont 2 agents (1 au travail)`) and asks `Redémarrer maintenant ? [o/N]`. Anything but `o` (or `oui`) cancels: the new version runs at Herdr's next start.
- Once confirmed, `herdr` is put in the clipboard (with `clip`, `pbcopy`, `wl-copy`, `xclip` or `xsel`, whichever works), and Enter stops the server (`herdr server stop`): **every pane closes and every agent stops**, as when Herdr is quit. Paste the command in the terminal to start Herdr again.

**Réglages** (`s`) opens in an editor, together:

- Herdr's `config.toml` (the path Herdr's help gives, which follows `HERDR_CONFIG_PATH`);
- the menu's `menu.toml`;
- the sidebar's `sidebar.toml`, when the [sidebar](sidebar.md) plugin is installed.

A plugin's file that does not exist yet is first created with every setting commented out, at its default. The editor is `VISUAL`, else `EDITOR` (arguments allowed: `code --wait`); without them, on Windows, VS Code (`code --wait`: the popup waits until the files' tabs are closed) when it is on `PATH`, else Notepad, one file after the other; elsewhere `nano`, else `vi`. Once the editor is closed, the popup checks `menu.toml`, then `config.toml` (`herdr config check`), and reloads Herdr's configuration (`herdr server reload-config`); an error stays on screen until Enter. The sidebar reads its file at each run; the `Tools` pane reads `menu.toml` when it opens (`prefix+space` twice).

The plugin only opens Herdr's `config.toml`: it never writes it ([ADR 0004](../adr/0004-plugin-settings.md), [ADR 0005](../adr/0005-herdr-update-entry.md)).

## How it works, implications

**The `Tools` pane**

- It is a normal Herdr pane. Herdr only splits to the right or downwards: the pane opens below the explorer, then swaps places with it, and the focus goes back to the pane that had it.
- It opens on Herdr's `pane.created` event of the explorer, once the explorer stops moving (it places and sizes itself right after it opens: about one second). It closes on the `pane.closed` event, as soon as its tab has no explorer, or no terminal pane besides it and the explorer's own panes (Preview, Search…), so that the explorer can close its tab as it does on its own.
- It asks for 5 rows, but Herdr never gives a side of a split less than a tenth of it: on a screen of fewer than 60 rows, it is taller. It takes its height back when the window is resized.
- **While it is open, the explorer no longer keeps its width**: `<` and `>` do nothing and its width follows the window's in proportion. The explorer only looks for its width at the full height of its own pane; a change to it is proposed upstream.
- The figures are measured in the background: the whole folder is read, which takes a moment on a large one (`node_modules`, build folders). The reading stops after 200,000 files and folders or 2 seconds, whichever comes first: the figures then start with `≥` (at least). A reading asked for with a click goes up to 5 million files and folders or 60 seconds.
- The figures are kept in a file of the plugin's state folder, one per folder (its path and its figures; if it cannot be written, the pane says so in red for 5 seconds): the panes of the same folder share them (while one of them reads the folder, the others wait for its figures), and a pane that opens again shows them without reading the folder. They are measured again once they are older than:

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

To drop a built-in entry, list in `menu.toml` the entries to keep. To turn the whole menu off, remove the key from `config.toml`, then `herdr plugin disable imsike.menu` (or `herdr plugin uninstall imsike.menu`): the `Tools` panes close with their explorer or at the next restart. `menu.toml` stays in the config folder, and the figures' files in the state folder, until you delete them.
