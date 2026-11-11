# ADR 0009 — Signaler les nouvelles versions, choisir quoi mettre à jour

- **Statut** : accepté
- **Date** : 2026-11-11

## Contexte

Rien ne se met à jour tout seul : Herdr par `herdr update`, ses plugins installés depuis GitHub par `herdr plugin update` ([ADR 0002](0002-plugin-versions.md)). L'entrée **Update** du menu ne mettait à jour que Herdr ([ADR 0005](0005-herdr-update-entry.md)), et rien ne disait qu'une nouvelle version était sortie : Herdr annonce les siennes dans son interface, pas celles des plugins.

Ce que Herdr offre pour savoir s'il y a du nouveau :

- `herdr status --json` donne la version installée et le canal (`stable` ou `preview`), mais pas la dernière version publiée. Herdr la lit, pour sa propre annonce, dans `https://herdr.dev/latest.json` (`preview.json` pour le canal preview) : une adresse qui figure dans son binaire, pas dans sa documentation.
- `plugin.list` (l'API) donne, pour un plugin installé depuis GitHub, son dépôt, la référence demandée à l'installation (`requested_ref`, absente pour la branche par défaut) et le commit installé (`resolved_commit`). `herdr plugin update` récupère de nouveau cette référence et ignore le plugin quand le commit n'a pas changé ; il n'a pas de mode « vérifier seulement ».
- Sous Windows, le serveur ne sait pas passer ses sessions à une nouvelle version : faire tourner le nouveau Herdr arrête tous les agents.

Options étudiées :

| Option | Avantages | Inconvénients |
|---|---|---|
| a. Mise à jour automatique, en arrière-plan | Rien à faire | Du code change sans qu'on le décide (les `SHA256SUMS` d'une release garantissent le fichier, pas son auteur) ; pour Herdr, un redémarrage arrêterait des agents au travail ; tous les plugins GitHub, ceux des autres compris |
| b. Signaler les nouvelles versions, mettre à jour d'un clic, en choisissant quoi | Rien ne change sans un clic ; le redémarrage reste confirmé | Une requête à herdr.dev et une par plugin GitHub toutes les quelques heures ; plus de code |
| c. Ne rien changer | Rien à écrire | Les nouvelles versions des plugins passent inaperçues |

## Décision

L'option b, choisie par le mainteneur :

1. **Détection**, active par défaut, désactivable (`check_updates = false` dans `menu.toml`, ligne « Signaler les mises à jour » des réglages). Elle a lieu au démarrage de Herdr (la commande `[[startup]]` du plugin) puis toutes les 4 heures (par un pane `Tools`, sur un fil à part), une seule à la fois (un verrou du dossier d'état) ; son résultat est gardé dans le dossier d'état (`updates.json`) pour tous les panes.
   - Herdr : la version de `herdr status --json` contre celle de `latest.json`, sur le canal stable seulement (`curl`, 20 secondes au plus).
   - Plugins installés depuis GitHub : `git ls-remote` de leur référence (`HEAD` sans `--ref`, sinon la branche, ou le commit du tag, de ce nom exact) contre le commit installé, sans invite ni fenêtre d'identifiants, 20 secondes au plus. Un plugin lié (`herdr plugin link`) n'est pas concerné : Herdr ne le met pas à jour.
2. **Affichage** : l'icône Update du pane `Tools` en vert, son nom au survol suivi de ce qui a une nouvelle version ; la ligne Update du menu en popup de même. Le menu en popup ne vérifie rien lui-même.
3. **Mise à jour** : la fenêtre de l'entrée Update vérifie de nouveau, puis liste ce qui a une nouvelle version, tout coché, chaque ligne décochable. Herdr y figure aussi quand sa nouvelle version n'est pas connue (canal preview, pas de réponse), comme avant. Elle lance `herdr update` d'abord (la nouvelle version d'un plugin peut demander celle de Herdr), puis `herdr plugin update <id>…` pour les plugins cochés, sans `--yes` : Herdr garde sa propre confirmation. La suite de l'ADR 0005 (redémarrage confirmé) ne change pas, et n'a lieu que si Herdr a été mis à jour.

## Conséquences

- Le plugin ne fait que comparer des versions : aucun code ne s'installe sans un clic, et le redémarrage du serveur reste demandé, avec ce qu'il arrête.
- Des requêtes réseau partent sans action de l'utilisateur : une vers herdr.dev, une vers GitHub par plugin installé depuis GitHub, au démarrage puis toutes les 4 heures tant qu'un pane `Tools` est ouvert. Hors ligne, rien ne passe au vert et rien n'est affiché en erreur. Sans pane `Tools`, seule la vérification du démarrage a lieu.
- `latest.json` n'est pas documenté : s'il change de forme, Herdr n'est plus signalé (sa version reste « inconnue »), mais l'entrée Update le propose toujours et `herdr update` fonctionne comme avant.
- Le commit d'un plugin est comparé, pas sa release : juste après un push sur `main`, avant la publication de la release, un plugin de ce dépôt peut passer au vert, et sa mise à jour compiler avec `cargo` ou échouer ([ADR 0008](0008-prebuilt-binaries.md)).
- Un commit qui ne touche pas un plugin d'un dépôt à plusieurs plugins le signale quand même : `herdr plugin update` le réinstallerait aussi.
- Une installation de Herdr par un gestionnaire de paquets (Homebrew, mise, Nix) est signalée comme les autres, alors que `herdr update` ne vaut que pour l'installateur de Herdr : elle se met à jour par son gestionnaire.
- Il faut `curl` et `git`, que Windows 10 (1803 et plus récent) et l'installation des plugins demandent déjà.
