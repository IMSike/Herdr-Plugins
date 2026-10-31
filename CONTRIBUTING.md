# Contributing

**English** · [Français](CONTRIBUTING.fr.md)

## Getting started

Requirements: Git, [Herdr](https://herdr.dev), the Rust toolchain ([rustup](https://rustup.rs)) and, on Windows, the Visual Studio Build Tools with the C++ workload.

```sh
git clone https://github.com/IMSike/Herdr-Plugins.git
cd Herdr-Plugins
git config core.hooksPath .githooks       # checks commit messages locally
```

A plugin under development is linked rather than installed: `herdr plugin link <absolute path to the plugin folder>`. Linking runs no build step, so build it yourself first; `herdr plugin unlink <plugin-id>` removes it and leaves the files alone.

## Principles

- **Quiet by default**: a plugin never blocks or slows Herdr or the agents; a failure is logged, never shown as a modal.
- **Herdr first**: what Herdr's configuration, native keybindings or popups can do is not a plugin.
- **No secrets, no personal data, no machine-specific paths** in the repository.
- **Documented**: each feature explained (what it shows or does, implications, how to turn it off), in both languages.

## Code conventions

- Rust, edition 2024, formatted by `cargo fmt`, with `cargo clippy` clean.
- Each plugin is a crate of the root Cargo workspace, in its own folder with its `herdr-plugin.toml`; code shared by several plugins lives in `crates/` ([ADR 0001](docs/en/adr/0001-rust-cargo-workspace.md)).
- Pure logic (formatting, computations) in modules of its own, without I/O, tested on their own.
- Identifiers and comments in English; user-facing text in French for now.
- Encoding: UTF-8 without BOM, LF. Enforced by `.gitattributes` and `.editorconfig`.

## Documentation

The documentation exists in **English** (`docs/en/`) and **French** (`docs/fr/`), with the same file names. Any change in behavior updates the documentation **in the same commit**, in **both languages**.

## Commit messages

Commit messages and pull request titles follow the [Angular convention](https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md):

```
<type>(<scope>): <subject>
```

| Type | Use |
|---|---|
| `feat` | New feature (new plugin, new feature of a plugin) |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `perf` | Performance improvement |
| `test` | Tests only |
| `build` | Build, dependencies, tooling |
| `ci` | GitHub Actions workflows |
| `revert` | Reverts a previous commit |

Usual scopes: the plugin's name, `repo`. The subject is in the imperative, lowercase, without a final period. One commit = one coherent change, about one plugin.

Messages are checked by the `.githooks/commit-msg` hook and by the [Commit messages](.github/workflows/commit-lint.yml) workflow.

## Versions

Each push to `main` (direct or merged pull request) is tagged and released by the [Version](.github/workflows/release.yml) workflow, following [semantic versioning](https://semver.org):

| Commits since the last version | Bump |
|---|---|
| at least one `feat` | minor: `0.1.0` → `0.2.0` |
| anything else | patch: `0.1.0` → `0.1.1` |
| maintainer's decision only | major: run the workflow manually with `major` |

The first version is `v0.0.0`. There is no changelog for now. The workflow writes the version into every `herdr-plugin.toml` and commits it to `main` before tagging; a new plugin starts at `0.0.0` ([ADR 0002](docs/en/adr/0002-plugin-versions.md)).

## Pull requests

1. Branch from `main`.
2. `cargo fmt --check`, `cargo clippy` and `cargo test` pass, and the change was tried with the plugin linked in Herdr.
3. Documentation updated in both languages.
4. Title following the commit convention (it becomes the commit message on squash merge).

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
