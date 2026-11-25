# links

[English](../../en/plugins/links.md) · **Français**

Ouvre les chemins que le plugin Claude Code [dx](https://github.com/IMSike/CC-Plugins/blob/main/docs/fr/plugins/dx.md) rend cliquables dans les réponses de Claude, sur un **ctrl+clic** dans Herdr : dans l'aperçu de herdr-explorer, sélectionnés dans l'Explorateur de fichiers, ou avec le programme que Windows associe au fichier. Un fichier qui peut lancer du code n'est jamais ouvert avec son programme sans votre accord. Windows seulement. Pourquoi ainsi : [ADR 0012](../adr/0012-link-dispatch.md).

## Installation

```sh
herdr plugin install IMSike/Herdr-Plugins/links
```

Vérifiez ensuite ce dont les clics ont besoin, plus bas. dx voit le plugin au début de sa prochaine session Claude Code.

## Ce que fait un clic

dx dessine un chemin qui existe en lien `dx-open:///C:/dossier/fichier.rs#L12-L14`. Sur un ctrl+clic, le plugin lit le chemin dans le lien, et ses lignes (`#L12-L14`), rien d'autre : ce qu'il fait se règle dans `links.toml` (plus bas), le même pour chaque lien.

| Cible | Ce qui se passe |
|---|---|
| Un dossier | Sélectionné dans son parent, dans l'Explorateur de fichiers. Jamais ouvert en `explorer.exe <dossier>`, qui lancerait un fichier mis à sa place. |
| Un fichier, `file = "preview"` (par défaut) | Montré dans l'aperçu de herdr-explorer, dans l'onglet du pane cliqué, défilé jusqu'à ses lignes. Sans herdr-explorer, ou quand l'aperçu échoue, comme `"application"`. Quand herdr-explorer ne répond pas en 3 s, rien d'autre ne s'ouvre, car l'aperçu peut encore s'afficher : une notification « Aperçu lent » le dit. |
| Un fichier, `file = "select"` | Sélectionné dans l'Explorateur de fichiers, sans être ouvert avec son programme ; l'Explorateur peut tout de même le lire (voir la fin de [Danger](#danger)). |
| Un fichier, `file = "application"` | Ouvert avec le programme que Windows lui associe, quand son type est dans `open_types`. Tout autre est sélectionné dans l'Explorateur de fichiers, avec une notification Herdr qui dit pourquoi, sauf si les réglages **Danger** le permettent. |

Avant tout, le plugin vérifie la vraie cible : liens et jonctions suivis, noms courts (`PROGRA~1`) rendus longs, un lecteur `subst` remplacé par son dossier. Un chemin sur un partage réseau ou hors d'un lecteur local, introuvable, de 260 unités UTF-16 ou plus (la limite `MAX_PATH` de Windows ; « chemin trop long »), ou dont l'Explorateur de fichiers lirait le nom autrement (une virgule, un point-virgule, un guillemet droit, deux-points, un signe pour cent, refusé pour qu'aucun `%NOM%` ne puisse jamais être lu comme une variable en chemin vers l'Explorateur de fichiers, un nom qui finit par un point ou une espace, un nom de périphérique Windows comme `CON`, `NUL`, `COM1` ou `LPT1`, avec ou sans extension, un CLSID comme extension, `x.{…}`…), ou qui contient un caractère qui ne s'affiche pas tel quel (invisible, à usage privé, braille vide, non-caractère Unicode : la liste est dans l'[ADR 0012](../adr/0012-link-dispatch.md#décision), étape 1), n'est pas ouvert : une notification Herdr dit pourquoi. Le type est lu sur le vrai nom : un lien `notes.txt` qui mène à `outil.exe` est un `.exe`.

Le plugin n'ouvre jamais rien de lui-même : seul votre clic le lance, une fois.

## Réglages

`links.toml`, dans le dossier de configuration du plugin (`herdr plugin config-dir imsike.links`), relu à chaque clic. L'écran des réglages du [menu](menu.md) les règle, dans ses sections **Liens** et **Danger**. Un fichier ou un réglage absent garde sa valeur par défaut. Un type faux dans `open_types` est laissé de côté et journalisé, les autres gardés. Un fichier inutilisable (une erreur de syntaxe, un réglage inconnu, une valeur fausse d'un autre réglage) est mis de côté : chaque clic le dit dans une notification Herdr et ne fait que sélectionner les fichiers, sans jamais ouvrir plus que ce que vous avez choisi. De même pour un fichier que le plugin ne peut pas trouver, quand Herdr ne lui donne pas de dossier de configuration (`HERDR_PLUGIN_CONFIG_DIR` absent) : vous en avez peut-être écrit un. Le modèle, chaque réglage commenté à sa valeur par défaut : [`links/templates/links.toml`](../../../links/templates/links.toml).

| Réglage | Par défaut | Ce qu'il fait |
|---|---|---|
| `file` | `"preview"` | Ce que fait un clic sur un fichier : `"preview"`, `"select"` ou `"application"` (voir plus haut). |
| `open_types` | voir plus bas | Les types (sans le point, casse indifférente) que l'application par défaut peut ouvrir. |
| `run_programs` | `false` | **Danger.** Un fichier d'un type hors de `open_types` s'ouvre aussi avec son programme : un programme, un script ou un installeur se lance. |
| `confirm_programs` | `true` | Avec `run_programs` : un popup montre le vrai chemin du fichier et son type, et ne l'ouvre que sur Entrée (Échap ou toute autre touche annule aussitôt, même pendant ses 1,1 premières secondes). L'écran des réglages du menu le réactive chaque fois qu'il active `run_programs`. |

### Types ouverts

Par défaut, des types qui ne lancent pas de code par eux-mêmes quand Windows les ouvre avec le programme qu'il leur associe, tel qu'installé (ce programme peut toujours avoir des failles, comme toute visionneuse), sauf les risques dits plus bas :

- texte et documents : `txt md markdown rst adoc log pdf docx xlsx pptx` ;
- données et configuration : `csv tsv json jsonc json5 jsonl yaml yml toml ini cfg conf env properties lock sql diff patch` ;
- code source que Windows n'exécute pas : `c h cc cpp cxx hpp cs go rs java kt swift ts tsx jsx vue svelte css scss sass less` ;
- images : `png jpg jpeg gif bmp webp ico tif tiff avif heic` ;
- son et vidéo : `mp3 wav flac ogg m4a mp4 mkv webm mov avi`.

Un `docx`, `xlsx` ou `pptx` ne contient pas de macro à lui, mais peut nommer un fichier ailleurs : un `.docx` peut nommer son modèle sur un partage réseau (`attachedTemplate`, `\\hôte\…`), que Word va chercher en l'ouvrant, sans demander, et Windows peut alors envoyer à cet hôte votre hachage NTLM ; les macros du modèle restent derrière l'avertissement d'Office. Retirez ces types de `open_types` si cela compte pour vous.

Un `pdf` peut contenir du JavaScript, que son lecteur (Acrobat, celui d'Edge) peut exécuter en ouvrant le fichier (une `OpenAction`), dans son bac à sable. Retirez `pdf` de `open_types` si cela compte pour vous.

Excel, quand il ouvre un `csv` ou un `tsv`, évalue les formules que le fichier contient (une cellule qui commence par `=`). Retirez ces types de `open_types` si cela compte pour vous.

Une application Adobe installée (Photoshop, Illustrator…) peut associer `jsx` à ses scripts ExtendScript, qu'elle exécute alors à l'ouverture du fichier, après un avertissement. Retirez `jsx` de `open_types` si cela compte pour vous.

Laissés de côté exprès : programmes et scripts (`exe`, `bat`, `cmd`, `ps1`, `js`, `vbs`, `py`, `sh`… ; Python et Git for Windows font exécuter `.py` et `.sh` à l'ouverture), raccourcis et installeurs (`lnk`, `url`, `msi`, `reg`…), documents à macros (`docm`, `xlsm`…, les anciens `doc`, `xls`, `ppt`, et les OpenDocument `odt`, `ods`, `odp`, avec leurs macros LibreOffice Basic), `rtf`, et ce qu'un navigateur ouvre avec ses scripts (`html`, `svg`, `xml`), et `epub`, dont EPUB 3 permet aux documents de contenu de porter des scripts. Un fichier sans type (`Makefile`) n'est jamais dans `open_types` : il est traité comme tout type hors de la liste ; un nom qui commence par un point en a un : `.env` a le type `env`.

Toute extension peut être ajoutée (`$$$`, `!ut`, `(x)`…), avec ou sans son point, casse indifférente. Seule la dernière extension compte : écrire `gz`, pas `tar.gz` (laissé de côté, en le disant). Un type qui contient une espace, ou un caractère qu'aucun nom de lien ne peut porter (comme plus haut), est laissé de côté aussi ; de même qu'un type qui finit par un point (`md.` : écrire `md`). N'ajoutez un type que si vous savez que son programme n'exécute rien ; un type qui exécute du code ajouté à la liste s'ouvre sans les réglages Danger.

### Danger

Un lien peut venir de n'importe où : Claude peut écrire un lien vers n'importe quel fichier sous n'importe quel texte, et tout programme d'un pane peut en afficher un. Avec `run_programs` désactivé, aucun lien ne peut faire ouvrir au plugin un fichier hors de `open_types` avec son programme. Ne l'activez qu'en cas de besoin, et gardez `confirm_programs` : le popup montre le vrai chemin, pas le texte du lien, et le type du fichier (`Type .exe`), et vérifie de nouveau le chemin avant de l'ouvrir. Le chemin d'un raccourci n'est pas ce qu'il lance : pour tout raccourci (`.lnk`, `.url`, `.pif`, `.website`, `.application`, `.appref-ms`, `.rdp`, `.scf`, `.settingcontent-ms`, `.shb`, `.xnk`, `.library-ms`, `.searchConnector-ms`, `.search-ms`, `.wsh`, `.jnlp`, `.ica`), le popup ajoute une ligne rouge « Raccourci : ouvre une autre cible, non affichée ». Il ne lit pas ce qu'ouvre le raccourci, pas même l'adresse d'un raccourci Internet : Windows lit un `.url` d'une façon non documentée, et l'adresse montrée pourrait différer de celle ouverte. Il ne se confirme pas par accident : seule une Entrée simple, tapée un instant après l'ouverture du popup et tout le chemin affiché, confirme ; toute autre touche annule (l'[ADR 0012](../adr/0012-link-dispatch.md#décision) dit comment).

Quels que soient les réglages, un lien peut faire afficher à l'Explorateur de fichiers un dossier choisi par le modèle, et l'Explorateur lit ce qu'il affiche, comme le ferait toute fenêtre de l'Explorateur sur ce dossier, mais ici c'est le plugin qui ouvre la fenêtre :

- un `desktop.ini`, un `.lnk`, un `.url`, un `.scf` ou un `.library-ms` qui s'y trouve et dont l'icône pointe vers un partage réseau (`\\hôte\…`, comme le `IconFile=\\hôte\…` d'un `.url`) peut faire se connecter Windows à cet hôte et lui envoyer votre hachage NTLM ;
- un fichier sélectionné n'est pas ouvert avec son programme, mais avec le volet de visualisation de l'Explorateur activé, le sélectionner lance le gestionnaire d'aperçu de son type (celui d'Office, d'un lecteur PDF…), qui lit le fichier, et les miniatures du dossier sont faites par les gestionnaires de miniatures des types de ses fichiers, qui les lisent aussi : une faille de l'un d'eux rencontre un fichier choisi par le modèle.

Avant de refuser un chemin sur un partage réseau, le plugin suit les liens symboliques du lien pour trouver la vraie cible : un lien symbolique vers `\\hôte\…` fait se connecter Windows à cet hôte, et peut lui envoyer votre hachage NTLM, avant que le clic soit refusé.

## Prérequis

| Où | Quoi | Pourquoi |
|---|---|---|
| `settings.json` de Claude Code → `env` | `"FORCE_HYPERLINK": "1"` | Claude Code ne connaît pas Herdr et écrirait sinon `texte (url)` au lieu d'un lien. dx signale son absence. |
| Herdr | `imsike.links` installé et activé | Le gestionnaire de liens. dx signale son absence. |
| `config.toml` de Herdr | `ui.mouse_capture` activé (par défaut) | Les clics sur les liens n'atteignent les plugins qu'ainsi. |
| `settings.json` de Windows Terminal | `"experimental.detectURLs": false` dans `profiles.defaults` | Sinon Windows Terminal confie à Windows le texte `file://` cliqué avec ctrl avant que Herdr ne le voie. |
| Facultatif | [herdr-explorer](https://github.com/gcaudan/herdr-explorer) | L'aperçu ; sans lui, `"preview"` ouvre avec le programme, dans la limite de `open_types`. |

## Journal

Chaque clic qui ne fait pas ce qu'il aurait fait, et chaque échec, est journalisé ; un Explorateur de fichiers ou un popup de confirmation qui ne peut pas s'ouvrir est aussi signalé par une notification Herdr, ou dans le popup lui-même après une confirmation :

```sh
herdr plugin log list --plugin imsike.links
```

## Désactiver

```sh
herdr plugin disable imsike.links
```

dx ne dessine alors plus de liens, dès sa session suivante. Le réglage **Chemins cliquables** de dx coupe les liens de son côté ; `herdr plugin uninstall imsike.links` retire le plugin.
