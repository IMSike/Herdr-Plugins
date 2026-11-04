# Documentation

**English** · [Français](../fr/README.md)

## Plugins

| Page | Contents |
|---|---|
| [menu](plugins/menu.md) | `Tools` pane above the file explorer, or popup menu, that starts a tool (lazygit, gh-dash, VS Code…); its entries and settings, `menu.toml`; updating Herdr, opening the settings |
| [sidebar](plugins/sidebar.md) | Workspace → agents tree with CPU, memory and shells, as sidebar tokens; the rows to add to `config.toml`, `sidebar.toml` |

## Contributing

| Page | Contents |
|---|---|
| [Adding a plugin](adding-plugins.md) | Layout, crate, manifest, code rules, trying it, tests and docs |
| [Architecture decisions](adr/) | [0001](adr/0001-rust-cargo-workspace.md) Rust, one repository, one Cargo workspace · [0002](adr/0002-plugin-versions.md) plugin versions and updates · [0003](adr/0003-menu-bar-above-explorer.md) the menu as a pane above the file explorer · [0004](adr/0004-plugin-settings.md) plugin settings · [0005](adr/0005-herdr-update-entry.md) updating Herdr from the menu · [0006](adr/0006-threshold-colors.md) threshold colors · [0007](adr/0007-state-per-session.md) state per Herdr session |

See also the [contributing guide](../../CONTRIBUTING.md).
