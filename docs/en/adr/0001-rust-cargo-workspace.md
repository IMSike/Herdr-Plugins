# ADR 0001 — Plugins in Rust, one repository, one Cargo workspace

- **Status**: accepted
- **Date**: 2026-10-31

## Context

A Herdr plugin is a folder with a `herdr-plugin.toml` manifest and argv commands that Herdr runs; the whole `herdr` CLI and the socket API (a named pipe on Windows) are its API. The implementation language is free. The plugins planned here keep running beside the agents (sidebar layout refreshed on events, process sampling), so their own memory and CPU use matter.

Herdr installs a plugin with `herdr plugin install <owner>/<repo>[/<subdir>]`: it clones the whole repository (shallow, default branch unless `--ref`), runs the manifest's `[[build]]` commands in the plugin's folder, then registers it. One repository can hold several plugins, each installable on its own; the Herdr marketplace shows the repository as one card listing them. There is no catalog file.

Options studied for the language:

| Option | Pros | Cons |
|---|---|---|
| Rust | Herdr's own language (its API types are readable in its sources); a few MB resident, no garbage collector; mature process and Windows API crates; one native binary | Steep learning curve; slow first build; needs the MSVC Build Tools on Windows; `plugin install` builds with `cargo` on every machine |
| Go | Simple; fast builds; trivial cross-compilation; good TUI libraries | Garbage collector, about 10–20 MB resident; `plugin install` builds with `go` on every machine |
| Node / Python / scripts | Fastest to write | A runtime to install; one process per `herdr` call is costly on Windows for frequent work |

Options studied for the layout: one repository per plugin, or one repository for all of them.

## Decision

- Plugins are written in **Rust**.
- One repository holds every plugin, each in its own folder at the root (`<plugin>/herdr-plugin.toml`), installed with `herdr plugin install IMSike/Herdr-Plugins/<plugin>`.
- The repository root is a **Cargo workspace**: each plugin is a member crate, and code shared between plugins (Herdr client, types) lives in `crates/`.

## Consequences

- Building needs the Rust toolchain, and the MSVC Build Tools on Windows, on every machine that installs a plugin from GitHub. To be reassessed if plugins are used beyond the maintainer's machines: prebuilt binaries published by a release workflow and downloaded by the `[[build]]` step.
- A plugin's build writes its binary into its own folder (`cargo build --release --target-dir target`), so its manifest commands use a path relative to the plugin folder.
- `herdr plugin update` compares the repository commit: a commit touching one plugin also rebuilds the others installed from this repository.
