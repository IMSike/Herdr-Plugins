# ADR 0006 — Couleurs par seuil

- **Statut** : accepté
- **Date** : 2026-11-01

## Contexte

Le mainteneur voulait colorer les chiffres de CPU et de mémoire en vert, jaune, orange ou rouge selon des seuils qu'il peut régler, dans le pane `Tools` du menu et sur les agents de la sidebar.

Le pane `Tools` dessine son propre texte : il peut le colorer directement. La sidebar ne fait que fournir des valeurs de tokens ; Herdr les colore, et seulement par les règles de son `config.toml`, qu'aucun plugin n'écrit. Une règle teste la valeur du token lui-même avec `equals`, `contains`, `starts_with`, `gt` ou `lt` ; `gt` et `lt` ne correspondent qu'à une valeur qui est un nombre seul, pas à `4% 853M`.

Options étudiées :

| Option | Avantages | Inconvénients |
|---|---|---|
| a. Marqueurs invisibles : le plugin termine une valeur par un à quatre espaces de largeur nulle (U+200B) selon le niveau ; quatre règles `contains` par token dans `config.toml` | Seuils dans `sidebar.toml`, réglables depuis l'écran des réglages ; valeurs inchangées à l'écran ; un seul token comme avant | Un caractère caché dans la valeur ; quatre règles par token à ajouter une fois |
| b. Un token par niveau (`$usage_green` … `$usage_red`), un seul fourni | Pas de caractère caché ; seuils dans `sidebar.toml` | Quatre tokens par valeur dans les lignes de `config.toml` |
| c. Nombres seuls (`$cpu_n = "45"`) avec des règles `gt` | Le mécanisme propre à Herdr | Seuils seulement dans `config.toml`, pas réglables depuis l'écran ; chiffres affichés sans unité |

Seuils : le CPU d'un agent est une part de toute la machine, et la mémoire utilisée de la machine dépasse souvent la moitié : un seul jeu pour les deux laisserait les agents sans couleur ou la machine toujours rouge.

## Décision

L'option a. Le réglage `colors` de la sidebar (désactivé par défaut) active les marqueurs ; la sémantique de `gt` de Herdr s'applique (au-dessus d'un seuil, strictement, c'est le niveau suivant). Deux sortes de seuils, chacune un jeu de trois valeurs croissantes :

- les processus (agents, workspaces, total de Herdr) : `cpu_levels` en pourcentage de la machine, `memory_levels` en Go ;
- la machine : `machine_cpu_levels` et `machine_memory_levels`, en pourcentage.

`$cpu` et `$ram` portent leur propre niveau ; `$usage`, `$stats` et `$ws_stats` le plus haut de leur CPU et de leur mémoire. La sidebar écrit les niveaux des totaux dans `totals.json`, que le pane `Tools` colore avec les mêmes couleurs que les règles d'exemple.

## Conséquences

- Les couleurs des agents demandent les règles dans le `config.toml` de Herdr (`dotfiles/herdr/` d'OS-Setup) ; les pages des plugins les donnent.
- Couleurs activées, une règle `equals` sur un token de chiffres ne correspond plus.
- Si Herdr se mettait à dessiner les caractères de largeur nulle, ou à les retirer des valeurs, les marqueurs s'afficheraient ou ne correspondraient plus ; un seul réglage désactive les couleurs.
