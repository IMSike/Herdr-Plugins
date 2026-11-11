# ADR 0002 — Versions des plugins : tags Git seulement, manifestes figés à `0.0.0`

- **Statut** : accepté
- **Date** : 2026-11-01

## Contexte

Herdr exige une `version` dans chaque `herdr-plugin.toml`, mais ne s'en sert pas pour mettre à jour : `herdr plugin update` récupère de nouveau la référence enregistrée à l'installation (la branche par défaut, `main`, sauf si `--ref` a été donné), ignore le plugin quand le commit n'a pas changé, et sinon recompile et enregistre la nouvelle copie. Il n'y a pas de mise à jour automatique ; les panes en cours gardent leurs fichiers jusqu'à leur fin, les nouveaux lancements utilisent la nouvelle copie. Les plugins liés localement (`herdr plugin link`) ne sont jamais mis à jour par Herdr. `herdr plugin list` affiche le commit depuis lequel un plugin a été installé.

Le dépôt est versionné comme OS-Setup et CC-Plugins : chaque push sur `main` reçoit un tag calculé d'après les messages de commit (`v0.0.0`, puis mineure pour un `feat`, correctif sinon). Leur workflow de release pose le tag et publie une release, sans rien commiter.

Options étudiées :

| Option | Avantages | Inconvénients |
|---|---|---|
| Une version par plugin, incrémentée à la main | L'historique de chaque plugin se lit seul | Un second schéma de version ; facile à oublier, et seulement informatif puisque les mises à jour suivent les commits |
| La version du dépôt, écrite dans les manifestes par le workflow de release | `herdr plugin list` montre de quelle version vient un plugin | Le workflow commite sur `main`, qui a alors un commit absent de `dev` : il faut le reporter sur `dev` (avance rapide si `dev` n'a pas bougé, sinon commit de fusion, rebase avec push forcé ou commit en double), et `dev` bouge sur GitHub après chaque release |
| Un `0.0.0` figé, la version dans le tag Git seulement | Rien à maintenir ; même schéma que les autres dépôts ; `main` et `dev` restent linéaires et `dev` se fusionne dans `main` en avance rapide | `herdr plugin list` affiche un numéro sans signification, à côté du commit |

## Décision

Chaque `herdr-plugin.toml` garde `version = "0.0.0"`, jamais modifié. La version n'existe que dans le tag Git et la release GitHub que le workflow de release crée à chaque push sur `main` ; le workflow ne commite rien.

## Conséquences

- Les utilisateurs mettent à jour avec `herdr plugin update` (tous les plugins installés depuis GitHub, ou ceux nommés), ou par OS-Setup ; rien ne se met à jour tout seul.
- La version d'un plugin se retrouve à partir du commit qu'affiche `herdr plugin list` : le tag de ce commit, ou le dernier avant lui.
- Le manifeste d'un nouveau plugin indique aussi `0.0.0`.
- La release porte aussi les binaires des plugins, que l'étape de build retrouve par le commit installé, pas par une version ([ADR 0008](0008-prebuilt-binaries.md)).

*Révisé le 2026-11-11 : le menu signale quand un plugin installé depuis GitHub a un nouveau commit, et son entrée Update lance `herdr plugin update` pour les plugins cochés ; rien ne se met pour autant à jour tout seul ([ADR 0009](0009-new-version-check.md)).*
