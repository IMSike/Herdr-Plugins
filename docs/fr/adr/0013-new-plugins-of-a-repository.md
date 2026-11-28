# ADR 0013 — Proposer les nouveaux plugins d'un dépôt

- **Statut** : accepté
- **Date** : 2026-11-28

## Contexte

Chaque plugin de ce dépôt s'installe seul (`herdr plugin install IMSike/Herdr-Plugins/<plugin>`). Quand un plugin s'ajoute au dépôt (links, après menu et sidebar), la mise à jour ([ADR 0009](0009-new-version-check.md)) mettait à jour les plugins déjà installés, mais rien ne signalait le nouveau : il fallait l'installer à la main, et seulement si on le savait.

Ce qu'offre Herdr :

- `herdr plugin update` ne met à jour que les plugins installés ; il ne sait pas installer les autres d'un dépôt.
- Pour un plugin installé depuis un dossier d'un dépôt GitHub, `plugin.list` donne le clone du dépôt (`source.managed_path`) et le dossier (`source.subdir`). Herdr clone le dépôt entier : les autres dossiers, avec leur `herdr-plugin.toml`, sont dans ce clone, au commit installé.

Options étudiées :

| Option | Pour | Contre |
|---|---|---|
| a. Une liste des plugins du dépôt écrite dans le menu | Simple | Pour ce dépôt seulement ; le menu installé ne connaît que les plugins de son propre commit |
| b. Lire les dossiers voisins de chaque plugin installé dans son clone | Aucune requête à GitHub ; vaut pour tout dépôt à plusieurs plugins ; un plugin apporté par une mise à jour se voit juste après elle | Suppose que Herdr garde le dépôt entier dans son clone ; un plugin dont l'utilisateur ne veut pas se revoit à chaque vérification |
| c. Demander à GitHub l'arborescence du dépôt | Voit un nouveau plugin avant la mise à jour | Une requête de plus par dépôt, limitée sans jeton (60 par heure) |
| d. Ne rien changer | Rien à écrire | Un nouveau plugin passe inaperçu |

## Décision

L'option b, choisie par le mainteneur, avec ce qui suit :

1. **Détection**, à chaque vérification des nouvelles versions (`updates.json` la garde) : pour chaque plugin installé depuis un dossier d'un dépôt GitHub, les dossiers voisins dans son clone dont le manifeste donne un id non installé (un plugin lié compte comme installé), dont les `platforms`, quand il y en a, contiennent le système en cours, et que l'utilisateur n'a pas refusés. Un plugin à la racine de son dépôt n'a pas de dossier voisin : rien n'est lu.
2. **Affichage** : comme une nouvelle version, en vert, le nom du plugin suivi de `(nouveau)` (`Settings · Links (nouveau)`), sauf avec `check_updates = false`.
3. **Installation** : la fenêtre de mise à jour les liste après les mises à jour, **décochés** ; un plugin coché s'installe après les mises à jour, par `herdr plugin install <owner>/<repo>/<dossier>`, depuis la même branche ou le même tag que le plugin à côté duquel il a été trouvé (`--ref`), sans `--yes` : Herdr montre ce que le plugin peut faire et demande. Les plugins que la mise à jour vient d'apporter sont proposés juste après elle, dans une deuxième liste, de la même façon.
4. **Refus** : un nouveau plugin laissé décoché une fois la liste validée (pas annulée) est retenu dans le dossier d'état (`declined-plugins.json`) et n'est plus proposé ni signalé. La ligne **Reproposer les plugins refusés** des réglages, affichée tant qu'il y en a un, les oublie : ils sont proposés de nouveau à la vérification suivante.

## Conséquences

- Un plugin ajouté à un dépôt est proposé à la mise à jour suivante d'un plugin de ce dépôt, sans requête de plus que celles de l'ADR 0009.
- Cela repose sur le fait que Herdr clone le dépôt entier pour un plugin installé depuis l'un de ses dossiers, ce que `plugin.list` montre (`managed_path`) sans le documenter comme une promesse : si un Herdr futur ne clonait que le dossier, plus rien ne serait proposé, et le reste marcherait comme avant.
- Un plugin du dépôt à plusieurs plugins de quelqu'un d'autre est proposé de la même façon : il ne s'installe qu'une fois coché, et la confirmation de Herdr s'affiche toujours.
- Un plugin désinstallé exprès (`herdr plugin uninstall`) est de nouveau proposé, décoché, jusqu'à ce qu'il soit refusé.
- Avant la mise à jour qui l'apporte, un nouveau plugin n'est pas vu : le clone est au commit installé.
