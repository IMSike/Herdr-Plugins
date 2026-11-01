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

L'entrée intégrée **Réglages** du menu ouvre le `config.toml` de Herdr, `menu.toml` et, quand la sidebar est installée, `sidebar.toml` dans l'éditeur de l'utilisateur, en créant d'abord le fichier d'un plugin à partir d'un modèle commenté ; à la fermeture de l'éditeur, elle vérifie les fichiers et recharge la configuration de Herdr. Aucun plugin n'écrit le `config.toml` de Herdr.

Un écran de réglages pourra venir plus tard, en écrivant les mêmes fichiers.

## Conséquences

- Couleurs et disposition restent dans le `config.toml` de Herdr ; les fichiers des plugins ne contiennent que ce que Herdr ne sait pas régler.
- Les fichiers peuvent être versionnés avec le reste de la configuration (`dotfiles/herdr/` d'OS-Setup).
- Un nouveau réglage, c'est un champ avec sa valeur par défaut, le modèle et la page du plugin, dans les deux langues.
- Tant que l'action `install` de la sidebar n'a pas tourné une fois après une mise à jour, ses passages de la barre d'onglets prennent les valeurs par défaut.
