# ADR 0004 — Réglages des plugins

- **Statut** : accepté
- **Date** : 2026-11-01

## Contexte

Le mainteneur voulait régler ce que les plugins écrivent (format des nombres, durée de vie d'une valeur, processus comptés comme shells) sans toucher à leur code, et atteindre tous les réglages depuis le menu.

Herdr donne à chaque plugin un dossier de configuration (`HERDR_PLUGIN_CONFIG_DIR`, `herdr plugin config-dir <id>`), mais aucun réglage propre à un plugin, ni d'écran pour les modifier. L'apparence des valeurs de la sidebar (couleurs, ordre, lignes) relève déjà de Herdr : son `config.toml`, `[ui.sidebar.*]`. Les passages de la sidebar lancés par la barre d'onglets n'ont pas l'environnement du plugin, donc pas de dossier de configuration.

Options étudiées :

| Option | Avantages | Inconvénients |
|---|---|---|
| Un fichier TOML par plugin dans son dossier de configuration, et une entrée du menu qui ouvre les fichiers dans un éditeur | Texte simple, commenté, versionnable (OS-Setup) ; Herdr et les plugins valident ; peu de code | Pas de formulaire ; une faute n'est signalée qu'à la lecture du fichier |
| Un écran de réglages (TUI) dans le menu | Guidé, chaque valeur vérifiée à la saisie | Beaucoup de code pour peu de réglages ; à tenir à jour avec les réglages de chaque plugin |
| Les réglages dans le `config.toml` de Herdr | Un seul fichier | Herdr refuse les clés inconnues ; un plugin ne doit jamais écrire ce fichier |

## Décision

Chaque plugin lit son propre fichier TOML dans son dossier de configuration : `menu.toml` (les réglages en tête, puis les entrées), `sidebar.toml`. Un fichier ou un réglage absent garde sa valeur par défaut ; un réglage inconnu fait ignorer le fichier, avec la raison affichée (menu) ou journalisée (sidebar). L'action `install` de la sidebar écrit le chemin de son dossier de configuration dans son dossier d'état pour les passages de la barre d'onglets.

L'entrée intégrée **Settings** du menu ouvre un écran de réglages (TUI) dans un popup, pour les quelques réglages des plugins (format des nombres, disposition des icônes, réglages de la sidebar). Chaque changement est écrit aussitôt dans le fichier du plugin avec `toml_edit`, qui garde ses commentaires et ses autres réglages ; un réglage en commentaire dans le fichier y prend sa valeur à sa place. Le fichier d'un plugin qui n'existe pas est d'abord créé depuis son modèle commenté.

L'écran ouvre aussi le `config.toml` de Herdr, `menu.toml` et, quand la sidebar est installée, `sidebar.toml` dans l'éditeur de l'utilisateur, puis se ferme : un popup bloque Herdr tant qu'il est ouvert, l'édition se fait donc en dehors. Un éditeur qui a sa propre fenêtre démarre détaché ; un éditeur en terminal s'ouvre dans un onglet de Herdr, qui propose de recharger la configuration de Herdr une fois l'éditeur fermé. Le pane `Tools` d'où l'écran a été ouvert propose le rechargement par un bouton, et un retour à ses icônes. Aucun plugin n'écrit le `config.toml` de Herdr.

*Révisé le 2026-11-01 : l'entrée ouvrait d'abord les fichiers dans un éditeur à l'intérieur de son popup, ce qui bloquait Herdr jusqu'à la fermeture de l'éditeur ; l'écran de réglages était prévu pour plus tard.*

## Conséquences

- Couleurs et disposition restent dans le `config.toml` de Herdr ; les fichiers des plugins ne contiennent que ce que Herdr ne sait pas régler.
- Les fichiers peuvent être versionnés avec le reste de la configuration (`dotfiles/herdr/` d'OS-Setup).
- Un nouveau réglage, c'est un champ avec sa valeur par défaut, le modèle, une ligne de l'écran des réglages et la page du plugin, dans les deux langues.
- Le menu écrit `menu.toml` et `sidebar.toml`, jamais le `config.toml` de Herdr ; une copie versionnée des fichiers des plugins (OS-Setup) se met à jour à la main après un changement fait à l'écran.
- Tant que l'action `install` de la sidebar n'a pas tourné une fois après une mise à jour, ses passages de la barre d'onglets prennent les valeurs par défaut.
- Les réglages de la sidebar sont lus par la sidebar et par le menu avec le même code, dans `crates/herdr-api` ; le modèle de `sidebar.toml` reste dans `sidebar/templates/`, avec le plugin dont c'est le fichier.
