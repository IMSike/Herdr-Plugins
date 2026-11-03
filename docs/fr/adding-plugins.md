# Ajouter un plugin

[English](../en/adding-plugins.md) · **Français**

Un plugin de ce dépôt est une crate Rust du workspace Cargo racine, dans son propre dossier avec son manifeste Herdr ([ADR 0001](adr/0001-rust-cargo-workspace.md)). Vérifiez d'abord que Herdr ne sait pas le faire seul (configuration, raccourcis natifs, popups, entrées de la barre d'onglets) : un plugin n'existe que pour ce qu'il ne sait pas faire.

## Organisation

```text
<nom>/
├── Cargo.toml           crate herdr-<nom>, réglages repris du workspace
├── herdr-plugin.toml    manifeste Herdr
├── README.md            court résumé, avec un lien vers docs/{en,fr}/plugins/<nom>.md
└── src/
    ├── main.rs          les sous-commandes que lance Herdr
    └── <module>.rs      une fonctionnalité avec ses entrées-sorties, ou la logique pure
                         (mise en forme, calculs) dans un module à part, sans I/O, avec ses tests
```

Un plugin qui a des réglages garde le modèle de son fichier (tous les réglages en commentaire à leur valeur par défaut) dans `<nom>/templates/`, dans le dossier du plugin dont c'est le fichier, même quand son code se trouve dans `crates/herdr-api` ou qu'un autre plugin l'écrit.

Ajoutez le dossier à `members` dans le `Cargo.toml` racine. Le code utile à plusieurs plugins (requêtes à Herdr, ses variables d'environnement, écritures atomiques) se trouve dans `crates/herdr-api` : utilisez-le avec `herdr-api.workspace = true` plutôt que de le réécrire.

## Crate

`Cargo.toml` reprend `version`, `edition`, `rust-version`, `license`, `repository` et `publish` du workspace (`version.workspace = true`…) et a une `description`. Une dépendance utilisée par plusieurs crates (`serde`, `serde_json`, `sysinfo`) a sa version dans `[workspace.dependencies]` du `Cargo.toml` racine ; une crate la cite avec `workspace = true` et ses propres `features`. Le binaire s'appelle `herdr-<nom>`.

## Manifeste

`herdr-plugin.toml` :

- `id = "imsike.<nom>"`, un `name`, `version = "0.0.0"` jamais modifié ([ADR 0002](adr/0002-plugin-versions.md)), `min_herdr_version`, une `description` et `platforms = ["linux", "windows"]`.
- Une seule étape de build, qui écrit le binaire dans le dossier du plugin :

  ```toml
  [[build]]
  command = ["cargo", "build", "--release", "--locked", "--target-dir", "target"]
  ```

- Les commandes (`[[actions]]`, `[[panes]]`, `[[events]]`, `[[startup]]`) lancent `target/release/herdr-<nom>` avec une sous-commande. Les titres sont en anglais.

La documentation de Herdr fait foi pour le manifeste et l'API : `herdr --help`, `herdr api schema --json`, et le guide des plugins listé par `https://herdr.dev/llms.txt`.

## Code

- Discret par défaut : un échec est écrit sur stderr (Herdr le garde dans `herdr plugin log list`), jamais affiché en fenêtre modale (seul un outil que l'utilisateur vient d'ouvrir, dans son propre popup, peut afficher pourquoi il a échoué et attendre une touche) ; un passage ne bloque jamais Herdr ni les agents, et le travail en arrière-plan s'arrête avec la commande ou le pane qui l'a lancé.
- Aucun chemin écrit en dur : les dossiers d'état et de configuration viennent de Herdr (`herdr_api::state_dir()`, `herdr_api::env`).
- Ce que le plugin affiche ou écrit est en français, sauf les noms des entrées intégrées du menu (Disk, Update, Settings), d'où viennent leurs touches ; identifiants et commentaires en anglais.

## Essayer

```sh
cd <nom>
cargo build --release --locked --target-dir target
herdr plugin link <chemin absolu du dossier du plugin>
```

La liaison ne lance aucune étape de build : recompilez après chaque modification. Sous Windows, un pane en cours garde son binaire verrouillé : fermez-le avant de compiler. `herdr plugin unlink imsike.<nom>` retire le plugin sans toucher aux fichiers.

## Tests et documentation

1. `cargo fmt --all --check`, `cargo clippy --locked --all-targets -- -D warnings` et `cargo test --locked` passent à la racine.
2. Écrivez `docs/en/plugins/<nom>.md` et `docs/fr/plugins/<nom>.md` : chaque fonctionnalité, ce qu'elle affiche ou fait, ses implications, comment la désactiver.
3. Ajoutez le plugin aux tableaux de `README.md`, `README.fr.md` et `docs/{en,fr}/README.md`.
4. Un choix important fait l'objet d'un ADR dans `docs/{en,fr}/adr/`.
