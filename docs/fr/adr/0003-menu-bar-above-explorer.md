# ADR 0003 — Le menu en pane au-dessus de l'explorateur de fichiers

- **Statut** : acceptée
- **Date** : 2026-11-01

## Contexte

Le plugin `menu` était d'abord un popup ouvert par `prefix+space`. Le mainteneur le voulait toujours visible, sous forme d'icônes cliquables dans la colonne de l'explorateur de fichiers de [herdr-explorer](https://github.com/gcaudan/herdr-explorer), un plugin d'un autre auteur.

Cette colonne n'est pas une zone de Herdr : l'explorateur est un pane `split` ordinaire, placé et dimensionné par ses propres hooks. N'importe quel plugin peut le couper. Herdr ne coupe que vers la droite ou vers le bas, et ne donne jamais moins d'un dixième d'un split à chacun de ses côtés.

Options étudiées :

| Option | Pour | Contre |
|---|---|---|
| Une pull request à herdr-explorer qui ajoute les icônes à son pane | Un seul pane ; un seul plugin à suivre | Dépend d'un autre mainteneur ; mêle un menu d'outils à un explorateur de fichiers |
| Un pane de ce plugin empilé au-dessus de l'explorateur, trouvé par son nom | Reste dans ce dépôt ; le popup demeure là où il n'y a pas d'explorateur | Repose sur le nom du pane de l'explorateur (`Explorer`), pas sur une API ; l'explorateur cesse de garder sa largeur tant que le pane est là ; au moins un dixième de la colonne en hauteur |
| Garder le seul popup | Rien à maintenir | Pas visible en permanence |

## Décision

Le menu ouvre un pane `Tools` au-dessus de l'explorateur : coupé sous lui, puis échangé avec lui. Il suit l'explorateur par les événements de Herdr : le `pane.created` d'un pane nommé `Explorer` l'ouvre, une fois l'explorateur immobile ; `pane.closed` le ferme quand son onglet n'a plus d'explorateur, ou plus d'autre pane qu'eux deux. `prefix+space` le ferme et le rouvre dans un onglet avec l'explorateur, et ouvre le menu en popup ailleurs.

La gestion de la largeur de l'explorateur, qui ne regarde que la hauteur entière de son pane, est corrigée chez lui par une pull request à herdr-explorer plutôt que contournée ici.

## Conséquences

- Sans herdr-explorer, ou avec une version qui renomme son pane, le menu est le popup, comme avant.
- Tant que la correction de l'explorateur n'est pas publiée, `<` / `>` et la gestion de sa largeur ne font plus rien quand le pane `Tools` est ouvert.
- Le pane fait au moins un dixième de la colonne en hauteur : sur un petit écran, il montre des lignes vides.
- Après un redémarrage de Herdr, les shells `Tools` restaurés sont fermés par un hook de démarrage ; l'explorateur se rouvre avec un nouveau pane.
