# menu

[English](../../en/plugins/menu.md) · **Français**

Lance un outil (lazygit, gh-dash, VS Code…) d'un clic ou d'une touche, met à jour Herdr et les plugins installés depuis GitHub, et règle les plugins. Dans un onglet qui affiche l'explorateur de fichiers de [herdr-explorer](https://github.com/gcaudan/herdr-explorer), le menu est un petit pane `Tools` au-dessus de lui, dans sa colonne : une ligne d'icônes, le nom de celle sous la souris, et le CPU et la mémoire de Herdr et de la machine. Ailleurs, c'est un petit popup. Les outils en terminal s'ouvrent dans un grand popup au-dessus du workspace courant ; les onglets et les panes restent tels quels derrière, et quitter l'outil ferme le popup.

```
   󰊢    󰊤    󰨞    󰋊    󰄪    󰚰    󰒓
                btop
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

**Avec l'explorateur.** Le pane `Tools` s'ouvre avec l'explorateur, quelle que soit la façon dont il s'ouvre (`prefix+f`, ouverture automatique dans un nouvel onglet), et se ferme avec lui. Cliquez sur une icône, ou tapez la touche de l'entrée quand le pane a le focus ; le survol affiche l'entrée en jaune. Un outil qui ne peut pas démarrer affiche pourquoi en rouge, sous les icônes, pendant 5 secondes. Un double-clic, ou une touche maintenue, lance l'entrée une seule fois. Seuls `prefix+space`, qui ferme le pane et le rouvre, et la fermeture de l'explorateur ferment le pane : aucune touche tapée dans le pane (Échap, `q`, Ctrl+C) ne le ferme, pour qu'une touche Échap de trop ne le fasse pas disparaître. Pour que l'explorateur, lui aussi, ne se ferme plus que par `prefix+f`, retirez les touches de son action `close` dans son `keys.json` (`{ "explorer": { "close": [] } }`, voir [sa documentation](https://github.com/gcaudan/herdr-explorer/blob/main/docs/configuration.md)) ; Échap ferme toujours la preview.

Les icônes tiennent sur une ligne ; la ligne du dessous affiche le nom de l'icône sous la souris, et rien sinon.

Sous les icônes, le CPU et la mémoire des processus de Herdr (son serveur et tout ce qu'il lance, panes de tous les workspaces compris, pas seulement ceux du workspace affiché), puis de la machine, tels que la [sidebar](sidebar.md) les mesure, en gras, affichés de deux façons, selon `totals` :

- `line` (par défaut) : une ligne, `2% 3,1G · 󰍹 10% 22,4G`.
- `bars` : quatre lignes, `CPU :`, `RAM :`, `CPU 󰍹 :`, `RAM 󰍹 :`, chacune avec une barre aussi large que le pane le permet, puis sa valeur. Une barre fine (`━━━━╸────`) se remplit, par demi-case, de la part de la machine (mémoire de Herdr : de la mémoire de la machine), chaque case dans la couleur que prend sa place dans un dégradé du vert au rouge passant par les [seuils](sidebar.md#couleurs-par-seuil) de son chiffre ; sa partie vide est teintée, en atténué, de la couleur de la zone de chaque seuil, si bien que la barre montre à quelle distance est le seuil suivant. La part de CPU de Herdr manque au premier passage de la sidebar : sa barre reste vide, avec `–`.

Avec les couleurs par seuil de la sidebar (`colors = true` dans `sidebar.toml`), chaque chiffre est vert, jaune, orange ou rouge selon ses seuils. Sans la sidebar, ou quand ses chiffres ont plus de 30 secondes, les lignes restent vides. Le pane demande 2 lignes, pour ses icônes et le nom en dessous, plus celles de ses totaux (1 ou 4).

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
| `b` | btop | `btop` (sous Windows, `btop4win` quand seul lui est dans le `PATH`) : le moniteur système [btop](https://github.com/aristocratos/btop) (CPU, mémoire, disques, réseau, processus) |
| `u` | Update | action intégrée `update` : met à jour Herdr et les plugins installés depuis GitHub, en vert quand l'un d'eux a une nouvelle version, voir [plus bas](#entrées-intégrées) |
| `s` | Settings | action intégrée `settings` : l'écran des réglages, voir [plus bas](#entrées-intégrées) |

Le plugin n'installe aucun de ces outils. Un outil absent ouvre tout de même son popup, qui affiche `« btop » introuvable dans le PATH : installez-le, ou changez sa commande dans menu.toml` et attend Entrée. btop s'installe avec le gestionnaire de paquets du système (`apt install btop`, `brew install btop`…) ; sous Windows, c'est [btop4win](https://github.com/aristocratos/btop4win) (`winget install aristocratos.btop4win`, ou `scoop install btop`). Scoop l'installe sous le nom `btop` ; winget aussi quand il peut créer ce lien (mode développeur ou administrateur), et sinon met son dossier dans le `PATH`, où il s'appelle `btop4win`. L'entrée lance `btop`, ou `btop4win` quand seul lui est dans le `PATH` ; Herdr voit un `PATH` modifié par une installation une fois redémarré.

Pour choisir les vôtres, écrivez `menu.toml` dans le dossier de configuration du plugin (`herdr plugin config-dir imsike.menu` l'affiche ; l'écran des réglages le crée, avec tous les réglages en commentaire, la première fois qu'il en enregistre un ou qu'il ouvre les fichiers). Ses entrées remplacent toutes les entrées intégrées ; un fichier sans entrée les garde :

```toml
[[entries]]
key = "l"                 # un caractère
label = "lazygit"
command = ["lazygit"]     # programme et arguments, lancés sans shell
icon = "󰊢"                # facultatif : un glyphe Nerd Font ; la touche sinon

