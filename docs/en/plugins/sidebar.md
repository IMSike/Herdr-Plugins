# sidebar

**English** · [Français](../../fr/plugins/sidebar.md)

Turns Herdr's Agents panel into a tree: each workspace's agents listed under its name, with CPU and memory per workspace and per agent and the number of shells each agent is running. The tab bar shows the total of all panes and the whole machine's. The plugin only reports values; how they look is set by the sidebar rows of Herdr's `config.toml`.

```
 agents                 arbre
api · 6% 1.2G
├ ◐ Général · 4% 853M  1
└ ○ docs · 0% 275M
web
└ ○ 1 · 1% 562M
```

Clicking an agent line opens that agent. Workspaces are still opened from the Spaces panel above, reduced to one line each (`api · main`); drag the divider between the two panels to give the tree more room.

## Installation

```sh
herdr plugin install IMSike/Herdr-Plugins/sidebar
```

When Herdr starts, the plugin copies its binary into its state folder and sorts the Agents panel in tree order. Right after installing, updating or linking it, do it once by hand (or restart Herdr):

```sh
herdr plugin action invoke imsike.sidebar.install
```

Then add to Herdr's `config.toml`:

```toml
[ui]
sidebar_width = 34
sidebar_max_width = 40
# Runs the plugin every 5 seconds and shows its totals at the right of the tab bar, then the time (Herdr's own entry).
# Windows (the state folder is under %LOCALAPPDATA%):
tab_bar_right = [
  { type = "command", command = '"%LOCALAPPDATA%\herdr\plugins\imsike.sidebar\herdr-sidebar.exe" tick', interval_seconds = 5, timeout_seconds = 5 },
  { type = "datetime", format = "%H:%M" },
]
tab_bar_right_separator = " · "

[ui.sidebar.agents]
rows = [
  [{ token = "$ws", fg = "#ebdbb2", bold = true }, { token = "$ws_stats", fg = "#7c6f64" }],
  [{ token = "$line", fg = "#bdae93", rules = [
    { contains = "◆", fg = "#fb4934" },
    { contains = "◐", fg = "#fabd2f" },
    { contains = "●", fg = "#b8bb26" },
  ] }, { token = "$stats", fg = "#665c54" }],
]

[ui.sidebar.spaces]
rows = [
  ["workspace", { token = "branch", fg = "#fabd2f" }],
]

# Background of the selected agent (and workspace)
[theme.custom]
active_row_bg = "#4a4426"
```

The colours suit the gruvbox theme.

On Linux, point the command at the copy in the plugin's state folder (`herdr-sidebar tick`, without `.exe`): `install` prints its path, which `herdr plugin log list --plugin imsike.sidebar` shows.

## What it shows

Agent tokens (Agents panel):

| Token | Value |
|---|---|
| `$ws` | on the first agent of each workspace only: the workspace's name (`api`) |
| `$ws_stats` | next to `$ws`, only when the workspace has more than one agent (otherwise the agent's figures already tell it): the workspace's CPU and memory (`6% 1.2G`) |
| `$line` | tree branch (`├`, `└` for the last agent of the workspace), state and the name of the agent's tab (`├ ◐ Général`) |
| `$stats` | the agent's CPU, memory and shells (`4% 853M  1`) |
| `$cpu`, `$ram`, `$shells` | the same figures on their own (`4%`, `853M`, `1 shell`), to build other layouts |
| `$order` | the tree's sort key; not meant to be displayed |

State glyphs: `◐` working, `◆` blocked, `●` done (finished, not seen yet), `○` idle, `·` unknown. The colour rules above match these glyphs.

Workspace tokens (Spaces panel): `$cpu` and `$ram`.

Tab bar: `<cpu> <memory> · 󰍹 <cpu> <memory>` (`6% 2.4G · 󰍹 35% 21.3G`): the total of every pane, then the whole machine's (after the Nerd Font monitor icon), updated at each run. The time after them (`· 14:05`) is Herdr's own `datetime` entry: it stays when the plugin fails. The machine's figures count every process, those the plugin cannot read too (protected processes, virtual machines): its CPU share comes from the system's CPU times, its memory is the memory in use (what the Task Manager shows as "In use" on Windows).

- **CPU**: used since the previous run, as a share of the whole machine, rounded to the percent (`0%`, `12%`: below 0.5% shows `0%`); missing on the first run, and when the previous run is less than a second or more than two minutes old.
- **Memory**: working set on Windows, resident memory elsewhere (`850M`, `1.2G`).
- An agent's figures cover its pane's whole process tree: the shell, the agent and everything they started. A workspace adds up all its panes, agents or not.
- **Shells** (``, the Nerd Font terminal icon; without a Nerd Font it shows as a blank or a box): `bash`, `sh`, `zsh`, `fish`, `dash`, `nu`, `pwsh`, `powershell` or `cmd` processes below the agent. A shell started by another shell does not count again: Git Bash on Windows chains several `bash.exe` for one command, which count as one. Short commands appear only for an instant; long-running ones (a server, a watcher) stay visible.

To check the values without the sidebar, `herdr-sidebar show` (from the state folder) prints them, measured over one second, without reporting them.

## How it works, implications

- Herdr has no long-running plugin process: the tab bar entry runs `herdr-sidebar tick` every 5 seconds, and Herdr never starts a run while the previous one is still going. Each run reads every process of the machine, then exits after a fraction of a second. It asks Herdr for a pane's shell process only the first time, or once the pane shows another terminal: later runs find it in `sample.json`.
- When an agent appears or changes state, Herdr runs the plugin at once and so does focusing a pane, which marks a done agent as seen (`pane.agent_detected`, `pane.agent_status_changed`, `pane.focused`, `workspace.created` events): the tree's states follow without waiting for the next run. These runs only place the agents in the tree, without reading the processes, and send only the agents whose place or line changed: the figures are those of the last run, and a new agent gets its own at the next one. A renamed workspace appears at the next run.
- The Agents panel is sorted by `$order` through a Herdr agent view (labelled `arbre` in the panel header), which replaces the panel's own sort while the plugin is enabled. The tree follows the order of the workspaces in the sidebar and of the tabs in the tab bar: a moved tab takes its new place at the next run.
- The CPU share compares with the previous run, saved in `sample.json` in the state folder.
- An agent whose processes cannot be read stays in the tree with the figures of the last run, until they expire.
- Values expire after 20 seconds. If the runs stop, the tree lines disappear instead of going stale, and agents are listed without lines.
- Herdr indents a block's second row by two cells, so the first agent of a workspace (under its header) sits further right than the others. The plugin pads the others' lines with two blank braille characters (U+2800), since Herdr strips ordinary spaces at the start of a value. A line longer than the sidebar is cut.
- The `◐` glyph replaces Herdr's state icon.
- The name shown is the agent's tab name: a tab never renamed shows its number, and two agents in one tab share a name. A renamed tab appears at the next run.
- Herdr itself puts ` · ` between two tokens of a row (`Général · 4% 853M`); it cannot be changed.
- The values carry their unit, so numeric colour rules (`gt`, `lt`) do not apply to them.

## Turn it off

Remove the `command` entry of `tab_bar_right` and the `$ws`, `$ws_stats`, `$line`, `$stats`, `$cpu`, `$ram`, `$shells` tokens from `config.toml` and the `[ui.sidebar.*]` rows (the default rows come back), then `herdr plugin disable imsike.sidebar` (or `herdr plugin uninstall imsike.sidebar`), which also gives the Agents panel its own sort back. The copied binary and `sample.json` stay in the state folder until you delete them.
