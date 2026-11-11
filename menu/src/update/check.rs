//! Whether Herdr or a plugin installed from GitHub has a new version: what
//! the `Update` entry offers, and what turns its icon green. Herdr's latest
//! stable version is read from the file its own check reads
//! (`herdr.dev/latest.json`); a plugin's latest commit from its repository
//! (`git ls-remote`), on the branch or tag it was installed from. The
//! result is kept in the plugin's state folder for every `Tools` pane, and
//! checked again at Herdr's start and every `CHECK_EVERY`.

use crate::herdr;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How often the versions are checked again.
pub const CHECK_EVERY: Duration = Duration::from_secs(4 * 3600);
/// The file of the last check, in the plugin's state folder.
const FILE: &str = "updates.json";
/// Held by the check that runs: one at a time, for every pane.
const LOCK: &str = "updates.lock";
/// Herdr's latest stable version, as its own check reads it.
const LATEST_URL: &str = "https://herdr.dev/latest.json";
/// How long a download or a repository may take to answer.
const ANSWER_WITHIN: Duration = Duration::from_secs(20);

/// Herdr's version, as the last check found it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Herdr {
    Newer {
        installed: String,
        latest: String,
    },
    UpToDate,
    /// Not known: why (the preview channel, no answer…).
    Unknown(String),
}

/// A plugin installed from GitHub that has a new commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
}

/// What can be updated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Found {
    pub herdr: Herdr,
    pub plugins: Vec<Plugin>,
}

impl Found {
    /// Whether a new version is known.
    pub fn any(&self) -> bool {
        matches!(self.herdr, Herdr::Newer { .. }) || !self.plugins.is_empty()
    }

    /// `Herdr 0.9.6 · Explorer`: what has a new version.
    pub fn summary(&self) -> String {
        let herdr = match &self.herdr {
            Herdr::Newer { latest, .. } => Some(format!("Herdr {latest}")),
            _ => None,
        };
        herdr
            .into_iter()
            .chain(self.plugins.iter().map(|p| p.name.clone()))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// Without what was updated: Herdr when `herdr`, and `plugins` (ids).
    pub fn without(mut self, herdr: bool, plugins: &[String]) -> Self {
        if herdr {
            self.herdr = Herdr::UpToDate;
        }
        self.plugins.retain(|p| !plugins.contains(&p.id));
        self
    }
}

/// A check, and when it ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checked {
    /// Seconds since the Unix epoch.
    pub at: u64,
    pub found: Found,
}

impl Checked {
    /// Whether it is time to check again.
    pub fn is_due(&self) -> bool {
        self.is_due_at(now())
    }

    /// Whether it is time to check again at `now`: `CHECK_EVERY` later, or
    /// at once when it is dated in the future (the clock was set back).
    fn is_due_at(&self, now: u64) -> bool {
        now.checked_sub(self.at)
            .is_none_or(|age| age >= CHECK_EVERY.as_secs())
    }
}

fn now() -> u64 {
    herdr_api::unix_millis() / 1000
}

fn file() -> Result<PathBuf, String> {
    Ok(herdr_api::state_dir()?.join(FILE))
}

/// The last check; `None` before the first one.
pub fn last() -> Option<Checked> {
    herdr_api::read_json(&file().ok()?)
}

/// The file of the last check, whose changes a pane watches.
pub fn last_file() -> Option<PathBuf> {
    file().ok()
}

/// Keeps `found` as the last check, made now.
pub fn keep(found: Found) -> Result<(), String> {
    let checked = Checked { at: now(), found };
    herdr_api::write_json(&file()?, &checked).map_err(|e| e.to_string())
}

/// Checks when the last check is `CHECK_EVERY` old, or `always`, unless
/// another process checks meanwhile: it keeps its result.
pub fn when_due(always: bool) {
    let Ok(lock) = file().map(|f| f.with_file_name(LOCK)) else {
        return;
    };
    let Ok(Some(_lock)) = herdr_api::Lock::take(&lock) else {
        return;
    };
    if always || last().is_none_or(|c| c.is_due()) {
        let _ = keep(check());
    }
}

/// Checks Herdr and the plugins installed from GitHub, now.
pub fn check() -> Found {
    Found {
        herdr: herdr_version(),
        plugins: plugins().unwrap_or_default(),
    }
}

