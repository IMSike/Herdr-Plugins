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

L'option a a d'abord été essayée, puis abandonnée pour les agents : dans le panneau Agents, les chiffres colorés ne rendaient pas aussi bien que prévu. Les couleurs restent dans le seul pane `Tools`, qui les dessine lui-même :

- le réglage `colors` de la sidebar (désactivé par défaut) les active ; la sémantique de `gt` de Herdr est gardée (au-dessus d'un seuil, strictement, c'est le niveau suivant) ;
- deux sortes de seuils, chacune un jeu de trois valeurs croissantes : le total de Herdr (son serveur et tout ce qu'il lance), `cpu_levels` en pourcentage de la machine et `memory_levels` en Go ; la machine, `machine_cpu_levels` et `machine_memory_levels` en pourcentage ;
- la sidebar écrit les totaux avec leurs seuils dans `totals.json` ; le pane `Tools` en déduit les niveaux. Ses barres (`totals = "bars"` dans `menu.toml`) passent du vert au rouge en dégradé par les mêmes seuils, que `colors` soit activé ou non.

Les valeurs de la sidebar ne portent aucun marqueur : le panneau Agents garde les couleurs du `config.toml` de Herdr.

## Conséquences

- Aucune règle à ajouter au `config.toml` de Herdr ; les chiffres des agents ne sont pas colorés par niveau.
- `memory_levels` est en Go des processus de Herdr alors que sa barre se remplit de la part de la mémoire de la machine : le dégradé suit les seuils en Go le long de la barre.
- Colorer à nouveau les agents demanderait l'option a ou b, une fois le problème de rendu résolu.
