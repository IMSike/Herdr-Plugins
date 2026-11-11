//! The title of a Claude Code conversation, to tell one agent from another:
//! the last `ai-title` its transcript holds (`<projects>/<folder>/<id>.jsonl`,
//! a line of JSON per event), else its last prompt (`last-prompt`).
//!
//! The transcript is Claude Code's own file, not a documented one: when it
//! moves or changes, no title is found, and the conversation's id shows
//! alone. Nothing is written, and only these two kinds of lines are read.

use serde::Deserialize;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// The folder of Claude Code's settings, which Claude Code reads from
/// `CLAUDE_CONFIG_DIR` when set.
const CONFIG_ENV: &str = "CLAUDE_CONFIG_DIR";

/// The title of the conversation `id`, if its transcript is found.
pub fn title(id: &str) -> Option<String> {
    title_in(&projects_dir()?, id)
}

fn projects_dir() -> Option<PathBuf> {
    let config = std::env::var_os(CONFIG_ENV)
        .map(PathBuf::from)
        .or_else(|| std::env::home_dir().map(|home| home.join(".claude")))?;
    Some(config.join("projects"))
}

/// The title of `id` among the projects of `projects`.
fn title_in(projects: &Path, id: &str) -> Option<String> {
    // An id read from a file names no other file.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let file = format!("{id}.jsonl");
    std::fs::read_dir(projects)
        .ok()?
        .flatten()
        .map(|project| project.path().join(&file))
        .find(|path| path.is_file())
        .and_then(|path| read_title(&path))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Line {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    ai_title: Option<String>,
    #[serde(default)]
    last_prompt: Option<String>,
}

/// The last title of a transcript, else its last prompt, on one line.
fn read_title(path: &Path) -> Option<String> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    let (mut title, mut prompt) = (None, None);
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        // Only these lines are parsed: the others can be large.
        if !contains(&line, b"\"ai-title\"") && !contains(&line, b"\"last-prompt\"") {
            continue;
        }
        let Ok(parsed) = serde_json::from_slice::<Line>(&line) else {
            continue;
        };
        match parsed.kind.as_str() {
            "ai-title" => title = parsed.ai_title.or(title),
            "last-prompt" => prompt = parsed.last_prompt.or(prompt),
            _ => {}
        }
    }
    title
        .or(prompt)
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|text| !text.is_empty())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_last_title_else_the_last_prompt() {
        let dir = herdr_api::TempDir::new("menu-claude").unwrap();
        let project = dir.join("C--src-app");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            project.join("1234-abcd.jsonl"),
            concat!(
                "{\"type\":\"user\",\"message\":\"\\\"ai-title\\\"\"}\n",
                "{\"type\":\"last-prompt\",\"lastPrompt\":\"Corrige le\\ntest\"}\n",
                "{\"type\":\"ai-title\",\"aiTitle\":\"Premier titre\"}\n",
                "{\"type\":\"ai-title\",\"aiTitle\":\"Titre final\"}\n",
                "pas du JSON \"ai-title\"\n",
            ),
        )
        .unwrap();
        std::fs::write(
            project.join("5678-ef.jsonl"),
            "{\"type\":\"last-prompt\",\"lastPrompt\":\"Corrige le\\ntest\"}",
        )
        .unwrap();
        assert_eq!(title_in(&dir, "1234-abcd").as_deref(), Some("Titre final"));
        assert_eq!(
            title_in(&dir, "5678-ef").as_deref(),
            Some("Corrige le test")
        );
        assert_eq!(title_in(&dir, "9999"), None);
        assert_eq!(title_in(&dir, "../C--src-app/1234-abcd"), None);
        assert_eq!(title_in(&dir.join("none"), "1234-abcd"), None);
    }
}
