# sidebar

**English** · [Français](../../fr/plugins/sidebar.md)

Turns Herdr's Agents panel into a tree: each workspace's agents listed under its name, with CPU and memory per workspace and per agent and the number of shells each agent is running. The total of Herdr's processes and the whole machine's show in the [menu](menu.md)'s `Tools` pane, or in the tab bar. The plugin only reports values; how they look is set by the sidebar rows of Herdr's `config.toml`.

```
 agents                 arbre
api · 6% 1,2G
├ ◐ Général · 4% 853M ·  1
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
# Runs the plugin every 5 seconds: keep it even with nothing shown. It shows its totals at the
# right of the tab bar only with tab_bar = true (sidebar.toml); then the time (Herdr's own entry).
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
  ] }, { token = "$usage", fg = "#665c54" }, { token = "$shell_count", fg = "#fabd2f" }],
]

[ui.sidebar.spaces]
rows = [
  ["workspace", { token = "branch", fg = "#fabd2f" }],
]

# Background of the selected agent (and workspace)
[theme.custom]
active_row_bg = "#4a4426"
```

The colors suit the gruvbox theme.

On Linux, point the command at the copy in the plugin's state folder (`herdr-sidebar tick`, without `.exe`): `install` prints its path, which `herdr plugin log list --plugin imsike.sidebar` shows.

## What it shows

Agent tokens (Agents panel):