/// Herdr's installed version against the latest stable one.
fn herdr_version() -> Herdr {
    let status = match status() {
        Ok(status) => status,
        Err(e) => return Herdr::Unknown(e),
    };
    let client = &status["client"];
    if client["channel"].as_str().is_some_and(|c| c != "stable") {
        return Herdr::Unknown("canal preview".into());
    }
    let Some(installed) = client["version"].as_str() else {
        return Herdr::Unknown("version installée inconnue".into());
    };
    let latest = match latest_version() {
        Ok(latest) => latest,
        Err(e) => return Herdr::Unknown(e),
    };
    if is_newer(&latest, installed) {
        Herdr::Newer {
            installed: installed.into(),
            latest,
        }
    } else {
        Herdr::UpToDate
    }
}

/// `herdr status --json`.
fn status() -> Result<Value, String> {
    let mut command = herdr::command(&["status", "--json"]);
    let text = output_within(&mut command, ANSWER_WITHIN)
        .map_err(|e| format!("« herdr status » : {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("état de Herdr illisible : {e}"))
}

/// The `version` of `latest.json`.
fn latest_version() -> Result<String, String> {
    let mut command = Command::new("curl");
    let seconds = ANSWER_WITHIN.as_secs().to_string();
    command.args(["-fsSL", "--max-time", &seconds, LATEST_URL]);
    let text = output_within(&mut command, ANSWER_WITHIN + Duration::from_secs(5))
        .map_err(|e| format!("{LATEST_URL} : {e}"))?;
    let latest: Value =
        serde_json::from_str(&text).map_err(|e| format!("{LATEST_URL} illisible : {e}"))?;
    latest["version"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{LATEST_URL} : pas de version"))
}

/// Whether `latest` comes after `installed`: `0.10.0` after `0.9.5`. A
/// version that cannot be read is not newer.
fn is_newer(latest: &str, installed: &str) -> bool {
    let numbers = |version: &str| -> Option<Vec<u64>> {
        let core = version.trim_start_matches('v');
        let core = core.split(['-', '+']).next()?;
        core.split('.').map(|n| n.parse().ok()).collect()
    };
    match (numbers(latest), numbers(installed)) {
        (Some(latest), Some(installed)) => latest > installed,
        _ => false,
    }
}

/// What `plugin.list` tells of a plugin's source.
#[derive(Debug, Deserialize)]
struct Source {
    kind: String,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    repo: String,
    #[serde(default)]
    requested_ref: Option<String>,
    #[serde(default)]
    resolved_commit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Installed {
    plugin_id: String,
    name: String,
    #[serde(default)]
    source: Option<Source>,
}

/// The plugins installed from GitHub whose ref has moved since.
fn plugins() -> Result<Vec<Plugin>, String> {
    let installed: Vec<Installed> = herdr_api::request_field("plugin.list", json!({}), "plugins")?;
    Ok(installed
        .into_iter()
        .filter(|p| {
            p.source
                .as_ref()
                .is_some_and(|s| s.kind == "github" && has_moved(s))
        })
        .map(|p| Plugin {
            id: p.plugin_id,
            name: p.name,
        })
        .collect())
}

/// Whether the ref a plugin was installed from now names another commit. A
/// commit installed by its hash never moves; an unanswered repository is
/// not known to have moved.
fn has_moved(source: &Source) -> bool {
    let Some(installed) = &source.resolved_commit else {
        return false;
    };
    let url = format!("https://github.com/{}/{}.git", source.owner, source.repo);
    let names = ref_names(source.requested_ref.as_deref());
    let mut command = Command::new("git");
    // No prompt, nor credentials window, for a repository gone private.
    command
        .args(["-c", "credential.helper=", "ls-remote", &url])
        .args(&names)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never");
    output_within(&mut command, ANSWER_WITHIN)
        .ok()
        .and_then(|text| commit_of(&text, &names))
        .is_some_and(|latest| latest != *installed)
}

/// The full names a ref may stand for, by preference: the default branch
/// (`HEAD`); a branch, then a tag's commit, then the tag; a full name as
/// it is.
fn ref_names(wanted: Option<&str>) -> Vec<String> {
    match wanted {
        None => vec!["HEAD".into()],
        Some(full) if full.starts_with("refs/") => vec![format!("{full}^{{}}"), full.into()],
        Some(name) => vec![
            format!("refs/heads/{name}"),
            format!("refs/tags/{name}^{{}}"),
            format!("refs/tags/{name}"),
        ],
    }
}

/// The commit of the first of `names` that `git ls-remote` listed: only
/// a name in full, never one that merely ends like it.
fn commit_of(listing: &str, names: &[String]) -> Option<String> {
    let lines: Vec<(&str, &str)> = listing
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .collect();
    names.iter().find_map(|name| {
        lines
            .iter()
            .find(|(_, listed)| listed == name)
            .map(|(commit, _)| commit.to_string())
    })
}

/// What `command` writes, once it ended well within `limit`; it is stopped
/// past that. No window opens for it on Windows: the check also runs from
/// Herdr's start, without a console.
fn output_within(command: &mut Command, limit: Duration) -> Result<String, String> {
    crate::launch::no_window(command);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().ok_or("sortie illisible")?;
    // Read meanwhile: a full pipe would hold the program up. A program it
    // started may keep the pipe open once it ended: the reading is not
    // waited for past `limit`.
    let (send, read) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = send.send(stdout.read_to_string(&mut text).map(|_| text));
    });
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => {
                let left = deadline.saturating_duration_since(Instant::now());
                return match read.recv_timeout(left) {
                    Ok(text) => text.map_err(|e| e.to_string()),
                    Err(_) => Err("sortie illisible".into()),
                };
            }
            Some(status) => return Err(format!("échec ({status})")),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("pas de réponse".into());
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_by_their_numbers() {
        assert!(is_newer("0.10.0", "0.9.5"));
        assert!(is_newer("v1.0.0", "0.9.5"));
        assert!(!is_newer("0.9.5", "0.9.5"));
        assert!(!is_newer("0.9.4", "0.9.5"));
        assert!(!is_newer("0.9.5", "0.9.5-preview.1"));
        assert!(!is_newer("nouvelle", "0.9.5"));
    }

    #[test]
    fn a_ref_is_found_by_its_full_name_only() {
        let listing = "a\trefs/heads/x/main\nb\trefs/tags/main\nc\trefs/tags/main^{}\n";
        // A branch first, then a tag's commit rather than the tag object.
        assert_eq!(
            commit_of(listing, &ref_names(Some("main"))).as_deref(),
            Some("c")
        );
        let branch = format!("{listing}d\trefs/heads/main\n");
        assert_eq!(
            commit_of(&branch, &ref_names(Some("main"))).as_deref(),
            Some("d")
        );
        assert_eq!(
            commit_of("e\tHEAD\n", &ref_names(None)).as_deref(),
            Some("e")
        );
        assert_eq!(
            commit_of("a\trefs/heads/x/dev\n", &ref_names(Some("dev"))),
            None
        );
        assert_eq!(
            ref_names(Some("refs/tags/v1")),
            ["refs/tags/v1^{}", "refs/tags/v1"]
        );
    }

    #[test]
    fn a_check_is_due_after_four_hours_or_from_the_future() {
        let checked = |at| Checked {
            at,
            found: Found {
                herdr: Herdr::UpToDate,
                plugins: Vec::new(),
            },
        };
        let hours = |h: u64| h * 3600;
        assert!(!checked(hours(10)).is_due_at(hours(13)));
        assert!(checked(hours(10)).is_due_at(hours(14)));
        assert!(checked(hours(10)).is_due_at(hours(9)));
    }

    #[test]
    fn says_what_has_a_new_version_and_forgets_what_was_updated() {
        let found = Found {
            herdr: Herdr::Newer {
                installed: "0.9.5".into(),
                latest: "0.9.6".into(),
            },
            plugins: vec![Plugin {
                id: "gcaudan.explorer".into(),
                name: "Explorer".into(),
            }],
        };
        assert!(found.any());
        assert_eq!(found.summary(), "Herdr 0.9.6 · Explorer");
        let rest = found.clone().without(true, &[]);
        assert_eq!(rest.summary(), "Explorer");
        let none = found.without(true, &["gcaudan.explorer".into()]);
        assert!(!none.any());
        let unknown = Found {
            herdr: Herdr::Unknown("canal preview".into()),
            plugins: Vec::new(),
        };
        assert!(!unknown.any());
    }
}
