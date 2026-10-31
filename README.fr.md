# Herdr-Plugins

[English](README.md) · **Français**

Des plugins [Herdr](https://herdr.dev) : ceux qu'[OS-Setup](https://github.com/IMSike/OS-Setup) propose d'installer, utilisables aussi seuls.

> **Statut : début du projet.** Les textes affichés sont en français pour l'instant.

## Plugins

| Plugin | Contenu |
|---|---|
| [`menu`](docs/fr/plugins/menu.md) | Un menu en popup, ouvert par une touche, qui lance un outil (lazygit, gh-dash…) dans un grand popup au-dessus du workspace courant |

## Installation

Chaque plugin s'installe seul, depuis son dossier dans ce dépôt :

```sh
herdr plugin install IMSike/Herdr-Plugins/<plugin>
```

La compilation demande le toolchain Rust (`cargo`) et, sous Windows, les Build Tools de Visual Studio avec la charge de travail C++ ([ADR 0001](docs/fr/adr/0001-rust-cargo-workspace.md)).

Les mises à jour ne sont jamais automatiques : `herdr plugin update` récupère la dernière version de tous les plugins installés depuis GitHub, ou `herdr plugin update <id-du-plugin>` celle d'un seul ([ADR 0002](docs/fr/adr/0002-plugin-versions.md)).

## Documentation

| | |
|---|---|
| [Plugins](docs/fr/README.md) | Ce que chaque plugin affiche ou fait, implications, comment le désactiver |
| [Décisions d'architecture](docs/fr/adr/) | Pourquoi Rust et un workspace Cargo, versions et mises à jour |

## Contribuer

Voir [CONTRIBUTING.fr.md](CONTRIBUTING.fr.md) : conventions, messages de commit, tests.

## Licence

[MIT](LICENSE)
