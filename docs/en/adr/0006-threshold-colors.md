# ADR 0006 — Threshold colors

- **Status**: accepted
- **Date**: 2026-11-01

## Context

The maintainer wanted the CPU and memory figures colored green, yellow, orange or red against thresholds they can set, in the menu's `Tools` pane and on the agents of the sidebar.

The `Tools` pane draws its own text: it can color it directly. The sidebar only reports token values; Herdr colors them, and only through the rules of its `config.toml`, which no plugin writes. A rule tests the token's own value with `equals`, `contains`, `starts_with`, `gt` or `lt`; `gt` and `lt` only match a value that is a bare number, not `4% 853M`.

Options studied:

| Option | Pros | Cons |
|---|---|---|
| a. Invisible markers: the plugin ends a value with one to four zero-width spaces (U+200B) by level; four `contains` rules per token in `config.toml` | Thresholds in `sidebar.toml`, settable from the settings screen; values unchanged on screen; one token as before | A hidden character in the value; four rules per token to add once |
| b. One token per level (`$usage_green` … `$usage_red`), only one reported | No hidden character; thresholds in `sidebar.toml` | Four tokens per value in the rows of `config.toml` |
| c. Bare numbers (`$cpu_n = "45"`) with `gt` rules | Herdr's own mechanism | Thresholds only in `config.toml`, not settable from the screen; figures shown without unit |

Thresholds: an agent's CPU is a share of the whole machine, and the machine's memory in use is often above half: one set for both would leave the agents uncolored or the machine always red.

## Decision

Option a. The sidebar's `colors` setting (off by default) turns the markers on; Herdr's `gt` semantics apply (above a threshold, strictly, is the next level). Two kinds of thresholds, each a set of three increasing values:

- processes (agents, workspaces, Herdr's total): `cpu_levels` in percent of the machine, `memory_levels` in GiB;
- the machine: `machine_cpu_levels` and `machine_memory_levels`, in percent.

`$cpu` and `$ram` carry their own level; `$usage`, `$stats` and `$ws_stats` the higher of their CPU and memory. The sidebar writes the totals' levels in `totals.json`, which the `Tools` pane colors with the same colors as the example rules.

## Consequences

- The colors of the agents need the rules in Herdr's `config.toml` (OS-Setup's `dotfiles/herdr/`); the plugin pages give them.
- With the colors on, an `equals` rule on a figure token no longer matches.
- If Herdr ever drew zero-width characters, or trimmed them from values, the markers would show or stop matching; the colors can be turned off with one setting.
