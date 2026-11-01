# ADR 0003 — The menu as a pane above the file explorer

- **Status**: accepted
- **Date**: 2026-11-01

## Context

The `menu` plugin started as a popup opened by `prefix+space`. The maintainer wanted it always in view, as clickable icons in the column of the file explorer of [herdr-explorer](https://github.com/gcaudan/herdr-explorer), a plugin by another author.

That column is not a Herdr area: the explorer is an ordinary `split` pane, placed and sized by its own hooks. Any plugin can split it. Herdr only splits to the right or downwards, and never gives a side of a split less than a tenth of it.

Options studied:

| Option | Pros | Cons |
|---|---|---|
| A pull request to herdr-explorer adding the icons to its pane | One pane; one plugin to follow | Depends on another maintainer; mixes a tools menu with a file explorer |
| A pane of this plugin stacked above the explorer, found by its label | Stays in this repository; the popup remains where there is no explorer | Relies on the explorer's pane label (`Explorer`), not an API; the explorer stops keeping its width while the pane is there; at least a tenth of the column high |
| Keep the popup only | Nothing to maintain | Not in view |

## Decision

The menu opens a `Tools` pane above the explorer: split below it, then swapped with it. It follows the explorer through Herdr events: `pane.created` of a pane labeled `Explorer` opens it, once the explorer has stopped moving; `pane.closed` closes it when its tab has no explorer, or no terminal pane besides the two and the explorer's own panes (Preview, Search…). `prefix+space` closes and reopens it in a tab with the explorer, and opens the popup menu elsewhere.

The explorer's width keeping, which only looks at the full height of its pane, is fixed upstream by a pull request to herdr-explorer rather than worked around here.

## Consequences

- Without herdr-explorer, or with a version that renames its pane, the menu is the popup, as before.
- Until the explorer's fix is released, `<` / `>` and the explorer's width keeping do nothing while the `Tools` pane is open.
- The pane is at least a tenth of the column high: on a short screen it shows empty rows.
- After a Herdr restart the restored `Tools` shells are closed by a startup hook; the explorer reopens with a new pane.
