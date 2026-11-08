# menu

**English** · [Français](../../fr/plugins/menu.md)

Starts a tool (lazygit, gh-dash, VS Code…) from one click or one key, updates Herdr, and sets the plugins. In a tab that shows the file explorer of [herdr-explorer](https://github.com/gcaudan/herdr-explorer), the menu is a short `Tools` pane above it, in its column: icons and their names, and the CPU and memory of Herdr and of the machine. Elsewhere it is a small popup. Tools that run in the terminal open in a large popup over the current workspace; tabs and panes stay as they are behind it, and quitting the tool closes the popup.

```
      󰊢         󰊤         󰄪         󰨞
   lazygit   gh-dash     btop    VS Code
           󰋊         󰚰         󰒓
          Disk     Update   Settings
            2% 3,1G · 󰍹 10% 22,4G
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

**With the explorer.** The `Tools` pane opens with the explorer, however it is opened (`prefix+f`, opening on its own in a new tab), and closes with it. Click an icon, or type the entry's key while the pane has the focus; hovering shows the entry in yellow. A tool that cannot start shows why in red, under the icons, for 5 seconds. A double click, or a key held down, starts the entry once. Only `prefix+space`, which closes the pane and opens it again, and the closing of the explorer close the pane: no key typed in the pane (Escape, `q`, Ctrl+C) closes it, so that one Escape too many does not take it away. For the explorer, too, to close with `prefix+f` alone, take the keys away from its `close` action in its `keys.json` (`{ "explorer": { "close": [] } }`, see [its documentation](https://github.com/gcaudan/herdr-explorer/blob/main/docs/configuration.md)); Escape still closes the preview.

The icons stand on one row; the row under it shows the name of the icon under the mouse, and nothing otherwise.

Under the icons, the CPU and memory of Herdr's processes (its server and everything it runs, every workspace's panes included, not the current workspace's alone), then of the machine, as the [sidebar](sidebar.md) measures them, in bold, shown one of two ways, set by `totals`:

- `line` (default): one row, `2% 3,1G · 󰍹 10% 22,4G`.
- `bars`: four rows, `CPU :`, `RAM :`, `CPU 󰍹 :`, `RAM 󰍹 :`, each with a bar as wide as the pane allows, then its value. A thin bar (`━━━━╸────`) fills by half cells with the share of the machine (Herdr's memory: of the machine's memory), each cell in the color its place takes in a gradient from green to red through the [thresholds](sidebar.md#threshold-colors) of its figure; its empty part is tinted, dimmed, with the color of each threshold's zone, so the bar shows how far the next threshold is. Herdr's CPU share is missing on the sidebar's first run: its bar stays empty, with `–`.

With the sidebar's threshold colors on (`colors = true` in `sidebar.toml`), each figure is green, yellow, orange or red as its thresholds set. Without the sidebar, or when its figures are more than 30 seconds old, the rows stay empty. The pane asks for its icons' 2 rows plus its totals' (1 or 4).

**Without the explorer.** `prefix+space` opens the popup menu, titled with the workspace's folder name. Choose an entry with its key, with the arrows (or `j`/`k`) and Enter, or with a click. Escape, `q` or Ctrl+C closes the menu. An entry's own key wins over `j`, `k` and `q`. The **Disk** entry belongs to the `Tools` pane only: the popup menu leaves it out.

A terminal tool starts in the workspace's folder (or a terminal's folder, see below) in a popup of 95% of the terminal. When it cannot start or ends with an error, the popup shows why and waits for Enter. A program with a window of its own (VS Code) starts directly, without the popup, and keeps running when the menu or the `Tools` pane closes.

## Entries

Without configuration the menu offers:

| Key | Entry | Command |
|---|---|---|
| `l` | lazygit | `lazygit` |
| `g` | gh-dash | `gh dash` (the [gh-dash](https://github.com/dlvhdr/gh-dash) extension of the GitHub CLI) |
| `b` | btop | `btop`: the [btop](https://github.com/aristocratos/btop) system monitor (CPU, memory, disks, network, processes) |
| `c` | VS Code | `code .`: the workspace's folder in VS Code |
| `d` | Disk | built-in action `disk`: the figures on the workspace folder, see [below](#built-in-entries) |
| `u` | Update | built-in action `update`: updates Herdr, see [below](#built-in-entries) |
| `s` | Settings | built-in action `settings`: the settings screen, see [below](#built-in-entries) |

The plugin installs none of these tools. One that is missing opens its popup anyway, which says `« btop » introuvable dans le PATH : installez-le, ou changez sa commande dans menu.toml` and waits for Enter. btop is installed by the system's package manager (`apt install btop`, `brew install btop`…); on Windows it is [btop4win](https://github.com/aristocratos/btop4win) (`winget install aristocratos.btop4win`, or `scoop install btop`), whose program is `btop4win.exe`: when the package does not also put a `btop` on `PATH`, give the entry `command = ["btop4win"]` in `menu.toml`. The entry keeps `btop`, the name of the program everywhere else.

To choose your own, write `menu.toml` in the plugin's config folder (`herdr plugin config-dir imsike.menu` prints it; the settings screen creates it, with every setting commented out, when it first saves one or opens the files). Its entries replace the built-in ones, all of them; a file without entries keeps them:

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
label = "Update"
action = "update"         # or "settings", "disk": a built-in entry, in place of command
```

The folder a tool starts in:

| `cwd` | From the `Tools` pane | From the popup menu |
|---|---|---|
| `workspace` | the explorer's folder | the workspace's folder |
| `pane` | the folder of the tab's first terminal | the focused pane's folder |

A command name is looked up in the folders of `PATH` written in full: an empty entry (`PATH=:/usr/bin`) or `.` would run a program of that name found in the workspace's folder, a cloned repository's for instance. On Windows it is looked up before the system folders (`bash` is the first one on `PATH`, Git Bash for instance, not WSL's), and a batch file (`code` is `code.cmd`) runs without naming its extension; a name given with its extension (`code.cmd`, `bash.exe`) is looked up the same way.

A file that cannot be read or is invalid (unknown field, empty command, both a command and an action, a key used twice) is ignored: the menu shows the built-in entries, and shows the error: the popup menu above its help line, the `Tools` pane in red under the icons for 5 seconds. The popup menu reads the file each time it opens; the `Tools` pane reads it again within 2 seconds of a change, and takes the height of its new totals. A tool starts as the menu showed it, even when the file changed in between.

## Settings

At the top of `menu.toml`, before the entries; the settings screen changes them too:

| Setting | Default | Effect |
|---|---|---|
| `decimal_comma` | `true` | `1,5G` (`true`) or `1.5G` (`false`) |
| `percent_space` | `false` | `0,2%` (`false`) or `0,2 %` with a narrow no-break space (`true`) |
| `cpu_decimal` | `false` | the CPU shares of the totals: `4%` (`false`) or `4,2%` (`true`) |
| `memory_unit` | `"M"` | `853M`, `2,4G` (`"M"`); `853Mo`, `2,4Go` (`"Mo"`); `853MB`, `2,4GB` (`"MB"`); `K`, `M` or `G` as the size needs |
| `memory_space` | `false` | `2,4Go` (`false`) or `2,4 Go` with a narrow no-break space before the unit (`true`) |
| `totals` | `"line"` | the totals of the `Tools` pane: `"line"` (one row) or `"bars"` (four rows with bars), see [Use](#use) |

The `layout` setting of earlier versions, which offered two rows of icons, is gone: a file that still sets it stays valid, the setting is ignored.

## Built-in entries

**Disk** (`d`) turns the `Tools` pane to the figures on the workspace folder (the explorer's folder), under a **󰁍 Retour** button (Escape, `q` or Backspace go back too):

- the total size of its files, every file included (`.git` and build folders too), and its share of the disk it is on. A file linked several times (hard links, as pnpm makes) counts each time, and a cloud file not downloaded (OneDrive) counts at its full size;
- its number of files, those of `.git` left out, how old the figures are from an hour on (`· 3 h`), and an icon: a click on the figures, or `r`, measures the folder again (`…` while it is read).

The folder is only measured while this page is shown: going back stops the measuring. A folder that cannot be read (deleted, refused) or does not answer in time (a network share gone) is said so in red, and no figures are kept for it; so is an explorer whose folder Herdr does not give. Either way, `r` tries again.

**Update** (`u`) opens a terminal window of its own, outside Herdr: Herdr refuses to update from one of its panes. On Windows it is a new console window (opened by `cmd /c start`); elsewhere, the first terminal found among `x-terminal-emulator`, `gnome-terminal`, `konsole`, `xfce4-terminal`, `kitty`, `alacritty`, `wezterm`, `foot` and `xterm`. In it:

1. `herdr update` installs the latest Herdr (with `--handoff` where the server can hand its sessions over to the new version, which it cannot on Windows).
2. `herdr status --json` tells whether the running server is still the old version. If not, the window says Herdr is up to date: the client already open keeps its old version until you detach (`prefix+q`) and start `herdr` again.
3. If so, it says how many panes close and how many agents stop (`5 panes, dont 2 agents (1 au travail)`) and asks `Redémarrer maintenant ? [o/N]`. Anything but `o`, `oui`, `y` or `yes` cancels: the new version runs at the server's next start.
4. Once confirmed, it stops the server (`herdr server stop`): **every pane closes and every agent stops**, as when Herdr is quit. Then it offers to start Herdr again in that window (`[o/N]`); otherwise it puts `herdr` in the clipboard (with `clip`, `wl-copy`, `xclip` or `xsel`, whichever works), to paste in a terminal.

The window waits for Enter before closing, an error included. While it is open, another click on Update says that an update is under way instead of opening a second one. Without a graphical display (over SSH), or when the terminal closes at once, the `Tools` pane says so in red: run `herdr update` outside Herdr.

**Settings** (`s`) opens a settings screen in a popup of 70% of the terminal's width and 80% of its height:

```
 󰒓 Réglages                                                               1/17

  Format des nombres ────────────────────────────────────── 4% · 853M · 2,4G
  ▌ Virgule décimale                 2.4G   2,4G
    Espace avant %                   4%   4 %
    Unités de la mémoire             2,4G   2,4Go   2,4GB
  …
  Sidebar ───────────────────────────────────────────────────────────────────
    Durée de vie des valeurs        ‹ 20 s ›
    Shells                           bash, sh, zsh, fish, dash, nu, pwsh
  …
  ───────────────────────────────────────────────────────────────────────────
    Une virgule ou un point avant les décimales.  decimal_comma · menu.toml, sidebar.toml
    ✓ enregistré dans menu.toml
 ↑↓ naviguer   ←→ choisir   Espace suivant   Tab section   Échap fermer
```

- The settings are grouped in sections, each value in a column of its own, centered in a wide popup. A setting with a few values shows them all, the current one highlighted (in yellow on the selected line); the number formats are shown as they would write a figure, with the current format, and the header of their section shows a CPU share and two sizes in that format. A number (`‹ 20 s ›`) shows the arrows that change it; a set of thresholds shows the four colors around its values.
- Under the list: what the selected line does, with the setting and the files it writes; the last thing done (`✓` in green, or `✗` in red with the reason); and the keys of the selected line.
- Keys: up and down (or `j`/`k`) choose a line, Home/End (or `g`/`G`) the first or last, Tab the first line of the next section, Shift+Tab the first line of its section, or of the one above. Left and right (or `h`/`l`) move a choice towards its first or last value (stopping there) or a number by a step; Enter or Space go to the next value, round the list, or start typing; each change is saved at once. Escape, `q` or Ctrl+C closes the screen.
- Mouse: hovering a line chooses it, a click on one of its values chooses that value, a click on `‹` or `›` steps a number, a click elsewhere on the line acts as Enter, and the wheel moves the choice.
- On a small popup, the list scrolls (a scrollbar on its right says where it is), and the description, then the status line, give way to it; a name or value too long is cut with `…`, and typed text shows its end.

A file that is a link (deployed by stow or chezmoi, say) stays a link: the file it points to is written. When the file stays ignored after a change (another of its settings is invalid), the screen says so in red, with the reason, and shows the new value, which the file holds: the next change turns it back.

| Line | Effect | File |
|---|---|---|
| Virgule décimale, Espace avant %, CPU avec une décimale, Unités de la mémoire, Espace avant l'unité | `decimal_comma`, `percent_space`, `cpu_decimal`, `memory_unit`, `memory_space` | `menu.toml` and `sidebar.toml` |
| Totaux CPU / mémoire | `totals` | `menu.toml` |
| Durée de vie des valeurs | `ttl_seconds`, by 5 seconds, from 5 to 600 | `sidebar.toml` |
| Shells | `shells`: Enter to type them, separated by commas, Enter to save, Escape to cancel | `sidebar.toml` |
| Totaux dans la barre d'onglets | `tab_bar` | `sidebar.toml` |
| Couleurs | `colors`: the [threshold colors](sidebar.md#threshold-colors) | `sidebar.toml` |
| Seuils CPU, Seuils mémoire (Herdr; machine) | the four sets of thresholds: Enter to type three increasing values separated by spaces (`10 25 50`), Enter to save, Escape to cancel | `sidebar.toml` |
| Ouvrir les fichiers de configuration… | opens the files in an editor, see below | |
| Recharger la configuration de Herdr | reloads Herdr's `config.toml` and says whether Herdr applied it, in part (a section it refuses or that needs a restart keeps its current settings: the first reason is shown), or refused it | |

The Sidebar and color lines only show when the [sidebar](sidebar.md) plugin is installed. When the sidebar ignores its `sidebar.toml` (unknown setting, thresholds out of order…), the screen says why in red and shows the defaults, the values the sidebar uses. A file is written with its comments and its other settings kept: a setting commented out in it takes its value in its place. A plugin's file that does not exist yet is first created from its template, every setting commented out at its default.

**Ouvrir les fichiers de configuration…** opens, together, Herdr's `config.toml` (the path Herdr's help gives, which follows `HERDR_CONFIG_PATH`), `menu.toml`, and `sidebar.toml` when the sidebar is installed. The editor is `VISUAL`, else `EDITOR` (arguments allowed); without them, on Windows, VS Code when it is on `PATH`, else Notepad, one window per file; elsewhere `nano`, else `vi`. The settings screen closes and Herdr stays free to use:

- an editor with a window of its own (VS Code, Notepad, Sublime Text, gedit…) starts on its own, outside Herdr;
- a terminal editor (`nano`, `vim`, `hx`…) opens in a new Herdr tab; once it is closed, the tab asks `Recharger la configuration de Herdr ? [O/n]`, then closes on Enter.

Opened from a `Tools` pane, the screen also turns that pane to two buttons: **󰑓 Recharger** (or `r`, Enter) reloads Herdr's configuration once the files are saved, **󰁍 Retour** brings the icons back. The sidebar reads its file at each run, and the `Tools` pane re-reads `menu.toml` when it changes: only Herdr's `config.toml` needs the reload.

The plugin only opens Herdr's `config.toml`: it never writes it ([ADR 0004](../adr/0004-plugin-settings.md), [ADR 0005](../adr/0005-herdr-update-entry.md)).

## How it works, implications

**The `Tools` pane**

- It is a normal Herdr pane. Herdr only splits to the right or downwards: the pane opens below the explorer, then swaps places with it, and the focus goes back to the pane that had it.
- It opens on Herdr's `pane.created` event of the explorer, once the explorer stops moving (it places and sizes itself right after it opens: about one second). It closes on the `pane.closed` event, as soon as its tab has no explorer, or no terminal pane besides it and the explorer's own panes (Preview, Search…), so that the explorer can close its tab as it does on its own.
- A `Tools` pane is known by its label and by an empty file named after its pane id in the `bars` folder of the session's folder (see below), written when it opens and removed when it closes: a pane you rename `Tools` is never closed or taken for the bar. The explorer's panes are still known by their label alone (Explorer, Preview…): a terminal renamed so no longer counts as a terminal. `prefix+space` and the opening of an explorer take turns on a tab's bar, through a lock of the tab (`opening-<tab id>.lock`, removed at Herdr's start once its tab is gone): a tab never gets two bars.
- Herdr gives a plugin one state folder for all its sessions (`herdr --session <name>`), whose pane and tab ids start over at `w1`. What names a pane or a tab (the `bars` folder, the tabs' locks, the flags of the files open from a `Tools` pane, `files-open-*`) is therefore kept in a folder of the session, `sessions/<session>/` in the state folder (`default` for the session `herdr` runs without `--session`). The figures of a folder and the update's lock are shared by every session.
- It asks for 3 rows (6 with `totals = "bars"`), but Herdr never gives a side of a split less than a tenth of it: on a screen of fewer than ten times that (30 rows, 60 with the bars), it is taller. It takes its height back when the window is resized.
- **While it is open, the explorer no longer keeps its width**: `<` and `>` do nothing and its width follows the window's in proportion. The explorer only looks for its width at the full height of its own pane; a change to it is proposed upstream.
- The totals rows read a file the sidebar writes in its folder of the session at each of its runs (every 5 seconds), every 2 seconds: the pane measures nothing itself.
- The disk figures are measured in the background, only while the Disk page is shown (going back stops a reading under way, unless the folder is on a network share that no longer answers: its reading waits for it, and the folder's next readings wait for that one): the whole folder is read, which takes a moment on a large one (`node_modules`, build folders). The reading stops after 200,000 files and folders or 2 seconds, whichever comes first: the figures then start with `≥` (at least). A reading asked for with a click goes up to 5 million files and folders or 60 seconds.
- The figures are kept in a file of the plugin's state folder, one per folder (its path and its figures; if it cannot be written, the pane says so in red for 5 seconds): the panes of the same folder share them (while one of them reads the folder, the others wait for its figures), and a pane that shows them again does not read the folder. They are measured again once they are older than:

  | Files and folders read | Measured again after |
  | --- | --- |
  | fewer than 10,000 | 2 minutes |
  | fewer than 50,000 | 15 minutes |
  | fewer than 200,000 | 2 hours |
  | more, or a reading stopped at its limit | 1 day |

- It finds the explorer by its pane's label, `Explorer`; a later version of the explorer that renamed it would leave the menu as a popup only.
- After a Herdr restart, Herdr restores the pane as a plain shell; the plugin closes it at startup, and the explorer, when it reopens, gets a new one.

**The popup menu**

- Herdr shows one popup at a time. The action therefore stays running while the menu is open: it waits for the menu to write the choice into a file of the plugin's state folder, then opens the tool's popup once the menu's is gone. While waiting it uses about 4 MB of memory, checks the file 20 times a second, and checks once a second that the menu still runs, through a lock the menu holds, which the system releases when the menu ends, killed included.
- If the menu popup is closed by something else than the menu itself, the action sees within a second that the menu is gone, and stops.

**Settings and update**

- The settings screen is a popup: Herdr waits while it is open, but it only stays open while you change settings. The editors run outside it, in their window or in a tab.
- The update window runs outside Herdr, without the variable by which Herdr tells its panes (`HERDR_ENV`); it keeps running if the `Tools` pane closes.
- A program started on its own (`popup = false`, an editor with a window of its own) and Herdr started again from the update window get none of the variables that only hold in the `Tools` pane: `HERDR_ENV`, `HERDR_PANE_ID`, `HERDR_WORKSPACE_ID`, `HERDR_TAB_ID`, `HERDR_BIN_PATH`, `HERDR_PLUGIN_*` and `HERDR_MENU_*`.

Commands run as your user, with your environment, like any program you would start in a pane.

## Turn it off

To drop a built-in entry, list in `menu.toml` the entries to keep. To turn the whole menu off, remove the key from `config.toml`, then `herdr plugin disable imsike.menu` (or `herdr plugin uninstall imsike.menu`): the `Tools` panes close with their explorer; those Herdr restores at its next start, plain shells by then, are closed by hand. `menu.toml` stays in the config folder, and the figures' files (`stats-<hash>.json`, with their lock `stats-<hash>.lock`), the update's lock (`update.lock`), and the sessions' folders (`sessions`: the tabs' locks and the `bars` folder of each session) in the state folder, until you delete them.
