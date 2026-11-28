//! The plugins a repository holds beside one installed from it, and that
//! are not installed: what the update offers to install, a plugin added to
//! a repository after the others were installed. Herdr clones the whole
//! repository of a plugin installed from one of its folders (`plugin.list`
//! gives the clone, `managed_path`, and the folder, `subdir`): the other
//! folders are read there, without asking GitHub. A plugin the user did
//! not want, left unticked, is not offered again until the settings say so.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The plugins left unticked, by id, in the plugin's state folder.
const DECLINED: &str = "declined-plugins.json";
/// A plugin's manifest, in its folder.
const MANIFEST: &str = "herdr-plugin.toml";

/// A plugin of a repository, not installed, that the update offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewPlugin {
    pub id: String,
    pub name: String,
    /// What `herdr plugin install` takes: `owner/repo/folder`.
    pub source: String,
    /// The branch or tag the plugin it was found beside was installed
    /// from, to install it from the same; `None` for the default branch.
    pub git_ref: Option<String>,
}

impl NewPlugin {
    /// The arguments of `herdr` that install it.
    pub fn install_args(&self) -> Vec<&str> {
        let mut args = vec!["plugin", "install", &self.source];
        if let Some(git_ref) = &self.git_ref {
            args.extend(["--ref", git_ref]);
        }
        args
    }
}

/// A plugin installed from a folder of a GitHub repository.
#[derive(Debug)]
pub struct Origin<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    /// The repository's clone.
    pub clone: &'a Path,
    /// The plugin's folder in it, `/`-separated.
    pub subdir: &'a str,
    pub git_ref: Option<&'a str>,
}

/// What the update reads of a manifest.
#[derive(Debug, Deserialize)]
struct Manifest {
    id: String,
    name: Option<String>,
    /// Every system when missing, as Herdr reads it.
    platforms: Option<Vec<String>>,
}

/// The plugins beside each of `origins`, in the same folder of their
/// repository, for this system, that are neither `installed` nor
/// `declined`: each once, by folder. A folder that cannot be read, or whose
/// manifest cannot, is left out.
pub fn beside(origins: &[Origin], installed: &[String], declined: &[String]) -> Vec<NewPlugin> {
    let mut found: Vec<NewPlugin> = Vec::new();
    for origin in origins {
        let Some((parent, prefix)) = parent_of(origin.subdir) else {
            continue;
        };
        let Ok(entries) = std::fs::read_dir(origin.clone.join(&parent)) else {
            continue;
        };
        let mut folders: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| !name.starts_with('.') && !name.chars().any(char::is_control))
            .collect();
        folders.sort();
        for folder in folders {
            let manifest = origin.clone.join(&parent).join(&folder).join(MANIFEST);
            // Neither an id nor a folder holding a control character, which
            // would act on the terminal where they are printed.
            let Some(manifest) = read_manifest(&manifest)
                .filter(|m| !m.id.is_empty() && !m.id.chars().any(char::is_control))
            else {
                continue;
            };
            let known = |id: &String| *id == manifest.id;
            if installed.iter().any(known)
                || declined.iter().any(known)
                || found.iter().any(|p| p.id == manifest.id)
                || !for_this_system(manifest.platforms.as_deref())
            {
                continue;
            }
            found.push(NewPlugin {
                name: (manifest.name.as_deref())
                    .map(|name| name.chars().filter(|c| !c.is_control()).collect::<String>())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| manifest.id.clone()),
                id: manifest.id,
                source: format!("{}/{}/{prefix}{folder}", origin.owner, origin.repo),
                git_ref: origin.git_ref.map(str::to_string),
            });
        }
    }
    found
}

/// The folder holding a plugin's folder `subdir`, in its repository, and
/// the prefix of its plugins' sources: `("", "")` for `menu`, `("a", "a/")`
/// for `a/menu`. `None` for a plugin at the repository's root, which has no
/// folder beside it, or a folder that leaves the clone.
fn parent_of(subdir: &str) -> Option<(PathBuf, String)> {
    let parts: Vec<&str> = subdir
        .split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    if parts.is_empty() || parts.contains(&"..") {
        return None;
    }
    let parent = &parts[..parts.len() - 1];
    let prefix: String = parent.iter().map(|p| format!("{p}/")).collect();
    Some((parent.iter().collect(), prefix))
}

