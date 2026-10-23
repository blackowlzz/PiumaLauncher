use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::{backup, instance, library, nbt, paths};

const MAX_IMPORT_BYTES: u64 = 16 << 30;
const MAX_IMPORT_FILES: usize = 400_000;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorldEntry {
    instance_id: String,
    folder: String,
    name: String,
    size: u64,
    modified_ms: u64,
    mode: Option<i64>,
    hardcore: bool,
    version: Option<String>,
    icon: Option<String>,
}

fn saves(instance_id: &str) -> PathBuf {
    paths::game_dir(instance_id).join("saves")
}

fn details(dir: &Path) -> Option<(String, Option<i64>, bool, Option<String>)> {
    let f = std::fs::File::open(dir.join("level.dat")).ok()?;
    let root = nbt::read(flate2::read::GzDecoder::new(std::io::BufReader::new(f)).take(16 * 1024 * 1024)).ok()?;
    let data = root.get("Data")?;
    let name = data.str("LevelName").map(String::from).filter(|n| !n.trim().is_empty())?;
    let version = data.get("Version").and_then(|v| v.str("Name")).map(String::from);
    Some((name, data.int("GameType"), data.int("hardcore").unwrap_or(0) != 0, version))
}

fn modified_ms(dir: &Path) -> u64 {
    std::fs::metadata(dir.join("level.dat"))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

pub fn all() -> Vec<WorldEntry> {
    let mut out = vec![];
    for inst in instance::list() {
        for e in std::fs::read_dir(saves(&inst.id)).into_iter().flatten().flatten() {
            let dir = e.path();
            let folder = e.file_name().to_string_lossy().into_owned();
            if !dir.join("level.dat").is_file() || backup::check_world(&folder).is_err() {
                continue;
            }
            let (name, mode, hardcore, version) = details(&dir).unwrap_or_else(|| (folder.clone(), None, false, None));
            out.push(WorldEntry {
                instance_id: inst.id.clone(),
                name: crate::servers::strip_codes(&name),
                size: library::dir_size(&dir),
                modified_ms: modified_ms(&dir),
                icon: crate::servers::world_icon(&dir),
                folder,
                mode,
                hardcore,
                version,
            });
        }
    }
    out.sort_by_key(|w| std::cmp::Reverse(w.modified_ms));
    out
}

fn free_name(dir: &Path, base: &str) -> String {
    let base = base.trim();
    if !dir.join(base).exists() {
        return base.to_string();
    }
    (2..).map(|n| format!("{base} ({n})")).find(|n| !dir.join(n).exists()).unwrap_or_default()
}

fn source(instance_id: &str, folder: &str) -> Result<PathBuf> {
    instance::check_id(instance_id)?;
    backup::check_world(folder)?;
    let dir = saves(instance_id).join(folder);
    if dir.join("level.dat").is_file() { Ok(dir) } else { Err(Error::msg("World not found")) }
}

pub fn transfer(from: &str, folder: &str, to: &str, remove_source: bool) -> Result<String> {
    let src = source(from, folder)?;
    instance::check_id(to)?;
    if from == to && remove_source {
        return Ok(folder.to_string());
    }
    let target = saves(to);
    std::fs::create_dir_all(&target).map_err(Error::at(&target))?;
    let name = free_name(&target, folder);
    let dst = target.join(&name);
    if let Err(e) = library::copy_dir(&src, &dst) {
        let _ = std::fs::remove_dir_all(&dst);
        return Err(e);
    }
    if remove_source && library::dir_size(&dst) < library::dir_size(&src) {
        let _ = std::fs::remove_dir_all(&dst);
        return Err(Error::msg("The copy is incomplete, the world was left where it was"));
    }
    let _ = std::fs::remove_file(dst.join("session.lock"));
    if remove_source {
        backup::create(from, folder, "move")?;
        std::fs::remove_dir_all(&src).map_err(Error::at(&src))?;
    }
    crate::diag::log(format!("World {from}/{folder} {} to {to}/{name}", if remove_source { "moved" } else { "copied" }));
    Ok(name)
}

fn file_name_for(name: &str) -> String {
    let clean: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') { c } else { '_' }).collect();
    let clean = clean.trim().trim_matches('.').to_string();
    if clean.is_empty() { "world".into() } else { clean.chars().take(64).collect() }
}

pub fn export(instance_id: &str, folder: &str) -> Result<PathBuf> {
    let dir = dirs::download_dir().or_else(dirs::home_dir).ok_or_else(|| Error::msg("Downloads folder not found"))?;
    export_to(instance_id, folder, &dir)
}

fn export_to(instance_id: &str, folder: &str, dir: &Path) -> Result<PathBuf> {
    let src = source(instance_id, folder)?;
    let name = details(&src).map_or_else(|| folder.to_string(), |d| crate::servers::strip_codes(&d.0));
    let base = file_name_for(&name);
    let target = std::iter::once(dir.join(format!("{base}.zip")))
        .chain((2..).map(|n| dir.join(format!("{base} ({n}).zip"))))
        .find(|p| !p.exists())
        .ok_or_else(|| Error::msg("File not found"))?;
    let tmp = target.with_extension("zip.part");
    let written = (|| -> Result<()> {
        let file = std::fs::File::create(&tmp).map_err(Error::at(&tmp))?;
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
        zip.add_directory(format!("{folder}/"), zip::write::SimpleFileOptions::default())?;
        backup::add_dir(&mut zip, &saves(instance_id), folder)?;
        zip.finish()?.flush().map_err(Error::at(&tmp))?;
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, &target).map_err(Error::at(&target))?;
    Ok(target)
}

fn world_root(names: &[String]) -> Option<String> {
    names
        .iter()
        .filter_map(|n| n.strip_suffix("level.dat"))
        .filter(|p| p.is_empty() || p.ends_with('/'))
        .filter(|p| p.matches('/').count() <= 3 && !p.contains("__MACOSX"))
        .min_by_key(|p| p.len())
        .map(String::from)
}

pub fn import(instance_id: &str, archive: &Path, fallback_name: &str) -> Result<String> {
    instance::check_id(instance_id)?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive).map_err(Error::at(archive))?)
        .map_err(|_| Error::msg("This file isn't a Minecraft world"))?;
    if zip.len() > MAX_IMPORT_FILES {
        return Err(Error::msg("This file isn't a Minecraft world"));
    }
    let names: Vec<String> = (0..zip.len()).filter_map(|i| zip.by_index(i).ok().and_then(|e| e.name().ok().map(|n| n.replace('\\', "/")))).collect();
    let prefix = world_root(&names).ok_or_else(|| Error::msg("This file isn't a Minecraft world"))?;
    let base = prefix.trim_end_matches('/').rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or(fallback_name);
    let base = file_name_for(base);
    let target = saves(instance_id);
    std::fs::create_dir_all(&target).map_err(Error::at(&target))?;
    let staging = target.join(format!(".import-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| -> Result<()> {
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let Ok(name) = entry.name().map(|n| n.replace('\\', "/")) else { continue };
            let Some(rest) = name.strip_prefix(&prefix) else { continue };
            let Some(rel) = Path::new(rest).components().all(|c| matches!(c, std::path::Component::Normal(_))).then(|| PathBuf::from(rest)) else { continue };
            if rest.is_empty() || rel.as_os_str().is_empty() {
                continue;
            }
            let out = staging.join(&rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out).map_err(Error::at(&out))?;
                continue;
            }
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(Error::at(parent))?;
            }
            let mut file = std::fs::File::create(&out).map_err(Error::at(&out))?;
            total += std::io::copy(&mut (&mut entry).take(MAX_IMPORT_BYTES - total), &mut file).map_err(Error::at(&out))?;
            if total >= MAX_IMPORT_BYTES {
                return Err(Error::msg("This file isn't a Minecraft world"));
            }
        }
        if !staging.join("level.dat").is_file() {
            return Err(Error::msg("This file isn't a Minecraft world"));
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    let _ = std::fs::remove_file(staging.join("session.lock"));
    let name = free_name(&target, &base);
    std::fs::rename(&staging, target.join(&name)).map_err(Error::at(&target))?;
    crate::diag::log(format!("Imported a world into {instance_id}/{name}"));
    Ok(name)
}