| Token | Value |
|---|---|
| `$ws` | on the first agent of each workspace only: the workspace's name (`api`) |
| `$ws_stats` | next to `$ws`, only when the workspace has more than one agent (otherwise the agent's figures already tell it): the workspace's CPU and memory (`6% 1,2G`) |
| `$line` | tree branch (`├`, `└` for the last agent of the workspace), state and the name of the agent's tab (`├ ◐ Général`) |
| `$usage` | the agent's CPU and memory (`4% 853M`) |
| `$shell_count` | the agent's shells (` 1`), cleared when it has none. A token of its own takes a color of its own: a shell below an agent means the agent is at work, which the example colors like a working agent |
| `$stats` | CPU, memory and shells in one token, one color (`4% 853M  1`) |
| `$cpu`, `$ram`, `$shells` | the same figures on their own (`4%`, `853M`, `1 shell`), to build other layouts |
| `$order` | the tree's sort key; not meant to be displayed |

State glyphs: `◐` working, `◆` blocked, `●` done (finished, not seen yet), `○` idle, `·` unknown. The color rules above match these glyphs.

Totals: `<cpu> <memory> · 󰍹 <cpu> <memory>` (`6% 2,4G · 󰍹 35% 21,3G`): the total of Herdr's processes, then the whole machine's (after the Nerd Font monitor icon), updated at each run. Herdr's processes are its server and everything it runs: every pane of every workspace (agents, terminals, the explorer's and the menu's panes), their console hosts on Windows (`OpenConsole.exe`), the popups and the plugins' runs. The server is the process that started the panes' shells; while no shell is known, the total is that of the panes alone. It does not follow the workspace shown: it is the same in every `Tools` pane and stays when you switch workspaces. It is therefore larger than the sum of the agents, and than a workspace's `$ws_stats`, which counts its panes alone. Each run writes them in `totals.json`, in the folder of the session (`sessions/<session>/` in the state folder: each Herdr session has its own), which the menu's `Tools` pane shows under its icons. The tab bar shows them only with `tab_bar = true`; otherwise its entry stays empty and Herdr leaves it out, separator included. The time (`14:05`) is Herdr's own `datetime` entry: it stays when the plugin fails. The machine's figures count every process, those the plugin cannot read too (protected processes, virtual machines): its CPU share comes from the system's CPU times, its memory is the memory in use (what the Task Manager shows as "In use" on Windows).

- **CPU**: used since the previous run, as a share of the whole machine, rounded to the percent (`0%`, `12%`: below 0.5% shows `0%`); missing on the first run, and when the previous run is less than a second or more than two minutes old.
- **Memory**: working set on Windows, resident memory elsewhere (`850M`, `1,2G`).
- An agent's figures cover its pane's whole process tree: the shell, the agent and everything they started. A workspace adds up all its panes, agents or not.
- **Shells** (``, the Nerd Font terminal icon; without a Nerd Font it shows as a blank or a box): `bash`, `sh`, `zsh`, `fish`, `dash`, `nu`, `pwsh`, `powershell` or `cmd` processes below the agent (the `shells` setting). A shell started by another shell does not count again: Git Bash on Windows chains several `bash.exe` for one command, which count as one. Short commands appear only for an instant; long-running ones (a server, a watcher) stay visible.

To check the values without the sidebar, `herdr-sidebar show` (from the state folder) prints them, measured over one second, without reporting them.

## Settings

`sidebar.toml`, in the plugin's config folder (`herdr plugin config-dir imsike.sidebar` prints it), sets how the values are written. The menu's settings screen (**Settings**) changes it, or opens it, and first creates it with every setting commented out. A missing setting keeps its default:

| Setting | Default | Effect |
|---|---|---|
| `decimal_comma` | `true` | `2,4G` (`true`) or `2.4G` (`false`) |
| `percent_space` | `false` | `4%` (`false`) or `4 %` with a narrow no-break space (`true`) |
| `cpu_decimal` | `false` | `4%` (`false`) or `4,2%` (`true`) |
| `memory_unit` | `"M"` | `853M`, `2,4G` (`"M"`); `853Mo`, `2,4Go` (`"Mo"`); `853MB`, `2,4GB` (`"MB"`); `K`, `M` or `G` as the size needs |
| `memory_space` | `false` | `2,4Go` (`false`) or `2,4 Go` with a narrow no-break space before the unit (`true`) |
| `ttl_seconds` | `20` | how long a value lasts when no run refreshes it; at least 1 |
| `shells` | `["bash", "sh", "zsh", "fish", "dash", "nu", "pwsh", "powershell", "cmd"]` | process names counted as shells (without `.exe`, any case) |
| `tab_bar` | `false` | also show the totals in the tab bar (`true`) |
| `colors` | `false` | threshold colors of the totals in the menu's `Tools` pane, see [below](#threshold-colors) |
| `cpu_levels`, `memory_levels`, `machine_cpu_levels`, `machine_memory_levels` | see below | their thresholds |

```toml
percent_space = true
shells = ["bash", "pwsh", "nu"]
```

Each run reads the file: a change shows at the next run. A file that cannot be read or is invalid (unknown setting, `ttl_seconds = 0`, thresholds not in increasing order) is ignored: the runs use the defaults and say why in the plugin's log (`herdr plugin log list --plugin imsike.sidebar`). Colors, order and layout of the Agents panel stay in Herdr's `config.toml`.

Herdr runs the tab bar command without the plugin's environment, so without its config folder: `install` writes the folder's path in `config-dir`, in the state folder, where the tab bar runs read it. Until `install` has run once (at Herdr's start, or by hand), those runs use the defaults.

## Threshold colors

The sidebar writes its totals, with their thresholds, in `totals.json`; the menu's `Tools` pane shows them (see [menu](menu.md#use)). With `colors = true`, each of its figures is green, yellow, orange or red against three thresholds: above the first (strictly) it is yellow, above the second orange, above the third red. Its bars (`totals = "bars"` in `menu.toml`) fade through the same colors at the same thresholds, with or without `colors`.

| Thresholds | Default | For |
|---|---|---|
| `cpu_levels` | `[10, 25, 50]` (% of the machine) | the CPU of Herdr's processes (its server and everything it runs) |
| `memory_levels` | `[1, 2, 4]` (GiB) | their memory |
| `machine_cpu_levels` | `[50, 70, 90]` (%) | the machine's CPU |
| `machine_memory_levels` | `[60, 75, 90]` (% of its memory) | the machine's memory in use |

The Agents panel keeps the colors of Herdr's `config.toml`: coloring each agent's figures (through invisible markers that Herdr's `contains` rules match) was tried, and dropped for its rendering.

## How it works, implications

- Herdr has no long-running plugin process: the tab bar entry runs `herdr-sidebar tick` every 5 seconds, and Herdr never starts a run while the previous one is still going. Each run reads every process of the machine, then exits after a fraction of a second. It asks Herdr for a pane's shell process only the first time, or once the pane shows another terminal: later runs find it in `sample.json`.
- When an agent appears or changes state, Herdr runs the plugin at once, and so do a new workspace, a renamed or moved tab and a closed pane (`pane.agent_detected`, `pane.agent_status_changed`, `workspace.created`, `tab.renamed`, `tab.moved`, `pane.closed` events; viewing a done agent turns it idle, which is a state change): the tree's states follow without waiting for the next run. These runs only place the agents in the tree, without reading the processes, and send only the agents whose place or line changed: the figures are those of the last run, and a new agent gets its own at the next one. A renamed workspace appears at the next run.
- The Agents panel is sorted by `$order` through a Herdr agent view (labeled `arbre` in the panel header), which replaces the panel's own sort while the plugin is enabled. The tree follows the order of the workspaces in the sidebar and of the tabs in the tab bar: a moved tab takes its new place at the next run.
- The CPU share compares with the previous run, saved in `sample.json` in the folder of the session, as a share of all the machine's cores (a CPU quota given to Herdr does not change it). The time between the two runs comes from the system's CPU times (Windows) or its uptime (Linux), which a change of the clock does not move; a process is recognized by its start time, which follows such a change on Linux: the plugin allows for it.
- Each report carries a sequence number, kept in `seq` in the state folder: when the clock is set back, the reports go on in order instead of being ignored by Herdr, and two runs started at the same moment do not share one. It is taken once the session is read: of two runs at once, the one that read it last wins.
- A run still waiting for Herdr after 4 seconds stops asking it, and fails (the tab bar entry is empty until the next run): a slow Herdr is not sent more requests.
- An agent whose processes cannot be read stays in the tree with the figures of the last run, until they expire.
- Values expire after 20 seconds (`ttl_seconds`). If the runs stop, the tree lines disappear instead of going stale, and agents are listed without lines.
- Herdr indents a block's second row by two cells, so the first agent of a workspace (under its header) would sit further right than the others. The plugin pads the others' lines with two blank braille characters (U+2800), since Herdr strips ordinary spaces at the start of a value. A line longer than the sidebar is cut.
- The `◐` glyph replaces Herdr's state icon.
- The name shown is the agent's tab name: a tab never renamed shows its number, and two agents in one tab share a name. A renamed tab appears at the next run.
- Herdr itself puts ` · ` between two tokens of a row (`Général · 4% 853M ·  1`); it cannot be changed. `$stats` keeps the figures in one token, without it.
- The values carry their unit, so numeric color rules (`gt`, `lt`) do not apply to them.

## Turn it off

Remove the `command` entry of `tab_bar_right` and the `$ws`, `$ws_stats`, `$line`, `$usage`, `$shell_count`, `$stats`, `$cpu`, `$ram`, `$shells` tokens from `config.toml` and the `[ui.sidebar.*]` rows (the default rows come back), then `herdr plugin disable imsike.sidebar` (or `herdr plugin uninstall imsike.sidebar`), which also gives the Agents panel its own sort back. The copied binary (on Windows, with the previous copies still in use when it was replaced, `*.old`), the sessions' folders (`sessions`, with `sample.json` and `totals.json`), `seq`, `config-dir` and the install's lock, `install.lock`, stay in the state folder, and `sidebar.toml` in the config folder, until you delete them.
