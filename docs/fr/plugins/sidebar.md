# sidebar

[English](../../en/plugins/sidebar.md) · **Français**

Transforme le panneau Agents de Herdr en arbre : les agents de chaque workspace listés sous son nom, avec CPU et mémoire par workspace et par agent, et le nombre de shells lancés par chaque agent. Le total des processus de Herdr et celui de la machine entière s'affichent dans le pane `Tools` du [menu](menu.md), ou dans la barre d'onglets. Le plugin ne fait que fournir les valeurs ; leur affichage se règle dans les lignes de sidebar du `config.toml` de Herdr.

```
 agents                 arbre
api · 6% 1,2G
├ ◐ Général · 4% 853M ·  1
└ ○ docs · 0% 275M
web
└ ○ 1 · 1% 562M
```

Un clic sur la ligne d'un agent ouvre cet agent. Les workspaces s'ouvrent toujours depuis le panneau Spaces au-dessus, réduit à une ligne chacun (`api · main`) ; déplacez la séparation entre les deux panneaux pour donner plus de place à l'arbre.

## Installation

```sh
herdr plugin install IMSike/Herdr-Plugins/sidebar
```

Au démarrage de Herdr, le plugin copie son binaire dans son dossier d'état et trie le panneau Agents dans l'ordre de l'arbre. Juste après l'avoir installé, mis à jour ou lié, faites-le une fois à la main (ou redémarrez Herdr) :

```sh
herdr plugin action invoke imsike.sidebar.install
```

Puis ajoutez au `config.toml` de Herdr :

```toml
[ui]
sidebar_width = 34
sidebar_max_width = 40
# Lance le plugin toutes les 5 secondes : à garder même sans rien afficher. Ses totaux ne s'affichent
# à droite de la barre d'onglets qu'avec tab_bar = true (sidebar.toml) ; puis l'heure (entrée propre à Herdr).
# Windows (le dossier d'état est sous %LOCALAPPDATA%) :
tab_bar_right = [
  { type = "command", command = '"%LOCALAPPDATA%\herdr\plugins\imsike.sidebar\herdr-sidebar.exe" tick', interval_seconds = 5, timeout_seconds = 5 },
  { type = "datetime", format = "%H:%M" },
]
tab_bar_right_separator = " · "

[ui.sidebar.agents]
rows = [
  [{ token = "$ws", fg = "#ebdbb2", bold = true }, { token = "$ws_stats", fg = "#7c6f64" }],
  [{ token = "$line", fg = "#bdae93", rules = [
    { contains = "◆", fg = "#fb4934" },
    { contains = "◐", fg = "#fabd2f" },
    { contains = "●", fg = "#b8bb26" },
  ] }, { token = "$usage", fg = "#665c54" }, { token = "$shell_count", fg = "#fabd2f" }],
]

[ui.sidebar.spaces]
rows = [
  ["workspace", { token = "branch", fg = "#fabd2f" }],
]

# Fond de l'agent (et du workspace) sélectionné
[theme.custom]
active_row_bg = "#4a4426"
```

Les couleurs vont avec le thème gruvbox.

Sous Linux, faites pointer la commande vers la copie du dossier d'état du plugin (`herdr-sidebar tick`, sans `.exe`) : `install` affiche son chemin, que montre `herdr plugin log list --plugin imsike.sidebar`.

## Ce qui est affiché

Tokens des agents (panneau Agents) :

| Token | Valeur |
|---|---|
| `$ws` | sur le premier agent de chaque workspace seulement : nom du workspace (`api`) |
| `$ws_stats` | à côté de `$ws`, seulement quand le workspace a plus d'un agent (sinon les chiffres de l'agent suffisent) : CPU et mémoire du workspace (`6% 1,2G`) |
| `$line` | branche de l'arbre (`├`, `└` pour le dernier agent du workspace), état et nom de l'onglet de l'agent (`├ ◐ Général`) |
| `$usage` | CPU et mémoire de l'agent (`4% 853M`) |
| `$shell_count` | les shells de l'agent (` 1`), effacé quand il n'en a aucun. Un token à part prend une couleur à part : un shell sous un agent signifie que l'agent travaille, ce que l'exemple colore comme un agent au travail |
| `$stats` | CPU, mémoire et shells en un seul token, d'une seule couleur (`4% 853M  1`) |
| `$cpu`, `$ram`, `$shells` | les mêmes chiffres séparément (`4%`, `853M`, `1 shell`), pour composer d'autres dispositions |
| `$order` | la clé de tri de l'arbre ; pas destinée à l'affichage |

