//! Writes one setting into a TOML file and keeps the rest of it as it is:
//! comments, order, entries.

use std::path::Path;
use toml_edit::{Array, DocumentMut, Item, Value};

/// Sets `key` at the top level of the file at `path`, created from
/// `template` when it does not exist.
pub fn set(path: &Path, template: &str, key: &str, value: Value) -> Result<(), String> {
    set_all(path, template, [(key, value)])
}

/// Sets each key of `keys`, in that order, as `set` does, and writes the
/// file once: all of them or none.
pub fn set_all<'a>(
    path: &Path,
    template: &str,
    keys: impl IntoIterator<Item = (&'a str, Value)>,
) -> Result<(), String> {
    let mut text = read(path, template)?;
    for (key, value) in keys {
        text = with_value(&text, key, value)?;
    }
    write(path, &text)
}

/// Changes the list `key` at the top level of the file at `path` (created
/// from `template` when it does not exist) as `edit` does, in place: its
/// comments, its layout and the entries `edit` keeps stay as written. When
/// the file does not set it, `edit` gets `unset`, the list that stands for,
/// then written as `set` writes a value. `edit` says whether it changed the
/// list, or why it refuses to: the file is written only when it did; so is
/// what this returns.
pub fn edit_array(
    path: &Path,
    template: &str,
    key: &str,
    unset: Array,
    edit: impl FnOnce(&mut Array) -> Result<bool, String>,
) -> Result<bool, String> {
    match with_array(&read(path, template)?, key, unset, edit)? {
        Some(text) => write(path, &text).map(|()| true),
        None => Ok(false),
    }
}

/// The text of the file at `path`, `template` when it does not exist.
fn read(path: &Path, template: &str) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(template.to_string()),
        Err(e) => Err(format!("{} : {e}", path.display())),
    }
}

/// Writes `text` to the file at `path`, its folder created when missing;
/// the error names the path.
pub fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    }
    herdr_api::write_atomic(path, text.as_bytes()).map_err(|e| format!("{} : {e}", path.display()))
}

fn parse(text: &str) -> Result<DocumentMut, String> {
    text.parse()
        .map_err(|e: toml_edit::TomlError| e.to_string().trim().to_string())
}

/// `text` with `key` set: in place when the file sets it, on its
/// commented-out line (`# key = …`, before any table, commented out or
/// not) when it has one, else added to the top level.
fn with_value(text: &str, key: &str, value: Value) -> Result<String, String> {
    let mut doc = parse(text)?;
    if let Some(item) = doc.get_mut(key) {
        let old = item
            .as_value_mut()
            .ok_or_else(|| format!("« {key} » n'est pas une valeur"))?;
        let decor = old.decor().clone();
        *old = value;
        *old.decor_mut() = decor;
        return Ok(doc.to_string());
    }
    let commented = format!("# {key} = ");
    let lines: Vec<&str> = text.split('\n').collect();
    let first_table = lines
        .iter()
        .position(|l| is_table(l))
        .unwrap_or(lines.len());
    if let Some(at) = lines[..first_table]
        .iter()
        .position(|l| l.trim_start().starts_with(&commented))
    {
        let set_line = format!("{key} = {}", value.to_string().trim());
        let mut lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        lines[at] = set_line;
        return Ok(lines.join("\n"));
    }
    doc.insert(key, Item::Value(value));
    Ok(doc.to_string())
}

/// `text` with the list `key` changed by `edit` (see `edit_array`); `None`
/// when `edit` changed nothing.
fn with_array(
    text: &str,
    key: &str,
    unset: Array,
    edit: impl FnOnce(&mut Array) -> Result<bool, String>,
) -> Result<Option<String>, String> {
    let mut doc = parse(text)?;
    if let Some(item) = doc.get_mut(key) {
        let array = item
            .as_array_mut()
            .ok_or_else(|| format!("« {key} » n'est pas une liste"))?;
        return Ok(edit(array)?.then(|| doc.to_string()));
    }
    let mut array = unset;
    if !edit(&mut array)? {
        return Ok(None);
    }
    with_value(text, key, array.into()).map(Some)
}

/// What is written before `value` in its list: spaces, line breaks and the
/// comments on the lines above it.
fn prefix_of(value: &Value) -> String {
    value
        .decor()
        .prefix()
        .and_then(|p| p.as_str())
        .unwrap_or_default()
        .to_string()
}

