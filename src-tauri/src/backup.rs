use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

use crate::error::{Error, Result};
use crate::{diag, instance, paths, settings};

const KEEP_AUTO: usize = 5;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub id: String,
    pub world: String,
    pub created_ms: u64,
    pub size: u64,
    pub reason: String,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub(crate) fn check_world(folder: &str) -> Result<()> {
    let ok = !folder.trim().is_empty() && !folder.starts_with('.') && !folder.contains(['/', '\\', '\0']);
    if ok { Ok(()) } else { Err(Error::msg("invalid world")) }
}

fn check_id(id: &str) -> Result<()> {
    let ok = id.ends_with(".zip") && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.') && !id.starts_with('.');
    if ok { Ok(()) } else { Err(Error::msg("invalid backup")) }
}

fn saves(instance_id: &str) -> PathBuf {
    paths::game_dir(instance_id).join("saves")
}

fn world_backups(instance_id: &str, world: &str) -> PathBuf {
    paths::backups().join(instance_id).join(world)
}

fn parse(world: &str, path: &Path) -> Option<Backup> {
    let id = path.file_name()?.to_string_lossy().into_owned();
    let (ms, reason) = id.strip_suffix(".zip")?.split_once('-')?;
    Some(Backup {
        created_ms: ms.parse().ok()?,
        reason: reason.to_string(),
        size: std::fs::metadata(path).ok()?.len(),
        world: world.to_string(),
        id,
    })
}

fn of_world(instance_id: &str, world: &str) -> Vec<Backup> {
    let mut out: Vec<Backup> = std::fs::read_dir(world_backups(instance_id, world))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| parse(world, &e.path()))
        .collect();
    out.sort_by(|a, b| b.created_ms.cmp(&a.created_ms));
    out
}

pub fn list(instance_id: &str) -> Result<Vec<Backup>> {
    instance::check_id(instance_id)?;
    let mut out: Vec<Backup> = std::fs::read_dir(paths::backups().join(instance_id))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .flat_map(|e| of_world(instance_id, &e.file_name().to_string_lossy()))
        .collect();
    out.sort_by(|a, b| b.created_ms.cmp(&a.created_ms));
    Ok(out)
}

pub(crate) fn add_dir<W: Write + std::io::Seek>(zip: &mut zip::ZipWriter<W>, root: &Path, rel: &str) -> Result<()> {
    let dir = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };
    for entry in std::fs::read_dir(&dir).map_err(Error::at(&dir))? {
        let entry = entry.map_err(Error::at(&dir))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !rel.contains('/') && name == "session.lock" {
            continue;
        }
        let child = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            zip.add_directory(format!("{child}/"), SimpleFileOptions::default())?;
            add_dir(zip, root, &child)?;
        } else if kind.is_file() {
            let path = entry.path();
            let big = entry.metadata().is_ok_and(|m| m.len() >= u32::MAX as u64);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).compression_level(Some(1)).large_file(big);
            zip.start_file(child, opts)?;
            let mut file = std::fs::File::open(&path).map_err(Error::at(&path))?;
            std::io::copy(&mut file, zip).map_err(Error::at(&path))?;
        }
    }
    Ok(())
}

pub fn create(instance_id: &str, world: &str, reason: &str) -> Result<Backup> {
    instance::check_id(instance_id)?;
    check_world(world)?;
    let src = saves(instance_id).join(world);
    if !src.join("level.dat").is_file() {
        return Err(Error::msg("World not found"));
    }
    let dir = world_backups(instance_id, world);
    std::fs::create_dir_all(&dir).map_err(Error::at(&dir))?;
    let mut created = now_ms();
    while dir.join(format!("{created}-{reason}.zip")).exists() {
        created += 1;
    }
    let target = dir.join(format!("{created}-{reason}.zip"));
    let tmp = dir.join(format!(".{created}-{reason}.zip.part"));
    let written = (|| -> Result<()> {
        let file = std::fs::File::create(&tmp).map_err(Error::at(&tmp))?;
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
        add_dir(&mut zip, &src, "")?;
        zip.finish()?.flush().map_err(Error::at(&tmp))?;
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        diag::log(format!("Backup of {instance_id}/{world} failed: {e}"));
        return Err(e);
    }
    std::fs::rename(&tmp, &target).map_err(Error::at(&target))?;
    if reason != "manual" {
        prune(instance_id, world);
    }
    let backup = parse(world, &target).ok_or_else(|| Error::msg("invalid backup"))?;
    diag::log(format!("Backed up {instance_id}/{world} ({reason}, {} KB)", backup.size / 1024));
    Ok(backup)
}

fn prune(instance_id: &str, world: &str) {
    for old in of_world(instance_id, world).into_iter().filter(|b| b.reason != "manual").skip(KEEP_AUTO) {
        let _ = std::fs::remove_file(world_backups(instance_id, world).join(old.id));
    }
}

fn modified_ms(world: &Path) -> u64 {
    std::fs::metadata(world.join("level.dat"))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

fn changed_worlds(instance_id: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(saves(instance_id))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("level.dat").is_file())
        .filter_map(|p| {
            let world = p.file_name()?.to_string_lossy().into_owned();
            let last = of_world(instance_id, &world).first().map_or(0, |b| b.created_ms);
            (check_world(&world).is_ok() && modified_ms(&p) > last).then_some(world)
        })
        .collect();
    out.sort();
    out
}

pub fn auto(instance_id: &str, reason: &str, progress: &dyn Fn(u32, u32)) -> u32 {
    if !settings::load().auto_backup || instance::check_id(instance_id).is_err() {
        return 0;
    }
    let worlds = changed_worlds(instance_id);
    let total = worlds.len() as u32;
    let mut done = 0;
    for (i, world) in worlds.iter().enumerate() {
        progress(i as u32, total);
        if create(instance_id, world, reason).is_ok() {
            done += 1;
        }
    }
    done
}

fn extract(zip_path: &Path, dest: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip_path).map_err(Error::at(zip_path))?)?;
    std::fs::create_dir_all(dest).map_err(Error::at(dest))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else { continue };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(Error::at(&out))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(Error::at(parent))?;
        }
        let mut file = std::fs::File::create(&out).map_err(Error::at(&out))?;
        std::io::copy(&mut entry, &mut file).map_err(Error::at(&out))?;
    }
    Ok(())
}