pub fn import_bytes(instance_id: &str, bytes: &[u8]) -> Result<String> {
    if !bytes.starts_with(b"PK") {
        return Err(Error::msg("This file isn't a Minecraft world"));
    }
    let dir = paths::root().join("tmp");
    std::fs::create_dir_all(&dir).map_err(Error::at(&dir))?;
    let tmp = dir.join(format!("world-{}.zip", uuid::Uuid::new_v4().simple()));
    std::fs::write(&tmp, bytes).map_err(Error::at(&tmp))?;
    let result = import(instance_id, &tmp, "World");
    let _ = std::fs::remove_file(&tmp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_move_export_and_import_keep_worlds_safe() {
        let tag = std::process::id();
        let a = instance::create(&format!("test worlds a {tag}"), "1.21.1", crate::loaders::Loader::Vanilla, None, None).unwrap();
        let b = instance::create(&format!("test worlds b {tag}"), "1.21.1", crate::loaders::Loader::Vanilla, None, None).unwrap();
        let world = saves(&a.id).join("World");
        std::fs::create_dir_all(world.join("region")).unwrap();
        std::fs::write(world.join("level.dat"), b"level").unwrap();
        std::fs::write(world.join("region/r.0.0.mca"), vec![7u8; 5000]).unwrap();
        std::fs::write(world.join("session.lock"), b"lock").unwrap();

        assert!(all().iter().any(|w| w.instance_id == a.id && w.folder == "World"));
        assert!(transfer(&a.id, "../x", &b.id, false).is_err());

        assert_eq!(transfer(&a.id, "World", &b.id, false).unwrap(), "World");
        assert_eq!(transfer(&a.id, "World", &b.id, false).unwrap(), "World (2)");
        let copied = saves(&b.id).join("World");
        assert_eq!(std::fs::read(copied.join("region/r.0.0.mca")).unwrap().len(), 5000);
        assert!(!copied.join("session.lock").exists());
        assert!(world.join("level.dat").is_file());

        let out = std::env::temp_dir().join(format!("piuma-world-export-{tag}"));
        std::fs::create_dir_all(&out).unwrap();
        let zip = export_to(&b.id, "World", &out).unwrap();
        assert!(zip.is_file());
        assert_eq!(import(&b.id, &zip, "World").unwrap(), "World (3)");
        let imported = saves(&b.id).join("World (3)");
        assert_eq!(std::fs::read(imported.join("level.dat")).unwrap(), b"level");
        assert_eq!(std::fs::read(imported.join("region/r.0.0.mca")).unwrap().len(), 5000);
        assert!(!saves(&b.id).read_dir().unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with(".import")));

        let bad = out.join("bad.zip");
        std::fs::write(&bad, b"PK not really").unwrap();
        assert!(import(&b.id, &bad, "x").is_err());

        assert_eq!(transfer(&a.id, "World", &b.id, true).unwrap(), "World (4)");
        assert!(!world.exists());
        assert!(backup::list(&a.id).unwrap().iter().any(|bk| bk.world == "World" && bk.reason == "move"));

        let _ = std::fs::remove_dir_all(&out);
        let _ = instance::delete(&a.id);
        let _ = instance::delete(&b.id);
    }

    #[test]
    #[ignore]
    fn reads_real_worlds() {
        let worlds = all();
        let named = worlds.iter().filter(|w| w.name != w.folder).count();
        let versioned = worlds.iter().filter(|w| w.version.is_some()).count();
        let moded = worlds.iter().filter(|w| w.mode.is_some()).count();
        let iconed = worlds.iter().filter(|w| w.icon.is_some()).count();
        eprintln!("mondi {} · con nome {} · con versione {} · con modalità {} · con icona {}", worlds.len(), named, versioned, moded, iconed);
        eprintln!("versioni: {:?}", worlds.iter().filter_map(|w| w.version.clone()).collect::<std::collections::BTreeSet<_>>());
    }

    #[test]
    fn reads_name_mode_and_version_from_level_dat() {
        use nbt::Tag;
        let dir = std::env::temp_dir().join(format!("piuma-level-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let root = Tag::Compound(vec![(
            "Data".into(),
            Tag::Compound(vec![
                ("LevelName".into(), Tag::String("§aIsola Felice".into())),
                ("GameType".into(), Tag::Int(1)),
                ("hardcore".into(), Tag::Byte(1)),
                ("Version".into(), Tag::Compound(vec![("Name".into(), Tag::String("1.21.4".into())), ("Id".into(), Tag::Int(4189))])),
            ]),
        )]);
        let mut gz = flate2::write::GzEncoder::new(std::fs::File::create(dir.join("level.dat")).unwrap(), flate2::Compression::default());
        nbt::write(&mut gz, &root).unwrap();
        gz.finish().unwrap();
        let (name, mode, hardcore, version) = details(&dir).unwrap();
        assert_eq!((crate::servers::strip_codes(&name).as_str(), mode, hardcore, version.as_deref()), ("Isola Felice", Some(1), true, Some("1.21.4")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_the_world_inside_an_archive() {
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(world_root(&names(&["level.dat", "region/r.0.0.mca"])).as_deref(), Some(""));
        assert_eq!(world_root(&names(&["My World/", "My World/level.dat", "My World/region/r.0.0.mca"])).as_deref(), Some("My World/"));
        assert_eq!(world_root(&names(&["pack/saves/A/level.dat", "pack/level.dat"])).as_deref(), Some("pack/"));
        assert_eq!(world_root(&names(&["__MACOSX/W/level.dat", "W/level.dat"])).as_deref(), Some("W/"));
        assert!(world_root(&names(&["mods/a.jar", "notlevel.dat"])).is_none());
        assert_eq!(file_name_for("My: World?"), "My_ World_");
        assert_eq!(file_name_for("..."), "world");
    }
}
