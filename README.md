# Herdr-Plugins

**English** · [Français](README.fr.md)

[Herdr](https://herdr.dev) plugins: the ones [OS-Setup](https://github.com/IMSike/OS-Setup) offers to install, usable on their own too.

> **Status: early stage.** The plugins need Herdr 0.9.5 or later. Displayed text is in French for now.

## Plugins

| Plugin | Contents |
|---|---|
| [`menu`](docs/en/plugins/menu.md) | Starts a tool (lazygit, gh-dash, VS Code…) from a `Tools` pane of icons above the file explorer, or from a popup menu; figures on the workspace folder; updates Herdr and the plugins, green when a new version is out; opens the settings |
| [`sidebar`](docs/en/plugins/sidebar.md) | A workspace → agents tree in Herdr's Agents panel, with CPU, memory and shell counts; totals of Herdr's processes and of the whole machine for the menu's `Tools` pane or the tab bar |

## Installation

Each plugin installs on its own, from its folder in this repository:

```sh
herdr plugin install IMSike/Herdr-Plugins/<plugin>
```

Nothing to build: the install downloads the binary of the commit's release (Linux x86_64 and aarch64, Windows) and checks it against the release's `SHA256SUMS`. It needs Git, plus `curl` on Linux. Only a commit without a release (just pushed to `main` and still being published, or installed with `--ref`) is built, with the Rust toolchain (`cargo`, Rust 1.95 or later) and on Windows the Visual Studio Build Tools with the C++ workload ([ADR 0008](docs/en/adr/0008-prebuilt-binaries.md)).

Updates are never automatic: `herdr plugin update` fetches the latest commit of `main` (each push to `main` is a release) for every plugin installed from GitHub, or `herdr plugin update <plugin-id>` of one ([ADR 0002](docs/en/adr/0002-plugin-versions.md)). The menu's Update entry shows when one has a new commit, and updates the ones you tick ([ADR 0009](docs/en/adr/0009-new-version-check.md)).

## Documentation

| | |
|---|---|
| [Plugins](docs/en/README.md) | What each plugin shows or does, implications, how to turn it off |
| [Adding a plugin](docs/en/adding-plugins.md) | Layout, crate, manifest, trying it, docs |
| [Architecture decisions](docs/en/adr/) | Why Rust and one Cargo workspace, versions and updates, the menu above the file explorer, plugin settings, updating Herdr from the menu, threshold colors, state per session, prebuilt binaries, new versions shown |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md): conventions, commit messages, tests.

## License

[MIT](LICENSE)
