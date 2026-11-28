# ADR 0013 — Offering the new plugins of a repository

- **Status**: accepted
- **Date**: 2026-11-28

## Context

Each plugin of this repository is installed on its own (`herdr plugin install IMSike/Herdr-Plugins/<plugin>`). When a plugin is added to the repository (links, after menu and sidebar), the update ([ADR 0009](0009-new-version-check.md)) updated the plugins already installed, but nothing said a new one was there: it had to be installed by hand, and only by someone who knew.

What Herdr offers:

- `herdr plugin update` only updates the plugins installed; it has no way to install the others of a repository.
- For a plugin installed from a folder of a GitHub repository, `plugin.list` gives the repository's clone (`source.managed_path`) and the folder (`source.subdir`). Herdr clones the whole repository: the other folders, with their `herdr-plugin.toml`, are in that clone, at the commit installed.

Options considered:

| Option | For | Against |
|---|---|---|
| a. A list of the repository's plugins written in the menu | Simple | Only for this repository; the menu installed knows only the plugins of its own commit |
| b. Read the folders beside each plugin installed in its clone | No request to GitHub; works for any repository with several plugins; a plugin brought by an update is seen right after it | Relies on Herdr keeping the whole repository in its clone; a plugin the user did not want is seen again at each check |
| c. Ask GitHub for the repository's tree | Sees a new plugin before the update | One more request per repository, limited without a token (60 an hour) |
| d. Change nothing | Nothing to write | A new plugin goes unnoticed |

## Decision

Option b, chosen by the maintainer, with what follows:

1. **Detection**, at each check of the new versions (`updates.json` keeps it): for each plugin installed from a folder of a GitHub repository, the folders beside it in its clone whose manifest gives an id that is not installed (a linked plugin counts as installed), whose `platforms`, when given, holds the running system, and that the user has not declined. A plugin at the root of its repository has no folder beside it: nothing is read.
2. **Display**: as a new version, in green, the plugin's name followed by `(nouveau)` (`Settings · Links (nouveau)`), unless `check_updates = false`.
3. **Install**: the update's window lists them after the updates, **unticked**; a ticked one is installed after the updates, with `herdr plugin install <owner>/<repo>/<folder>`, from the same branch or tag as the plugin it was found beside (`--ref`), without `--yes`: Herdr shows what the plugin may do and asks. The plugins that the update just brought are offered right after it, in a second list, the same way.
4. **Declined**: a new plugin left unticked once the list is confirmed (not cancelled) is kept in the state folder (`declined-plugins.json`) and no longer offered nor shown. The **Reproposer les plugins refusés** line of the settings, shown while there is one, forgets them: they are offered again at the next check.

## Consequences

- A plugin added to a repository is offered at the next update of a plugin of that repository, without a request more than ADR 0009's.
- It relies on Herdr cloning the whole repository for a plugin installed from one of its folders, which `plugin.list` shows (`managed_path`) without documenting it as a promise: should a later Herdr clone only the folder, nothing more is offered, and the rest works as before.
- A plugin of another person's repository with several plugins is offered the same way: it is installed only once ticked, and Herdr's own confirmation still shows.
- A plugin uninstalled on purpose (`herdr plugin uninstall`) is offered again, unticked, until it is declined.
- Before the update that brings it, a new plugin is not seen: the clone is at the commit installed.
