# ADR 0002 — Plugin versions: the repository's version, written by the release workflow

- **Status**: accepted
- **Date**: 2026-10-31

## Context

Herdr requires a `version` in every `herdr-plugin.toml`, but does not use it to update: `herdr plugin update` fetches again the reference recorded at install time (the default branch, `main`, unless `--ref` was given), skips the plugin when the commit is unchanged, and otherwise rebuilds and registers the new copy. There is no automatic update; running panes keep their files until they exit, new runs use the new copy. Locally linked plugins (`herdr plugin link`) are never updated by Herdr.

The repository is versioned like OS-Setup and CC-Plugins: each push to `main` is tagged from the commit messages (`v0.0.0`, then minor for a `feat`, patch otherwise).

Options studied:

| Option | Pros | Cons |
|---|---|---|
| A version per plugin, bumped by hand | Each plugin's history readable on its own | A second version scheme; easily forgotten, and informative only since updates follow commits |
| A fixed `0.0.0` | Nothing to maintain | `herdr plugin list` shows a meaningless number |
| The repository's version, written into the manifests by the release workflow | One version scheme; `herdr plugin list` shows the release a plugin comes from | The workflow commits to `main`; every manifest gets the new number, changed or not |

## Decision

Manifests carry the repository's version. On each push to `main`, the release workflow computes the next version, writes it into every `herdr-plugin.toml`, commits that as `build(release): vX.Y.Z`, then tags that commit and publishes the release. On `dev`, manifests keep whatever number they have; a new plugin starts at `0.0.0`.

## Consequences

- Users update with `herdr plugin update` (all GitHub-installed plugins, or the ones named), or through OS-Setup; nothing updates on its own.
- `main` only receives what is released: its head is always a tagged release commit.
- The release commit exists on `main` only; merging `dev` into `main` does not conflict with it as long as `dev` does not edit a manifest's `version` line.
