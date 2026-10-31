# Contribuer

[English](CONTRIBUTING.md) · **Français**

## Pour commencer

Prérequis : Git, [Herdr](https://herdr.dev), le toolchain Rust, 1.88 ou plus récent ([rustup](https://rustup.rs)) et, sous Windows, les Build Tools de Visual Studio avec la charge de travail C++.

```sh
git clone https://github.com/IMSike/Herdr-Plugins.git
cd Herdr-Plugins
git config core.hooksPath .githooks       # vérifie les messages de commit en local
```

Un plugin en développement est lié plutôt qu'installé : `herdr plugin link <chemin absolu du dossier du plugin>`. La liaison ne lance aucune étape de build : compilez-le d'abord vous-même ; `herdr plugin unlink <id-du-plugin>` le retire sans toucher aux fichiers.

## Principes

- **Discret par défaut** : un plugin ne bloque ni ne ralentit jamais Herdr ou les agents ; un échec est journalisé, jamais affiché en fenêtre modale.
- **Herdr d'abord** : ce que la configuration, les raccourcis natifs ou les popups de Herdr savent faire n'est pas un plugin.
- **Aucun secret, aucune donnée personnelle, aucun chemin propre à une machine** dans le dépôt.
- **Documenté** : chaque fonctionnalité est expliquée (ce qu'elle affiche ou fait, implications, comment la désactiver), dans les deux langues.

## Conventions de code

- Rust, édition 2024, formaté par `cargo fmt`, sans avertissement de `cargo clippy`.
- Chaque plugin est une crate du workspace Cargo racine, dans son propre dossier avec son `herdr-plugin.toml` ; le code partagé par plusieurs plugins vit dans `crates/` ([ADR 0001](docs/fr/adr/0001-rust-cargo-workspace.md)).
- La logique pure (mise en forme, calculs) dans des modules à part, sans entrées-sorties, testés seuls.
- Identifiants et commentaires en anglais ; textes affichés à l'utilisateur en français pour l'instant.
- Encodage : UTF-8 sans BOM, LF. Imposé par `.gitattributes` et `.editorconfig`.

## Documentation

La documentation existe en **anglais** (`docs/en/`) et en **français** (`docs/fr/`), avec les mêmes noms de fichiers. Tout changement de comportement met à jour la documentation **dans le même commit**, dans **les deux langues**.

## Messages de commit

Les messages de commit et les titres de pull request suivent la [convention Angular](https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md) :

```
<type>(<portée>): <sujet>
```

| Type | Usage |
|---|---|
| `feat` | Nouvelle fonctionnalité (nouveau plugin, nouvelle fonctionnalité d'un plugin) |
| `fix` | Correction de bug |
| `docs` | Documentation seule |
| `refactor` | Changement de code qui ne corrige pas de bug et n'ajoute pas de fonctionnalité |
| `perf` | Amélioration des performances |
| `test` | Tests seuls |
| `build` | Build, dépendances, outillage |
| `ci` | Workflows GitHub Actions |
| `revert` | Annule un commit précédent |

Portées habituelles : le nom du plugin, `repo`. Le sujet est à l'impératif, en minuscules, sans point final. Un commit = un changement cohérent, sur un seul plugin.

Les messages sont vérifiés par le hook `.githooks/commit-msg` et par le workflow [Commit messages](.github/workflows/commit-lint.yml).

## Versions

Chaque push sur `main` (direct ou pull request fusionnée) reçoit un tag et une release du workflow [Version](.github/workflows/release.yml), selon le [versionnage sémantique](https://semver.org/lang/fr/) :

| Commits depuis la dernière version | Incrément |
|---|---|
| au moins un `feat` | mineure : `0.1.0` → `0.2.0` |
| autre chose | correctif : `0.1.0` → `0.1.1` |
| décision du mainteneur seulement | majeure : lancer le workflow à la main avec `major` |

La première version est `v0.0.0`. Pas de changelog pour l'instant. Le workflow écrit la version dans chaque `herdr-plugin.toml` et la commite sur `main` avant de poser le tag ; un nouveau plugin commence à `0.0.0` ([ADR 0002](docs/fr/adr/0002-plugin-versions.md)).

## Pull requests

1. Partir de `main`.
2. `cargo fmt --check`, `cargo clippy` et `cargo test` passent, et le changement a été essayé avec le plugin lié dans Herdr.
3. Documentation à jour dans les deux langues.
4. Titre conforme à la convention de commit (il devient le message de commit au squash merge).

## Licence

En contribuant, vous acceptez que vos contributions soient publiées sous la [licence MIT](LICENSE).
