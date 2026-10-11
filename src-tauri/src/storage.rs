
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::library::dir_size;
use crate::paths;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub key: &'static str,
    pub bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Runtime {
    pub name: String,
    pub bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    pub root: String,
    pub items: Vec<Item>,
    pub runtimes: Vec<Runtime>,
}

pub fn info() -> Storage {
    let root = paths::root();
    let known = [
        ("instances", paths::instances()),
        ("versions", paths::versions()),
        ("libraries", paths::libraries()),
        ("assets", paths::assets()),
        ("runtimes", paths::runtimes()),
        ("skins", root.join("skins")),
    ];
    let mut items: Vec<Item> = known.iter().map(|(key, p)| Item { key, bytes: dir_size(p) }).collect();
    let total = dir_size(&root);
    let used: u64 = items.iter().map(|i| i.bytes).sum();
    items.push(Item { key: "other", bytes: total.saturating_sub(used) });

    let mut runtimes: Vec<Runtime> = std::fs::read_dir(paths::runtimes())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| Runtime { name: e.file_name().to_string_lossy().into_owned(), bytes: dir_size(&e.path()) })
        .filter(|r| !r.name.starts_with('.'))
        .collect();
    runtimes.sort_by(|a, b| a.name.cmp(&b.name));
    Storage { root: root.to_string_lossy().into_owned(), items, runtimes }
}

pub fn delete_runtime(name: &str) -> Result<()> {
    let ok = !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\', '\0']);
    if !ok {
        return Err(Error::msg("invalid runtime name"));
    }
    let path = paths::runtimes().join(name);
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn remove(path: &Path) -> Result<()> {
    let r = if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
    match r {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn targets(scope: &str) -> Result<Vec<PathBuf>> {
    let root = paths::root();
    Ok(match scope {
        "accounts" => vec![paths::accounts_file(), paths::microsoft_file()],
        "settings" => vec![root.join("settings.json")],
        "stats" => vec![root.join("sessions.json")],
        "instances" => vec![paths::instances(), root.join("natives")],
        "gamefiles" => vec![paths::versions(), paths::libraries(), paths::assets(), root.join("natives"), paths::manifest_cache(), root.join("tmp")],
        "runtimes" => vec![paths::runtimes()],
        "skins" => vec![root.join("skins")],
        "all" => vec![root],
        _ => return Err(Error::msg("unknown reset option")),
    })
}

pub fn reset(scope: &[String]) -> Result<()> {
    for s in scope {
        for path in targets(s)? {
            let root = paths::root();
            let inside = path == root || path.starts_with(&root);
            let named = root.file_name().is_some_and(|n| n == "PiumaLauncher");
            if !inside || !named {
                return Err(Error::msg("refusing to delete outside the launcher folder"));
            }
            remove(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_scope_is_rejected() {
        assert!(targets("../etc").is_err());
        assert!(targets("accounts").is_ok());
    }

    #[test]
    fn runtime_names_are_plain() {
        assert!(delete_runtime("../x").is_err());
        assert!(delete_runtime(".hidden").is_err());
        assert!(delete_runtime("").is_err());
    }
}