[[entries]]
key = "b"
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

Une commande est cherchée dans les dossiers du `PATH` écrits en entier : une entrée vide (`PATH=:/usr/bin`) ou `.` lancerait un programme de ce nom trouvé dans le dossier du workspace, celui d'un dépôt cloné par exemple. Sous Windows, elle est cherchée avant les dossiers du système (`bash` est le premier du `PATH`, Git Bash par exemple, pas celui de WSL), et un fichier batch (`code` est `code.cmd`) se lance sans préciser son extension ; un nom donné avec son extension (`code.cmd`, `bash.exe`) est cherché de la même façon.

Un fichier illisible ou invalide (champ inconnu, commande vide, commande et action à la fois, touche utilisée deux fois) est ignoré : le menu affiche les entrées intégrées, et affiche l'erreur : le menu en popup au-dessus de sa ligne d'aide, le pane `Tools` en rouge sous les icônes pendant 5 secondes. Le menu en popup lit le fichier à chaque ouverture ; le pane `Tools` le relit dans les 2 secondes qui suivent un changement, et prend la hauteur de ses nouveaux totaux. Un outil démarre tel que le menu l'affichait, même si le fichier a changé entre-temps.

## Réglages

En tête de `menu.toml`, avant les entrées ; l'écran des réglages les change aussi :

