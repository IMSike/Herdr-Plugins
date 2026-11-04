# ADR 0007 — État par session de Herdr

- **Statut** : accepté
- **Date** : 2026-11-04

## Contexte

Herdr fait tourner des sessions nommées côte à côte (`herdr --session <nom>`), chacune avec son serveur et son socket (`…/sessions/<nom>/herdr.sock` ; celui de la session par défaut est dans le dossier de configuration de Herdr). Les plugins sont installés pour l'utilisateur, dans toutes les sessions, et Herdr donne à chaque plugin un seul dossier d'état pour toutes (`HERDR_PLUGIN_STATE_DIR`). Les ids de panes et d'onglets recommencent à `w1` dans chaque session.

Le menu y gardait ses barres (`bars/<id du pane>`) et les verrous des onglets, et chaque `pane.closed` retire les barres que Herdr ne liste plus : une deuxième session retirait celles de la première, qui ouvrait alors une deuxième barre sur `prefix+space`. Le `totals.json` et le `sample.json` de la sidebar étaient écrasés par la dernière session passée.

Options étudiées :

| Option | Avantages | Inconvénients |
|---|---|---|
| a. Un dossier par session dans le dossier d'état, au nom du socket de la session | Les sessions ne voient plus les fichiers des autres ; aucun réglage | Un niveau de dossier de plus ; un socket placé ailleurs par `HERDR_SOCKET_PATH` compte comme la session par défaut |
| b. Documenter « une seule session de Herdr à la fois » | Pas de code | Les sessions nommées cassent la barre et mélangent les totaux |

## Décision

Option a. `herdr_api::session_dir` est `sessions/<session>/` du dossier d'état du plugin : `<session>` est le dossier d'un socket `…/sessions/<nom>/herdr.sock`, `default` sinon. Il garde ce qui nomme un pane ou un onglet, ou décrit la session :

- menu : le dossier `bars`, les verrous des onglets (`opening-*.lock`), les drapeaux des fichiers ouverts depuis un pane `Tools` (`files-open-*`) ;
- sidebar : `sample.json` et `totals.json`, que le pane `Tools` lit dans le dossier de la sidebar pour la même session.

Ce qui ne tient pas à une session reste en haut du dossier d'état : les chiffres d'un dossier (`stats-*`), le verrou de la mise à jour (le binaire de Herdr est le même pour toutes les sessions), les fichiers de choix du menu, le binaire de la sidebar et `config-dir`, et `seq`, qui n'a qu'à croître.

## Conséquences

- Deux sessions ont chacune leurs barres et leurs totaux.
- Les fichiers écrits avant ce changement en haut du dossier d'état (`bars`, `opening-*.lock`, `totals.json`, `sample.json`) ne sont plus lus ; on peut les supprimer.
- Une session supprimée laisse son dossier, comme les autres fichiers du dossier d'état.
