# ADR 0008 — Binaires précompilés, liés au commit installé

- **Statut** : accepté
- **Date** : 2026-11-04

## Contexte

`herdr plugin install` et `herdr plugin update` lancent les commandes `[[build]]` du manifeste dans une copie de la référence enregistrée à l'installation (`main` sauf si `--ref` a été donné). Cette copie est un clone Git superficiel : `.git` est là, avec `origin` et `HEAD` sur le commit installé. `herdr plugin link` ne lance aucune commande de build. Jusqu'ici l'étape de build lançait `cargo` : chaque machine qui installait un plugin avait besoin du toolchain Rust, et sous Windows des Build Tools de Visual Studio ([ADR 0001](0001-rust-cargo-workspace.md)).

herdr-explorer, lui aussi en Rust, télécharge le binaire de la release dont la version est écrite dans son manifeste, que release-please commite sur `main`.

Options étudiées :

| Option | Pour | Contre |
|---|---|---|
| Version dans les manifestes, commitée sur `main` par un bot de release (herdr-explorer) | Commande d'installation habituelle ; un modèle éprouvé | Le workflow commite sur `main`, que `dev` doit récupérer ([ADR 0002](0002-plugin-versions.md)) ; deux merges par release ; un token pour que les vérifications tournent sur la pull request du bot ; des commits `chore` ; aucun binaire pendant quelques minutes après le merge |
| Version dans les manifestes, écrite sur `dev` par un script avant le merge, avec une vérification requise | Historique plat | Une étape à ne pas oublier ; la vérification bloque la pull request d'un contributeur vers `main` tant que personne n'y a monté la version |
| Une branche `dist` qui contient les binaires, installée avec `--ref dist` | Rien à télécharger à l'installation | La commande habituelle (celle de la marketplace) demande toujours Rust ; des binaires dans Git |
| Le binaire de la release taguée sur le commit installé | Commande d'installation habituelle ; aucune version à écrire, ADR 0002 inchangée ; rien de demandé aux contributeurs ; le binaire est compilé depuis ce commit même | Deux scripts (sh, PowerShell) ; entre un push sur `main` et sa release, compile avec `cargo` ou échoue |

## Décision

- À chaque push sur `main`, le workflow [Version](../../../.github/workflows/release.yml) joint à sa release le binaire de chaque plugin pour Linux x86_64 et aarch64 (musl, statique) et Windows x86_64 (que Windows sur ARM64 exécute en émulation), nommé `herdr-<plugin>-<cible>[.exe]`, et leurs `SHA256SUMS`. Il reprend ceux que le workflow [Plugins](../../../.github/workflows/validate.yml) a compilés sur `dev` pour le même commit (un merge en fast-forward amène ce commit sur `main`), ou les compile ([Binaries](../../../.github/workflows/binaries.yml)). La release est créée en brouillon, ce qui ne crée aucun tag, et publiée une fois complète : le tag apparaît avec tous les binaires.
- L'étape de build de chaque plugin lance `scripts/fetch-bin.sh` (Linux) ou `scripts/fetch-bin.ps1` (Windows). La copie doit être inchangée (`git status`) ; le tag posé sur `HEAD` est cherché sur `origin` (`git ls-remote`) ; le binaire est téléchargé depuis cette release, vérifié avec ses `SHA256SUMS` et placé dans `target/release/`, d'où les commandes du manifeste le lancent déjà.
- Si cela échoue (pas de release pour ce commit, une copie modifiée, un autre système, pas de réseau), l'étape compile avec `cargo` s'il est là, et sinon échoue en disant pourquoi.

## Conséquences

- Installer depuis GitHub ne demande ni Rust ni les Build Tools : Git (dont Herdr se sert lui-même pour installer), plus `curl` et `sha256sum` ou `shasum` sous Linux, Windows PowerShell et `curl.exe` (Windows 10 1803 et plus récent) sous Windows.
- Un binaire n'est installé que s'il a été compilé depuis le commit installé : release trouvée par ce commit, copie inchangée, `SHA256SUMS` vérifiés. Il vient des releases GitHub de ce dépôt, compilé par ses workflows : la même confiance que ses sources.
- Un commit sans release (`--ref` vers une branche, un commit entre deux releases) est compilé avec `cargo` ; un fork utilise ses propres releases (son `origin`).
- Installer ou mettre à jour juste après un push sur `main`, avant la publication de sa release (quelques secondes avec les binaires de `dev`, quelques minutes quand le workflow les compile), compile avec `cargo` ou échoue ; `herdr plugin update` une fois la release là.
- Le développement ne change pas : un plugin lié ne lance aucune étape de build et se compile avec `cargo`.
- Chaque push sur `dev` compile aussi les binaires (trois jobs de plus). Les artefacts sont gardés 90 jours par défaut : un commit plus ancien fusionné dans `main` voit ses binaires recompilés.
- Un nouveau plugin reprend les deux mêmes étapes de build ; les workflows le trouvent par son dossier (`<dossier>/herdr-plugin.toml`, binaire `herdr-<dossier>`).
