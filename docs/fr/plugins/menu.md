# menu

[English](../../en/plugins/menu.md) · **Français**

Lance un outil (lazygit, gh-dash, VS Code…) d'un clic ou d'une touche, met à jour Herdr et règle les plugins. Dans un onglet qui affiche l'explorateur de fichiers de [herdr-explorer](https://github.com/gcaudan/herdr-explorer), le menu est un petit pane `Tools` au-dessus de lui, dans sa colonne : icônes et leurs noms, et le CPU et la mémoire de Herdr et de la machine. Ailleurs, c'est un petit popup. Les outils en terminal s'ouvrent dans un grand popup au-dessus du workspace courant ; les onglets et les panes restent tels quels derrière, et quitter l'outil ferme le popup.

```
      󰊢         󰊤         󰨞
   lazygit   gh-dash   VS Code
      󰋊         󰚰         󰒓
     Disk     Update   Settings
       2% 3,1G · 󰍹 10% 22,4G
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

**Avec l'explorateur.** Le pane `Tools` s'ouvre avec l'explorateur, quelle que soit la façon dont il s'ouvre (`prefix+f`, ouverture automatique dans un nouvel onglet), et se ferme avec lui. Cliquez sur une icône ou sur son nom, ou tapez la touche de l'entrée quand le pane a le focus ; le survol affiche l'entrée en jaune. Un outil qui ne peut pas démarrer affiche pourquoi en rouge, sous les icônes, pendant 5 secondes. `prefix+space` ferme le pane et le rouvre ; Échap, `q` ou Ctrl+C dans le pane le ferme. La touche propre à une entrée l'emporte sur `q`.

Les icônes se disposent de deux façons, selon `layout` (voir [Réglages](#réglages)) :

- `double` (par défaut) : deux lignes d'icônes, chacune avec son nom en dessous. Le pane demande 5 lignes.
- `single` : une ligne d'icônes ; la ligne du dessous affiche le nom de l'icône sous la souris, et rien sinon. Le pane demande 3 lignes.

La dernière ligne affiche le CPU et la mémoire des processus de Herdr, puis de la machine (`2% 3,1G · 󰍹 10% 22,4G`), tels que la [sidebar](sidebar.md) les mesure, en gras ; avec les couleurs par seuil de la sidebar (`colors = true` dans `sidebar.toml`), chaque chiffre est vert, jaune, orange ou rouge selon ses [seuils](sidebar.md#couleurs-par-seuil). Sans la sidebar, ou quand ses chiffres ont plus de 30 secondes, la ligne reste vide.

**Sans l'explorateur.** `prefix+space` ouvre le menu en popup, titré du nom du dossier du workspace. Choisissez une entrée par sa touche, par les flèches (ou `j`/`k`) et Entrée, ou d'un clic. Échap, `q` ou Ctrl+C ferme le menu. La touche propre à une entrée l'emporte sur `j`, `k` et `q`. L'entrée **Disk** n'appartient qu'au pane `Tools` : le menu en popup l'omet.

Un outil en terminal démarre dans le dossier du workspace (ou celui d'un terminal, voir plus bas), dans un popup de 95 % du terminal. Quand il ne peut pas démarrer ou se termine en erreur, le popup affiche pourquoi et attend Entrée. Un programme qui a sa propre fenêtre (VS Code) démarre directement, sans popup, et continue de tourner quand le menu ou le pane `Tools` se ferme.

## Entrées

Sans configuration, le menu propose :

| Touche | Entrée | Commande |
|---|---|---|
| `l` | lazygit | `lazygit` |
| `g` | gh-dash | `gh dash` (l'extension [gh-dash](https://github.com/dlvhdr/gh-dash) de la CLI GitHub) |
| `c` | VS Code | `code .` : le dossier du workspace dans VS Code |
| `d` | Disk | action intégrée `disk` : les chiffres du dossier du workspace, voir [plus bas](#entrées-intégrées) |
| `u` | Update | action intégrée `update` : met à jour Herdr, voir [plus bas](#entrées-intégrées) |
| `s` | Settings | action intégrée `settings` : l'écran des réglages, voir [plus bas](#entrées-intégrées) |

Pour choisir les vôtres, écrivez `menu.toml` dans le dossier de configuration du plugin (`herdr plugin config-dir imsike.menu` l'affiche ; l'écran des réglages le crée, avec tous les réglages en commentaire, la première fois qu'il en enregistre un ou qu'il ouvre les fichiers). Ses entrées remplacent toutes les entrées intégrées ; un fichier sans entrée les garde :

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
label = "Update"
action = "update"         # ou "settings", "disk" : une entrée intégrée, à la place de command
```