/// What is written after `value` in its list, before its comma or, for the
/// last entry of a list without a comma after it, before the list's end.
fn suffix_of(value: &Value) -> String {
    value
        .decor()
        .suffix()
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string()
}

/// What follows comment lines moved in front of `after`, the text before
/// the next entry or the list's end: `after` without its first line when it
/// is blank, else `after` on a line of its own, indented by `indent`.
fn below_comments<'a>(after: &'a str, indent: &str) -> std::borrow::Cow<'a, str> {
    match after.split_once('\n') {
        Some((first, rest)) if first.trim().is_empty() => rest.into(),
        _ => format!("{indent}{}", after.trim_start()).into(),
    }
}

/// Removes the entries of `array` that `drop` names. The comments written
/// above an entry removed stay, moved above the next one, or before the
/// list's end for the last one; a comment after an entry removed that
/// starts its line goes with it. The list keeps its layout, with or without
/// a comma after its last entry.
pub fn remove_where(array: &mut Array, drop: impl Fn(&Value) -> bool) {
    let mut at = 0;
    while at < array.len() {
        if !array.get(at).is_some_and(&drop) {
            at += 1;
            continue;
        }
        let removed = array.remove(at);
        let prefix = prefix_of(&removed);
        // The comment lines, up to the last line break, and the entry's
        // indentation, after it.
        let (comments, indent) = match prefix.rfind('\n') {
            Some(end) if prefix.contains('#') => prefix.split_at(end + 1),
            _ => ("", ""),
        };
        let kept = |after: &str| match comments {
            "" => after.to_string(),
            _ => format!("{comments}{}", below_comments(after, indent)),
        };
        // A comment on the line of an entry that started it is that entry's:
        // it goes with it, rather than onto the line before.
        let own_line = at == 0 || prefix.contains('\n');
        let unnoted = |after: String| match own_line {
            true => without_note(&after),
            false => after,
        };
        if let Some(next) = array.get_mut(at) {
            let before = kept(&unnoted(prefix_of(next)));
            next.decor_mut().set_prefix(before);
        } else if !array.trailing_comma() {
            // The last entry, without a comma after it: what followed it,
            // the line break before the list's end included, now follows
            // the entry before it, or opens the list's end.
            let end = kept(&unnoted(suffix_of(&removed)));
            match at.checked_sub(1).and_then(|before| array.get_mut(before)) {
                Some(before) => {
                    let after = format!("{}{end}", suffix_of(before));
                    before.decor_mut().set_suffix(after);
                }
                None => {
                    let trailing = array.trailing().as_str().unwrap_or_default();
                    array.set_trailing(format!("{end}{trailing}"));
                }
            }
        } else {
            let after = array.trailing().as_str().unwrap_or_default().to_string();
            array.set_trailing(kept(&unnoted(after)));
        }
    }
}

/// `after`, the text after an entry, without the comment ending its first
/// line, if one does: the line break stays.
fn without_note(after: &str) -> String {
    match after.split_once('\n') {
        Some((first, rest)) if first.trim_start().starts_with('#') => format!("\n{rest}"),
        _ => after.to_string(),
    }
}

/// Adds `value` at the end of `array`, laid out as its last entry: on a
/// line of its own, as indented, in a list written one entry per line, with
/// a comma after it only if the list has one after its last entry. In an
/// empty list, it is the only entry: below the comments the list holds, if
/// any, else on the list's line (`["md"]`).
pub fn push_like_last(array: &mut Array, value: impl Into<Value>) {
    let comma = array.trailing_comma();
    let Some(last) = array.iter_mut().last() else {
        push_into_empty(array, value.into());
        return;
    };
    let before = prefix_of(last);
    // Without a comma after the last entry, what follows it up to the
    // list's end, its line break included, passes to the new last entry;
    // a comment on its line stays there, after the comma.
    let (comment, end) = match comma {
        // After the comma, a comment on the last entry's line is the list's
        // trailing text: in a list written one entry per line, it stays on
        // that line, above the new entry.
        true => {
            let trailing = array.trailing().as_str().unwrap_or_default().to_string();
            match trailing.rfind('\n') {
                Some(at) if before.contains('\n') => {
                    array.set_trailing(&trailing[at..]);
                    (trailing[..at].trim_end().to_string(), String::new())
                }
                _ => (String::new(), String::new()),
            }
        }
        false => {
            let after = suffix_of(last);
            last.decor_mut().set_suffix("");
            match after.rfind('\n') {
                Some(at) => (after[..at].trim_end().to_string(), after[at..].to_string()),
                None => (String::new(), after),
            }
        }
    };
    // In a list written on one line, the new entry follows the last on its
    // line, and the comment ending that line, if any, follows the new entry:
    // written before it, it would hold it.
    let (prefix, suffix) = match before.rfind('\n') {
        Some(at) => (format!("{comment}\n{}", &before[at + 1..]), end),
        None => (" ".to_string(), format!("{comment}{end}")),
    };
    let mut value = value.into();
    value.decor_mut().set_prefix(prefix);
    value.decor_mut().set_suffix(suffix);
    array.push_formatted(value);
}

