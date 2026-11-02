# Adding a plugin

**English** · [Français](../fr/adding-plugins.md)

A plugin of this repository is a Rust crate of the root Cargo workspace, in a folder of its own with its Herdr manifest ([ADR 0001](adr/0001-rust-cargo-workspace.md)). First check that Herdr cannot do it on its own (configuration, native keybindings, popups, tab bar entries): a plugin exists only for what it cannot do.

## Layout

```text
<name>/
├── Cargo.toml           crate herdr-<name>, settings from the workspace
├── herdr-plugin.toml    Herdr manifest
├── README.md            short summary, linking to docs/{en,fr}/plugins/<name>.md
└── src/
    ├── main.rs          the subcommands Herdr runs
    └── <module>.rs      a feature with its I/O, or pure logic (formatting, computations)
                         in a module of its own, without I/O, with its tests
```

A plugin with settings keeps the template of its file (every setting commented out at its default) in `<name>/templates/`, in the folder of the plugin whose file it is, even when its code lives in `crates/herdr-api` or another plugin writes it.

Add the folder to `members` in the root `Cargo.toml`. Code that several plugins need (requests to Herdr, its environment variables, atomic writes) lives in `crates/herdr-api`: depend on it with `herdr-api.workspace = true` rather than writing it again.

## Crate

`Cargo.toml` takes `version`, `edition`, `rust-version`, `license`, `repository` and `publish` from the workspace (`version.workspace = true`…) and has a `description`. A dependency several crates use (`serde`, `serde_json`, `sysinfo`) has its version in `[workspace.dependencies]` of the root `Cargo.toml`; a crate names it with `workspace = true` and its own `features`. The binary is named `herdr-<name>`.

## Manifest

`herdr-plugin.toml`:

- `id = "imsike.<name>"`, a `name`, `version = "0.0.0"` never changed ([ADR 0002](adr/0002-plugin-versions.md)), `min_herdr_version`, a `description` and `platforms = ["linux", "windows"]`.
- One build step, which writes the binary into the plugin's own folder:

  ```toml
  [[build]]
  command = ["cargo", "build", "--release", "--locked", "--target-dir", "target"]
  ```

- Commands (`[[actions]]`, `[[panes]]`, `[[events]]`, `[[startup]]`) run `target/release/herdr-<name>` with a subcommand. Titles are in English.

Herdr's documentation is the authority on the manifest and the API: `herdr --help`, `herdr api schema --json`, and the plugin guide listed by `https://herdr.dev/llms.txt`.

## Code

- Quiet by default: a failure is written to stderr (Herdr keeps it in `herdr plugin log list`), never shown as a modal; a run never blocks Herdr or the agents, and background work stops with the command or pane that started it.
- No path written down: the state and config folders come from Herdr (`herdr_api::state_dir()`, `herdr_api::env`).
- What the plugin shows or writes is in French, except the names of the menu's built-in entries (Disk, Update, Settings), which their keys come from; identifiers and comments in English.

## Trying it

```sh
cd <name>
cargo build --release --locked --target-dir target
herdr plugin link <absolute path to the plugin folder>
```

Linking runs no build step: build again after each change. A running pane keeps its binary locked on Windows: close it before building. `herdr plugin unlink imsike.<name>` removes the plugin and leaves the files alone.

## Tests and documentation

1. `cargo fmt --all --check`, `cargo clippy --locked --all-targets -- -D warnings` and `cargo test --locked` pass at the root.
2. Write `docs/en/plugins/<name>.md` and `docs/fr/plugins/<name>.md`: each feature, what it shows or does, its implications, how to turn it off.
3. Add the plugin to the tables of `README.md`, `README.fr.md` and `docs/{en,fr}/README.md`.
4. A significant choice gets an ADR in `docs/{en,fr}/adr/`.