Le dossier où démarre un outil :

| `cwd` | Depuis le pane `Tools` | Depuis le menu en popup |
|---|---|---|
| `workspace` | le dossier de l'explorateur | le dossier du workspace |
| `pane` | le dossier du premier terminal de l'onglet | le dossier du pane actif |

Sous Windows, une commande est cherchée dans le `PATH`, avant les dossiers du système (`bash` est le premier du `PATH`, Git Bash par exemple, pas celui de WSL), et un fichier batch (`code` est `code.cmd`) se lance sans préciser son extension.

Un fichier illisible ou invalide (champ inconnu, commande vide, commande et action à la fois, touche utilisée deux fois) est ignoré : le menu affiche les entrées intégrées, et affiche l'erreur : le menu en popup au-dessus de sa ligne d'aide, le pane `Tools` en rouge sous les icônes pendant 5 secondes. Le menu en popup lit le fichier à chaque ouverture ; le pane `Tools` le relit dans les 2 secondes qui suivent un changement, et prend la hauteur de sa nouvelle disposition. Un outil démarre tel que le menu l'affichait, même si le fichier a changé entre-temps.

## Réglages

En tête de `menu.toml`, avant les entrées ; l'écran des réglages les change aussi :

| Réglage | Par défaut | Effet |
|---|---|---|
| `decimal_comma` | `true` | `1,5G` (`true`) ou `1.5G` (`false`) |
| `percent_space` | `false` | `0,2%` (`false`) ou `0,2 %` avec une espace fine insécable (`true`) |
| `cpu_decimal` | `false` | les parts de CPU des totaux : `4%` (`false`) ou `4,2%` (`true`) |
| `memory_unit` | `"M"` | `853M`, `2,4G` (`"M"`) ; `853 Mo`, `2,4 Go` (`"Mo"`) ; `853 MB`, `2,4 GB` (`"MB"`) ; `K`, `M` ou `G` selon la taille |
| `layout` | `"double"` | les icônes du pane `Tools` : `"double"` (deux lignes, avec leurs noms) ou `"single"` (une ligne, le nom de l'icône survolée en dessous) |

## Entrées intégrées

**Disk** (`d`) passe le pane `Tools` aux chiffres du dossier du workspace (celui de l'explorateur), sous un bouton **󰁍 Retour** (Échap, `q` ou Retour arrière reviennent aussi) :

- la taille totale de ses fichiers, tous fichiers compris (`.git` et dossiers de build aussi), et la part qu'elle représente de son disque. Un fichier lié plusieurs fois (liens durs, comme ceux de pnpm) compte à chaque fois, et un fichier cloud non téléchargé (OneDrive) compte pour sa taille entière ;
- son nombre de fichiers, hors `.git`, l'âge des chiffres à partir d'une heure (`· 3 h`), et une icône : un clic sur les chiffres, ou `r`, mesure à nouveau le dossier (`…` pendant la lecture).

Le dossier n'est mesuré que pendant que cette page est affichée : revenir arrête la mesure.

**Update** (`u`) ouvre une fenêtre de terminal à part, hors de Herdr : Herdr refuse de se mettre à jour depuis l'un de ses panes. Sous Windows, c'est une nouvelle fenêtre de console ; ailleurs, le premier terminal trouvé parmi `x-terminal-emulator`, `gnome-terminal`, `konsole`, `xfce4-terminal`, `kitty`, `alacritty`, `wezterm`, `foot` et `xterm`. Dans cette fenêtre :

1. `herdr update` installe la dernière version de Herdr (avec `--handoff` quand le serveur sait passer ses sessions à la nouvelle version, ce qu'il ne sait pas faire sous Windows).
2. `herdr status --json` indique si le serveur en cours est encore l'ancienne version. Sinon, la fenêtre dit que Herdr est à jour : le client déjà ouvert garde son ancienne version jusqu'à ce que vous vous détachiez (`prefix+q`) et relanciez `herdr`.
3. Si oui, elle dit combien de panes se ferment et combien d'agents s'arrêtent (`5 panes, dont 2 agents (1 au travail)`) et demande `Redémarrer maintenant ? [o/N]`. Toute autre réponse que `o` (ou `oui`) annule : la nouvelle version tournera au prochain démarrage du serveur.
4. Une fois confirmé, elle arrête le serveur (`herdr server stop`) : **tous les panes se ferment et tous les agents s'arrêtent**, comme quand on quitte Herdr. Puis elle propose de relancer Herdr dans cette fenêtre (`[o/N]`) ; sinon elle met `herdr` dans le presse-papiers (avec `clip`, `pbcopy`, `wl-copy`, `xclip` ou `xsel`, le premier qui fonctionne), à coller dans un terminal.

La fenêtre attend Entrée avant de se fermer, erreur comprise.

**Settings** (`s`) ouvre un écran de réglages dans un popup de 70 % de la largeur du terminal et 80 % de sa hauteur ; la liste défile quand elle ne tient pas (une flèche en haut à droite le signale). Haut et bas (ou `j`/`k`) choisissent une ligne ; gauche et droite (ou `h`/`l`), Entrée ou Espace la changent, et chaque changement est enregistré aussitôt ; Échap ou `q` ferme l'écran.

| Ligne | Effet | Fichier |
|---|---|---|
| Virgule décimale, Espace avant %, CPU avec une décimale, Unités de la mémoire | `decimal_comma`, `percent_space`, `cpu_decimal`, `memory_unit` | `menu.toml` et `sidebar.toml` |
| Icônes | `layout` | `menu.toml` |
| Durée de vie des valeurs | `ttl_seconds`, par pas de 5 secondes, de 5 à 600 | `sidebar.toml` |
| Shells | `shells` : Entrée pour les taper, séparés par des virgules, Entrée pour enregistrer, Échap pour annuler | `sidebar.toml` |
| Totaux dans la barre d'onglets | `tab_bar` | `sidebar.toml` |
| Couleurs | `colors` : les [couleurs par seuil](sidebar.md#couleurs-par-seuil) | `sidebar.toml` |
| Seuils CPU, Seuils mémoire (agents, Herdr ; machine) | les quatre jeux de seuils : Entrée pour taper trois valeurs croissantes séparées par des espaces (`10 25 50`), Entrée pour enregistrer, Échap pour annuler | `sidebar.toml` |
| Ouvrir les fichiers de configuration… | ouvre les fichiers dans un éditeur, voir plus bas | |
| Recharger la configuration de Herdr | recharge le `config.toml` de Herdr et dit si Herdr l'a accepté | |

Les lignes Sidebar et Couleurs n'apparaissent que si le plugin [sidebar](sidebar.md) est installé. Un fichier est écrit en gardant ses commentaires et ses autres réglages : un réglage en commentaire y prend sa valeur à sa place. Le fichier d'un plugin qui n'existe pas encore est d'abord créé depuis son modèle, tous les réglages en commentaire à leur valeur par défaut.

**Ouvrir les fichiers de configuration…** ouvre ensemble le `config.toml` de Herdr (le chemin que donne l'aide de Herdr, qui suit `HERDR_CONFIG_PATH`), `menu.toml`, et `sidebar.toml` quand la sidebar est installée. L'éditeur est `VISUAL`, sinon `EDITOR` (arguments permis) ; sans eux, sous Windows, VS Code quand il est dans le `PATH`, sinon le Bloc-notes, une fenêtre par fichier ; ailleurs `nano`, sinon `vi`. L'écran des réglages se ferme et Herdr reste libre d'utilisation :

- un éditeur qui a sa propre fenêtre (VS Code, Bloc-notes, Sublime Text, gedit…) démarre seul, hors de Herdr ;
- un éditeur en terminal (`nano`, `vim`, `hx`…) s'ouvre dans un nouvel onglet de Herdr ; une fois fermé, l'onglet demande `Recharger la configuration de Herdr ? [O/n]`, puis se ferme sur Entrée.

Ouvert depuis un pane `Tools`, l'écran fait aussi passer ce pane à deux boutons : **󰑓 Recharger** (ou `r`, Entrée) recharge la configuration de Herdr une fois les fichiers enregistrés, **󰁍 Retour** ramène les icônes. La sidebar lit son fichier à chaque passage, et le pane `Tools` relit `menu.toml` quand il change : seul le `config.toml` de Herdr demande le rechargement.

Le plugin ne fait qu'ouvrir le `config.toml` de Herdr : il ne l'écrit jamais ([ADR 0004](../adr/0004-plugin-settings.md), [ADR 0005](../adr/0005-herdr-update-entry.md)).

## Fonctionnement, implications

**Le pane `Tools`**

- C'est un pane Herdr ordinaire. Herdr ne coupe que vers la droite ou vers le bas : le pane s'ouvre sous l'explorateur, puis échange sa place avec lui, et le focus revient au pane qui l'avait.
- Il s'ouvre sur l'événement `pane.created` de l'explorateur, une fois que celui-ci ne bouge plus (il se place et se dimensionne juste après son ouverture : environ une seconde). Il se ferme sur l'événement `pane.closed`, dès que son onglet n'a plus d'explorateur, ou plus de pane terminal en dehors de lui et des panes propres à l'explorateur (Preview, Search…), pour que l'explorateur puisse fermer son onglet comme il le fait seul.
- Il demande 5 lignes (3 dans la disposition `single`), mais Herdr ne donne jamais moins d'un dixième d'un split à chacun de ses côtés : sur un écran de moins de 60 lignes (40 en `single`), il est plus haut. Il reprend sa hauteur quand la fenêtre change de taille.
- **Tant qu'il est ouvert, l'explorateur ne garde plus sa largeur** : `<` et `>` ne font plus rien, et sa largeur suit celle de la fenêtre en proportion. L'explorateur ne cherche sa largeur que sur toute la hauteur de son propre pane ; une correction lui est proposée.
- La ligne CPU et mémoire lit, toutes les 2 secondes, un fichier que la sidebar écrit dans son dossier d'état à chacun de ses passages (toutes les 5 secondes) : le pane ne mesure rien lui-même.
- Les chiffres du disque sont mesurés en arrière-plan, seulement pendant que la page Disk est affichée : tout le dossier est lu, ce qui prend un moment sur un gros dossier (`node_modules`, dossiers de build). La lecture s'arrête après 200 000 fichiers et dossiers ou 2 secondes, au premier atteint : les chiffres commencent alors par `≥` (au moins). Une lecture demandée d'un clic va jusqu'à 5 millions de fichiers et dossiers ou 60 secondes.
- Les chiffres sont gardés dans un fichier du dossier d'état du plugin, un par dossier (son chemin et ses chiffres ; s'il ne peut pas être écrit, le pane l'affiche en rouge pendant 5 secondes) : les panes d'un même dossier les partagent (pendant que l'un d'eux lit le dossier, les autres attendent ses chiffres), et un pane qui les affiche à nouveau ne relit pas le dossier. Ils sont mesurés à nouveau une fois plus vieux que :

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

**Réglages et mise à jour**

- L'écran des réglages est un popup : Herdr attend tant qu'il est ouvert, mais il ne reste ouvert que le temps de changer des réglages. Les éditeurs tournent en dehors, dans leur fenêtre ou dans un onglet.
- La fenêtre de mise à jour tourne hors de Herdr, sans la variable par laquelle Herdr signale ses panes (`HERDR_ENV`) ; elle continue si le pane `Tools` se ferme.

Les commandes tournent sous votre compte, avec votre environnement, comme tout programme lancé dans un pane.

## Désactiver

Pour retirer une entrée intégrée, listez dans `menu.toml` les entrées à garder. Pour désactiver tout le menu, retirez la touche de `config.toml`, puis `herdr plugin disable imsike.menu` (ou `herdr plugin uninstall imsike.menu`) : les panes `Tools` se ferment avec leur explorateur ou au redémarrage suivant. `menu.toml` reste dans le dossier de configuration, et les fichiers des chiffres dans le dossier d'état, jusqu'à ce que vous les supprimiez.