/// Adds `value` to the empty `array`: see `push_like_last`.
fn push_into_empty(array: &mut Array, mut value: Value) {
    let trailing = array.trailing().as_str().unwrap_or_default().to_string();
    match trailing.rfind('\n') {
        Some(end) if trailing.contains('#') => {
            let comments = &trailing[..=end];
            let last_line = comments[..end].rsplit('\n').next().unwrap_or_default();
            let indent = &last_line[..last_line.len() - last_line.trim_start().len()];
            // On a line of its own below them, with a comma after it if the
            // list had one after its last entry.
            value.decor_mut().set_prefix(format!("{comments}{indent}"));
            array.set_trailing("\n");
        }
        _ => {
            array.set_trailing("");
            array.set_trailing_comma(false);
        }
    }
    array.push_formatted(value);
}

/// Whether `line` starts a table (`[name]`, `[[name]]`), or would once
/// uncommented (`# [[entries]]`): the lines below it belong to the table.
fn is_table(line: &str) -> bool {
    let line = line.trim();
    let line = line.strip_prefix('#').map_or(line, str::trim);
    line.starts_with('[') && line.ends_with(']')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_rest_of_the_file() {
        let text = "# Comment.\ndecimal_comma = true # inline\n\n[[entries]]\nkey = \"l\"\n";
        let set = with_value(text, "decimal_comma", false.into()).unwrap();
        assert_eq!(
            set,
            "# Comment.\ndecimal_comma = false # inline\n\n[[entries]]\nkey = \"l\"\n"
        );
    }

    #[test]
    fn uncomments_the_template_line() {
        let text = "# Help.\n# totals = \"line\"\n\n# [[entries]]\n# key = \"l\"\n";
        let set = with_value(text, "totals", "bars".into()).unwrap();
        assert_eq!(
            set,
            "# Help.\ntotals = \"bars\"\n\n# [[entries]]\n# key = \"l\"\n"
        );
        // `key` is only commented out in the entries' example, below its
        // table: added, the example left as it is.
        let added = with_value(text, "key", "x".into()).unwrap();
        assert!(added.starts_with("key = \"x\"\n"), "{added:?}");
        assert!(added.contains("# key = \"l\""));
    }

    #[test]
    fn adds_before_the_tables() {
        let text = "[[entries]]\nkey = \"l\"\n";
        let set = with_value(text, "ttl_seconds", 30.into()).unwrap();
        let ttl = set.find("ttl_seconds").unwrap();
        assert!(ttl < set.find("[[entries]]").unwrap());
        assert!(with_value("a = [", "b", 1.into()).is_err());
    }

    /// `text`'s list `key` changed by `edit`.
    fn edited(text: &str, edit: impl FnOnce(&mut Array)) -> String {
        let changed = |a: &mut Array| {
            edit(a);
            Ok(true)
        };
        with_array(text, "types", Array::new(), changed)
            .unwrap()
            .unwrap()
    }

    const LIST: &str = "# Above.\ntypes = [\n    # Text.\n    \"txt\", \"md\", # notes\n    \"README\",\n    # Images.\n    \"png\",\n    # Sound.\n    \"mp3\",\n] # after\n";

    #[test]
    fn a_list_is_changed_in_place() {
        let added = edited(LIST, |a| push_like_last(a, "gif"));
        assert_eq!(
            added,
            LIST.replace("\"mp3\",\n]", "\"mp3\",\n    \"gif\",\n]")
        );
        let single = edited("types = [\"a\", \"b\"] # kept\n", |a| {
            push_like_last(a, "c")
        });
        assert_eq!(single, "types = [\"a\", \"b\", \"c\"] # kept\n");
    }

    #[test]
    fn removing_an_entry_keeps_the_comments_above_it() {
        let removed = edited(LIST, |a| remove_where(a, |v| v.as_str() == Some("png")));
        assert_eq!(
            removed,
            "# Above.\ntypes = [\n    # Text.\n    \"txt\", \"md\", # notes\n    \"README\",\n    # Images.\n    # Sound.\n    \"mp3\",\n] # after\n"
        );
        let last = edited(LIST, |a| remove_where(a, |v| v.as_str() == Some("mp3")));
        assert!(
            last.ends_with("    \"png\",\n    # Sound.\n] # after\n"),
            "{last}"
        );
        let first = edited(LIST, |a| remove_where(a, |v| v.as_str() == Some("txt")));
        assert!(
            first.contains("    # Text.\n    \"md\", # notes\n    \"README\""),
            "{first}"
        );
        // The comment after `md`, written before `README`, stays too.
        let md = edited(LIST, |a| remove_where(a, |v| v.as_str() == Some("md")));
        assert!(md.contains("\"txt\", # notes\n    \"README\""), "{md}");
    }

    #[test]
    fn a_list_without_a_comma_after_its_last_entry_keeps_its_layout() {
        let bare = "types = [\n    \"a\",\n    \"b\"\n]\n";
        let comma = "types = [\n    \"a\",\n    \"b\",\n]\n";
        let add = |text: &str| edited(text, |a| push_like_last(a, "c"));
        let remove = |text: &str, name: &'static str| {
            edited(text, move |a| remove_where(a, |v| v.as_str() == Some(name)))
        };
        assert_eq!(
            add(bare),
            "types = [\n    \"a\",\n    \"b\",\n    \"c\"\n]\n"
        );
        assert_eq!(
            add(comma),
            "types = [\n    \"a\",\n    \"b\",\n    \"c\",\n]\n"
        );
        assert_eq!(remove(bare, "b"), "types = [\n    \"a\"\n]\n");
        assert_eq!(remove(comma, "b"), "types = [\n    \"a\",\n]\n");
        assert_eq!(remove(bare, "a"), "types = [\n    \"b\"\n]\n");
        assert_eq!(remove(comma, "a"), "types = [\n    \"b\",\n]\n");
        // A comment on the last entry's line stays on it; one above the
        // entry removed stays above the list's end.
        let noted = "types = [\n    \"a\",\n    \"b\" # B\n]\n";
        assert_eq!(
            add(noted),
            "types = [\n    \"a\",\n    \"b\", # B\n    \"c\"\n]\n"
        );
        let noted = "types = [\n    \"a\",\n    \"b\", # B\n]\n";
        assert_eq!(
            add(noted),
            "types = [\n    \"a\",\n    \"b\", # B\n    \"c\",\n]\n"
        );
        let above = "types = [\n    \"a\",\n    # B\n    \"b\"\n]\n";
        assert_eq!(remove(above, "b"), "types = [\n    \"a\"\n    # B\n]\n");
        let only = edited("types = [\n    \"a\"\n]\n", |a| remove_where(a, |_| true));
        assert_eq!(only, "types = [\n]\n");
        // On one line, as before.
        assert_eq!(remove("types = [\"a\", \"b\"]\n", "b"), "types = [\"a\"]\n");
    }

    #[test]
    fn a_list_the_file_does_not_set_is_written_whole() {
        let text = "# Help.\n# types = [\"x\"]\n";
        let set = edited(text, |a| push_like_last(a, "y"));
        assert_eq!(set, "# Help.\ntypes = [\"y\"]\n");
        assert!(with_array("types = 1\n", "types", Array::new(), |_| Ok(true)).is_err());
        // Unset, the list starts as what the file stands for.
        let unset: Array = ["x"].into_iter().collect();
        let added = with_array(text, "types", unset, |a| {
            push_like_last(a, "y");
            Ok(true)
        });
        assert_eq!(added.unwrap().unwrap(), "# Help.\ntypes = [\"x\", \"y\"]\n");
    }

    /// A comment after an entry that starts its line is that entry's: it
    /// goes with it, not onto the line before (`[ # a`), and a comment above
    /// it stays.
    #[test]
    fn a_comment_on_the_line_of_an_entry_removed_goes_with_it() {
        let remove = |text: &str, name: &'static str| {
            edited(text, move |a| remove_where(a, |v| v.as_str() == Some(name)))
        };
        let first = "types = [\n    \"a\", # A\n    \"b\",\n]\n";
        assert_eq!(remove(first, "a"), "types = [\n    \"b\",\n]\n");
        let middle = "types = [\n    \"a\",\n    # Above.\n    \"b\", # B\n    \"c\",\n]\n";
        assert_eq!(
            remove(middle, "b"),
            "types = [\n    \"a\",\n    # Above.\n    \"c\",\n]\n"
        );
        let comma = "types = [\n    \"a\",\n    \"b\", # B\n]\n";
        assert_eq!(remove(comma, "b"), "types = [\n    \"a\",\n]\n");
        let bare = "types = [\n    \"a\",\n    \"b\" # B\n]\n";
        assert_eq!(remove(bare, "b"), "types = [\n    \"a\"\n]\n");
        // Second on its line, the comment is the line's: it stays.
        let shared = "types = [\n    \"a\", \"b\", # AB\n    \"c\",\n]\n";
        assert_eq!(
            remove(shared, "b"),
            "types = [\n    \"a\", # AB\n    \"c\",\n]\n"
        );
    }

    /// Emptied, then given an entry: on the list's line, or below the
    /// comments the list still holds.
    #[test]
    fn an_entry_added_to_an_emptied_list_is_laid_out_as_its_only_one() {
        let refill = |text: &str| {
            edited(text, |a| {
                remove_where(a, |_| true);
                push_like_last(a, "md");
            })
        };
        let list = "types = [\n    \"a\",\n    \"b\",\n]\n";
        assert_eq!(refill(list), "types = [\"md\"]\n");
        let bare = "types = [\n    \"a\"\n]\n";
        assert_eq!(refill(bare), "types = [\"md\"]\n");
        let noted = "types = [\n    # Mine.\n    \"a\",\n]\n";
        assert_eq!(refill(noted), "types = [\n    # Mine.\n    \"md\",\n]\n");
        let bare_noted = "types = [\n    # Mine.\n    \"a\"\n]\n";
        assert_eq!(
            refill(bare_noted),
            "types = [\n    # Mine.\n    \"md\"\n]\n"
        );
    }

    /// After a last entry without a comma and followed by a comment, an
    /// entry added stays out of the comment: the file still holds it.
    #[test]
    fn an_entry_added_after_a_comment_is_kept_out_of_it() {
        let add = |text: &str| {
            let added = edited(text, |a| push_like_last(a, "c"));
            let read: toml::Table = toml::from_str(&added).unwrap();
            let types = read["types"].as_array().unwrap();
            assert_eq!(types.last().and_then(|v| v.as_str()), Some("c"), "{added}");
            added
        };
        // On one line, the comment ending it follows the new entry.
        assert_eq!(
            add("types = [\"a\", \"b\" # B\n]\n"),
            "types = [\"a\", \"b\", \"c\" # B\n]\n"
        );
        assert_eq!(
            add("types = [\"a\" # A\n    # Later.\n]\n"),
            "types = [\"a\", \"c\" # A\n    # Later.\n]\n"
        );
        // One entry per line: below the comments that follow the last.
        assert_eq!(
            add("types = [\n    \"a\",\n    \"b\"\n    # Later.\n]\n"),
            "types = [\n    \"a\",\n    \"b\",\n    # Later.\n    \"c\"\n]\n"
        );
        assert_eq!(
            add("types = [\n    \"a\",\n    \"b\" # B\n    # Later.\n]\n"),
            "types = [\n    \"a\",\n    \"b\", # B\n    # Later.\n    \"c\"\n]\n"
        );
        assert_eq!(
            add("types = [ # Head.\n    \"a\" # A\n]\n"),
            "types = [ # Head.\n    \"a\", # A\n    \"c\"\n]\n"
        );
        // Empty lists, with or without comments.
        assert_eq!(add("types = [] # After.\n"), "types = [\"c\"] # After.\n");
        assert_eq!(
            add("types = [\n    # Mine.\n]\n"),
            "types = [\n    # Mine.\n    \"c\"\n]\n"
        );
        add("types = [ # Head.\n]\n");
    }

    #[test]
    fn a_list_left_as_it_was_or_refused_is_not_written() {
        let same = with_array(LIST, "types", Array::new(), |_| Ok(false));
        assert_eq!(same, Ok(None));
        let refused = with_array(LIST, "types", Array::new(), |_| Err("non".into()));
        assert_eq!(refused, Err("non".to_string()));
        let dir = herdr_api::TempDir::new("menu-store-unchanged").unwrap();
        let path = dir.join("a.toml");
        let same = edit_array(&path, LIST, "types", Array::new(), |_| Ok(false));
        assert_eq!(same, Ok(false));
        assert!(!path.exists());
    }
}
