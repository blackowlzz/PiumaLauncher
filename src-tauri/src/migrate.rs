use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::instance::{self, Instance};
use crate::loaders::{self, Loader};
use crate::{diag, minecraft, paths};

pub type Progress<'a> = &'a (dyn Fn(&str, u32, u32) + Sync);

const LOG_PEEK: u64 = 256 * 1024;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Prism,
    Polymc,
    Multimc,
    Curseforge,
    Modrinth,
    Atlauncher,
    Official,
    Folder,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub key: String,
    pub source: Source,
    pub name: String,
    pub game_dir: String,
    pub mc_version: Option<String>,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub sure: bool,
    pub memory_min_mb: Option<u32>,
    pub memory_max_mb: Option<u32>,
    pub mods: u32,
    pub worlds: u32,
    pub shared: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub key: String,
    pub mc_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
}

struct Detected {
    mc: Option<String>,
    loader: Loader,
    loader_version: Option<String>,
}

impl Detected {
    fn vanilla(mc: Option<String>) -> Self {
        Detected { mc, loader: Loader::Vanilla, loader_version: None }
    }
}

struct Latest {
    release: Option<String>,
    snapshot: Option<String>,
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

fn data() -> PathBuf {
    dirs::data_dir().unwrap_or_default()
}

fn flatpak(app: &str, sub: &str) -> PathBuf {
    home().join(".var").join("app").join(app).join("data").join(sub)
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

fn ini(path: &Path) -> HashMap<String, String> {
    let Ok(body) = std::fs::read_to_string(path) else { return HashMap::new() };
    body.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with(['[', '#', ';']))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| {
            let v = v.trim();
            let v = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(v);
            (k.trim().to_string(), v.to_string())
        })
        .collect()
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

fn folder_name(dir: &Path) -> String {
    dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn count_mods(game: &Path) -> u32 {
    std::fs::read_dir(game.join("mods"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".jar"))
        .count() as u32
}

fn count_worlds(game: &Path) -> u32 {
    children(&game.join("saves")).iter().filter(|w| w.join("level.dat").is_file()).count() as u32
}

fn loader_named(s: &str) -> Option<Loader> {
    match s.trim().to_lowercase().as_str() {
        "fabric" => Some(Loader::Fabric),
        "quilt" => Some(Loader::Quilt),
        "forge" => Some(Loader::Forge),
        "neoforge" => Some(Loader::NeoForge),
        "vanilla" | "" => Some(Loader::Vanilla),
        _ => None,
    }
}

fn looks_like_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    (2..=4).contains(&parts.len()) && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn make(source: Source, key: String, name: String, game: &Path, d: Detected, sure: bool) -> Found {
    Found {
        key,
        source,
        name,
        game_dir: game.to_string_lossy().into_owned(),
        sure: sure && d.mc.is_some(),
        mc_version: d.mc,
        loader: d.loader,
        loader_version: d.loader_version,
        memory_min_mb: None,
        memory_max_mb: None,
        mods: count_mods(game),
        worlds: count_worlds(game),
        shared: false,
    }
}

fn key_of(path: &Path) -> String {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()).to_string_lossy().into_owned()
}

fn from_log(game: &Path) -> Option<Detected> {
    let mut body = String::new();
    std::fs::File::open(game.join("logs").join("latest.log")).ok()?.take(LOG_PEEK).read_to_string(&mut body).ok()?;
    if let Some(i) = body.find("Loading Minecraft ") {
        let mut w = body[i + "Loading Minecraft ".len()..].split_whitespace();
        let mc = w.next().map(String::from);
        if w.next() == Some("with") {
            let loader = w.next().and_then(loader_named);
            if let (Some(loader), Some("Loader")) = (loader, w.next()) {
                return Some(Detected { mc, loader, loader_version: w.next().map(String::from) });
            }
        }
    }
    let arg = |flag: &str| {
        let i = body.find(flag)?;
        let rest = &body[i + flag.len()..];
        let end = rest.find([',', ']', '\n']).unwrap_or(rest.len());
        Some(rest[..end].trim().to_string()).filter(|s| !s.is_empty())
    };
    let mc = arg("--fml.mcVersion, ")?;
    if let Some(v) = arg("--fml.neoForgeVersion, ") {
        return Some(Detected { mc: Some(mc), loader: Loader::NeoForge, loader_version: Some(v) });
    }
    arg("--fml.forgeVersion, ").map(|v| Detected { mc: Some(mc), loader: Loader::Forge, loader_version: Some(v) })
}

fn from_name(name: &str) -> Detected {
    let lower = name.to_lowercase();
    let mc = lower
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map(|t| t.trim_matches('.'))
        .find(|t| looks_like_version(t))
        .map(String::from);
    let loader = ["neoforge", "fabric", "quilt", "forge"]
        .iter()
        .find(|l| lower.contains(*l))
        .and_then(|l| loader_named(l))
        .unwrap_or(Loader::Vanilla);
    Detected { mc, loader, loader_version: None }
}

fn guess(game: &Path, name: &str) -> Detected {
    from_log(game).unwrap_or_else(|| from_name(name))
}

fn prism_instances_dir(root: &Path) -> PathBuf {
    for cfg in ["prismlauncher.cfg", "polymc.cfg", "multimc.cfg"] {
        if let Some(dir) = ini(&root.join(cfg)).get("InstanceDir").filter(|d| !d.is_empty()) {
            let dir = PathBuf::from(dir);
            return if dir.is_absolute() { dir } else { root.join(dir) };
        }
    }
    root.join("instances")
}

fn prism_instance(source: Source, dir: &Path) -> Option<Found> {
    let cfg_file = dir.join("instance.cfg");
    if !cfg_file.is_file() {
        return None;
    }
    let cfg = ini(&cfg_file);
    let name = cfg.get("name").filter(|n| !n.trim().is_empty()).cloned().unwrap_or_else(|| folder_name(dir));
    let mut d = Detected::vanilla(cfg.get("IntendedVersion").filter(|v| !v.is_empty()).cloned());
    if let Some(pack) = read_json(&dir.join("mmc-pack.json")) {
        for c in pack["components"].as_array().into_iter().flatten() {
            let version = text(&c["version"]).or_else(|| text(&c["cachedVersion"]));
            let loader = match c["uid"].as_str().unwrap_or("") {
                "net.minecraft" => {
                    d.mc = version.or(d.mc.take());
                    continue;
                }
                "net.fabricmc.fabric-loader" => Loader::Fabric,
                "org.quiltmc.quilt-loader" => Loader::Quilt,
                "net.minecraftforge" => Loader::Forge,
                "net.neoforged" => Loader::NeoForge,
                _ => continue,
            };
            d.loader = loader;
            d.loader_version = version;
        }
    }
    let game = [".minecraft", "minecraft"].iter().map(|g| dir.join(g)).find(|p| p.is_dir()).unwrap_or_else(|| dir.join(".minecraft"));
    let mut found = make(source, key_of(dir), name, &game, d, true);
    if cfg.get("OverrideMemory").is_some_and(|v| v == "true") {
        found.memory_min_mb = cfg.get("MinMemAlloc").and_then(|v| v.parse().ok());
        found.memory_max_mb = cfg.get("MaxMemAlloc").and_then(|v| v.parse().ok());
    }
    Some(found)
}

fn curseforge_loader(b: &Value) -> (Loader, Option<String>) {
    let name = b["name"].as_str().unwrap_or("");
    let (kind, rest) = name.split_once('-').unwrap_or((name, ""));
    let rest = Some(rest.to_string()).filter(|r| !r.is_empty());
    match kind.to_lowercase().as_str() {
        "forge" => (Loader::Forge, text(&b["forgeVersion"]).or(rest)),
        "neoforge" => (Loader::NeoForge, rest),
        "fabric" => (Loader::Fabric, rest.map(|r| r.split('-').next().unwrap_or_default().to_string())),
        "quilt" => (Loader::Quilt, rest.map(|r| r.split('-').next().unwrap_or_default().to_string())),
        _ => match b["type"].as_i64() {
            Some(1) => (Loader::Forge, None),
            Some(4) => (Loader::Fabric, None),
            Some(5) => (Loader::Quilt, None),
            Some(6) => (Loader::NeoForge, None),
            _ => (Loader::Vanilla, None),
        },
    }
}

fn curseforge_instance(dir: &Path) -> Option<Found> {
    let v = read_json(&dir.join("minecraftinstance.json"))?;
    let name = text(&v["name"]).unwrap_or_else(|| folder_name(dir));
    let mc = text(&v["gameVersion"]).or_else(|| text(&v["baseModLoader"]["minecraftVersion"]));
    let (loader, loader_version) = curseforge_loader(&v["baseModLoader"]);
    Some(make(Source::Curseforge, key_of(dir), name, dir, Detected { mc, loader, loader_version }, true))
}

fn atlauncher_instance(dir: &Path) -> Option<Found> {
    let v = read_json(&dir.join("instance.json"))?;
    let launcher = v.get("launcher")?;
    let name = text(&launcher["name"]).unwrap_or_else(|| folder_name(dir));
    let mc = text(&v["id"]).filter(|id| looks_like_version(id) || id.contains('w'));
    let lv = &launcher["loaderVersion"];
    let loader = lv["type"].as_str().and_then(loader_named).unwrap_or(Loader::Vanilla);
    let loader_version = if loader == Loader::Vanilla { None } else { text(&lv["version"]) };
    Some(make(Source::Atlauncher, key_of(dir), name, dir, Detected { mc, loader, loader_version }, true))
}

fn modrinth_rows(db: &Path) -> HashMap<String, (String, Detected)> {
    let mut out = HashMap::new();
    let Ok(conn) = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX) else { return out };
    let _ = conn.busy_timeout(Duration::from_secs(2));
    let queries = [
        "SELECT i.path, i.name, s.game_version, s.loader, s.loader_version FROM instances i JOIN instance_content_sets s ON s.id = i.applied_content_set_id",
        "SELECT path, name, game_version, mod_loader, mod_loader_version FROM profiles",
    ];
    for q in queries {
        let Ok(mut stmt) = conn.prepare(q) else { continue };
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        });
        let Ok(rows) = rows else { continue };
        for (path, name, mc, loader, lv) in rows.flatten() {
            let Some(loader) = loader_named(loader.as_deref().unwrap_or("")) else { continue };
            let loader_version = if loader == Loader::Vanilla { None } else { lv.filter(|v| !v.is_empty()) };
            out.insert(path, (name, Detected { mc: mc.filter(|m| !m.is_empty()), loader, loader_version }));
        }
        if !out.is_empty() {
            break;
        }
    }
    out
}

