# ADR 0001 — Plugins en Rust, un seul dépôt, un workspace Cargo

- **Statut** : accepté
- **Date** : 2026-10-31

## Contexte

Un plugin Herdr est un dossier avec un manifeste `herdr-plugin.toml` et des commandes argv que Herdr lance ; toute la CLI `herdr` et l'API socket (un named pipe sous Windows) forment son API. Le langage d'implémentation est libre. Les plugins prévus ici tournent à côté des agents (mise en page de la sidebar rafraîchie sur événements, échantillonnage des processus) : leur propre consommation de mémoire et de CPU compte.

Herdr installe un plugin avec `herdr plugin install <owner>/<repo>[/<sous-dossier>]` : il clone tout le dépôt (superficiellement, branche par défaut sauf `--ref`), lance les commandes `[[build]]` du manifeste dans le dossier du plugin, puis l'enregistre. Un dépôt peut contenir plusieurs plugins, chacun installable seul ; la marketplace Herdr présente le dépôt comme une carte qui les liste. Il n'y a pas de fichier catalogue.

Options étudiées pour le langage :

| Option | Avantages | Inconvénients |
|---|---|---|
| Rust | Le langage de Herdr (ses types d'API se lisent dans ses sources) ; quelques Mo en mémoire, sans ramasse-miettes ; crates mûres pour les processus et l'API Windows ; un binaire natif | Apprentissage exigeant ; premier build lent ; demande les Build Tools MSVC sous Windows ; `plugin install` compile avec `cargo` sur chaque machine |
| Go | Simple ; builds rapides ; compilation croisée triviale ; bonnes bibliothèques de TUI | Ramasse-miettes, environ 10 à 20 Mo en mémoire ; `plugin install` compile avec `go` sur chaque machine |
| Node / Python / scripts | Les plus rapides à écrire | Un runtime à installer ; un processus par appel à `herdr` coûte cher sous Windows pour un travail fréquent |

Options étudiées pour l'organisation : un dépôt par plugin, ou un dépôt pour tous.

## Décision

- Les plugins sont écrits en **Rust**.
- Un seul dépôt contient tous les plugins, chacun dans son dossier à la racine (`<plugin>/herdr-plugin.toml`), installé avec `herdr plugin install IMSike/Herdr-Plugins/<plugin>`.
- La racine du dépôt est un **workspace Cargo** : chaque plugin en est une crate membre, et le code partagé entre plugins (client Herdr, types) vit dans `crates/`.

## Conséquences

- Compiler demande le toolchain Rust, et les Build Tools MSVC sous Windows, sur chaque machine qui installe un plugin depuis GitHub. À réévaluer si les plugins servent au-delà des machines du mainteneur : des binaires précompilés publiés par un workflow de release et téléchargés par l'étape `[[build]]`.
- Le build d'un plugin écrit son binaire dans son propre dossier (`cargo build --release --target-dir target`), donc les commandes de son manifeste utilisent un chemin relatif au dossier du plugin.
- `herdr plugin update` compare le commit du dépôt : un commit qui touche un plugin recompile aussi les autres plugins installés depuis ce dépôt.
