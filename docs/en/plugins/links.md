# links

**English** · [Français](../../fr/plugins/links.md)

Opens the paths the Claude Code plugin [dx](https://github.com/IMSike/CC-Plugins/blob/main/docs/en/plugins/dx.md) makes clickable in Claude's replies, on a **ctrl+click** in Herdr: in herdr-explorer's preview, selected in the File Explorer, or with the program Windows associates with the file. A file that may run code is never opened with its program unless you allow it. Windows only. Why it works this way: [ADR 0012](../adr/0012-link-dispatch.md).

## Installation

```sh
herdr plugin install IMSike/Herdr-Plugins/links
```

Then check what the clicks need, below. dx sees the plugin at the start of its next Claude Code session.

## What a click does

dx draws a path that exists as a link `dx-open:///C:/folder/file.rs#L12-L14`. On a ctrl+click, the plugin reads the path from the link, and its lines (`#L12-L14`), nothing else: what it does is set in `links.toml` (below), the same for every link.

| Target | What happens |
|---|---|
| A folder | Selected in its parent, in the File Explorer. Never opened as `explorer.exe <folder>`, which would launch a file put in its place. |
| A file, `file = "preview"` (default) | Shown in herdr-explorer's preview, in the clicked pane's tab, scrolled to its lines. Without herdr-explorer, or when the preview fails, as `"application"`. When herdr-explorer does not answer within 3 s, nothing else opens, since the preview may still show: a notification « Aperçu lent » says so. |
| A file, `file = "select"` | Selected in the File Explorer, not opened with its program; the File Explorer may still read it (see the end of [Danger](#danger)). |
| A file, `file = "application"` | Opened with the program Windows associates with it, when its type is in `open_types`. Any other one is selected in the File Explorer, with a Herdr notification saying why, unless the **Danger** settings allow it. |

Before anything, the plugin checks the real target: links and junctions followed, short names (`PROGRA~1`) made long, a `subst` drive replaced by its folder. A path on a network share or outside a local drive, a missing one, one of 260 UTF-16 code units or more (Windows' `MAX_PATH` limit; « chemin trop long »), or one whose name the File Explorer would read as something else (a comma, a semicolon, a double quote, a colon, a percent sign, refused so that no `%NAME%` can ever be read as a variable on the way to the File Explorer, a name ending with a period or a space, a Windows device name such as `CON`, `NUL`, `COM1` or `LPT1`, with or without an extension, a CLSID as its extension, `x.{…}`…), or holding a character that does not show as itself (an invisible, private-use or blank braille character, a Unicode noncharacter: the list is in [ADR 0012](../adr/0012-link-dispatch.md#decision), step 1), is not opened: a Herdr notification says why. The type is read on the real name, so a link `notes.txt` that leads to `tool.exe` is a `.exe`.

The plugin never opens anything by itself: only your click runs it, once.

## Settings

`links.toml`, in the plugin's config folder (`herdr plugin config-dir imsike.links`), read at each click. The [menu](menu.md)'s Settings screen sets them, in its **Liens** and **Danger** sections. A missing file or setting keeps its default. A wrong type in `open_types` is left out and logged, the others kept. A file that cannot be used at all (a syntax error, an unknown setting, a wrong value of another setting) is set aside: each click says so in a Herdr notification and only selects files, so it never opens more than you chose. So is a file the plugin cannot find, when Herdr gives it no config folder (`HERDR_PLUGIN_CONFIG_DIR` missing): you may have written one. The template, every setting commented out at its default: [`links/templates/links.toml`](../../../links/templates/links.toml).

| Setting | Default | What it does |
|---|---|---|
| `file` | `"preview"` | What a click on a file does: `"preview"`, `"select"` or `"application"` (see above). |
| `open_types` | see below | The types (without the dot, any case) the default application may open. |
| `run_programs` | `false` | **Danger.** A file of a type outside `open_types` opens with its program too: a program, a script or an installer runs. |
| `confirm_programs` | `true` | With `run_programs`: a popup shows the file's real path and its type, and opens it only on Enter (Esc or any other key cancels at once, even in its first 1.1 s). The menu's Settings screen turns it on again each time it turns `run_programs` on. |

### Open types

By default, types that do not run code by themselves when Windows opens them with the program it associates with them, as installed (that program may still have flaws, as any viewer), but for the risks below:

- text and documents: `txt md markdown rst adoc log pdf docx xlsx pptx`;
- data and configuration: `csv tsv json jsonc json5 jsonl yaml yml toml ini cfg conf env properties lock sql diff patch`;
- source code Windows does not run: `c h cc cpp cxx hpp cs go rs java kt swift ts tsx jsx vue svelte css scss sass less`;
- images: `png jpg jpeg gif bmp webp ico tif tiff avif heic`;
- sound and video: `mp3 wav flac ogg m4a mp4 mkv webm mov avi`.

A `docx`, `xlsx` or `pptx` holds no macro of its own, but may name a file elsewhere: a `.docx` may name its template on a network share (`attachedTemplate`, `\\host\…`), which Word fetches when it opens it, without asking, and Windows may then send that host your NTLM hash; the template's macros stay behind Office's warning. Remove these types from `open_types` if that matters to you.

A `pdf` may hold JavaScript, which its reader (Acrobat, Edge's) may run when it opens the file (an `OpenAction`), in the reader's sandbox. Remove `pdf` from `open_types` if that matters to you.

Excel, when it opens a `csv` or a `tsv`, evaluates the formulas the file holds (a cell starting with `=`). Remove these types from `open_types` if that matters to you.

An Adobe application installed (Photoshop, Illustrator…) may associate `jsx` with its ExtendScript scripts, which it then runs when it opens the file, after a warning. Remove `jsx` from `open_types` if that matters to you.

Left out on purpose: programs and scripts (`exe`, `bat`, `cmd`, `ps1`, `js`, `vbs`, `py`, `sh`…; Python and Git for Windows make `.py` and `.sh` run when opened), shortcuts and installers (`lnk`, `url`, `msi`, `reg`…), documents with macros (`docm`, `xlsm`…, the old `doc`, `xls`, `ppt`, and OpenDocument `odt`, `ods`, `odp`, with their LibreOffice Basic macros), `rtf`, and what a browser opens with its scripts (`html`, `svg`, `xml`), and `epub`, whose content documents EPUB 3 lets hold scripts. A file without a type (`Makefile`) is never in `open_types`: it is treated as any type outside it; a name that starts with a dot has one: `.env` has the type `env`.

Any extension may be added (`$$$`, `!ut`, `(x)`…), with or without its dot, in any case. Only the last extension counts: write `gz`, not `tar.gz` (which is left out, saying so). A type holding a space, or a character no name of a link may hold (as above), is left out too; so is one ending with a dot (`md.`: write `md`). Add a type only if you know its program runs nothing; a type that runs code added to the list is opened without the Danger settings.

### Danger

A link can come from anywhere: Claude can write a link to any file under any text, and any program in any pane can print one. With `run_programs` off, no link can make the plugin open a file outside `open_types` with its program. Turn it on only if you need it, and keep `confirm_programs` on: the popup shows the real path, not the link's text, and the file's type (`Type .exe`), and checks the path again before opening it. A shortcut's own path is not what it launches: for any shortcut (`.lnk`, `.url`, `.pif`, `.website`, `.application`, `.appref-ms`, `.rdp`, `.scf`, `.settingcontent-ms`, `.shb`, `.xnk`, `.library-ms`, `.searchConnector-ms`, `.search-ms`, `.wsh`, `.jnlp`, `.ica`), the popup adds a red line « Raccourci : ouvre une autre cible, non affichée ». It does not read what the shortcut opens, not even an Internet shortcut's address: Windows reads a `.url` in a way that is not documented, and the address shown could differ from the one opened. It cannot be confirmed by accident: only a plain Enter, typed a moment after the popup opens and with the whole path shown, confirms; any other key cancels ([ADR 0012](../adr/0012-link-dispatch.md#decision) says how).

Whatever the settings, a link can make the File Explorer show a folder the model chose, and the File Explorer reads what it shows, as any of its windows on that folder would, but here the plugin opens the window:

- a `desktop.ini`, a `.lnk`, a `.url`, a `.scf` or a `.library-ms` in it whose icon points to a network share (`\\host\…`, as a `.url`'s `IconFile=\\host\…`) can make Windows connect to that host and send it your NTLM hash;
- a file selected is not opened with its program, but with the File Explorer's preview pane on, selecting it runs the preview handler of its type (Office's, a PDF reader's…), which reads the file, and the folder's thumbnails are made by the thumbnail handlers of its files' types, which read them too: a flaw in one of them meets a file the model chose.

Before refusing a path on a network share, the plugin follows the link's symbolic links to find the real target: a symbolic link to `\\host\…` makes Windows connect to that host, and may send it your NTLM hash, before the click is refused.

## Requirements

| Where | What | Why |
|---|---|---|
| Claude Code `settings.json` → `env` | `"FORCE_HYPERLINK": "1"` | Herdr is unknown to Claude Code, which would otherwise print `text (url)` instead of a link. dx says when it is missing. |
| Herdr | `imsike.links` installed and enabled | The link handler. dx says when it is missing. |
| Herdr's `config.toml` | `ui.mouse_capture` on (the default) | Link clicks reach plugins only then. |
| Windows Terminal's `settings.json` | `"experimental.detectURLs": false` in `profiles.defaults` | Otherwise Windows Terminal hands ctrl+clicked `file://` text to Windows before Herdr sees it. |
| Optional | [herdr-explorer](https://github.com/gcaudan/herdr-explorer) | The preview; without it, `"preview"` opens with the program, within `open_types`. |

## Logs

Every click that does not do what it would have, and every failure, is logged; a File Explorer or a confirmation popup that cannot be opened is also said in a Herdr notification, or in the popup itself after a confirmation:

```sh
herdr plugin log list --plugin imsike.links
```

## Turning it off

```sh
herdr plugin disable imsike.links
```

dx then draws no more links, from its next session. dx's own **Chemins cliquables** setting turns the links off on its side; `herdr plugin uninstall imsike.links` removes the plugin.
