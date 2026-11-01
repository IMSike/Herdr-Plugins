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

Option a was tried first, then dropped for the agents: in the Agents panel, the colored figures did not render as well as expected. The colors stay in the `Tools` pane only, which draws them itself:

- the sidebar's `colors` setting (off by default) turns them on; Herdr's `gt` semantics are kept (above a threshold, strictly, is the next level);
- two kinds of thresholds, each a set of three increasing values: Herdr's total (all the workspaces), `cpu_levels` in percent of the machine and `memory_levels` in GiB; the machine, `machine_cpu_levels` and `machine_memory_levels` in percent;
- the sidebar writes the totals with their thresholds in `totals.json`; the `Tools` pane works out the levels. Its bars (`totals = "bars"` in `menu.toml`) fade from green to red through the same thresholds, whether `colors` is on or not.

The sidebar's values carry no marker: the Agents panel keeps the colors of Herdr's `config.toml`.

## Consequences

- No rule to add to Herdr's `config.toml`; the agents' figures are not colored by level.
- `memory_levels` is in GiB of Herdr's processes while its bar fills with the share of the machine's memory: the gradient follows the GiB thresholds along the bar.
- Coloring the agents again would mean option a or b, with the rendering issue to solve first.