fn modrinth(root: &Path) -> Vec<Found> {
    let profiles = root.join("profiles");
    if !profiles.is_dir() {
        return vec![];
    }
    let mut rows = modrinth_rows(&root.join("app.db"));
    children(&profiles)
        .into_iter()
        .map(|dir| {
            let folder = folder_name(&dir);
            let (name, d, sure) = match rows.remove(&folder) {
                Some((name, d)) => (name, d, true),
                None => {
                    let d = guess(&dir, &folder);
                    (folder, d, false)
                }
            };
            make(Source::Modrinth, key_of(&dir), name, &dir, d, sure)
        })
        .collect()
}

fn neoforge_mc(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor = parts.next()?;
    if major >= 26 {
        return None;
    }
    Some(if minor == "0" { format!("1.{major}") } else { format!("1.{major}.{minor}") })
}

fn version_id(root: &Path, id: &str) -> Detected {
    let inherits = read_json(&root.join("versions").join(id).join(format!("{id}.json"))).and_then(|v| text(&v["inheritsFrom"]));
    for (prefix, loader) in [("fabric-loader-", Loader::Fabric), ("quilt-loader-", Loader::Quilt)] {
        if let Some((lv, mc)) = id.strip_prefix(prefix).and_then(|r| r.split_once('-')) {
            return Detected { mc: inherits.or(Some(mc.to_string())), loader, loader_version: Some(lv.to_string()) };
        }
    }
    if let Some(v) = id.strip_prefix("neoforge-") {
        return Detected { mc: inherits.or_else(|| neoforge_mc(v)), loader: Loader::NeoForge, loader_version: Some(v.to_string()) };
    }
    if let Some((mc, rest)) = id.split_once("-forge") {
        let rest = rest.trim_start_matches('-');
        let lv = rest.strip_prefix(&format!("{mc}-")).unwrap_or(rest);
        return Detected { mc: inherits.or(Some(mc.to_string())), loader: Loader::Forge, loader_version: Some(lv.to_string()) };
    }
    Detected::vanilla(Some(inherits.unwrap_or_else(|| id.to_string())))
}

