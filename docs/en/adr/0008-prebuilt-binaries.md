# ADR 0008 — Prebuilt binaries, matched to the commit installed

- **Status**: accepted
- **Date**: 2026-11-04

## Context

`herdr plugin install` and `herdr plugin update` run the manifest's `[[build]]` commands in a checkout of the reference recorded at install time (`main` unless `--ref` was given). That checkout is a shallow Git clone: `.git` is there, with `origin` and `HEAD` on the commit installed. `herdr plugin link` runs no build command. Until now the build step ran `cargo`, so every machine installing a plugin needed the Rust toolchain, and on Windows the Visual Studio Build Tools ([ADR 0001](0001-rust-cargo-workspace.md)).

herdr-explorer, also in Rust, downloads the release binary of the version written in its manifest, which release-please commits to `main`.

Options studied:

| Option | Pros | Cons |
|---|---|---|
| Version in the manifests, committed to `main` by a release bot (herdr-explorer) | Usual install command; a proven model | The workflow commits to `main`, which `dev` must get back ([ADR 0002](0002-plugin-versions.md)); two merges per release; a token so that checks run on the bot's pull request; `chore` commits; no binary for a few minutes after the merge |
| Version in the manifests, written on `dev` by a script before the merge, with a required check | Flat history | A step to remember; the check blocks a contributor's pull request to `main` until someone bumps the version in it |
| A `dist` branch holding the binaries, installed with `--ref dist` | Nothing downloaded at install | The usual command (the marketplace's) still needs Rust; binaries in Git |
| The binary of the release tagged on the commit installed | Usual install command; no version to write, ADR 0002 unchanged; nothing asked of contributors; the binary is built from that very commit | Two scripts (sh, PowerShell); in the moment between a push to `main` and its release, builds with `cargo` or fails |

## Decision

- On each push to `main`, the [Version](../../../.github/workflows/release.yml) workflow attaches to its release every plugin's binary for x86_64 and aarch64 Linux (musl, static) and x86_64 Windows (which Windows on ARM64 runs in emulation), named `herdr-<plugin>-<target>[.exe]`, and their `SHA256SUMS`. It takes those the [Plugins](../../../.github/workflows/validate.yml) workflow built on `dev` for the same commit (a fast-forward merge brings that commit to `main`), or builds them ([Binaries](../../../.github/workflows/binaries.yml)). The release is created as a draft, which makes no tag, and published once complete: the tag appears with every binary.
- Each plugin's build step runs `scripts/fetch-bin.sh` (Linux) or `scripts/fetch-bin.ps1` (Windows). The checkout must be unchanged (`git status`); the tag on `HEAD` is looked up on `origin` (`git ls-remote`); the binary is downloaded from that release, checked against its `SHA256SUMS`, and put in `target/release/`, where the manifest's commands already run it.
- When that fails (no release for the commit, a changed checkout, another system, no network), the step builds with `cargo` if it is there, and otherwise fails saying why.

## Consequences

- Installing from GitHub needs neither Rust nor the Build Tools: Git (which Herdr itself uses to install), plus `curl` and `sha256sum` or `shasum` on Linux, Windows PowerShell and `curl.exe` (Windows 10 1803 and later) on Windows.
- A binary is installed only if it was built from the commit installed: release found by that commit, checkout unchanged, `SHA256SUMS` checked. It comes from this repository's GitHub releases, built by its workflows: the same trust as its sources.
- A commit without a release (`--ref` to a branch, a commit between two releases) is built with `cargo`; a fork uses its own releases (its `origin`).
- Installing or updating in the moment after a push to `main`, before its release is published (seconds with the binaries of `dev`, a few minutes when the workflow builds them), builds with `cargo`, or fails; `herdr plugin update` once the release is there.
- Development is unchanged: a linked plugin runs no build step, and is built with `cargo`.
- Each push to `dev` also builds the binaries (three more jobs). Artifacts are kept 90 days by default: an older commit merged into `main` has its binaries built again.
- A new plugin takes the same two build steps; the workflows find it by its folder (`<folder>/herdr-plugin.toml`, binary `herdr-<folder>`).
