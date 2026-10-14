
use std::path::Path;
use std::process::Command;
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::instance::{self, Instance};
use crate::paths;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct World {
    pub folder: String,
    pub name: Option<String>,
    pub size: u64,
    pub modified_ms: u64,
}

pub(crate) fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

fn millis(t: std::io::Result<std::time::SystemTime>) -> u64 {
    t.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

pub fn list_worlds(instance_id: &str) -> Result<Vec<World>> {
    instance::check_id(instance_id)?;
    let mut worlds: Vec<World> = std::fs::read_dir(paths::game_dir(instance_id).join("saves"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("level.dat").is_file())
        .map(|e| {
            let path = e.path();
            World {
                folder: e.file_name().to_string_lossy().into_owned(),
                name: crate::servers::level_info(&path).map(|(n, ..)| n),
                size: dir_size(&path),
                modified_ms: millis(std::fs::metadata(path.join("level.dat")).and_then(|m| m.modified())),
            }
        })
        .collect();
    worlds.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    Ok(worlds)
}

pub fn open_folder(instance_id: &str, which: &str) -> Result<()> {
    instance::check_id(instance_id)?;
    let game = paths::game_dir(instance_id);
    let dir = match which {
        "game" => game,
        "mods" => game.join("mods"),
        "saves" => game.join("saves"),
        _ => return Err(Error::msg("invalid folder")),
    };
    open_path(&dir)
}

pub fn open_root() -> Result<()> {
    open_path(&paths::root())
}

pub fn open_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|_| Error::msg("invalid link"))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err(Error::msg("Only https links can be opened"));
    }
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(opener)
        .arg(parsed.as_str())
        .spawn()
        .map_err(|e| Error::msg(format!("Couldn't open the link ({opener}): {e}")))?;
    Ok(())
}

pub fn open_path(dir: &Path) -> Result<()> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(opener)
        .arg(dir)
        .spawn()
        .map_err(|e| Error::msg(format!("Couldn't open the folder ({opener}): {e}")))?;
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let (from, to) = (entry.path(), dst.join(entry.file_name()));
        match entry.file_type()? {
            t if t.is_dir() => copy_dir(&from, &to)?,
            t if t.is_file() => {
                std::fs::copy(&from, &to)?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn duplicate(instance_id: &str) -> Result<Instance> {
    let original = instance::get(instance_id)?;
    let mut copy = instance::create(
        &format!("{} (copia)", original.name),
        &original.mc_version,
        original.loader,
        original.loader_version.clone(),
        original.launch_version.clone(),
    )?;
    copy.memory_max_mb = original.memory_max_mb;
    copy.memory_min_mb = original.memory_min_mb;
    copy.flag_profile = original.flag_profile;
    copy.java_path = original.java_path.clone();
    copy.extra_jvm_args = original.extra_jvm_args.clone();
    copy.icon_url = original.icon_url.clone();
    copy.banner_url = original.banner_url.clone();
    instance::save(&copy)?;

    let (from, to) = (paths::game_dir(&original.id), paths::game_dir(&copy.id));
    if let Err(e) = copy_dir(&from, &to) {
        let _ = instance::delete(&copy.id);
        return Err(e);
    }
    Ok(copy)
}
