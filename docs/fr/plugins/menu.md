# menu

[English](../../en/plugins/menu.md) · **Français**

Lance un outil (lazygit, gh-dash, VS Code…) d'un clic ou d'une touche, met à jour Herdr et ouvre les fichiers de réglages. Dans un onglet qui affiche l'explorateur de fichiers de [herdr-explorer](https://github.com/gcaudan/herdr-explorer), le menu est un petit pane `Tools` au-dessus de lui, dans sa colonne : icônes, noms et chiffres sur le dossier du workspace. Ailleurs, c'est un petit popup. Les outils en terminal s'ouvrent dans un grand popup au-dessus du workspace courant ; les onglets et les panes restent tels quels derrière, et quitter l'outil ferme le popup.

```
   󰊢        󰊤        󰨞       󰚰       󰒓
lazygit  gh-dash  VS Code   Herdr  Réglages

     1,5G · 0,2% du disque
         3 552 fichiers
```

## Installation

```sh
herdr plugin install IMSike/Herdr-Plugins/menu
```

Liez ensuite une touche à son action dans le `config.toml` de Herdr :

```toml
[[keys.command]]
key = "prefix+space"
type = "plugin_action"
command = "imsike.menu.open"
description = "menu des outils"
```

Les icônes sont des glyphes [Nerd Font](https://www.nerdfonts.com) : sans Nerd Font dans le terminal, elles s'affichent comme des blancs ou des carrés.

## Utilisation

**Avec l'explorateur.** Le pane `Tools` s'ouvre avec l'explorateur, quelle que soit la façon dont il s'ouvre (`prefix+f`, ouverture automatique dans un nouvel onglet), et se ferme avec lui. Cliquez sur une icône ou sur son nom, ou tapez la touche de l'entrée quand le pane a le focus ; le survol affiche l'entrée en jaune. Un outil qui ne peut pas démarrer affiche pourquoi en rouge, à la place des chiffres, pendant 5 secondes. `prefix+space` ferme le pane et le rouvre ; Échap, `q` ou Ctrl+C dans le pane le ferme. La touche propre à une entrée l'emporte sur `q`.

Sous les icônes, les chiffres du dossier du workspace (celui de l'explorateur) :

- la taille totale de ses fichiers, tous fichiers compris (`.git` et dossiers de build aussi), et la part qu'elle représente de son disque. Un fichier lié plusieurs fois (liens durs, comme ceux de pnpm) compte à chaque fois, et un fichier cloud non téléchargé (OneDrive) compte pour sa taille entière ;
- son nombre de fichiers, hors `.git`, l'âge des chiffres à partir d'une heure (`· 3 h`), et une icône : un clic sur les chiffres mesure à nouveau le dossier (`…` pendant la lecture).

**Sans l'explorateur.** `prefix+space` ouvre le menu en popup, titré du nom du dossier du workspace. Choisissez une entrée par sa touche, par les flèches (ou `j`/`k`) et Entrée, ou d'un clic. Échap, `q` ou Ctrl+C ferme le menu. La touche propre à une entrée l'emporte sur `j`, `k` et `q`.

Un outil en terminal démarre dans le dossier du workspace (ou celui d'un terminal, voir plus bas), dans un popup de 95 % du terminal. Quand il ne peut pas démarrer ou se termine en erreur, le popup affiche pourquoi et attend Entrée. Un programme qui a sa propre fenêtre (VS Code) démarre directement, sans popup, et continue de tourner quand le menu ou le pane `Tools` se ferme.

## Entrées

Sans configuration, le menu propose :

| Touche | Entrée | Commande |
|---|---|---|
| `l` | lazygit | `lazygit` |
| `g` | gh-dash | `gh dash` (l'extension [gh-dash](https://github.com/dlvhdr/gh-dash) de la CLI GitHub) |
| `c` | VS Code | `code .` : le dossier du workspace dans VS Code |
| `u` | Herdr | action intégrée `update-herdr` : met à jour Herdr, voir [plus bas](#entrées-intégrées) |
| `s` | Réglages | action intégrée `settings` : ouvre les fichiers de réglages, voir [plus bas](#entrées-intégrées) |

Pour choisir les vôtres, écrivez `menu.toml` dans le dossier de configuration du plugin (`herdr plugin config-dir imsike.menu` l'affiche ; l'entrée **Réglages** l'ouvre, après l'avoir créé avec tous les réglages en commentaire). Ses entrées remplacent toutes les entrées intégrées ; un fichier sans entrée les garde :

```toml
[[entries]]
key = "l"                 # un caractère
label = "lazygit"
command = ["lazygit"]     # programme et arguments, lancés sans shell
icon = "󰊢"                # facultatif : un glyphe Nerd Font ; la touche sinon

[[entries]]
key = "t"
label = "btop"
command = ["btop"]
cwd = "pane"              # "workspace" (par défaut) ou "pane", voir plus bas

[[entries]]
key = "c"
label = "VS Code"
command = ["code", "."]
popup = false             # un programme avec sa propre fenêtre : pas de popup

[[entries]]
key = "u"
label = "Herdr"
action = "update-herdr"   # ou "settings" : une entrée intégrée, à la place de command
```

En tête du fichier, avant les entrées, les réglages des chiffres sous les icônes :

| Réglage | Par défaut | Effet |
|---|---|---|
| `decimal_comma` | `true` | `1,5G` (`true`) ou `1.5G` (`false`) |
| `percent_space` | `false` | `0,2%` (`false`) ou `0,2 %` avec une espace fine insécable (`true`) |

Le dossier où démarre un outil :

| `cwd` | Depuis le pane `Tools` | Depuis le menu en popup |
|---|---|---|
| `workspace` | le dossier de l'explorateur | le dossier du workspace |
| `pane` | le dossier du premier terminal de l'onglet | le dossier du pane actif |

Sous Windows, une commande est cherchée dans le `PATH`, avant les dossiers du système (`bash` est le premier du `PATH`, Git Bash par exemple, pas celui de WSL), et un fichier batch (`code` est `code.cmd`) se lance sans préciser son extension.

Un fichier illisible ou invalide (champ inconnu, commande vide, commande et action à la fois, touche utilisée deux fois) est ignoré : le menu affiche les entrées intégrées, et affiche l'erreur : le menu en popup au-dessus de sa ligne d'aide, le pane `Tools` en rouge à la place des chiffres pendant 5 secondes. Le pane `Tools` lit le fichier à son ouverture, le menu en popup à chaque ouverture ; aucun rechargement n'est nécessaire. Un outil démarre tel que le menu l'affichait, même si le fichier a changé entre-temps.

## Entrées intégrées

Les deux tournent dans le grand popup, qui montre ce qu'elles font et demande avant tout ce qui ne peut pas être annulé.

**Herdr** (`u`) installe la dernière version de Herdr avec `herdr update` (avec `--handoff` quand le serveur sait passer ses sessions à la nouvelle version, ce qu'il ne sait pas faire sous Windows). Puis `herdr status --json` indique si le serveur en cours est encore l'ancienne version :

- sinon, le popup dit que Herdr est à jour et attend Entrée ;
- si oui, il dit combien de panes se ferment et combien d'agents s'arrêtent (`5 panes, dont 2 agents (1 au travail)`) et demande `Redémarrer maintenant ? [o/N]`. Toute autre réponse que `o` (ou `oui`) annule : la nouvelle version tournera au prochain démarrage de Herdr.
- Une fois confirmé, `herdr` est mis dans le presse-papiers (avec `clip`, `pbcopy`, `wl-copy`, `xclip` ou `xsel`, le premier qui fonctionne), et Entrée arrête le serveur (`herdr server stop`) : **tous les panes se ferment et tous les agents s'arrêtent**, comme quand on quitte Herdr. Collez la commande dans le terminal pour relancer Herdr.

**Réglages** (`s`) ouvre dans un éditeur, ensemble :

- le `config.toml` de Herdr (le chemin que donne l'aide de Herdr, qui suit `HERDR_CONFIG_PATH`) ;
- le `menu.toml` du menu ;
- le `sidebar.toml` de la sidebar, quand le plugin [sidebar](sidebar.md) est installé.

Le fichier d'un plugin qui n'existe pas encore est d'abord créé avec tous les réglages en commentaire, à leur valeur par défaut. L'éditeur est `VISUAL`, sinon `EDITOR` (arguments permis : `code --wait`) ; sans eux, sous Windows, VS Code (`code --wait` : le popup attend que les onglets des fichiers soient fermés) quand il est dans le `PATH`, sinon le Bloc-notes, un fichier après l'autre ; ailleurs `nano`, sinon `vi`. Une fois l'éditeur fermé, le popup vérifie `menu.toml`, puis `config.toml` (`herdr config check`), et recharge la configuration de Herdr (`herdr server reload-config`) ; une erreur reste affichée jusqu'à Entrée. La sidebar lit son fichier à chaque passage ; le pane `Tools` lit `menu.toml` à son ouverture (`prefix+space` deux fois).

Le plugin ne fait qu'ouvrir le `config.toml` de Herdr : il ne l'écrit jamais ([ADR 0004](../adr/0004-plugin-settings.md), [ADR 0005](../adr/0005-herdr-update-entry.md)).

## Fonctionnement, implications

**Le pane `Tools`**

- C'est un pane Herdr ordinaire. Herdr ne coupe que vers la droite ou vers le bas : le pane s'ouvre sous l'explorateur, puis échange sa place avec lui, et le focus revient au pane qui l'avait.
- Il s'ouvre sur l'événement `pane.created` de l'explorateur, une fois que celui-ci ne bouge plus (il se place et se dimensionne juste après son ouverture : environ une seconde). Il se ferme sur l'événement `pane.closed`, dès que son onglet n'a plus d'explorateur, ou plus de pane terminal en dehors de lui et des panes propres à l'explorateur (Preview, Search…), pour que l'explorateur puisse fermer son onglet comme il le fait seul.
- Il demande 5 lignes, mais Herdr ne donne jamais moins d'un dixième d'un split à chacun de ses côtés : sur un écran de moins de 60 lignes, il est plus haut. Il reprend sa hauteur quand la fenêtre change de taille.
- **Tant qu'il est ouvert, l'explorateur ne garde plus sa largeur** : `<` et `>` ne font plus rien, et sa largeur suit celle de la fenêtre en proportion. L'explorateur ne cherche sa largeur que sur toute la hauteur de son propre pane ; une correction lui est proposée.
- Les chiffres sont mesurés en arrière-plan : tout le dossier est lu, ce qui prend un moment sur un gros dossier (`node_modules`, dossiers de build). La lecture s'arrête après 200 000 fichiers et dossiers ou 2 secondes, au premier atteint : les chiffres commencent alors par `≥` (au moins). Une lecture demandée d'un clic va jusqu'à 5 millions de fichiers et dossiers ou 60 secondes.
- Les chiffres sont gardés dans un fichier du dossier d'état du plugin, un par dossier (son chemin et ses chiffres ; s'il ne peut pas être écrit, le pane l'affiche en rouge pendant 5 secondes) : les panes d'un même dossier les partagent (pendant que l'un d'eux lit le dossier, les autres attendent ses chiffres), et un pane qui se rouvre les affiche sans relire le dossier. Ils sont mesurés à nouveau une fois plus vieux que :

  | Fichiers et dossiers lus | Nouvelle mesure après |
  | --- | --- |
  | moins de 10 000 | 2 minutes |
  | moins de 50 000 | 15 minutes |
  | moins de 200 000 | 2 heures |
  | plus, ou lecture arrêtée à sa limite | 1 jour |

- Il trouve l'explorateur par le nom de son pane, `Explorer` ; une version de l'explorateur qui le renommerait ramènerait le menu au seul popup.
- Après un redémarrage de Herdr, Herdr restaure le pane comme un simple shell ; le plugin le ferme au démarrage, et l'explorateur, en se rouvrant, en reçoit un nouveau.

**Le menu en popup**

- Herdr n'affiche qu'un popup à la fois. L'action reste donc en cours tant que le menu est ouvert : elle attend que le menu écrive le choix dans un fichier du dossier d'état du plugin, puis ouvre le popup de l'outil une fois celui du menu disparu. Pendant l'attente, elle occupe environ 4 Mo de mémoire, vérifie le fichier 20 fois par seconde, et vérifie une fois par seconde que le menu tourne encore.
- Si le popup du menu est fermé par autre chose que le menu lui-même, l'action voit en moins d'une seconde que le menu n'est plus là, et s'arrête.

Les commandes tournent sous votre compte, avec votre environnement, comme tout programme lancé dans un pane.

## Désactiver

Pour retirer une entrée intégrée, listez dans `menu.toml` les entrées à garder. Pour désactiver tout le menu, retirez la touche de `config.toml`, puis `herdr plugin disable imsike.menu` (ou `herdr plugin uninstall imsike.menu`) : les panes `Tools` se ferment avec leur explorateur ou au redémarrage suivant. `menu.toml` reste dans le dossier de configuration, et les fichiers des chiffres dans le dossier d'état, jusqu'à ce que vous les supprimiez.