| Réglage | Par défaut | Effet |
|---|---|---|
| `decimal_comma` | `true` | `1,5G` (`true`) ou `1.5G` (`false`) |
| `percent_space` | `false` | `0,2%` (`false`) ou `0,2 %` avec une espace fine insécable (`true`) |
| `cpu_decimal` | `false` | les parts de CPU des totaux : `4%` (`false`) ou `4,2%` (`true`) |
| `memory_unit` | `"M"` | `853M`, `2,4G` (`"M"`) ; `853Mo`, `2,4Go` (`"Mo"`) ; `853MB`, `2,4GB` (`"MB"`) ; `K`, `M` ou `G` selon la taille |
| `memory_space` | `false` | `2,4Go` (`false`) ou `2,4 Go` avec une espace fine insécable avant l'unité (`true`) |
| `totals` | `"line"` | les totaux du pane `Tools` : `"line"` (une ligne) ou `"bars"` (quatre lignes avec barres), voir [Utilisation](#utilisation) |
| `check_updates` | `true` | l'entrée Update en vert quand Herdr ou un plugin installé depuis GitHub a une nouvelle version, voir [Nouvelles versions](#nouvelles-versions) |

Le réglage `layout` des versions précédentes, qui proposait deux lignes d'icônes, a disparu : un fichier qui le contient encore reste valide, le réglage est ignoré.

## Entrées intégrées

**Disk** (`d`) passe le pane `Tools` aux chiffres du dossier du workspace (celui de l'explorateur), sous un bouton **󰁍 Retour** (Échap, `q` ou Retour arrière reviennent aussi) :

- la taille totale de ses fichiers, tous fichiers compris (`.git` et dossiers de build aussi), et la part qu'elle représente de son disque. Un fichier lié plusieurs fois (liens durs, comme ceux de pnpm) compte à chaque fois, et un fichier cloud non téléchargé (OneDrive) compte pour sa taille entière ;
- son nombre de fichiers, hors `.git`, l'âge des chiffres à partir d'une heure (`· 3 h`), et une icône : un clic sur les chiffres, ou `r`, mesure à nouveau le dossier (`…` pendant la lecture).

Le dossier n'est mesuré que pendant que cette page est affichée : revenir arrête la mesure. Un dossier illisible (supprimé, refusé) ou qui ne répond pas à temps (un partage réseau disparu) est signalé en rouge, et aucun chiffre n'est gardé pour lui ; de même pour un explorateur dont Herdr ne donne pas le dossier. Dans les deux cas, `r` réessaie.

**Update** (`u`) ouvre une fenêtre de terminal à part, hors de Herdr : Herdr refuse de se mettre à jour depuis l'un de ses panes. Sous Windows, c'est une nouvelle fenêtre de console (ouverte par `cmd /c start`) ; ailleurs, le premier terminal trouvé parmi `x-terminal-emulator`, `gnome-terminal`, `konsole`, `xfce4-terminal`, `kitty`, `alacritty`, `wezterm`, `foot` et `xterm`. Dans cette fenêtre :

1. Elle cherche ce qui a une nouvelle version (voir [Nouvelles versions](#nouvelles-versions)) : Herdr, et chaque plugin installé depuis GitHub. Herdr y figure aussi quand sa nouvelle version n'a pas pu être connue (canal preview, pas de réponse), puisque `herdr update` la trouve seul. Quand rien n'a de nouvelle version, elle le dit et s'arrête.
2. Elle affiche la liste, tout coché : Espace ou un clic décoche (ou recoche) une ligne, haut et bas (ou `j`/`k`) se déplacent, Entrée met à jour ce qui reste coché, Échap annule.
3. Herdr d'abord, s'il est coché (la nouvelle version d'un plugin peut demander celle de Herdr) : `herdr update` installe sa dernière version, avec `--handoff` quand le serveur sait passer ses sessions à la nouvelle version, ce qu'il ne sait pas faire sous Windows.
4. Puis les plugins cochés : `herdr plugin update <id>…`, sans `--yes` : Herdr peut y demander confirmation, comme à l'installation. Leurs panes et leurs commandes prennent la nouvelle version à leur prochain lancement ; rien ne s'arrête.
5. Si Herdr a été mis à jour, `herdr status --json` indique si le serveur en cours est encore l'ancienne version. Sinon, la fenêtre dit que Herdr est à jour : le client déjà ouvert garde son ancienne version jusqu'à ce que vous vous détachiez (`prefix+q`) et relanciez `herdr`.
6. Si oui, elle dit combien de panes se ferment et combien d'agents s'arrêtent (`5 panes, dont 2 agents (1 au travail)`) et demande `Redémarrer maintenant ? [o/N]`. Toute autre réponse que `o`, `oui`, `y` ou `yes` annule : la nouvelle version tournera au prochain démarrage du serveur.
7. Une fois confirmé, elle arrête le serveur (`herdr server stop`) : **tous les panes se ferment et tous les agents s'arrêtent**, comme quand on quitte Herdr. Puis elle propose de relancer Herdr dans cette fenêtre (`[o/N]`) ; sinon elle met `herdr` dans le presse-papiers (avec `clip`, `wl-copy`, `xclip` ou `xsel`, le premier qui fonctionne), à coller dans un terminal.

La fenêtre attend Entrée avant de se fermer, erreur comprise. Tant qu'elle est ouverte, un nouveau clic sur Update dit qu'une mise à jour est en cours au lieu d'en ouvrir une deuxième. Sans affichage graphique (en SSH), ou quand le terminal se ferme aussitôt, le pane `Tools` le dit en rouge : lancez `herdr update` (et `herdr plugin update`) hors de Herdr. Un plugin lié depuis un dossier (`herdr plugin link`) n'est jamais proposé : Herdr ne le met pas à jour.

**Settings** (`s`) ouvre un écran de réglages dans un popup de 70 % de la largeur du terminal et 80 % de sa hauteur :

```
 󰒓 Réglages                                                              1/17

 Format des nombres ────────────────────────────────────── 4% · 853M · 2,4G ┃
 ▌ Virgule décimale                 2.4G   2,4G                             ┃
   Espace avant %                   4%   4 %                                ┃
   CPU avec une décimale            4%   4,2%                               ┃
   Unités de la mémoire             2,4G   2,4Go   2,4GB                    ┃
   Espace avant l'unité             2,4G   2,4 G                            ┃
                                                                            ┃
 Pane Tools ─────────────────────────────────────────────────────────────── ┃
   Totaux CPU / mémoire             une ligne   barres                      ┃
   Signaler les mises à jour        non   oui                               ┃
                                                                            ┃
 Sidebar ────────────────────────────────────────────────────────────────── ┃
   Durée de vie des valeurs        ‹ 20 s ›                                 │
   Shells                           bash, sh, zsh, fish, dash, nu, pwsh, …  │
   Totaux dans la barre d'onglets   non   oui                               │
                                                                            │
 Couleurs par seuil ─────────────────────────────────────────────────────── │
   Couleurs                         non   oui                               │
 ──────────────────────────────────────────────────────────────────────────
   Une virgule ou un point avant les décimales.
   ✓ enregistré dans menu.toml
 ↑↓ naviguer   ←→ choisir   Espace suivant   Tab section   Échap fermer
```

- Les réglages sont groupés en sections, chaque valeur dans une colonne à elle, au centre d'un popup large. Un réglage à quelques valeurs les montre toutes, l'actuelle en évidence (en jaune sur la ligne choisie) ; les formats des nombres sont montrés comme ils écriraient un chiffre, avec le format actuel, et l'en-tête de leur section montre une part de CPU et deux tailles dans ce format. Un nombre (`‹ 20 s ›`) montre les flèches qui le changent ; un jeu de seuils montre les quatre couleurs autour de ses valeurs.
- Sous la liste : ce que fait la ligne choisie, avec le réglage et les fichiers qu'elle écrit quand le popup est assez large ; la dernière chose faite (`✓` en vert, ou `✗` en rouge avec la raison) ; et les touches de la ligne choisie.
- Touches : haut et bas (ou `j`/`k`) choisissent une ligne, Début/Fin (ou `g`/`G`) la première ou la dernière, Tab la première ligne de la section suivante, Maj+Tab la première ligne de sa section, ou de celle au-dessus. Gauche et droite (ou `h`/`l`) déplacent un choix vers sa première ou sa dernière valeur (en s'y arrêtant) ou un nombre d'un pas, et ne font rien sur les autres lignes ; Entrée ou Espace passent un choix à sa valeur suivante, en faisant le tour, ajoutent un pas à un nombre, commencent la saisie ou lancent l'action de la ligne ; chaque changement est enregistré aussitôt. Échap, `q` ou Ctrl+C ferme l'écran.
- Souris : survoler une ligne la sélectionne, un clic sur l'une de ses valeurs choisit cette valeur, un clic sur `‹` ou `›` change un nombre d'un pas, un clic ailleurs sur la ligne fait comme Entrée, et la molette déplace la sélection ; la barre de défilement montre seulement où en est la liste.
- Dans un petit popup, la liste défile (une barre de défilement à sa droite dit où elle en est), et la description, puis la ligne d'état, lui laissent la place ; un nom ou une valeur trop longs sont coupés avec `…`, et un texte saisi montre sa fin.

Un fichier qui est un lien (déployé par stow ou chezmoi, par exemple) reste un lien : c'est le fichier vers lequel il pointe qui est écrit. Quand le fichier reste ignoré après un changement (un autre de ses réglages est invalide), l'écran le dit en rouge, avec la raison, et affiche la nouvelle valeur, que le fichier contient : le changement suivant la remet.

| Ligne | Effet | Fichier |
|---|---|---|
| Virgule décimale, Espace avant %, CPU avec une décimale, Unités de la mémoire, Espace avant l'unité | `decimal_comma`, `percent_space`, `cpu_decimal`, `memory_unit`, `memory_space` | `menu.toml` et `sidebar.toml` |
| Totaux CPU / mémoire | `totals` | `menu.toml` |
| Signaler les mises à jour | `check_updates` | `menu.toml` |
| Durée de vie des valeurs | `ttl_seconds`, par pas de 5 secondes, de 5 à 600 | `sidebar.toml` |
| Shells | `shells` : Entrée pour les taper, séparés par des virgules, Entrée pour enregistrer, Échap pour annuler | `sidebar.toml` |
| Totaux dans la barre d'onglets | `tab_bar` | `sidebar.toml` |
| Couleurs | `colors` : les [couleurs par seuil](sidebar.md#couleurs-par-seuil) | `sidebar.toml` |
| Seuils CPU, Seuils mémoire (Herdr ; machine) | les quatre jeux de seuils : Entrée pour taper trois valeurs croissantes séparées par des espaces (`10 25 50`), Entrée pour enregistrer, Échap pour annuler | `sidebar.toml` |
| Ouvrir les fichiers de configuration… | ouvre les fichiers dans un éditeur, voir plus bas | |
| Recharger la configuration de Herdr | recharge le `config.toml` de Herdr et dit si Herdr l'a appliqué, en partie (une section qu'il refuse ou qui demande un redémarrage garde ses réglages actuels : la première raison est affichée), ou refusé | |

Les lignes Sidebar et Couleurs n'apparaissent que si le plugin [sidebar](sidebar.md) est installé. Quand la sidebar ignore son `sidebar.toml` (réglage inconnu, seuils pas en ordre…), l'écran dit pourquoi en rouge et affiche les valeurs par défaut, celles que la sidebar utilise. Un fichier est écrit en gardant ses commentaires et ses autres réglages : un réglage en commentaire y prend sa valeur à sa place. Le fichier d'un plugin qui n'existe pas encore est d'abord créé depuis son modèle, tous les réglages en commentaire à leur valeur par défaut.

**Ouvrir les fichiers de configuration…** ouvre ensemble le `config.toml` de Herdr (le chemin que donne l'aide de Herdr, qui suit `HERDR_CONFIG_PATH`), `menu.toml`, et `sidebar.toml` quand la sidebar est installée. L'éditeur est `VISUAL`, sinon `EDITOR` (arguments permis) ; sans eux, sous Windows, VS Code quand il est dans le `PATH`, sinon le Bloc-notes, une fenêtre par fichier ; ailleurs `nano`, sinon `vi`. L'écran des réglages se ferme et Herdr reste libre d'utilisation :

- un éditeur qui a sa propre fenêtre (VS Code, Bloc-notes, Sublime Text, gedit…) démarre seul, hors de Herdr ;
- un éditeur en terminal (`nano`, `vim`, `hx`…) s'ouvre dans un nouvel onglet de Herdr ; une fois fermé, l'onglet demande `Recharger la configuration de Herdr ? [O/n]`, puis se ferme sur Entrée.

Ouvert depuis un pane `Tools`, l'écran fait aussi passer ce pane à deux boutons : **󰑓 Recharger** (ou `r`, Entrée) recharge la configuration de Herdr une fois les fichiers enregistrés, **󰁍 Retour** ramène les icônes. La sidebar lit son fichier à chaque passage, et le pane `Tools` relit `menu.toml` quand il change : seul le `config.toml` de Herdr demande le rechargement.

Le plugin ne fait qu'ouvrir le `config.toml` de Herdr : il ne l'écrit jamais ([ADR 0004](../adr/0004-plugin-settings.md), [ADR 0005](../adr/0005-herdr-update-entry.md)).

## Nouvelles versions

Quand Herdr ou un plugin installé depuis GitHub a une nouvelle version, l'icône Update du pane `Tools` passe en vert, et son nom au survol dit laquelle (`Update · Herdr 0.9.6 · Explorer`) ; dans le menu en popup, la ligne Update est verte, suivie de la même liste. La vérification a lieu au démarrage de Herdr, puis toutes les 4 heures tant qu'un pane `Tools` est ouvert ([ADR 0009](../adr/0009-new-version-check.md)) :

- **Herdr** : sa version installée (`herdr status --json`) est comparée à celle de `https://herdr.dev/latest.json`, le fichier qu'interroge la vérification de Herdr elle-même (avec `curl`). Sur le canal preview, rien n'est comparé : Herdr y annonce lui-même ses nouvelles versions.
- **Plugins** : pour chaque plugin installé depuis GitHub, le commit installé (que donne Herdr) est comparé à celui que la référence d'installation désigne maintenant sur GitHub (`git ls-remote` : la branche par défaut sans `--ref`, sinon la branche, ou le tag, de ce nom exact). Un plugin installé sur un commit précis ne change jamais.

Le résultat est gardé dans le dossier d'état du plugin (`updates.json`) et partagé par tous les panes `Tools` ; une seule vérification tourne à la fois (`updates.lock`). Pour ne plus rien vérifier ni afficher, `check_updates = false` (ligne **Signaler les mises à jour** des réglages) : l'entrée Update cherche toujours, elle, ce qui a une nouvelle version quand vous cliquez dessus.

## Fonctionnement, implications

**Le pane `Tools`**

- C'est un pane Herdr ordinaire. Herdr ne coupe que vers la droite ou vers le bas : le pane s'ouvre sous l'explorateur, puis échange sa place avec lui, et le focus revient au pane qui l'avait.
- Il s'ouvre sur l'événement `pane.created` de l'explorateur, une fois que celui-ci ne bouge plus (il se place et se dimensionne juste après son ouverture : environ une seconde). Il se ferme sur l'événement `pane.closed`, dès que son onglet n'a plus d'explorateur, ou plus de pane terminal en dehors de lui et des panes propres à l'explorateur (Preview, Search…), pour que l'explorateur puisse fermer son onglet comme il le fait seul.
- Un pane `Tools` se reconnaît à son libellé et à un fichier vide au nom de son id de pane dans le dossier `bars` du dossier de la session (voir plus bas), écrit à son ouverture et retiré à sa fermeture : un pane que vous renommez `Tools` n'est jamais fermé ni pris pour la barre. Les panes de l'explorateur se reconnaissent toujours à leur seul libellé (Explorer, Preview…) : un terminal renommé ainsi ne compte plus comme terminal. `prefix+space` et l'ouverture d'un explorateur passent chacun leur tour sur la barre d'un onglet, par un verrou de l'onglet (`opening-<id de l'onglet>.lock`, retiré au démarrage de Herdr une fois son onglet disparu) : un onglet n'a jamais deux barres.
- Herdr donne à un plugin un seul dossier d'état pour toutes ses sessions (`herdr --session <nom>`), dont les ids de panes et d'onglets recommencent à `w1`. Ce qui nomme un pane ou un onglet (le dossier `bars`, les verrous des onglets, les drapeaux des fichiers ouverts depuis un pane `Tools`, `files-open-*`) est donc gardé dans un dossier de la session, `sessions/<session>/` du dossier d'état (`default` pour la session que `herdr` lance sans `--session`). Les chiffres d'un dossier, le verrou de la mise à jour et la dernière recherche des nouvelles versions sont partagés par toutes les sessions.
- Il demande 3 lignes (6 avec `totals = "bars"`), mais Herdr ne donne jamais moins d'un dixième d'un split à chacun de ses côtés : sur un écran de moins de dix fois cela (30 lignes, 60 avec les barres), il est plus haut. Il reprend sa hauteur quand la fenêtre change de taille.
- **Tant qu'il est ouvert, l'explorateur ne garde plus sa largeur** : `<` et `>` ne font plus rien, et sa largeur suit celle de la fenêtre en proportion. L'explorateur ne cherche sa largeur que sur toute la hauteur de son propre pane ; une correction lui est proposée.
- Les lignes des totaux lisent, toutes les 2 secondes, un fichier que la sidebar écrit dans son dossier de la session à chacun de ses passages (toutes les 5 secondes) : le pane ne mesure rien lui-même.
- Les chiffres du disque sont mesurés en arrière-plan, seulement pendant que la page Disk est affichée (revenir arrête une lecture en cours, sauf si le dossier est sur un partage réseau qui ne répond plus : sa lecture l'attend, et les lectures suivantes du dossier attendent celle-là) : tout le dossier est lu, ce qui prend un moment sur un gros dossier (`node_modules`, dossiers de build). La lecture s'arrête après 200 000 fichiers et dossiers ou 2 secondes, au premier atteint : les chiffres commencent alors par `≥` (au moins). Une lecture demandée d'un clic va jusqu'à 5 millions de fichiers et dossiers ou 60 secondes.
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

- Herdr n'affiche qu'un popup à la fois. L'action reste donc en cours tant que le menu est ouvert : elle attend que le menu écrive le choix dans un fichier du dossier d'état du plugin, puis ouvre le popup de l'outil une fois celui du menu disparu. Pendant l'attente, elle occupe environ 4 Mo de mémoire, vérifie le fichier 20 fois par seconde, et vérifie une fois par seconde que le menu tourne encore, par un verrou que le menu tient et que le système libère quand le menu se termine, même tué.
- Si le popup du menu est fermé par autre chose que le menu lui-même, l'action voit en moins d'une seconde que le menu n'est plus là, et s'arrête.

**Réglages et mise à jour**

- L'écran des réglages est un popup : Herdr attend tant qu'il est ouvert, mais il ne reste ouvert que le temps de changer des réglages. Les éditeurs tournent en dehors, dans leur fenêtre ou dans un onglet.
- La fenêtre de mise à jour tourne hors de Herdr, sans la variable par laquelle Herdr signale ses panes (`HERDR_ENV`) ; elle continue si le pane `Tools` se ferme.
- La recherche des nouvelles versions interroge `herdr.dev` (une requête) et GitHub (une par plugin installé depuis GitHub), sans rien envoyer d'autre que ces requêtes, au démarrage de Herdr (par la commande de démarrage du plugin) puis toutes les 4 heures (par un pane `Tools`, sur un fil à part : il n'attend jamais). Chaque requête s'arrête au bout de 20 secondes ; hors ligne, rien ne passe au vert, et la vérification suivante a lieu 4 heures plus tard (10 minutes quand son résultat n'a pas pu être gardé).
- Un programme lancé à part (`popup = false`, un éditeur qui a sa propre fenêtre) et Herdr relancé depuis la fenêtre de mise à jour ne reçoivent aucune des variables qui ne valent que dans le pane `Tools` : `HERDR_ENV`, `HERDR_PANE_ID`, `HERDR_WORKSPACE_ID`, `HERDR_TAB_ID`, `HERDR_BIN_PATH`, `HERDR_PLUGIN_*` et `HERDR_MENU_*`.

Les commandes tournent sous votre compte, avec votre environnement, comme tout programme lancé dans un pane.

## Désactiver

Pour retirer une entrée intégrée, listez dans `menu.toml` les entrées à garder. Pour désactiver tout le menu, retirez la touche de `config.toml`, puis `herdr plugin disable imsike.menu` (ou `herdr plugin uninstall imsike.menu`) : les panes `Tools` se ferment avec leur explorateur ; ceux que Herdr restaure à son démarrage suivant, devenus de simples shells, se ferment à la main. `menu.toml` reste dans le dossier de configuration, et les fichiers des chiffres (`stats-<hash>.json`, avec leur verrou `stats-<hash>.lock`), le verrou de la mise à jour (`update.lock`), la dernière recherche des nouvelles versions (`updates.json`, avec son verrou `updates.lock`), et les dossiers des sessions (`sessions` : les verrous des onglets et le dossier `bars` de chaque session) dans le dossier d'état, jusqu'à ce que vous les supprimiez.
