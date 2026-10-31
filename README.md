# Herdr-Plugins

**English** · [Français](README.fr.md)

[Herdr](https://herdr.dev) plugins: the ones [OS-Setup](https://github.com/IMSike/OS-Setup) offers to install, usable on their own too.

> **Status: early stage.** Displayed text is in French for now.

## Plugins

| Plugin | Contents |
|---|---|
| [`menu`](docs/en/plugins/menu.md) | A popup menu, opened by one key, that starts a tool (lazygit, gh-dash…) in a large popup over the current workspace |

## Installation

Each plugin installs on its own, from its folder in this repository:

```sh
herdr plugin install IMSike/Herdr-Plugins/<plugin>
```

Building needs the Rust toolchain (`cargo`), and on Windows the Visual Studio Build Tools with the C++ workload ([ADR 0001](docs/en/adr/0001-rust-cargo-workspace.md)).

Updates are never automatic: `herdr plugin update` fetches the latest release of every plugin installed from GitHub, or `herdr plugin update <plugin-id>` of one ([ADR 0002](docs/en/adr/0002-plugin-versions.md)).

## Documentation

| | |
|---|---|
| [Plugins](docs/en/README.md) | What each plugin shows or does, implications, how to turn it off |
| [Architecture decisions](docs/en/adr/) | Why Rust and one Cargo workspace, versions and updates |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md): conventions, commit messages, tests.

## License

[MIT](LICENSE)