fn official(root: &Path, latest: &Latest) -> Vec<Found> {
    let Some(v) = read_json(&root.join("launcher_profiles.json")) else { return vec![] };
    let Some(profiles) = v["profiles"].as_object() else { return vec![] };
    let mut out: Vec<Found> = profiles
        .iter()
        .filter_map(|(key, p)| {
            let kind = p["type"].as_str().unwrap_or("custom");
            let id = match kind {
                "latest-release" => latest.release.clone(),
                "latest-snapshot" => latest.snapshot.clone(),
                _ => text(&p["lastVersionId"]),
            };
            let d = id.as_deref().map(|id| version_id(root, id)).unwrap_or_else(|| Detected::vanilla(None));
            let name = text(&p["name"]).unwrap_or_else(|| match kind {
                "latest-release" => "Latest release".into(),
                "latest-snapshot" => "Latest snapshot".into(),
                _ => format!("Minecraft {}", id.as_deref().unwrap_or("?")),
            });
            let custom = text(&p["gameDir"]).map(PathBuf::from);
            let game = custom.clone().unwrap_or_else(|| root.to_path_buf());
            if !game.is_dir() {
                return None;
            }
            let mut found = make(Source::Official, format!("{}#{key}", key_of(&game)), name, &game, d, true);
            found.shared = custom.is_none();
            Some(found)
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn detect_instance(dir: &Path) -> Option<Found> {
    prism_instance(Source::Prism, dir).or_else(|| curseforge_instance(dir)).or_else(|| atlauncher_instance(dir))
}

fn looks_like_game(dir: &Path) -> bool {
    ["mods", "saves", "options.txt", "resourcepacks"].iter().any(|n| dir.join(n).exists())
}

fn scan_folder(dir: &Path, latest: &Latest) -> Vec<Found> {
    if let Some(f) = detect_instance(dir) {
        return vec![f];
    }
    if dir.join("launcher_profiles.json").is_file() {
        return official(dir, latest);
    }
    if dir.join("profiles").is_dir() && dir.join("app.db").is_file() {
        return modrinth(dir);
    }
    let mut out: Vec<Found> = children(&prism_instances_dir(dir)).iter().filter_map(|d| detect_instance(d)).collect();
    if out.is_empty() {
        out = children(dir).iter().filter_map(|d| detect_instance(d)).collect();
    }
    if out.is_empty() && looks_like_game(dir) {
        let name = folder_name(dir);
        let d = guess(dir, &name);
        out.push(make(Source::Folder, key_of(dir), name, dir, d, false));
    }
    out
}

fn scan_known(latest: &Latest) -> Vec<Found> {
    let mut out = vec![];
    let prism_roots = [
        (Source::Prism, data().join("PrismLauncher")),
        (Source::Prism, flatpak("org.prismlauncher.PrismLauncher", "PrismLauncher")),
        (Source::Polymc, data().join("PolyMC")),
        (Source::Polymc, flatpak("org.polymc.PolyMC", "PolyMC")),
        (Source::Multimc, data().join("multimc")),
        (Source::Multimc, data().join("MultiMC")),
        (Source::Multimc, home().join("MultiMC")),
    ];
    for (source, root) in prism_roots {
        out.extend(children(&prism_instances_dir(&root)).iter().filter_map(|d| prism_instance(source, d)));
    }
    for root in [home().join("curseforge"), home().join("Documents").join("curseforge")] {
        out.extend(children(&root.join("minecraft").join("Instances")).iter().filter_map(|d| curseforge_instance(d)));
    }
    for root in [data().join("ModrinthApp"), data().join("com.modrinth.theseus")] {
        out.extend(modrinth(&root));
    }
    for root in [data().join("ATLauncher"), data().join("atlauncher"), home().join("ATLauncher")] {
        out.extend(children(&root.join("instances")).iter().filter_map(|d| atlauncher_instance(d)));
    }
    for root in [
        data().join(".minecraft"),
        data().join("minecraft"),
        home().join(".minecraft"),
        home().join(".var").join("app").join("com.mojang.Minecraft").join(".minecraft"),
    ] {
        out.extend(official(&root, latest));
    }
    out
}

fn expand(folder: &str) -> PathBuf {
    let folder = folder.trim();
    match folder.strip_prefix("~/").or_else(|| folder.strip_prefix("~\\")) {
        Some(rest) => home().join(rest),
        None if folder == "~" => home(),
        None => PathBuf::from(folder),
    }
}

fn ours(game_dir: &str) -> bool {
    let root = key_of(&paths::root());
    key_of(Path::new(game_dir)).starts_with(&root)
}

pub async fn scan(client: &reqwest::Client, folder: Option<String>) -> Vec<Found> {
    let latest = match minecraft::manifest(client).await {
        Ok(m) => Latest {
            release: m.versions.iter().find(|v| v.kind == "release").map(|v| v.id.clone()),
            snapshot: m.versions.iter().find(|v| v.kind == "snapshot").map(|v| v.id.clone()),
        },
        Err(_) => Latest { release: None, snapshot: None },
    };
    let folder = folder.filter(|f| !f.trim().is_empty()).map(|f| expand(&f));
    tokio::task::spawn_blocking(move || {
        let all = match folder {
            Some(dir) => scan_folder(&dir, &latest),
            None => scan_known(&latest),
        };
        let mut seen = HashSet::new();
        all.into_iter().filter(|f| !ours(&f.game_dir)).filter(|f| seen.insert(f.key.clone())).collect()
    })
    .await
    .unwrap_or_default()
}

fn skip_top(name: &str) -> bool {
    const DIRS: [&str; 13] = [
        "versions", "libraries", "assets", "runtime", "natives", "bin", "logs", "crash-reports", "webcache", "webcache2", "cache", ".cache", ".fabric",
    ];
    const FILES: [&str; 3] = ["minecraftinstance.json", "instance.json", "profile.json"];
    let lower = name.to_lowercase();
    DIRS.contains(&lower.as_str())
        || FILES.contains(&lower.as_str())
        || ["launcher_", "sklauncher", "tlauncher", "hs_err_pid"].iter().any(|p| lower.starts_with(p))
        || lower.ends_with(".log")
}

fn collect(src: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let dir = src.join(rel);
    for entry in std::fs::read_dir(&dir).map_err(Error::at(&dir))? {
        let entry = entry.map_err(Error::at(&dir))?;
        let name = entry.file_name();
        if rel.as_os_str().is_empty() && skip_top(&name.to_string_lossy()) {
            continue;
        }
        let Ok(kind) = entry.file_type() else { continue };
        let child = rel.join(&name);
        if kind.is_dir() {
            collect(src, &child, out)?;
        } else if kind.is_file() {
            out.push(child);
        }
    }
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path, files: &[PathBuf], done: &AtomicU32) -> Result<()> {
    for rel in files {
        let to = dst.join(rel);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(Error::at(parent))?;
        }
        let from = src.join(rel);
        std::fs::copy(&from, &to).map_err(Error::at(&from))?;
        done.fetch_add(1, Ordering::Relaxed);
    }
    Ok(())
}

async fn copy_with_progress(src: PathBuf, dst: PathBuf, progress: Progress<'_>) -> Result<u32> {
    progress("Copying files", 0, 0);
    let from = src.clone();
    let files = tokio::task::spawn_blocking(move || {
        let mut files = vec![];
        if from.is_dir() {
            collect(&from, Path::new(""), &mut files)?;
        }
        Ok::<_, Error>(files)
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
    let total = files.len() as u32;
    let done = Arc::new(AtomicU32::new(0));
    let counter = done.clone();
    let mut task = tokio::task::spawn_blocking(move || copy_tree(&src, &dst, &files, &counter));
    loop {
        tokio::select! {
            r = &mut task => {
                r.map_err(|e| Error::msg(e.to_string()))??;
                progress("Copying files", total, total);
                return Ok(total);
            }
            _ = tokio::time::sleep(Duration::from_millis(150)) => progress("Copying files", done.load(Ordering::Relaxed), total),
        }
    }
}

pub async fn import(client: &reqwest::Client, choice: Choice, folder: Option<String>, progress: Progress<'_>) -> Result<Instance> {
    let found = scan(client, folder)
        .await
        .into_iter()
        .find(|f| f.key == choice.key)
        .ok_or_else(|| Error::msg("This instance can't be found anymore. Search again"))?;
    let mc = choice.mc_version.trim().to_string();
    if mc.is_empty() {
        return Err(Error::msg("Choose the Minecraft version"));
    }
    let manifest = minecraft::manifest(client).await?;
    if !manifest.versions.iter().any(|v| v.id == mc) {
        return Err(Error::msg(format!("Unknown Minecraft version: {mc}")));
    }
    let (loader_version, launch_version) = match choice.loader {
        Loader::Vanilla => (None, None),
        loader => {
            let wanted = choice.loader_version.filter(|v| !v.trim().is_empty());
            let installed = loaders::install(client, loader, &mc, wanted, progress).await?;
            (Some(installed.loader_version), Some(installed.id))
        }
    };
    let mut inst = instance::create(&found.name, &mc, choice.loader, loader_version, launch_version)?;
    if let (Some(min), Some(max)) = (found.memory_min_mb, found.memory_max_mb) {
        inst.custom_performance = true;
        inst.memory_max_mb = max.clamp(512, 65_536);
        inst.memory_min_mb = min.clamp(128, inst.memory_max_mb);
        instance::save(&inst)?;
    }
    match copy_with_progress(PathBuf::from(&found.game_dir), paths::game_dir(&inst.id), progress).await {
        Ok(files) => {
            diag::log(format!(
                "Imported {} from {:?} as {} (Minecraft {mc} {:?}, {files} files)",
                found.name, found.source, inst.id, choice.loader
            ));
            Ok(inst)
        }
        Err(e) => {
            let _ = instance::delete(&inst.id);
            diag::log(format!("Import of {} from {:?} failed: {e}", found.name, found.source));
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("piuma-migrate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn official_version_ids() {
        let root = Path::new("/nonexistent");
        let d = version_id(root, "fabric-loader-0.15.11-1.20.4");
        assert_eq!((d.mc.as_deref(), d.loader, d.loader_version.as_deref()), (Some("1.20.4"), Loader::Fabric, Some("0.15.11")));
        let d = version_id(root, "quilt-loader-0.26.0-1.20.1");
        assert_eq!((d.mc.as_deref(), d.loader, d.loader_version.as_deref()), (Some("1.20.1"), Loader::Quilt, Some("0.26.0")));
        let d = version_id(root, "1.20.1-forge-47.2.0");
        assert_eq!((d.mc.as_deref(), d.loader, d.loader_version.as_deref()), (Some("1.20.1"), Loader::Forge, Some("47.2.0")));
        let d = version_id(root, "1.12.2-forge1.12.2-14.23.5.2860");
        assert_eq!((d.mc.as_deref(), d.loader, d.loader_version.as_deref()), (Some("1.12.2"), Loader::Forge, Some("14.23.5.2860")));
        let d = version_id(root, "neoforge-21.1.77");
        assert_eq!((d.mc.as_deref(), d.loader, d.loader_version.as_deref()), (Some("1.21.1"), Loader::NeoForge, Some("21.1.77")));
        let d = version_id(root, "neoforge-21.0.10");
        assert_eq!(d.mc.as_deref(), Some("1.21"));
        let d = version_id(root, "1.8.9");
        assert_eq!((d.mc.as_deref(), d.loader), (Some("1.8.9"), Loader::Vanilla));
    }

    #[test]
    fn curseforge_loader_names() {
        let v: Value = serde_json::json!({"name": "fabric-0.15.11-1.20.1"});
        assert_eq!(curseforge_loader(&v), (Loader::Fabric, Some("0.15.11".into())));
        let v: Value = serde_json::json!({"name": "forge-47.2.0"});
        assert_eq!(curseforge_loader(&v), (Loader::Forge, Some("47.2.0".into())));
        let v: Value = serde_json::json!({"name": "neoforge-21.1.77"});
        assert_eq!(curseforge_loader(&v), (Loader::NeoForge, Some("21.1.77".into())));
    }

    #[test]
    fn guesses_from_names_and_logs() {
        let d = from_name("Fabric 1.21.11 survival");
        assert_eq!((d.mc.as_deref(), d.loader), (Some("1.21.11"), Loader::Fabric));
        let d = from_name("All the Mods NeoForge 26.1");
        assert_eq!((d.mc.as_deref(), d.loader), (Some("26.1"), Loader::NeoForge));
        let dir = tmp("log");
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::write(dir.join("logs/latest.log"), "[main/INFO]: Loading Minecraft 1.20.1 with Fabric Loader 0.16.5\n").unwrap();
        let d = guess(&dir, "x");
        assert_eq!((d.mc.as_deref(), d.loader, d.loader_version.as_deref()), (Some("1.20.1"), Loader::Fabric, Some("0.16.5")));
        std::fs::write(
            dir.join("logs/latest.log"),
            "ModLauncher running: args [--launchTarget, forgeclient, --fml.neoForgeVersion, 21.1.77, --fml.fmlVersion, 4.0, --fml.mcVersion, 1.21.1]\n",
        )
        .unwrap();
        let d = guess(&dir, "x");
        assert_eq!((d.mc.as_deref(), d.loader, d.loader_version.as_deref()), (Some("1.21.1"), Loader::NeoForge, Some("21.1.77")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_prism_instances() {
        let root = tmp("prism");
        let inst = root.join("instances").join("Pack");
        std::fs::create_dir_all(inst.join(".minecraft/mods")).unwrap();
        std::fs::create_dir_all(inst.join(".minecraft/saves/World/")).unwrap();
        std::fs::write(inst.join(".minecraft/saves/World/level.dat"), b"x").unwrap();
        std::fs::write(inst.join(".minecraft/mods/a.jar"), b"x").unwrap();
        std::fs::write(inst.join("instance.cfg"), "[General]\nname=My \"Pack\"\nOverrideMemory=true\nMaxMemAlloc=6144\nMinMemAlloc=1024\n").unwrap();
        std::fs::write(
            inst.join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.15.11"}],"formatVersion":1}"#,
        )
        .unwrap();
        let latest = Latest { release: None, snapshot: None };
        let found = scan_folder(&root, &latest);
        assert_eq!(found.len(), 1);
        let f = &found[0];
        assert_eq!(f.name, "My \"Pack\"");
        assert_eq!((f.mc_version.as_deref(), f.loader, f.loader_version.as_deref()), (Some("1.20.1"), Loader::Fabric, Some("0.15.11")));
        assert_eq!((f.mods, f.worlds, f.memory_min_mb, f.memory_max_mb), (1, 1, Some(1024), Some(6144)));
        assert!(f.sure && f.game_dir.ends_with(".minecraft"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    #[ignore]
    async fn imports_prism_fixture() {
        let root = tmp("e2e");
        let inst = root.join("instances").join("Fixture");
        std::fs::create_dir_all(inst.join(".minecraft/saves/W")).unwrap();
        std::fs::write(inst.join(".minecraft/saves/W/level.dat"), b"x").unwrap();
        std::fs::write(inst.join(".minecraft/options.txt"), b"x").unwrap();
        std::fs::write(inst.join("instance.cfg"), "name=Fixture\n").unwrap();
        std::fs::write(
            inst.join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.15.11"}]}"#,
        )
        .unwrap();
        let client = crate::net::client();
        let folder = Some(root.to_string_lossy().into_owned());
        let found = scan(&client, folder.clone()).await;
        let f = &found[0];
        let choice = Choice { key: f.key.clone(), mc_version: "1.20.1".into(), loader: f.loader, loader_version: f.loader_version.clone() };
        let made = import(&client, choice, folder, &|s, d, t| eprintln!("{s} {d}/{t}")).await.unwrap();
        assert_eq!((made.loader, made.loader_version.as_deref()), (Loader::Fabric, Some("0.15.11")));
        let game = paths::game_dir(&made.id);
        assert!(game.join("saves/W/level.dat").is_file() && game.join("options.txt").is_file());
        instance::delete(&made.id).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn copy_skips_launcher_files() {
        let src = tmp("copy-src");
        let dst = tmp("copy-dst");
        for f in ["saves/w/level.dat", "mods/a.jar", "options.txt", "config/x/y.toml", "launcher_accounts.json", "logs/latest.log", "versions/1/1.json", "debug.log"] {
            let p = src.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"x").unwrap();
        }
        let mut files = vec![];
        collect(&src, Path::new(""), &mut files).unwrap();
        let done = AtomicU32::new(0);
        copy_tree(&src, &dst, &files, &done).unwrap();
        assert_eq!(done.load(Ordering::Relaxed), 4);
        assert!(dst.join("saves/w/level.dat").is_file() && dst.join("config/x/y.toml").is_file());
        assert!(!dst.join("launcher_accounts.json").exists() && !dst.join("logs").exists() && !dst.join("versions").exists());
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
    }
}
