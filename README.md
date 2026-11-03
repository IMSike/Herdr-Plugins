# Herdr-Plugins

**English** · [Français](README.fr.md)

[Herdr](https://herdr.dev) plugins: the ones [OS-Setup](https://github.com/IMSike/OS-Setup) offers to install, usable on their own too.

> **Status: early stage.** The plugins need Herdr 0.9.5 or later. Displayed text is in French for now.

## Plugins

| Plugin | Contents |
|---|---|
| [`menu`](docs/en/plugins/menu.md) | Starts a tool (lazygit, gh-dash, VS Code…) from a `Tools` pane of icons above the file explorer, or from a popup menu; figures on the workspace folder; updates Herdr, opens the settings |
| [`sidebar`](docs/en/plugins/sidebar.md) | A workspace → agents tree in Herdr's Agents panel, with CPU, memory and shell counts; totals of Herdr's processes and of the whole machine for the menu's `Tools` pane or the tab bar |

## Installation

Each plugin installs on its own, from its folder in this repository:

```sh
herdr plugin install IMSike/Herdr-Plugins/<plugin>
```

Building needs the Rust toolchain (`cargo`, Rust 1.89 or later), and on Windows the Visual Studio Build Tools with the C++ workload ([ADR 0001](docs/en/adr/0001-rust-cargo-workspace.md)).

Updates are never automatic: `herdr plugin update` fetches the latest commit of `main` (each push to `main` is a release) for every plugin installed from GitHub, or `herdr plugin update <plugin-id>` of one ([ADR 0002](docs/en/adr/0002-plugin-versions.md)).

## Documentation

| | |
|---|---|
| [Plugins](docs/en/README.md) | What each plugin shows or does, implications, how to turn it off |
| [Adding a plugin](docs/en/adding-plugins.md) | Layout, crate, manifest, trying it, docs |
| [Architecture decisions](docs/en/adr/) | Why Rust and one Cargo workspace, versions and updates, the menu above the file explorer, plugin settings, updating Herdr from the menu, threshold colors |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md): conventions, commit messages, tests.

## License

[MIT](LICENSE)
