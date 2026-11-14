# ADR 0011 — Une action par outil

- **Statut** : accepté
- **Date** : 2026-11-14

## Contexte

Le mainteneur voulait une touche par outil (`prefix+l` pour lazygit, `prefix+d` pour gh-dash…), sans passer par le menu. Les `[[keys.command]]` de Herdr avec `type = "popup"` savent lancer un programme, mais dans le dossier du pane actif plutôt que celui du workspace, sans le message d'erreur du menu, et sans tenir compte de `menu.toml`. Une action de plugin ne reçoit aucun argument : seulement son identifiant (`HERDR_PLUGIN_ACTION_ID`), et les actions sont figées dans le manifeste, alors que les entrées du menu, touches comprises, se règlent dans `menu.toml`.

Options étudiées :

| Option | Avantages | Inconvénients |
|---|---|---|
| a. Commandes `type = "popup"` / `type = "shell"` de Herdr | Aucun code | Dossier du pane actif ; pas le message d'erreur du menu ; `menu.toml` ignoré |
| b. Une action par touche du menu (`entry-a` … `entry-z`), qui lance l'entrée de cette touche | La touche reste valable quel que soit l'outil que `menu.toml` met derrière | 26 actions déclarées ; le raccourci nomme une lettre, pas un outil |
| c. Une action par outil intégré (`lazygit`, `gh-dash`, `vscode`, `btop`) | Quatre actions lisibles, nommées d'après ce qu'elles lancent | Une entrée renommée ou ajoutée dans `menu.toml` n'a pas d'action |

## Décision

Option c. Les quatre actions partagent une commande, `herdr-menu tool`, qui lit l'identifiant de l'action. Elle lance l'entrée de `menu.toml` qui porte le libellé intégré de l'outil (`lazygit`, `gh-dash`, `VS Code`, `btop`, sans tenir compte de la casse), l'entrée intégrée s'il n'y en a pas, comme le menu en popup lance une entrée choisie : le popup `run`, ou seul pour un programme qui a sa propre fenêtre, dans le dossier que demande son `cwd`. Le plugin ne lie aucune touche : le manifeste ne le permet pas, et les touches appartiennent au `config.toml` de l'utilisateur.

## Conséquences

- Une touche lance un outil, dans le même popup et le même dossier que depuis le menu.
- La commande ou le `cwd` d'une entrée changés dans `menu.toml` sont suivis ; une entrée renommée ne l'est pas : l'action lance l'entrée intégrée.
- Un outil ajouté dans `menu.toml` n'a pas d'action propre ; en ajouter une demande une nouvelle action dans le manifeste.
- Un `menu.toml` illisible est journalisé et l'entrée intégrée démarre.
