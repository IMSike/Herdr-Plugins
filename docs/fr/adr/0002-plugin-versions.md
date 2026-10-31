# ADR 0002 — Versions des plugins : celle du dépôt, écrite par le workflow de release

- **Statut** : acceptée
- **Date** : 2026-10-31

## Contexte

Herdr exige une `version` dans chaque `herdr-plugin.toml`, mais ne s'en sert pas pour mettre à jour : `herdr plugin update` récupère de nouveau la référence enregistrée à l'installation (la branche par défaut, `main`, sauf si `--ref` a été donné), ignore le plugin quand le commit n'a pas changé, et sinon recompile et enregistre la nouvelle copie. Il n'y a pas de mise à jour automatique ; les panes en cours gardent leurs fichiers jusqu'à leur fin, les nouveaux lancements utilisent la nouvelle copie. Les plugins liés localement (`herdr plugin link`) ne sont jamais mis à jour par Herdr.

Le dépôt est versionné comme OS-Setup et CC-Plugins : chaque push sur `main` reçoit un tag calculé d'après les messages de commit (`v0.0.0`, puis mineure pour un `feat`, correctif sinon).

Options étudiées :

| Option | Pour | Contre |
|---|---|---|
| Une version par plugin, incrémentée à la main | L'historique de chaque plugin se lit seul | Un second schéma de version ; facile à oublier, et seulement informatif puisque les mises à jour suivent les commits |
| Un `0.0.0` figé | Rien à maintenir | `herdr plugin list` affiche un numéro sans signification |
| La version du dépôt, écrite dans les manifestes par le workflow de release | Un seul schéma de version ; `herdr plugin list` montre de quelle version vient un plugin | Le workflow commite sur `main` ; tous les manifestes reçoivent le nouveau numéro, modifiés ou non |

## Décision

Les manifestes portent la version du dépôt. À chaque push sur `main`, le workflow de release calcule la version suivante, l'écrit dans chaque `herdr-plugin.toml`, commite sous `build(release): vX.Y.Z`, puis pose le tag sur ce commit et publie la release. Sur `dev`, les manifestes gardent le numéro qu'ils ont ; un nouveau plugin commence à `0.0.0`.

## Conséquences

- Les utilisateurs mettent à jour avec `herdr plugin update` (tous les plugins installés depuis GitHub, ou ceux nommés), ou par OS-Setup ; rien ne se met à jour tout seul.
- `main` ne reçoit que ce qui est publié : sa tête est toujours un commit de release tagué.
- Le commit de release n'existe que sur `main` ; fusionner `dev` dans `main` n'entre pas en conflit avec lui tant que `dev` ne modifie pas la ligne `version` d'un manifeste.