fn read_manifest(path: &Path) -> Option<Manifest> {
    toml::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Whether a manifest's `platforms` holds this system.
fn for_this_system(platforms: Option<&[String]>) -> bool {
    platforms.is_none_or(|p| p.iter().any(|s| s == std::env::consts::OS))
}

fn declined_file() -> Option<PathBuf> {
    Some(herdr_api::state_dir().ok()?.join(DECLINED))
}

/// The ids of the plugins left unticked.
pub fn declined() -> Vec<String> {
    declined_file()
        .and_then(|file| herdr_api::read_json(&file))
        .unwrap_or_default()
}

/// Keeps `ids` among the plugins left unticked.
pub fn decline(ids: &[String]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let file = declined_file().ok_or("dossier d'état introuvable")?;
    let mut declined = declined();
    for id in ids {
        if !declined.contains(id) {
            declined.push(id.clone());
        }
    }
    herdr_api::write_json(&file, &declined).map_err(|e| format!("{DECLINED} : {e}"))
}

/// Forgets the plugins left unticked; returns their ids.
pub fn forget_declined() -> Result<Vec<String>, String> {
    let declined = declined();
    if let Some(file) = declined_file() {
        match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(format!("{DECLINED} : {e}"));
            }
            _ => {}
        }
    }
    Ok(declined)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(dir: &Path, folder: &str, manifest: &str) {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
        std::fs::write(dir.join(folder).join(MANIFEST), manifest).unwrap();
    }

    #[test]
    fn finds_the_plugins_beside_one_installed() {
        let dir = herdr_api::TempDir::new("menu-discover").unwrap();
        let os = std::env::consts::OS;
        plugin(&dir, "menu", "id = \"o.menu\"\nname = \"Menu\"\n");
        plugin(
            &dir,
            "links",
            &format!("id = \"o.links\"\nname = \"Links\"\nplatforms = [\"{os}\"]\n"),
        );
        plugin(
            &dir,
            "other",
            "id = \"o.other\"\nplatforms = [\"nowhere\"]\n",
        );
        plugin(&dir, "sidebar", "id = \"o.sidebar\"\n");
        plugin(&dir, "broken", "id = \n");
        plugin(&dir, "escape", "id = \"o.\\u001b[2J\"\n");
        plugin(&dir, "zname", "id = \"o.z\"\nname = \"Z\\u001b[2J\"\n");
        plugin(&dir, ".github", "id = \"o.hidden\"\n");
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        let origin = |subdir| Origin {
            owner: "o",
            repo: "r",
            clone: &dir,
            subdir,
            git_ref: Some("dev"),
        };
        let installed = ["o.menu".to_string()];
        let found = beside(&[origin("menu"), origin("menu/")], &installed, &[]);
        assert_eq!(
            found,
            [
                NewPlugin {
                    id: "o.links".into(),
                    name: "Links".into(),
                    source: "o/r/links".into(),
                    git_ref: Some("dev".into()),
                },
                NewPlugin {
                    id: "o.sidebar".into(),
                    name: "o.sidebar".into(),
                    source: "o/r/sidebar".into(),
                    git_ref: Some("dev".into()),
                },
                NewPlugin {
                    id: "o.z".into(),
                    name: "Z[2J".into(),
                    source: "o/r/zname".into(),
                    git_ref: Some("dev".into()),
                },
            ]
        );
        assert_eq!(
            found[0].install_args(),
            ["plugin", "install", "o/r/links", "--ref", "dev"]
        );
        // Declined, it is no longer offered.
        let found = beside(&[origin("menu")], &installed, &["o.links".into()]);
        assert_eq!(found.len(), 2);
        // A plugin at the root has no folder beside it.
        assert!(beside(&[origin("")], &installed, &[]).is_empty());
    }

    #[test]
    fn a_plugin_s_folder_gives_its_parent_and_prefix() {
        assert_eq!(parent_of("menu"), Some((PathBuf::new(), String::new())));
        assert_eq!(
            parent_of("plugins/menu"),
            Some((PathBuf::from("plugins"), "plugins/".into()))
        );
        assert_eq!(parent_of(""), None);
        assert_eq!(parent_of("../menu"), None);
    }
}
