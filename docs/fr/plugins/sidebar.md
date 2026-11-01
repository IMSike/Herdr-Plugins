# sidebar

[English](../../en/plugins/sidebar.md) · **Français**

Transforme le panneau Agents de Herdr en arbre : les agents de chaque workspace listés sous son nom, avec CPU et mémoire par workspace et par agent, et le nombre de shells lancés par chaque agent. La barre d'onglets affiche le total de tous les panes et celui de la machine entière. Le plugin ne fait que fournir les valeurs ; leur affichage se règle dans les lignes de sidebar du `config.toml` de Herdr.

```
 agents                 arbre
api · 6% 1.2G
├ ◐ Général · 4% 853M  1
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
# Lance le plugin toutes les 5 secondes et affiche ses totaux à droite de la barre d'onglets, puis l'heure (entrée propre à Herdr).
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
  ] }, { token = "$stats", fg = "#665c54" }],
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

Sous Linux, la commande pointe vers la copie du dossier d'état du plugin (`herdr-sidebar tick`, sans `.exe`) : `install` affiche son chemin, que montre `herdr plugin log list --plugin imsike.sidebar`.

## Ce qui est affiché

Tokens des agents (panneau Agents) :

| Token | Valeur |
|---|---|
| `$ws` | sur le premier agent de chaque workspace seulement : nom du workspace (`api`) |
| `$ws_stats` | à côté de `$ws`, seulement quand le workspace a plus d'un agent (sinon les chiffres de l'agent suffisent) : CPU et mémoire du workspace (`6% 1.2G`) |
| `$line` | branche de l'arbre (`├`, `└` pour le dernier agent du workspace), état et nom de l'onglet de l'agent (`├ ◐ Général`) |
| `$stats` | CPU, mémoire et shells de l'agent (`4% 853M  1`) |
| `$cpu`, `$ram`, `$shells` | les mêmes chiffres séparément (`4%`, `853M`, `1 shell`), pour composer d'autres dispositions |
| `$order` | la clé de tri de l'arbre ; pas destinée à l'affichage |

Symboles d'état : `◐` au travail, `◆` bloqué, `●` terminé (pas encore vu), `○` inactif, `·` inconnu. Les règles de couleur ci-dessus se basent sur ces symboles.

Tokens des workspaces (panneau Spaces) : `$cpu` et `$ram`.

Barre d'onglets : `<cpu> <mémoire> · 󰍹 <cpu> <mémoire>` (`6% 2.4G · 󰍹 35% 21.3G`) : le total de tous les panes, puis celui de la machine entière (après l'icône écran Nerd Font), mis à jour à chaque passage. L'heure qui suit (`· 14:05`) est l'entrée `datetime` propre à Herdr : elle reste affichée si le plugin échoue. Les chiffres de la machine comptent tous les processus, y compris ceux que le plugin ne peut pas lire (processus protégés, machines virtuelles) : sa part de CPU vient des temps CPU du système, sa mémoire est la mémoire utilisée (ce que le Gestionnaire des tâches affiche comme « En cours d'utilisation » sous Windows).

- **CPU** : utilisé depuis le passage précédent, en part de la machine entière, arrondi au pourcent (`0%`, `12%` : moins de 0,5 % s'affiche `0%`) ; absent au premier passage, et quand le passage précédent date de moins d'une seconde ou de plus de deux minutes.
- **Mémoire** : working set sous Windows, mémoire résidente ailleurs (`850M`, `1.2G`).
- Les chiffres d'un agent couvrent tout l'arbre de processus de son pane : le shell, l'agent et tout ce qu'ils ont lancé. Un workspace additionne tous ses panes, agents ou non.
- **Shells** (``, l'icône terminal Nerd Font ; sans Nerd Font elle s'affiche comme un blanc ou un carré) : processus `bash`, `sh`, `zsh`, `fish`, `dash`, `nu`, `pwsh`, `powershell` ou `cmd` sous l'agent. Un shell lancé par un autre shell n'est pas recompté : Git Bash sous Windows enchaîne plusieurs `bash.exe` pour une seule commande, qui comptent pour un. Les commandes courtes n'apparaissent qu'un instant ; celles qui durent (un serveur, un watcher) restent visibles.

Pour vérifier les valeurs sans la sidebar, `herdr-sidebar show` (depuis le dossier d'état) les affiche, mesurées sur une seconde, sans les transmettre.

## Fonctionnement, implications

- Herdr n'a pas de processus de plugin permanent : l'entrée de la barre d'onglets lance `herdr-sidebar tick` toutes les 5 secondes, et Herdr n'en démarre jamais un nouveau tant que le précédent tourne. Chaque passage lit tous les processus de la machine, puis s'arrête au bout d'une fraction de seconde. Il ne demande à Herdr le processus shell d'un pane que la première fois, ou quand le pane affiche un autre terminal : les passages suivants le retrouvent dans `sample.json`.
- Quand un agent apparaît ou change d'état, Herdr lance aussitôt le plugin tout comme le focus d'un pane, qui marque un agent terminé comme vu (événements `pane.agent_detected`, `pane.agent_status_changed`, `pane.focused`, `workspace.created`) : les états de l'arbre suivent sans attendre le passage suivant. Ces passages ne font que placer les agents dans l'arbre, sans lire les processus, et n'envoient que les agents dont la place ou la ligne a changé : les chiffres sont ceux du dernier passage, et un nouvel agent reçoit les siens au suivant. Un workspace renommé apparaît au passage suivant.
- Le panneau Agents est trié par `$order` grâce à une vue d'agents de Herdr (nommée `arbre` dans l'en-tête du panneau), qui remplace le tri propre au panneau tant que le plugin est activé. L'arbre suit l'ordre des workspaces dans la sidebar et des onglets dans la barre d'onglets : un onglet déplacé prend sa nouvelle place au passage suivant.
- La part de CPU se calcule par rapport au passage précédent, enregistré dans `sample.json` du dossier d'état.
- Un agent dont les processus ne peuvent pas être lus reste dans l'arbre avec les chiffres du dernier passage, jusqu'à leur expiration.
- Les valeurs expirent après 20 secondes. Si les passages s'arrêtent, les lignes de l'arbre disparaissent au lieu de rester figées, et les agents sont listés sans lignes.
- Herdr décale la deuxième ligne d'un bloc de deux cases : le premier agent d'un workspace (sous son en-tête) serait plus à droite que les autres. Le plugin complète la ligne des autres avec deux caractères braille vides (U+2800), car Herdr supprime les espaces ordinaires en début de valeur. Une ligne plus longue que la sidebar est coupée.
- Le symbole `◐` remplace l'icône d'état de Herdr.
- Le nom affiché est celui de l'onglet de l'agent : un onglet jamais renommé affiche son numéro, deux agents d'un même onglet portent le même nom. Un onglet renommé apparaît au passage suivant.
- Herdr place lui-même ` · ` entre deux tokens d'une même ligne (`Général · 4% 853M`) ; il n'est pas réglable.
- Les valeurs portent leur unité, donc les règles de couleur numériques (`gt`, `lt`) ne s'y appliquent pas.

## Désactiver

Retirez l'entrée `command` de `tab_bar_right` et les tokens `$ws`, `$ws_stats`, `$line`, `$stats`, `$cpu`, `$ram`, `$shells` du `config.toml` et les lignes `[ui.sidebar.*]` (les lignes par défaut reviennent), puis `herdr plugin disable imsike.sidebar` (ou `herdr plugin uninstall imsike.sidebar`), ce qui rend aussi au panneau Agents son propre tri. Le binaire copié et `sample.json` restent dans le dossier d'état jusqu'à ce que vous les supprimiez.
