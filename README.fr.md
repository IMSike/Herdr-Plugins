# Herdr-Plugins

[English](README.md) · **Français**

Des plugins [Herdr](https://herdr.dev) : ceux qu'[OS-Setup](https://github.com/IMSike/OS-Setup) propose d'installer, utilisables aussi seuls.

> **Statut : début du projet.** Les plugins demandent Herdr 0.9.5 ou plus récent. Les textes affichés sont en français pour l'instant.

## Plugins

| Plugin | Contenu |
|---|---|
| [`menu`](docs/fr/plugins/menu.md) | Lance un outil (lazygit, gh-dash, VS Code…) depuis un pane `Tools` d'icônes au-dessus de l'explorateur de fichiers, ou depuis un menu en popup ; chiffres sur le dossier du workspace ; met à jour Herdr et les plugins, en vert quand une nouvelle version sort ; ouvre les réglages |
| [`sidebar`](docs/fr/plugins/sidebar.md) | Un arbre workspace → agents dans le panneau Agents de Herdr, avec CPU, mémoire et nombre de shells ; totaux des processus de Herdr et de la machine entière pour le pane `Tools` du menu ou la barre d'onglets |

## Installation

Chaque plugin s'installe seul, depuis son dossier dans ce dépôt :

```sh
herdr plugin install IMSike/Herdr-Plugins/<plugin>
```

Rien à compiler : l'installation télécharge le binaire de la release du commit (Linux x86_64 et aarch64, Windows) et le vérifie avec les `SHA256SUMS` de la release. Elle demande Git, plus `curl` sous Linux. Seul un commit sans release (tout juste poussé sur `main` et encore en publication, ou installé avec `--ref`) est compilé, avec le toolchain Rust (`cargo`, Rust 1.95 ou plus récent) et, sous Windows, les Build Tools de Visual Studio avec la charge de travail C++ ([ADR 0008](docs/fr/adr/0008-prebuilt-binaries.md)).

Les mises à jour ne sont jamais automatiques : `herdr plugin update` récupère le dernier commit de `main` (chaque push sur `main` est une version) pour tous les plugins installés depuis GitHub, ou `herdr plugin update <id-du-plugin>` celui d'un seul plugin ([ADR 0002](docs/fr/adr/0002-plugin-versions.md)). L'entrée Update du menu signale quand l'un d'eux a un nouveau commit, et met à jour ceux que vous cochez ([ADR 0009](docs/fr/adr/0009-new-version-check.md)).

## Documentation

| | |
|---|---|
| [Plugins](docs/fr/README.md) | Ce que chaque plugin affiche ou fait, implications, comment le désactiver |
| [Ajouter un plugin](docs/fr/adding-plugins.md) | Organisation, crate, manifeste, essai, documentation |
| [Décisions d'architecture](docs/fr/adr/) | Pourquoi Rust et un workspace Cargo, versions et mises à jour, le menu au-dessus de l'explorateur de fichiers, réglages des plugins, mise à jour de Herdr depuis le menu, couleurs par seuil, état par session, binaires précompilés, nouvelles versions signalées |

## Contribuer

Voir [CONTRIBUTING.fr.md](CONTRIBUTING.fr.md) : conventions, messages de commit, tests.

## Licence

[MIT](LICENSE)
