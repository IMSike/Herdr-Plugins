//! Writes one setting into a TOML file and keeps the rest of it as it is:
//! comments, order, entries.

use std::path::Path;
use toml_edit::{DocumentMut, Item, Value};

/// Sets `key` at the top level of the file at `path`, created from
/// `template` when it does not exist.
pub fn set(path: &Path, template: &str, key: &str, value: Value) -> Result<(), String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => template.to_string(),
        Err(e) => return Err(format!("{} : {e}", path.display())),
    };
    let text = with_value(&text, key, value)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    }
    herdr_api::write_atomic(path, text.as_bytes()).map_err(|e| format!("{} : {e}", path.display()))
}

/// `text` with `key` set: in place when the file sets it, on its
/// commented-out line (`# key = …`, before any table) when it has one, else
/// added to the top level.
pub fn with_value(text: &str, key: &str, value: Value) -> Result<String, String> {
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string().trim().to_string())?;
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
        .position(|l| l.trim_start().starts_with('['))
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
        let text = "# Help.\n# layout = \"double\"\n\n# [[entries]]\n# key = \"l\"\n";
        let set = with_value(text, "layout", "single".into()).unwrap();
        assert_eq!(
            set,
            "# Help.\nlayout = \"single\"\n\n# [[entries]]\n# key = \"l\"\n"
        );
        // `key` is only commented out below the entries' example: added.
        let added = with_value(text, "key", "x".into()).unwrap();
        assert!(added.contains("\nkey = \"x\""));
    }

    #[test]
    fn adds_before_the_tables() {
        let text = "[[entries]]\nkey = \"l\"\n";
        let set = with_value(text, "ttl_seconds", 30.into()).unwrap();
        let ttl = set.find("ttl_seconds").unwrap();
        assert!(ttl < set.find("[[entries]]").unwrap());
        assert!(with_value("a = [", "b", 1.into()).is_err());
    }
}
