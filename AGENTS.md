# AGENTS.md

Instructions for coding agents (Claude Code, Codex, Copilot…) working on this repository. Humans: see [CONTRIBUTING.md](CONTRIBUTING.md).

## Project

A set of [Herdr](https://herdr.dev) plugins: the ones [OS-Setup](https://github.com/IMSike/OS-Setup) offers to install, usable on their own too. The repository is **public** (MIT).

Read before changing anything:

- [Architecture decisions](docs/en/adr/): why Rust and one Cargo workspace, how versions and updates work.
- [CONTRIBUTING.md](CONTRIBUTING.md): code conventions, commit messages, versions.
- [Adding a plugin](docs/en/adding-plugins.md): layout, crate, manifest, trying it, docs.
- Herdr's own documentation is the authority on the plugin API, not memory: `herdr --help`, `herdr api schema --json`, and the plugin guide (`https://herdr.dev/llms.txt` lists the pages for the running version).

## Hard rules

1. **Never change the maintainer's Herdr setup** (`%APPDATA%\herdr\config.toml`, installed or linked plugins, running sessions) without explicit consent. That config file is deployed by OS-Setup: a change meant to last goes there (`dotfiles/herdr/`), not into the live file.
2. **Never install global tools** without asking.
3. **No secrets, personal data or machine-specific paths** in the repository: it is public. A path is read at run time (`HERDR_PLUGIN_*` variables, environment), never written down.
4. **Do not settle open design questions alone**: present the mechanism, the alternatives and a recommendation, then let the maintainer decide. Significant decisions go in an ADR (`docs/{en,fr}/adr/NNNN-title.md`: context, decision, consequences).
5. **Quiet by default**: a plugin never blocks or slows Herdr or the agents; a failure is logged, never shown as a modal (only a tool the user has just opened, in its own popup, may show why it failed and wait for a key); background work stops when its parent command or pane exits.
6. Prefer what Herdr already offers (configuration, native keybindings, popups) to a plugin: a plugin exists only for what Herdr cannot do on its own.

## Documentation

- English (`docs/en/`) and French (`docs/fr/`) trees have the **same files**. Any page you change is changed **in both languages, in the same commit** as the code change. `README.md` / `README.fr.md` and `CONTRIBUTING.md` / `CONTRIBUTING.fr.md` likewise.
- Every plugin has its page, `docs/{en,fr}/plugins/<name>.md`: what each feature shows or does, its implications, how to turn it off. The plugin's own `README.md` is a short summary linking to it.

## Maintainer notes

Work in progress, to-do lists and verification reports live in `.maintainer/` (ignored by Git), never in the documentation.

## Conventions

- Language: identifiers and code comments in English; user-facing text (what a plugin draws, its messages) in French for now, except the names of the menu's built-in entries (Disk, Settings), which their keys come from; documentation in both languages. Talk to the maintainer in French.
- Rust: see [CONTRIBUTING.md](CONTRIBUTING.md#code-conventions).
- Encoding: UTF-8 without BOM, LF.

## Tests

Before proposing a change (the [Plugins](.github/workflows/validate.yml) workflow runs the same on Linux and Windows):

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

## Git

- Maintainer's own work happens on the `dev` branch: never commit or push to `main` for them, they merge `dev` into `main`, which publishes a version. External contributions are pull requests to `main`. Commit messages and PR titles follow the **Angular convention** (`feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, `revert`); there is no `chore`. The scope is the plugin's name (`feat(sidebar): ...`), `herdr-api` for the shared crate alone, or `repo`.
- One commit = one coherent change; one plugin per commit.
- Keep this file up to date when a convention changes.
