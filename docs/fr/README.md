# Documentation

[English](../en/README.md) · **Français**

## Plugins

| Page | Contenu |
|---|---|
| [menu](plugins/menu.md) | Pane `Tools` au-dessus de l'explorateur de fichiers, ou menu en popup, qui lance un outil (lazygit, gh-dash, VS Code…) ; ses entrées et réglages, `menu.toml` ; mise à jour de Herdr, ouverture des réglages, récupération d'une session |
| [sidebar](plugins/sidebar.md) | Arbre workspace → agents avec CPU, mémoire et shells, en tokens de sidebar ; les lignes à ajouter au `config.toml`, `sidebar.toml` |

## Contribuer

| Page | Contenu |
|---|---|
| [Ajouter un plugin](adding-plugins.md) | Organisation, crate, manifeste, règles de code, essai, tests et documentation |
| [Décisions d'architecture](adr/) | [0001](adr/0001-rust-cargo-workspace.md) Rust, un seul dépôt, un workspace Cargo · [0002](adr/0002-plugin-versions.md) versions et mises à jour des plugins · [0003](adr/0003-menu-bar-above-explorer.md) le menu en pane au-dessus de l'explorateur de fichiers · [0004](adr/0004-plugin-settings.md) réglages des plugins · [0005](adr/0005-herdr-update-entry.md) mettre à jour Herdr depuis le menu · [0006](adr/0006-threshold-colors.md) couleurs par seuil · [0007](adr/0007-state-per-session.md) état par session de Herdr · [0008](adr/0008-prebuilt-binaries.md) binaires précompilés · [0009](adr/0009-new-version-check.md) signaler les nouvelles versions · [0010](adr/0010-session-recovery.md) récupérer une session à partir des snapshots de Herdr |

Voir aussi le [guide de contribution](../../CONTRIBUTING.fr.md).