Symboles d'état : `◐` au travail, `◆` bloqué, `●` terminé (pas encore vu), `○` inactif, `·` inconnu. Les règles de couleur ci-dessus se basent sur ces symboles.

Totaux : `<cpu> <mémoire> · 󰍹 <cpu> <mémoire>` (`6% 2,4G · 󰍹 35% 21,3G`) : le total des processus de Herdr, puis celui de la machine entière (après l'icône écran Nerd Font), mis à jour à chaque passage. Les processus de Herdr sont son serveur et tout ce qu'il lance : tous les panes de tous les workspaces (agents, terminaux, panes de l'explorateur et du menu), leurs hôtes de console sous Windows (`OpenConsole.exe`), les popups et les passages des plugins. Le serveur est le processus qui a lancé les shells des panes ; tant qu'aucun shell n'est connu, le total est celui des seuls panes. Il ne suit pas le workspace affiché : il est le même dans chaque pane `Tools` et ne change pas quand on change de workspace. Il dépasse donc la somme des agents, et le `$ws_stats` d'un workspace, qui ne compte que ses panes. Chaque passage les écrit dans `totals.json`, dans le dossier de la session (`sessions/<session>/` du dossier d'état : chaque session de Herdr a le sien), que le pane `Tools` du menu affiche sous ses icônes. La barre d'onglets ne les affiche qu'avec `tab_bar = true` ; sinon son entrée reste vide et Herdr l'omet, séparateur compris. L'heure (`14:05`) est l'entrée `datetime` propre à Herdr : elle reste affichée si le plugin échoue. Les chiffres de la machine comptent tous les processus, y compris ceux que le plugin ne peut pas lire (processus protégés, machines virtuelles) : sa part de CPU vient des temps CPU du système, sa mémoire est la mémoire utilisée (ce que le Gestionnaire des tâches affiche comme « En cours d'utilisation » sous Windows).

- **CPU** : utilisé depuis le passage précédent, en part de la machine entière, arrondi au pourcent (`0%`, `12%` : moins de 0,5 % s'affiche `0%`) ; absent au premier passage, et quand le passage précédent date de moins d'une seconde ou de plus de deux minutes.
- **Mémoire** : working set sous Windows, mémoire résidente ailleurs (`850M`, `1,2G`).
- Les chiffres d'un agent couvrent tout l'arbre de processus de son pane : le shell, l'agent et tout ce qu'ils ont lancé. Un workspace additionne tous ses panes, agents ou non.
- **Shells** (``, l'icône terminal Nerd Font ; sans Nerd Font elle s'affiche comme un blanc ou un carré) : processus `bash`, `sh`, `zsh`, `fish`, `dash`, `nu`, `pwsh`, `powershell` ou `cmd` sous l'agent (réglage `shells`). Un shell lancé par un autre shell n'est pas recompté : Git Bash sous Windows enchaîne plusieurs `bash.exe` pour une seule commande, qui comptent pour un. Les commandes courtes n'apparaissent qu'un instant ; celles qui durent (un serveur, un watcher) restent visibles.

Pour vérifier les valeurs sans la sidebar, `herdr-sidebar show` (depuis le dossier d'état) les affiche, mesurées sur une seconde, sans les transmettre.

## Réglages

`sidebar.toml`, dans le dossier de configuration du plugin (`herdr plugin config-dir imsike.sidebar` l'affiche), règle l'écriture des valeurs. L'écran des réglages du menu (**Settings**) le modifie, ou l'ouvre, après l'avoir créé avec tous les réglages en commentaire. Un réglage absent garde sa valeur par défaut :

| Réglage | Par défaut | Effet |
|---|---|---|
| `decimal_comma` | `true` | `2,4G` (`true`) ou `2.4G` (`false`) |
| `percent_space` | `false` | `4%` (`false`) ou `4 %` avec une espace fine insécable (`true`) |
| `cpu_decimal` | `false` | `4%` (`false`) ou `4,2%` (`true`) |
| `memory_unit` | `"M"` | `853M`, `2,4G` (`"M"`) ; `853Mo`, `2,4Go` (`"Mo"`) ; `853MB`, `2,4GB` (`"MB"`) ; `K`, `M` ou `G` selon la taille |
| `memory_space` | `false` | `2,4Go` (`false`) ou `2,4 Go` avec une espace fine insécable avant l'unité (`true`) |
| `ttl_seconds` | `20` | durée de vie d'une valeur quand aucun passage ne la rafraîchit ; au moins 1 |
| `shells` | `["bash", "sh", "zsh", "fish", "dash", "nu", "pwsh", "powershell", "cmd"]` | noms des processus comptés comme shells (sans `.exe`, casse indifférente) |
| `tab_bar` | `false` | affiche aussi les totaux dans la barre d'onglets (`true`) |
| `colors` | `false` | couleurs par seuil des totaux du pane `Tools` du menu, voir [plus bas](#couleurs-par-seuil) |
| `cpu_levels`, `memory_levels`, `machine_cpu_levels`, `machine_memory_levels` | voir plus bas | leurs seuils |

```toml
percent_space = true
shells = ["bash", "pwsh", "nu"]
```

Chaque passage lit le fichier : un changement apparaît au passage suivant. Un fichier illisible ou invalide (réglage inconnu, `ttl_seconds = 0`, seuils pas en ordre croissant) est ignoré : les passages prennent les valeurs par défaut, et ceux lancés par un événement disent pourquoi dans le journal du plugin (`herdr plugin log list --plugin imsike.sidebar` ; les passages de la barre d'onglets sont des commandes propres à Herdr, que ce journal ne garde pas). Couleurs, ordre et disposition du panneau Agents restent dans le `config.toml` de Herdr.

Herdr lance la commande de la barre d'onglets sans l'environnement du plugin, donc sans son dossier de configuration : `install` écrit le chemin du dossier dans `config-dir`, dans le dossier d'état, où les passages de la barre d'onglets le lisent. Tant qu'`install` n'a pas tourné une fois (au démarrage de Herdr, ou à la main), ces passages prennent les valeurs par défaut.

## Couleurs par seuil

La sidebar écrit ses totaux, avec leurs seuils, dans `totals.json` ; le pane `Tools` du menu les affiche (voir [menu](menu.md#utilisation)). Avec `colors = true`, chacun de ses chiffres est vert, jaune, orange ou rouge selon trois seuils : au-dessus du premier (strictement) il est jaune, au-dessus du deuxième orange, au-dessus du troisième rouge. Ses barres (`totals = "bars"` dans `menu.toml`) passent par les mêmes couleurs aux mêmes seuils, en dégradé, avec ou sans `colors`.

| Seuils | Par défaut | Pour |
|---|---|---|
| `cpu_levels` | `[10, 25, 50]` (% de la machine) | le CPU des processus de Herdr (son serveur et tout ce qu'il lance) |
| `memory_levels` | `[1, 2, 4]` (Go) | leur mémoire |
| `machine_cpu_levels` | `[50, 70, 90]` (%) | le CPU de la machine |
| `machine_memory_levels` | `[60, 75, 90]` (% de sa mémoire) | la mémoire utilisée de la machine |

Le panneau Agents garde les couleurs du `config.toml` de Herdr : colorer les chiffres de chaque agent (par des marqueurs invisibles que reconnaissent les règles `contains` de Herdr) a été essayé, puis abandonné pour son rendu.

## Fonctionnement, implications

- Herdr n'a pas de processus de plugin permanent : l'entrée de la barre d'onglets lance `herdr-sidebar tick` toutes les 5 secondes, et Herdr n'en démarre jamais un nouveau tant que le précédent tourne. Chaque passage lit tous les processus de la machine, puis s'arrête au bout d'une fraction de seconde. Il ne demande à Herdr le processus shell d'un pane que la première fois, ou quand le pane affiche un autre terminal : les passages suivants le retrouvent dans `sample.json`.
- Quand un agent apparaît ou change d'état, Herdr lance aussitôt le plugin, tout comme un nouveau workspace, un onglet renommé ou déplacé et un pane fermé (événements `pane.agent_detected`, `pane.agent_status_changed`, `workspace.created`, `tab.renamed`, `tab.moved`, `pane.closed` ; voir un agent terminé le rend inactif, ce qui est un changement d'état) : les états de l'arbre suivent sans attendre le passage suivant. Ces passages ne font que placer les agents dans l'arbre, sans lire les processus, et n'envoient que les agents dont la place ou la ligne a changé : les chiffres sont ceux du dernier passage, et un nouvel agent reçoit les siens au suivant. Un workspace renommé apparaît au passage suivant.
- Le panneau Agents est trié par `$order` grâce à une vue d'agents de Herdr (nommée `arbre` dans l'en-tête du panneau), qui remplace le tri propre au panneau tant que le plugin est activé. L'arbre suit l'ordre des workspaces dans la sidebar et des onglets dans la barre d'onglets : un onglet déplacé prend aussitôt sa nouvelle place (voir les événements plus bas).
- La part de CPU se calcule par rapport au passage précédent, enregistré dans `sample.json` du dossier de la session, en part de tous les cœurs de la machine (un quota de CPU donné à Herdr ne la change pas). Le temps entre les deux passages vient des temps CPU du système (Windows) ou de sa durée de fonctionnement (Linux), qu'un changement d'heure ne déplace pas ; un processus est reconnu à son heure de démarrage, qui suit un tel changement sous Linux : le plugin en tient compte.
- Chaque rapport porte un numéro d'ordre, gardé dans `seq` du dossier d'état : quand l'horloge recule, les rapports continuent dans l'ordre au lieu d'être ignorés par Herdr, et deux passages lancés au même moment n'ont pas le même. Il est pris une fois la session lue : de deux passages simultanés, celui qui l'a lue en dernier l'emporte.
- Un passage qui attend encore Herdr au bout de 4 secondes cesse de l'interroger et échoue (l'entrée de la barre d'onglets reste vide jusqu'au passage suivant) : un Herdr lent ne reçoit pas davantage de requêtes.
- Un agent dont les processus ne peuvent pas être lus reste dans l'arbre avec les chiffres du dernier passage, jusqu'à leur expiration.
- Les valeurs expirent après 20 secondes (`ttl_seconds`). Si les passages s'arrêtent, les lignes de l'arbre disparaissent au lieu de rester figées, et les agents sont listés sans lignes.
- Herdr décale la deuxième ligne d'un bloc de deux cases : le premier agent d'un workspace (sous son en-tête) serait plus à droite que les autres. Le plugin complète la ligne des autres avec deux caractères braille vides (U+2800), car Herdr supprime les espaces ordinaires en début de valeur. Une ligne plus longue que la sidebar est coupée.
- Le symbole `◐` remplace l'icône d'état de Herdr.
- Le nom affiché est celui de l'onglet de l'agent : un onglet jamais renommé affiche son numéro, deux agents d'un même onglet portent le même nom. Un onglet renommé apparaît aussitôt.
- Herdr place lui-même ` · ` entre deux tokens d'une même ligne (`Général · 4% 853M ·  1`) ; il n'est pas réglable. `$stats` garde les chiffres en un seul token, sans lui.
- Les valeurs portent leur unité, donc les règles de couleur numériques (`gt`, `lt`) ne s'y appliquent pas.

## Désactiver

Retirez l'entrée `command` de `tab_bar_right` et les tokens `$ws`, `$ws_stats`, `$line`, `$usage`, `$shell_count`, `$stats`, `$cpu`, `$ram`, `$shells` du `config.toml` et les lignes `[ui.sidebar.*]` (les lignes par défaut reviennent), puis `herdr plugin disable imsike.sidebar` (ou `herdr plugin uninstall imsike.sidebar`), ce qui rend aussi au panneau Agents son propre tri. Le binaire copié (sous Windows, avec les copies précédentes encore utilisées quand il a été remplacé, `*.old`), les dossiers des sessions (`sessions`, avec `sample.json` et `totals.json`), `seq`, `config-dir` et le verrou de l'installation, `install.lock`, restent dans le dossier d'état, et `sidebar.toml` dans le dossier de configuration, jusqu'à ce que vous les supprimiez.