fn free_name(saves: &Path, world: &str) -> String {
    let first = format!("{world} (restored)");
    if !saves.join(&first).exists() {
        return first;
    }
    (2..1000).map(|n| format!("{world} (restored {n})")).find(|n| !saves.join(n).exists()).unwrap_or(first)
}

pub fn restore(instance_id: &str, world: &str, id: &str, as_copy: bool) -> Result<String> {
    instance::check_id(instance_id)?;
    check_world(world)?;
    check_id(id)?;
    let zip_path = world_backups(instance_id, world).join(id);
    if !zip_path.is_file() {
        return Err(Error::msg("Backup not found"));
    }
    let saves = saves(instance_id);
    let staging = saves.join(format!(".{world}.restoring"));
    let _ = std::fs::remove_dir_all(&staging);
    if let Err(e) = extract(&zip_path, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    let target_name = if as_copy { free_name(&saves, world) } else { world.to_string() };
    let target = saves.join(&target_name);
    if !as_copy && target.exists() {
        if target.join("level.dat").is_file() {
            if let Err(e) = create(instance_id, world, "restore") {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(e);
            }
        }
        crate::fsx::retry(|| std::fs::remove_dir_all(&target)).map_err(Error::at(&target))?;
    }
    crate::fsx::retry(|| std::fs::rename(&staging, &target)).map_err(Error::at(&target))?;
    diag::log(format!("Restored {instance_id}/{world} from {id} into {target_name}"));
    Ok(target_name)
}

pub fn delete(instance_id: &str, world: &str, id: &str) -> Result<()> {
    instance::check_id(instance_id)?;
    check_world(world)?;
    check_id(id)?;
    let path = world_backups(instance_id, world).join(id);
    std::fs::remove_file(&path).map_err(Error::at(&path))?;
    Ok(())
}

pub fn folder(instance_id: &str) -> Result<PathBuf> {
    instance::check_id(instance_id)?;
    Ok(paths::backups().join(instance_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn backup_restore_roundtrip() {
        let id = "backup-probe";
        let world = saves(id).join("My World");
        std::fs::create_dir_all(world.join("region")).unwrap();
        std::fs::write(world.join("level.dat"), b"v1").unwrap();
        std::fs::write(world.join("region/r.0.0.mca"), vec![7u8; 100_000]).unwrap();
        std::fs::write(world.join("session.lock"), b"lock").unwrap();

        assert_eq!(changed_worlds(id), vec!["My World".to_string()]);
        let b = create(id, "My World", "manual").unwrap();
        assert!(changed_worlds(id).is_empty());

        std::fs::write(world.join("level.dat"), b"v2").unwrap();
        std::fs::remove_file(world.join("region/r.0.0.mca")).unwrap();
        let name = restore(id, "My World", &b.id, false).unwrap();
        assert_eq!(name, "My World");
        assert_eq!(std::fs::read(world.join("level.dat")).unwrap(), b"v1");
        assert_eq!(std::fs::read(world.join("region/r.0.0.mca")).unwrap().len(), 100_000);
        assert!(!world.join("session.lock").exists());
        let all = list(id).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|b| b.reason == "restore"));

        let copy = restore(id, "My World", &b.id, true).unwrap();
        assert_eq!(copy, "My World (restored)");
        assert!(saves(id).join(&copy).join("level.dat").is_file());

        for n in 0..8 {
            std::fs::write(world.join("level.dat"), format!("v{n}")).unwrap();
            create(id, "My World", "launch").unwrap();
        }
        let left = list(id).unwrap();
        assert_eq!(left.iter().filter(|b| b.reason != "manual").count(), KEEP_AUTO);
        assert!(left.iter().any(|b| b.reason == "manual"));

        assert!(restore(id, "../x", &b.id, false).is_err());
        assert!(delete(id, "My World", "../../x.zip").is_err());
        delete(id, "My World", &b.id).unwrap();
        let _ = std::fs::remove_dir_all(paths::instance_dir(id));
        let _ = std::fs::remove_dir_all(paths::backups().join(id));
    }
}
