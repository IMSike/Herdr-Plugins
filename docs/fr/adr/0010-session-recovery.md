# ADR 0010 — Récupérer une session à partir des snapshots de Herdr

- **Statut** : accepté
- **Date** : 2026-11-11

## Contexte

Sous Windows, les panes de Herdr lancent `pwsh.exe`. PowerShell 7 du Microsoft Store se met à jour en fermant tous les `pwsh.exe` de son paquet, sans moyen de l'éviter à part ne pas utiliser ce paquet. Le serveur Herdr survit, mais chaque pane dont le shell meurt est retiré de la mise en page, son agent (un enfant du shell) s'arrête avec lui, et `session.json` est réécrit sans eux. Les conversations elles-mêmes sont gardées par les agents (`claude --resume <id>`), mais plus rien dans Herdr n'y mène.

Ce que Herdr offre (mesuré sur Herdr 0.9.5, et dans sa documentation, *Session state and restore*) :

- Jusqu'à 48 copies de la mise en page dans `session-snapshots/`, à côté de `session.json`, une toutes les 15 minutes au plus : quand la mise en page change, la précédente est copiée si la dernière copie est plus ancienne que cela. Lors d'un test, tuer le shell de trois panes plus de 15 minutes après la dernière copie a laissé un snapshot avec les 4 panes et les 3 agents, et un `session.json` avec 2 panes et aucun agent.
- Une récupération documentée, à la main : arrêter le serveur, garder une copie de `session.json`, copier le snapshot choisi à sa place, relancer la session. À son démarrage, Herdr recrée les panes dans leurs dossiers et relance leurs agents (`[session] resume_agents_on_restore`, actif par défaut) : mesuré avec un agent qui donne lui-même sa commande de reprise, et avec un vrai `claude --resume`, revenu sur sa conversation dans son dossier.
- Aucun écran pour choisir un snapshot : ce sont des fichiers nommés d'après une heure en nanosecondes, tous semblables.
- `herdr session list --json` donne le dossier et le socket de chaque session ; la documentation de Herdr arrête la session par défaut par `herdr server stop`, une session nommée par `herdr session stop <nom>` ; une fois arrêté, le serveur a écrit `session.json` pour la dernière fois. Une fenêtre ouverte par `cmd /d /c start` depuis un pane continue de tourner une fois le serveur arrêté.
- Un `session.json` d'une version inconnue n'est pas chargé : Herdr en garde une copie dans `session-backups/`.

Questions tranchées :

| Question | Options | Choix |
|---|---|---|
| Quelles copies | a. Les snapshots de Herdr seulement. b. Des copies écrites aussi par le plugin, à chaque événement d'agent | a : le Store ferme `pwsh` pendant que la session est verrouillée, en général longtemps après le dernier changement, quand la copie de Herdr est exacte. Seul ce qui a changé dans les 15 minutes d'avant est perdu. b écrirait des fichiers pendant que les agents travaillent, pour un cas rare |
| Ce qui est restauré | a. Tout le snapshot, serveur arrêté. b. Seulement les panes manquants, recréés par l'API, serveur en marche | a : la voie documentée et mesurée. b demande bien plus de code, jamais mesuré, et devrait reconstruire les découpages |
| Où | a. Une ligne de l'écran des réglages. b. Une entrée du menu aussi | a : une action rare, qui ne mérite pas une place dans la barre |
| Quelles sessions | a. La session courante. b. N'importe laquelle | a : restaurer une autre session arrêterait un serveur depuis une session que l'utilisateur ne regarde pas |
| Relancer | a. Dans la fenêtre. b. La commande dans le presse-papiers | b, choisi par le mainteneur : la fenêtre le dit et affiche la commande |
| Titres de Claude Code | a. L'identifiant de la conversation seul. b. Son titre lu dans la transcription de Claude Code | b, choisi par le mainteneur, avec a en repli |

## Décision

Une ligne **Récupérer une session…** de l'écran des réglages, section **Session**, ouvre un écran dans le même popup :

1. Les snapshots de la session courante (trouvée par `herdr session list --json` et `HERDR_SOCKET_PATH`), du plus récent au plus ancien : leur date locale et leur âge, ce qui manque aujourd'hui à la session par rapport à chacun (`+3 absents` : un agent par sa conversation, ou par sa commande de reprise ; un shell par son dossier), et leurs espaces, onglets, panes et agents. Un snapshot d'une autre version que la 3, ou illisible, est listé et ne peut pas être restauré.
2. Le snapshot choisi en détail : espaces, onglets, panes avec leur dossier, agents avec leur type et le début de l'identifiant de leur conversation, les panes qui manquent à la session marqués `+`. Pour Claude Code, le dernier titre de la conversation (`ai-title`), sinon son dernier message, lus dans `~/.claude/projects/*/<id>.jsonl` (`CLAUDE_CONFIG_DIR` s'il est défini) ; un fichier interne de Claude Code, qui ne donne plus que l'identifiant s'il change.
3. Entrée ouvre une fenêtre de terminal à part, hors de Herdr, comme Update ([ADR 0005](0005-herdr-update-entry.md)), qui lance `herdr-menu recover`. Elle vérifie que le snapshot est bien un de la session, dit ce qu'il contient et ce qui se ferme, et demande `[o/N]` ; une fois confirmé, elle arrête le serveur (`herdr server stop`, ou `herdr session stop <nom>`) et attend que Herdr le dise arrêté, copie le `session.json` qu'il a laissé dans `session-before-restore-<date>-<heure>.json` à côté de lui, écrit le snapshot à la place de `session.json` d'un seul coup (un fichier renommé par-dessus), et met `herdr` (`herdr --session <nom>` pour une session nommée) dans le presse-papiers, en l'affichant.

## Conséquences

- Récupérer des agents perdus demande les réglages, une ligne, Entrée, `o`, et un collage dans un terminal ; chaque agent revient sur sa conversation, dans son dossier, après son dernier message enregistré : le tour en cours quand il s'est arrêté est perdu.
- Tous les panes se ferment, y compris ceux qui avaient survécu : Herdr les recrée depuis le snapshot. Ce qui n'est pas un agent (un serveur de développement, un watcher) revient comme un shell dans son dossier.
- Les changements faits dans les 15 minutes avant la mort des panes peuvent manquer à tous les snapshots.
- Le `session.json` d'avant reste à côté de lui jusqu'à ce qu'on le supprime : recopié à la place de `session.json`, serveur arrêté, il remet la session comme elle était.
- Les titres dépendent d'un fichier de Claude Code qui n'est pas documenté ; s'il change, l'écran n'affiche plus que les identifiants.
- Les dates passent par `chrono` (fuseau horaire local), une nouvelle dépendance du menu.
