# menu

[English](../../en/plugins/menu.md) · **Français**

Un petit menu en popup, ouvert par une touche, qui lance un outil (lazygit, gh-dash…) dans un grand popup au-dessus du workspace courant. Les onglets et les panes restent tels quels derrière ; quitter l'outil ferme le popup.

## Installation

```sh
herdr plugin install IMSike/Herdr-Plugins/menu
```

Puis lier une touche à son action dans le `config.toml` de Herdr :

```toml
[[keys.command]]
key = "prefix+space"
type = "plugin_action"
command = "imsike.menu.open"
description = "menu des outils"
```

## Utilisation

`prefix+space` ouvre le menu, titré du nom du dossier du workspace. Choisir une entrée par sa touche, par les flèches (ou `j`/`k`) et Entrée, ou d'un clic. Échap, `q` ou Ctrl+C ferme le menu. La touche propre à une entrée l'emporte sur `j`, `k` et `q`.

L'outil démarre dans le dossier du workspace (ou celui du pane actif, voir plus bas), dans un popup de 95 % du terminal. Quand l'outil ne peut pas démarrer ou se termine en erreur, le popup affiche pourquoi et attend Entrée.

## Entrées

Sans configuration, le menu propose :

| Touche | Entrée | Commande |
|---|---|---|
| `l` | lazygit | `lazygit` |
| `g` | gh-dash | `gh dash` (l'extension [gh-dash](https://github.com/dlvhdr/gh-dash) de la CLI GitHub) |

Pour choisir les vôtres, écrire `menu.toml` dans le dossier de configuration du plugin (`herdr plugin config-dir imsike.menu` l'affiche). Le fichier remplace les entrées intégrées :

```toml
[[entries]]
key = "l"                 # un caractère
label = "lazygit"
command = ["lazygit"]     # programme et arguments, lancés sans shell

[[entries]]
key = "t"
label = "btop"
command = ["btop"]
cwd = "pane"              # "workspace" (par défaut) ou "pane" : le dossier du pane actif
```

Un fichier illisible ou invalide (champ inconnu, commande vide, touche utilisée deux fois) est ignoré : le menu affiche les entrées intégrées et l'erreur sur sa dernière ligne. Le fichier est relu à chaque ouverture du menu ; aucun rechargement n'est nécessaire.

## Fonctionnement, implications

- Herdr n'affiche qu'un popup à la fois. L'action reste donc en cours tant que le menu est ouvert : elle attend que le menu écrive le choix dans un fichier du dossier d'état du plugin, puis ouvre le popup de l'outil une fois celui du menu disparu. Pendant l'attente, elle occupe environ 4 Mo de mémoire et vérifie le fichier 20 fois par seconde.
- Si le popup du menu est fermé par autre chose que le menu lui-même, l'action continue d'attendre, sans effet visible, une heure au plus.
- Les commandes tournent sous votre compte, avec votre environnement, comme tout programme lancé dans un pane.

## Désactiver

Retirer la touche de `config.toml`, puis `herdr plugin disable imsike.menu` (ou `herdr plugin uninstall imsike.menu`). `menu.toml` reste dans le dossier de configuration jusqu'à ce que vous le supprimiez.
