# ADR 0005 — Mettre à jour Herdr depuis le menu

- **Statut** : accepté
- **Date** : 2026-11-01

## Contexte

Le mainteneur voulait mettre à jour Herdr depuis le pane `Tools`. `herdr update` installe la nouvelle version, mais le serveur en cours garde l'ancien binaire jusqu'à son redémarrage. La passation à chaud (`herdr update --handoff`) garde les sessions, mais le serveur ne la propose pas sous Windows (`herdr status --json` : `server.capabilities.live_handoff = false`). Là, le serveur doit s'arrêter : tous les panes se ferment et tous les agents s'arrêtent, parfois au milieu de leur travail. Un processus qui tourne dans Herdr ne peut pas relancer Herdr après l'avoir arrêté, et `herdr update` refuse de tourner dans Herdr (il vérifie `HERDR_ENV`).

Options étudiées :

| Option | Avantages | Inconvénients |
|---|---|---|
| a. Mettre à jour seulement ; la nouvelle version tourne au prochain démarrage | Rien ne s'arrête | La mise à jour attend que l'utilisateur quitte Herdr, sans qu'il le sache |
| b. Mettre à jour, puis demander avant d'arrêter le serveur, en disant quels agents s'arrêtent | Une touche ; rien ne s'arrête sans un oui qui en connaît le prix | L'utilisateur relance Herdr à la main |
| c. Mettre à jour et redémarrer sans demander | Une touche | Arrête des agents au travail sans prévenir |

## Décision

L'option b, en entrée intégrée **Update** du menu (`update`), lancée dans une fenêtre de terminal à part, hors de Herdr : une nouvelle console sous Windows, le premier émulateur de terminal trouvé ailleurs, démarrée sans `HERDR_ENV` :

1. `herdr update`, avec `--handoff` quand le serveur le propose ;
2. `herdr status --json` : quand ni `restart_needed` ni `server_binary_stale` n'est vrai, la fenêtre dit que Herdr est à jour ;
3. sinon elle donne le nombre de panes qui se ferment et d'agents qui s'arrêtent (et combien sont au travail), et demande `[o/N]` ; toute autre réponse que oui annule ;
4. une fois confirmé, elle arrête le serveur (`herdr server stop`, `herdr session stop <nom>` pour une session nommée, comme la documentation de Herdr les donne), puis propose de relancer Herdr dans cette fenêtre ; sinon elle met `herdr` dans le presse-papiers, `herdr --session <nom>` pour une session nommée (clip, wl-copy, xclip ou xsel).

*Révisé le 2026-11-01 : l'entrée tournait d'abord dans le popup du menu, où `herdr update` refusait de se lancer.*

*Révisé le 2026-11-11 : la fenêtre liste d'abord ce qui a une nouvelle version, Herdr et les plugins installés depuis GitHub, chaque ligne décochable, et ne lance `herdr update` que si Herdr reste coché ([ADR 0009](0009-new-version-check.md)).*

*Révisé le 2026-11-14 : la mise à jour est une ligne de l'écran des réglages, plus une entrée du menu ([ADR 0009](0009-new-version-check.md)).*

## Conséquences

- Aucun agent ne s'arrête sans une confirmation qui le dit.
- Là où la passation fonctionne, la confirmation n'est pas demandée : rien ne s'arrête.
- Relancer Herdr demande un `o` de plus dans la fenêtre de mise à jour, ou de coller le presse-papiers.
- Sous Linux, l'entrée demande un émulateur de terminal de sa liste ; sans lui, elle le dit.
- L'entrée dépend des champs `update` de `herdr status --json` ; s'ils disparaissent, l'entrée dit que Herdr est à jour et la nouvelle version tourne au prochain démarrage.
