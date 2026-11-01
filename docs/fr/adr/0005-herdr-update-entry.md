# ADR 0005 — Mettre à jour Herdr depuis le menu

- **Statut** : accepté
- **Date** : 2026-11-01

## Contexte

Le mainteneur voulait mettre à jour Herdr depuis le pane `Tools`. `herdr update` installe la nouvelle version, mais le serveur en cours garde l'ancien binaire jusqu'à son redémarrage. La passation à chaud (`herdr update --handoff`) garde les sessions, mais le serveur ne la propose pas sous Windows (`herdr status --json` : `server.capabilities.live_handoff = false`). Là, le serveur doit s'arrêter : tous les panes se ferment et tous les agents s'arrêtent, parfois au milieu de leur travail. Un processus qui tourne dans Herdr ne peut pas relancer Herdr après l'avoir arrêté.

Options étudiées :

| Option | Avantages | Inconvénients |
|---|---|---|
| a. Mettre à jour seulement ; la nouvelle version tourne au prochain démarrage | Rien ne s'arrête | La mise à jour attend que l'utilisateur quitte Herdr, sans qu'il le sache |
| b. Mettre à jour, puis demander avant d'arrêter le serveur, en disant quels agents s'arrêtent | Une touche ; rien ne s'arrête sans un oui qui en connaît le prix | L'utilisateur relance Herdr à la main |
| c. Mettre à jour et redémarrer sans demander | Une touche | Arrête des agents au travail sans prévenir |

## Décision

L'option b, en entrée intégrée **Herdr** du menu (`update-herdr`), lancée dans le grand popup :

1. `herdr update`, avec `--handoff` quand le serveur le propose ;
2. `herdr status --json` : quand ni `restart_needed` ni `server_binary_stale` n'est vrai, le popup dit que Herdr est à jour ;
3. sinon il donne le nombre de panes qui se ferment et d'agents qui s'arrêtent (et combien sont au travail), et demande `[o/N]` ; toute autre réponse que oui annule ;
4. une fois confirmé, il met `herdr` dans le presse-papiers (clip, pbcopy, wl-copy, xclip ou xsel), attend Entrée, puis lance `herdr server stop`.

## Conséquences

- Aucun agent ne s'arrête sans une confirmation qui le dit.
- Là où la passation fonctionne, la confirmation n'est pas demandée : rien ne s'arrête.
- Relancer Herdr reste une étape manuelle, facilitée par le presse-papiers.
- L'entrée dépend des champs `update` de `herdr status --json` ; s'ils disparaissent, l'entrée dit que Herdr est à jour et la nouvelle version tourne au prochain démarrage.
