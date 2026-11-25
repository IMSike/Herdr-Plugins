# ADR 0012 — Opening dx's path links: a plugin that dispatches, and protects

- **Status**: accepted
- **Date**: 2026-11-22

## Context

The Claude Code plugin dx ([CC-Plugins ADR 0009](https://github.com/IMSike/CC-Plugins/blob/main/docs/en/adr/0009-clickable-paths.md)) draws the paths of Claude's replies as terminal hyperlinks `dx-open:///C:/…/file#L12-L14`, inside Herdr only, and only paths that exist. dx, a function-hook plugin of Claude Code, cannot open a file in another program without running one itself, and Herdr is what receives the click.

Herdr routes a ctrl+click on a URL to a plugin action when one of the plugin's `[[link_handlers]]` patterns matches it, while mouse capture is on; otherwise it opens only http(s) links. The action runs without a shell, from the plugin's folder, with `HERDR_PLUGIN_CLICKED_URL` and `HERDR_PANE_ID`. A live test (Herdr 0.9.5, Windows) ran such an action with no console window. herdr-explorer (`gcaudan.explorer`) shows a file in its preview, `herdr-explorer peek <file>[:<lines>]`, run from a Herdr pane; it refuses folders, and it reads `HERDR_PLUGIN_ID`, `HERDR_PLUGIN_STATE_DIR` and `HERDR_PLUGIN_CONTEXT_JSON` as its own when they are set.

The maintainer wants a click to open what the user chose: herdr-explorer's preview, the file selected in the File Explorer, or the program Windows associates with it; a folder always in the File Explorer; and a file that may run code when opened (a program, a script, an installer, a shortcut, a document with macros…) never opened that way unless a setting, off by default, allows it. The link's URL can come from anywhere: the model can write a Markdown link with any text and target, and any program in any pane can print an OSC 8 link.

Options studied:

| Option | Pros | Cons |
|---|---|---|
| Herdr's own handling | Nothing to write | Herdr opens only http(s); nothing for a custom scheme |
| The action in the menu plugin | No new install | dx's feature tied to the menu; the menu's binary grows with an unrelated job |
| **A plugin of its own, `links`** | Installed with dx alone; small; the opening and the protection in one tested place | One more plugin |
| The action in PowerShell | No build | Outside the repository's Rust ([ADR 0001](0001-rust-cargo-workspace.md)); `powershell` from `PATH`; slower; untested |
| What to do written in the URL (`dx-open:default/…`) | The click carries the choice | Anyone printing a link chooses; the protection would still need a setting |
| **The URL holds the path alone; the choice and the protection are the plugin's settings** | A forged link does at most what a real one does | The choice is set in Herdr, not in Claude Code |
| A list of the types that run (a denylist) | Opens anything else | A type it forgets, or a program installed later that runs its files, opens code |
| **A list of the types that may open (an allowlist), the user's to change** | A type nobody thought of is selected, not opened | A harmless type missing from the list is selected until the user adds it |

## Decision

A new plugin, `links` (`imsike.links`, binary `herdr-links`, Windows only), declares an action `open` and a link handler `pattern = "^dx-open:"`. `herdr-links open`:

1. **Reads the URL, and nothing else from it**: `dx-open:///X:/…`, at most 2048 bytes, no host and no query, its segments percent-decoded as strict UTF-8, an optional fragment `#L12` or `#L12-L14`. Refused: a control character, a separator or a colon inside a segment (an alternate data stream), a `.`, `..` or empty segment, a name ending with a period or a space, a character Windows refuses in a name or reads as a wildcard (`* ? < > |`), a comma, a semicolon or a double quote (the File Explorer splits its command line at commas), a percent sign (so that no `%NAME%` can ever be read as a variable on the way to the File Explorer), a Windows device name (`CON`, `PRN`, `AUX`, `NUL`, `COM0`–`COM9`, `LPT0`–`LPT9`, `COM¹`–`COM³`, `LPT¹`–`LPT³`, `CONIN$`, `CONOUT$`, in any case, with or without an extension), a CLSID as the last extension (`x.{8-4-4-4-12}`, which the File Explorer opens as the object it names), an invisible character: any character of the Unicode General_Category Cf (format), Zl or Zp (line and paragraph separators), or of the property Default_Ignorable_Code_Point, as Unicode 16.0 lists them (zero-width characters, fillers, variation selectors, tags, bidirectional controls, the interlinear annotation characters U+FFF9–U+FFFB, the format controls of other scripts…), which would make a name look like another one; a private-use character (the General_Category Co), which a font may draw as anything; the blank braille pattern U+2800, which shows as a blank; a Unicode noncharacter (U+FDD0–U+FDEF, and the last two code points of each plane, U+FFFE, U+FFFF, U+1FFFE… U+10FFFF).
2. **Checks the real target**: the path made canonical (symbolic links and junctions followed, DOS short names made long, a `subst` drive replaced by its folder), refused when that fails or lands on a network share or outside a local drive (a path of a volume or device namespace), or is 260 UTF-16 code units long or more (Windows' `MAX_PATH` limit), or when its real names break the rules of step 1. The type is read on the real name, and every command is built from that canonical path.
3. **Reads its settings** from `links.toml` in its config folder ([ADR 0004](0004-plugin-settings.md)), at each click:
   - `file`: `"preview"` (default), `"select"` or `"application"`;
   - `open_types`: the types the default application may open, without the dot, any case; by default types that do not run code by themselves (their viewer may still have flaws), but for the risks the consequences list: text and documents, data and configuration, source code Windows does not run, images, sound and video (not `.js`, `.py`, `.ps1`, `.sh`, nor `.html`, `.svg`, `.xml`, which a browser opens with their scripts, nor OpenDocument files with their macros, nor `.rtf`);
   - a **Danger** section: `run_programs` (`false` by default), which lets a file of another type open with its program; `confirm_programs` (`true` by default), which then asks first.

   Any extension may be a type, with or without its dot, in any case (`$$$`, `!ut`, `(x)`…); only the last one counts: `gz`, not `tar.gz`. A type holding a space, or a character no name of a link may hold (step 1), or ending with a dot (`md.`), is a wrong type. A missing file or setting keeps its default; a wrong type is left out, logged, the others kept. A file that cannot be used at all (a syntax error, an unknown setting, a wrong value of another setting) is set aside, said in a notification at each click, and files are then only selected: a broken file never opens more than the user chose. A file that cannot be found, Herdr giving no config folder, is treated the same.
4. **Opens**, the File Explorer always as `%SystemRoot%\explorer.exe` (`%windir%` without it), by its full path, from that folder, without Herdr's `HERDR_*` variables (what it opens runs outside Herdr; when a File Explorer window is already open, `explorer.exe` hands the request to that running Explorer, which starts the program with its own environment: the variables are left out only of what the `explorer.exe` this plugin starts runs itself), its command line written as it reads it:
   - a folder: `/select,"<folder>"`, the folder selected in its parent; never `explorer.exe <folder>`, which would launch a file put in its place;
   - a file with `preview`: herdr-explorer, found through Herdr's `plugin.list` (`bin\herdr-explorer.exe` under its root, enabled), run as `peek <file>[:<lines>]` from its folder, without the `HERDR_PLUGIN_*` variables, with `HERDR_PANE_ID`, no console window, 3 s at most: past them, herdr-explorer is stopped and nothing else opens, since what it started may still show the file (a `herdr` it runs outlives it), and a notification « Aperçu lent » says so. Without herdr-explorer, or when the preview fails, as `application`;
   - a file with `select`: `/select,"<file>"`, never opened with its program;
   - a file with `application`: `explorer.exe "<file>"`, the program Windows associates with it, when its type is in `open_types`. Another one is selected instead, with a Herdr notification saying why, unless `run_programs` is on; then, with `confirm_programs`, a popup of the plugin shows the canonical path and the file's type, for a shortcut type ([links](../plugins/links.md#danger)) a red line saying its target is not shown, and opens it only on Enter, after checking the path and `run_programs` again (only an Enter typed on its own, without Ctrl, Alt or Shift, after 1.1 s confirms: any other key, Esc or a modified Enter included, cancels at once, even in the first 1.1 s; an Enter heard only after 1.1 s without any Enter, longer than the keyboard's longest repeat delay, 1 s (a longer delay set by FilterKeys, an accessibility setting, is not covered), each one in that time starting the wait again; the Enter taken when it is released, pressed once and not repeated; an Enter followed at once by more input taken as a paste, no confirmation while the whole path, and a shortcut's line, do not fit); without, it opens at once.

A link refused, a path missing or not local, or a File Explorer or a popup that cannot be opened, is a Herdr notification (`notification.show`, without sound), or a line of the popup itself after a confirmation, never a modal; every failure is also logged (`herdr plugin log list --plugin imsike.links`). The menu's Settings screen gains a **Liens** section, and a **Danger** one, when the plugin is installed.

## Consequences

- No URL can make the plugin open a file of a type outside `open_types` with its program while `run_programs` is off; on, each such opening is confirmed, unless `confirm_programs` is off too.
- A type missing from `open_types` is selected, not opened; a type added that runs code (`exe`, `js`…) runs.
- What a click does is chosen in Herdr (`links.toml`, the menu's Settings), the same for every link; dx holds no such setting. dx checks that a path exists and that this plugin would take it, and also skips paths too broad, Claude Code's settings files and anything outside its grammar; it allows fewer characters in a name than this plugin does.
- The file checked, or a shortcut's target, can be replaced between the check (or the confirmation) and the opening (a TOCTOU race); replacing it takes write access to its folder.
- A folder the File Explorer shows can hold icons pointing to `\\host\…`, which may make Windows send that host the user's NTLM hash.
- A file selected is still read by the File Explorer's preview and thumbnail handlers.
- Symbolic links are followed before a network path is refused: one to `\\host\…` may send the NTLM hash before the refusal.
- The default `open_types` keep types with risks of their own: a `.docx`'s template on a network share, a PDF's JavaScript, the formulas Excel evaluates in a `csv` or `tsv`, a `jsx` an Adobe application runs ([links](../plugins/links.md#open-types)).
- A shortcut's target and an Internet shortcut's address are not shown, only said to be hidden: Windows' reader of a `.url` is not documented.
- herdr-explorer stays optional: without it a file opens with its program, within `open_types`.
- Herdr's mouse capture must be on for a click to reach the plugin; Windows Terminal's `experimental.detectURLs` off keeps it from handing ctrl+clicked `file://` text to Windows first. Both are documented.
- Windows only: the crate compiles on Linux, where both commands only log.
- To be reassessed if Herdr changes its link handlers, or if dx stops using `dx-open:`.
