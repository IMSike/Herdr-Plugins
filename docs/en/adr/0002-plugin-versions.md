# ADR 0002 — Plugin versions: Git tags only, manifests fixed at `0.0.0`

- **Status**: accepted
- **Date**: 2026-11-01

## Context

Herdr requires a `version` in every `herdr-plugin.toml`, but does not use it to update: `herdr plugin update` fetches again the reference recorded at install time (the default branch, `main`, unless `--ref` was given), skips the plugin when the commit is unchanged, and otherwise rebuilds and registers the new copy. There is no automatic update; running panes keep their files until they exit, new runs use the new copy. Locally linked plugins (`herdr plugin link`) are never updated by Herdr. `herdr plugin list` shows the commit a plugin was installed from.

The repository is versioned like OS-Setup and CC-Plugins: each push to `main` is tagged from the commit messages (`v0.0.0`, then minor for a `feat`, patch otherwise). Their release workflow tags and publishes a release, and commits nothing.

Options studied:

| Option | Pros | Cons |
|---|---|---|
| A version per plugin, bumped by hand | Each plugin's history readable on its own | A second version scheme; easily forgotten, and informative only since updates follow commits |
| The repository's version, written into the manifests by the release workflow | `herdr plugin list` shows the release a plugin comes from | The workflow commits to `main`, which then has a commit `dev` lacks: it must be brought back to `dev` (fast-forward when `dev` has not moved, otherwise a merge commit, a rebase with a forced push, or a duplicate commit), and `dev` moves on GitHub after each release |
| A fixed `0.0.0`, the version in the Git tag only | Nothing to maintain; same scheme as the other repositories; `main` and `dev` stay linear and `dev` merges into `main` as a fast-forward | `herdr plugin list` shows a meaningless number, next to the commit |

## Decision

Every `herdr-plugin.toml` keeps `version = "0.0.0"`, never changed. The version exists only as the Git tag and GitHub release that the release workflow creates on each push to `main`; the workflow commits nothing.

## Consequences

- Users update with `herdr plugin update` (all GitHub-installed plugins, or the ones named), or through OS-Setup; nothing updates on its own.
- The release a plugin comes from is found from the commit `herdr plugin list` shows: the tag on that commit, or the latest one before it.
- A new plugin's manifest also says `0.0.0`.
