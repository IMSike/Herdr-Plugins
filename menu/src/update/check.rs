//! Whether Herdr or a plugin installed from GitHub has a new version: what
//! the `Update` entry offers, and what turns its icon green. Herdr's latest
//! stable version is read from `herdr.dev/latest.json`; a plugin's latest
//! commit from its repository (`git ls-remote`), on the branch or tag it
//! was installed from. The result is kept in the plugin's state folder for
//! every pane, and checked again at Herdr's start and every `CHECK_EVERY`.

use crate::herdr;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How often the versions are checked again.
const CHECK_EVERY: Duration = Duration::from_secs(4 * 3600);
/// The file of the last check, in the plugin's state folder.
const FILE: &str = "updates.json";
/// Held by the check that runs: one at a time, for every pane and window.
const LOCK: &str = "updates.lock";
/// How long the update window waits for a check under way to end.
const LOCK_WAIT: Duration = Duration::from_secs(60);
/// Herdr's latest stable version.
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
    /// `Herdr 0.9.6 · Explorer`: what has a new version; `None` when
    /// nothing has one.
    pub fn news(&self) -> Option<String> {
        let herdr = match &self.herdr {
            Herdr::Newer { latest, .. } => Some(format!("Herdr {latest}")),
            _ => None,
        };
        let names: Vec<String> = herdr
            .into_iter()
            .chain(self.plugins.iter().map(|p| p.name.clone()))
            .collect();
        (!names.is_empty()).then(|| names.join(" · "))
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

/// Seconds since the Unix epoch.
fn now() -> u64 {
    herdr_api::unix_millis() / 1000
}

/// The file of the last check, whose changes a pane watches.
pub fn last_file() -> Option<PathBuf> {
    Some(herdr_api::state_dir().ok()?.join(FILE))
}

/// The last check; `None` before the first one.
pub fn last() -> Option<Checked> {
    herdr_api::read_json(&last_file()?)
}

/// Checks when the last check is `CHECK_EVERY` old, or `always`, unless
/// another process checks meanwhile: it keeps its result.
pub fn when_due(always: bool) {
    let Some(lock) = last_file().map(|f| f.with_file_name(LOCK)) else {
        return;
    };
    let Ok(Some(_lock)) = herdr_api::Lock::take(&lock) else {
        return;
    };
    if always || last().is_none_or(|c| c.is_due()) {
        check_and_keep();
    }
}

/// Checks now, once the check under way, if any, has ended (or after
/// `LOCK_WAIT`): the update window's, whose result no older check may
/// overwrite.
pub fn now_and_keep() -> Found {
    let lock = last_file().map(|f| f.with_file_name(LOCK));
    let deadline = Instant::now() + LOCK_WAIT;
    let _lock = loop {
        match lock.as_deref().map(herdr_api::Lock::take) {
            Some(Ok(None)) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Some(Ok(Some(lock))) => break Some(lock),
            _ => break None,
        }
    };
    check_and_keep()
}

/// Checks Herdr and the plugins installed from GitHub, and keeps the
/// result when something answered. What did not answer keeps what the
/// last check found: offline, a new version known stays shown.
fn check_and_keep() -> Found {
    let last = last().map(|c| c.found);
    let (found, answered) = merged(last, herdr_version(), plugins());
    if answered && let Some(file) = last_file() {
        let checked = Checked {
            at: now(),
            found: found.clone(),
        };
        let _ = herdr_api::write_json(&file, &checked);
    }
    found
}

/// What a check found, `herdr` and `plugins` merged with the `last` one
/// where they did not answer; and whether anything answered.
fn merged(
    last: Option<Found>,
    (herdr, herdr_answered): (Herdr, bool),
    plugins: Result<Vec<(Plugin, Option<bool>)>, String>,
) -> (Found, bool) {
    let answered = herdr_answered || plugins.iter().flatten().any(|(_, moved)| moved.is_some());
    let herdr = match last.as_ref().map(|l| &l.herdr) {
        Some(known @ Herdr::Newer { .. }) if !herdr_answered => known.clone(),
        _ => herdr,
    };
    let was_new = |plugin: &Plugin| {
        last.as_ref()
            .is_some_and(|l| l.plugins.iter().any(|p| p.id == plugin.id))
    };
    let plugins = match plugins {
        Ok(plugins) => plugins
            .into_iter()
            .filter(|(plugin, moved)| moved.unwrap_or_else(|| was_new(plugin)))
            .map(|(plugin, _)| plugin)
            .collect(),
        Err(_) => last.map(|l| l.plugins).unwrap_or_default(),
    };
    (Found { herdr, plugins }, answered)
}

/// Herdr's installed version against the latest stable one, and whether
/// they could be read (the preview channel, which is not compared, counts).
fn herdr_version() -> (Herdr, bool) {
    let status = match status() {
        Ok(status) => status,
        Err(e) => return (Herdr::Unknown(e), false),
    };
    let client = &status["client"];
    if client["channel"].as_str().is_some_and(|c| c != "stable") {
        return (Herdr::Unknown("canal preview".into()), true);
    }
    let Some(installed) = client["version"].as_str() else {
        return (Herdr::Unknown("version installée inconnue".into()), true);
    };
    match latest_version() {
        Ok(latest) if is_newer(&latest, installed) => (
            Herdr::Newer {
                installed: installed.into(),
                latest,
            },
            true,
        ),
        Ok(_) => (Herdr::UpToDate, true),
        Err(e) => (Herdr::Unknown(e), false),
    }
}

/// `herdr status --json`.
pub fn status() -> Result<Value, String> {
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

/// Whether `latest` comes after `installed`: `0.10.0` after `0.9.5`, a
/// missing number counting as 0. A version that cannot be read is not newer.
fn is_newer(latest: &str, installed: &str) -> bool {
    let numbers = |version: &str| -> Option<Vec<u64>> {
        let core = version.trim().trim_start_matches('v');
        let core = core.split(['-', '+']).next()?;
        core.split('.').map(|n| n.parse().ok()).collect()
    };
    let (Some(mut latest), Some(mut installed)) = (numbers(latest), numbers(installed)) else {
        return false;
    };
    let len = latest.len().max(installed.len());
    latest.resize(len, 0);
    installed.resize(len, 0);
    latest > installed
}

/// What `plugin.list` tells of a plugin's source.
#[derive(Debug, Deserialize)]
struct Source {
    kind: String,
    owner: Option<String>,
    repo: Option<String>,
    /// Missing for the default branch.
    requested_ref: Option<String>,
    resolved_commit: Option<String>,
}

/// A plugin as `plugin.list` gives it.
#[derive(Debug, Deserialize)]
struct Installed {
    plugin_id: String,
    name: Option<String>,
    source: Option<Source>,
}

/// The plugins installed from GitHub, each with whether its branch or tag
/// now names another commit (`None`: no answer). The repositories are
/// asked all at once.
fn plugins() -> Result<Vec<(Plugin, Option<bool>)>, String> {
    let listed: Vec<Value> = herdr_api::request_field("plugin.list", json!({}), "plugins")?;
    // An entry Herdr describes otherwise (its manifest gone…) is left out.
    let from_github: Vec<(Plugin, Source)> = listed
        .into_iter()
        .filter_map(|p| serde_json::from_value::<Installed>(p).ok())
        .filter_map(|p| {
            let source = p.source.filter(|s| s.kind == "github")?;
            let plugin = Plugin {
                name: p.name.unwrap_or_else(|| p.plugin_id.clone()),
                id: p.plugin_id,
            };
            Some((plugin, source))
        })
        .collect();
    Ok(std::thread::scope(|scope| {
        let asked: Vec<_> = from_github
            .iter()
            .map(|(_, source)| scope.spawn(|| has_moved(source)))
            .collect();
        from_github
            .iter()
            .zip(asked)
            .map(|((plugin, _), moved)| (plugin.clone(), moved.join().ok().flatten()))
            .collect()
    }))
}

/// Whether the branch or tag a plugin was installed from now names another
/// commit; `None` when the repository did not answer. A plugin installed by
/// a commit's hash never moves.
fn has_moved(source: &Source) -> Option<bool> {
    let (Some(owner), Some(repo), Some(installed)) =
        (&source.owner, &source.repo, &source.resolved_commit)
    else {
        return Some(false);
    };
    let url = format!("https://github.com/{owner}/{repo}.git");
    let names = ref_names(source.requested_ref.as_deref());
    let mut command = Command::new("git");
    // No prompt, nor credentials window, for a repository gone private.
    command
        .args([
            "-c",
            "credential.helper=",
            "-c",
            "core.askPass=",
            "ls-remote",
            &url,
        ])
        .args(&names)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("GCM_INTERACTIVE", "never");
    // Not from the pane's folder: the configuration of a repository found
    // there could run a command, or send the request elsewhere.
    let neutral = herdr_api::state_dir()
        .ok()
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    command
        .current_dir(&neutral)
        .env("GIT_CEILING_DIRECTORIES", &neutral);
    let listing = output_within(&mut command, ANSWER_WITHIN).ok()?;
    Some(commit_of(&listing, &names).is_some_and(|latest| latest != *installed))
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
        assert!(is_newer(" 0.9.6\n", "0.9.5"));
        assert!(!is_newer("0.10.0", "0.10"));
        assert!(is_newer("0.10.1", "0.10"));
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
    fn says_what_has_a_new_version() {
        let explorer = Plugin {
            id: "gcaudan.explorer".into(),
            name: "Explorer".into(),
        };
        let found = |herdr, plugins| Found { herdr, plugins };
        let newer = Herdr::Newer {
            installed: "0.9.5".into(),
            latest: "0.9.6".into(),
        };
        assert_eq!(
            found(newer, vec![explorer.clone()]).news().as_deref(),
            Some("Herdr 0.9.6 · Explorer")
        );
        assert_eq!(
            found(Herdr::UpToDate, vec![explorer]).news().as_deref(),
            Some("Explorer")
        );
        assert_eq!(
            found(Herdr::Unknown("canal preview".into()), Vec::new()).news(),
            None
        );
    }

    #[test]
    fn what_did_not_answer_keeps_what_was_found() {
        let plugin = |id: &str| Plugin {
            id: id.into(),
            name: id.into(),
        };
        let newer = Herdr::Newer {
            installed: "0.9.5".into(),
            latest: "0.9.6".into(),
        };
        let last = Found {
            herdr: newer.clone(),
            plugins: vec![plugin("a"), plugin("b")],
        };
        let offline = || (Herdr::Unknown("pas de réponse".into()), false);
        // Nothing answered: all is kept, and nothing is saved.
        let (found, answered) = merged(
            Some(last.clone()),
            offline(),
            Ok(vec![(plugin("a"), None), (plugin("c"), None)]),
        );
        assert_eq!(found.herdr, newer);
        assert_eq!(found.plugins, [plugin("a")]);
        assert!(!answered);
        // What answered wins.
        let (found, answered) = merged(
            Some(last.clone()),
            (Herdr::UpToDate, true),
            Ok(vec![(plugin("a"), Some(false)), (plugin("b"), None)]),
        );
        assert_eq!(found.herdr, Herdr::UpToDate);
        assert_eq!(found.plugins, [plugin("b")]);
        assert!(answered);
        // Herdr's server unreachable: the plugins found are kept.
        let (found, _) = merged(Some(last.clone()), offline(), Err("".into()));
        assert_eq!(found, last);
        // A new version only known as unknown stays unknown.
        let (found, _) = merged(None, offline(), Ok(Vec::new()));
        assert_eq!(found.herdr, offline().0);
    }
}
