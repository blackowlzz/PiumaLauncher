use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::instance::{self, Instance};
use crate::loaders::{self, Loader};
use crate::net::{self, Check, Job};
use crate::paths;

const API: &str = "https://api.modrinth.com/v2";
const ALLOWED_HOSTS: [&str; 4] = ["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com"];
const PAGE_SIZE: u32 = 20;
const MAX_EXTRACT_BYTES: u64 = 4 << 30;

pub type Progress<'a> = &'a (dyn Fn(&str, u32, u32) + Sync);


#[derive(Deserialize)]
struct RawSearch {
    hits: Vec<RawHit>,
    total_hits: u32,
}

#[derive(Deserialize)]
struct RawHit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    author: String,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    versions: Vec<String>,
    #[serde(default)]
    featured_gallery: Option<String>,
    #[serde(default)]
    gallery: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub downloads: u64,
    pub author: String,
    pub categories: Vec<String>,
    pub latest_mc: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub hits: Vec<ProjectHit>,
    pub total: u32,
    pub page_size: u32,
}

fn safe_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn loader_tags(loader: Loader) -> &'static [&'static str] {
    match loader {
        Loader::Vanilla => &[],
        Loader::Fabric => &["fabric"],
        Loader::Quilt => &["quilt", "fabric"],
        Loader::Forge => &["forge"],
        Loader::NeoForge => &["neoforge"],
    }
}

pub async fn search(
    client: &reqwest::Client,
    query: &str,
    project_type: &str,
    mc_version: Option<&str>,
    loaders: &[String],
    categories: &[String],
    sort: &str,
    offset: u32,
) -> Result<SearchResult> {
    if !matches!(project_type, "modpack" | "mod" | "resourcepack" | "shader") {
        return Err(Error::msg("invalid project type"));
    }
    let mut facets = vec![vec![format!("project_type:{project_type}")]];
    if let Some(v) = mc_version.filter(|v| !v.is_empty()) {
        facets.push(vec![format!("versions:{v}")]);
    }
    if !loaders.is_empty() {
        facets.push(loaders.iter().filter(|l| safe_id(l)).map(|l| format!("categories:{l}")).collect());
    }
    for c in categories.iter().filter(|c| safe_id(c)) {
        facets.push(vec![format!("categories:{c}")]);
    }
    let index = if matches!(sort, "downloads" | "follows" | "newest" | "updated") { sort } else { "relevance" };

    let resp = client
        .get(format!("{API}/search"))
        .query(&[
            ("query", query),
            ("facets", &serde_json::to_string(&facets)?),
            ("index", index),
            ("limit", &PAGE_SIZE.to_string()),
            ("offset", &offset.to_string()),
        ])
        .send()
        .await?
        .error_for_status()?;
    let raw: RawSearch = resp.json().await?;
    Ok(SearchResult {
        total: raw.total_hits,
        page_size: PAGE_SIZE,
        hits: raw
            .hits
            .into_iter()
            .map(|h| ProjectHit {
                latest_mc: h.versions.last().cloned(),
                banner_url: h.featured_gallery.or_else(|| h.gallery.into_iter().next()),
                project_id: h.project_id,
                slug: h.slug,
                title: h.title,
                description: h.description,
                icon_url: h.icon_url,
                downloads: h.downloads,
                author: h.author,
                categories: h.categories,
            })
            .collect(),
    })
}


#[derive(Deserialize, Clone)]
struct Version {
    #[serde(default)]
    id: String,
    #[serde(default)]
    version_number: String,
    #[serde(default)]
    name: String,
    version_type: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    date_published: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    files: Vec<VFile>,
    #[serde(default)]
    dependencies: Vec<Dep>,
}

#[derive(Deserialize, Clone)]
struct VFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    size: Option<u64>,
    hashes: Hashes,
}

#[derive(Deserialize, Clone, Default)]
struct Hashes {
    #[serde(default)]
    sha1: Option<String>,
}

#[derive(Deserialize, Clone)]
struct Dep {
    #[serde(default)]
    project_id: Option<String>,
    dependency_type: String,
}

async fn versions_of(
    client: &reqwest::Client,
    project: &str,
    mc: Option<&str>,
    loaders: &[&str],
) -> Result<Vec<Version>> {
    if !safe_id(project) {
        return Err(Error::msg("invalid project id"));
    }
    let mut params: Vec<(&str, String)> = vec![];
    if let Some(mc) = mc {
        params.push(("game_versions", serde_json::to_string(&[mc])?));
    }
    if !loaders.is_empty() {
        params.push(("loaders", serde_json::to_string(loaders)?));
    }
    let resp = client
        .get(format!("{API}/project/{project}/version"))
        .query(&params)
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json().await?)
}

fn best(versions: Vec<Version>) -> Option<Version> {
    let first_release = versions.iter().position(|v| v.version_type == "release");
    versions.into_iter().nth(first_release.unwrap_or(0))
}

fn primary(v: &Version) -> Option<&VFile> {
    v.files.iter().find(|f| f.primary).or(v.files.first())
}

fn allowed_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|_| Error::msg("invalid download URL"))?;
    let host_ok = parsed.host_str().is_some_and(|h| ALLOWED_HOSTS.contains(&h));
    if parsed.scheme() == "https" && host_ok {
        Ok(())
    } else {
        Err(Error::msg(format!("download from a disallowed host: {url}")))
    }
}

fn plain_file_name(name: &str) -> Result<&str> {
    let ok = !name.is_empty()
        && !name.starts_with('.')
        && !name.contains(['/', '\\', '\0'])
        && Path::new(name).components().count() == 1;
    if ok { Ok(name) } else { Err(Error::msg(format!("invalid file name: {name}"))) }
}


#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Content {
    Mods,
    Resourcepacks,
    Shaderpacks,
}

impl Content {
    fn folder(self) -> &'static str {
        match self {
            Content::Mods => "mods",
            Content::Resourcepacks => "resourcepacks",
            Content::Shaderpacks => "shaderpacks",
        }
    }

    fn accepts(self, name: &str) -> bool {
        match self {
            Content::Mods => name.ends_with(".jar") || name.ends_with(".jar.disabled"),
            _ => name.ends_with(".zip"),
        }
    }
}


pub async fn install_content(
    client: &reqwest::Client,
    inst: &Instance,
    project_id: &str,
    kind: Content,
    progress: Progress<'_>,
) -> Result<Vec<String>> {
    let tags: &[&str] = if kind == Content::Mods { loader_tags(inst.loader) } else { &[] };
    if kind == Content::Mods && tags.is_empty() {
        return Err(Error::msg("This instance is Vanilla: create one with a mod loader to use mods"));
    }
    let dir = paths::game_dir(&inst.id).join(kind.folder());
    tokio::fs::create_dir_all(&dir).await?;

    let mut queue = vec![project_id.to_string()];
    let mut seen: HashSet<String> = HashSet::new();
    let mut installed = vec![];

    while let Some(project) = queue.pop() {
        if !seen.insert(project.clone()) || seen.len() > 40 {
            continue;
        }
        let is_root = project == project_id;
        let Some(version) = best(versions_of(client, &project, Some(&inst.mc_version), tags).await?) else {
            if is_root {
                return Err(Error::msg(format!("No version compatible with Minecraft {}", inst.mc_version)));
            }
            continue;
        };
        let file = primary(&version).ok_or_else(|| Error::msg("this version has no files"))?;
        allowed_url(&file.url)?;
        let name = plain_file_name(&file.filename)?;
        if !kind.accepts(name) {
            return Err(Error::msg(format!("unexpected format for {}: {name}", kind.folder())));
        }
        progress(&format!("Downloading {name}"), 0, 0);
        net::download(
            client,
            &Job {
                url: file.url.clone(),
                path: dir.join(name),
                check: Check::sha1(file.hashes.sha1.clone()),
                size: file.size,
            },
        )
        .await?;
        installed.push(name.to_string());

        if kind == Content::Mods {
            queue.extend(
                version
                    .dependencies
                    .iter()
                    .filter(|d| d.dependency_type == "required")
                    .filter_map(|d| d.project_id.clone()),
            );
        }
    }
    Ok(installed)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub file_name: String,
    pub size: u64,
    pub enabled: bool,
}

pub fn list_content(instance_id: &str, kind: Content) -> Result<Vec<ModFile>> {
    instance::check_id(instance_id)?;
    let dir = paths::game_dir(instance_id).join(kind.folder());
    let mut out: Vec<ModFile> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            (meta.is_file() && kind.accepts(&name)).then(|| ModFile {
                enabled: !name.ends_with(".disabled"),
                file_name: name,
                size: meta.len(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()));
    Ok(out)
}

fn content_path(instance_id: &str, kind: Content, file_name: &str) -> Result<PathBuf> {
    instance::check_id(instance_id)?;
    let name = plain_file_name(file_name)?;
    if !kind.accepts(name) {
        return Err(Error::msg("invalid file type"));
    }
    Ok(paths::game_dir(instance_id).join(kind.folder()).join(name))
}

pub fn remove_content(instance_id: &str, kind: Content, file_name: &str) -> Result<()> {
    std::fs::remove_file(content_path(instance_id, kind, file_name)?)?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferResult {
    pub done: u32,
    pub skipped: u32,
}

pub fn transfer_content(from: &str, to: &str, kind: Content, files: &[String], remove_source: bool) -> Result<TransferResult> {
    instance::check_id(from)?;
    instance::check_id(to)?;
    if from == to {
        return Err(Error::msg("Pick a different instance"));
    }
    let dest_dir = paths::game_dir(to).join(kind.folder());
    std::fs::create_dir_all(&dest_dir)?;
    let mut result = TransferResult { done: 0, skipped: 0 };
    for file in files {
        let src = content_path(from, kind, file)?;
        let base = file.strip_suffix(".disabled").unwrap_or(file);
        let exists = dest_dir.join(base).exists() || dest_dir.join(format!("{base}.disabled")).exists();
        if exists || !src.is_file() {
            result.skipped += 1;
            continue;
        }
        let dest = dest_dir.join(file);
        let tmp = dest_dir.join(format!(".{file}.part"));
        std::fs::copy(&src, &tmp)?;
        std::fs::rename(&tmp, &dest)?;
        if remove_source {
            std::fs::remove_file(&src)?;
        }
        result.done += 1;
    }
    Ok(result)
}

pub fn set_mod_enabled(instance_id: &str, file_name: &str, enabled: bool) -> Result<()> {
    let from = content_path(instance_id, Content::Mods, file_name)?;
    let base = file_name.strip_suffix(".disabled").unwrap_or(file_name);
    let target = if enabled { base.to_string() } else { format!("{base}.disabled") };
    let to = from.with_file_name(target);
    if to != from {
        std::fs::rename(from, to)?;
    }
    Ok(())
}

pub async fn categories(client: &reqwest::Client, project_type: &str) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Cat {
        name: String,
        project_type: String,
    }
    let all: Vec<Cat> = serde_json::from_str(&net::get_text(client, &format!("{API}/tag/category")).await?)?;
    let mut names: Vec<String> = all.into_iter().filter(|c| c.project_type == project_type).map(|c| c.name).collect();
    names.sort();
    Ok(names)
}


static HASH_CACHE: std::sync::Mutex<Option<HashMap<(PathBuf, u64, u64), String>>> = std::sync::Mutex::new(None);

fn sha1_of(path: &Path) -> Result<String> {
    use sha1::{Digest, Sha1};
    let meta = std::fs::metadata(path)?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let key = (path.to_path_buf(), meta.len(), mtime);
    if let Some(h) = HASH_CACHE.lock().unwrap().get_or_insert_with(HashMap::new).get(&key) {
        return Ok(h.clone());
    }
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = std::io::Read::read(&mut file, &mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hash = hex::encode(hasher.finalize());
    HASH_CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, hash.clone());
    Ok(hash)
}

#[derive(Deserialize, Clone)]
struct ApiVersion {
    id: String,
    project_id: String,
    version_number: String,
    #[serde(default)]
    files: Vec<VFile>,
}

#[derive(Deserialize)]
struct ApiProject {
    id: String,
    slug: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    icon_url: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionMeta {
    pub id: String,
    pub number: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentEntry {
    pub file_name: String,
    pub size: u64,
    pub enabled: bool,
    pub project: Option<ProjectMeta>,
    pub version: Option<VersionMeta>,
    pub update: Option<VersionMeta>,
}

fn update_loaders(inst: &Instance, kind: Content) -> Option<Vec<String>> {
    match kind {
        Content::Mods => Some(loader_tags(inst.loader).iter().map(|s| s.to_string()).collect()),
        Content::Resourcepacks => Some(vec!["minecraft".into()]),
        Content::Shaderpacks => None,
    }
}

async fn lookup_versions(client: &reqwest::Client, hashes: &[String]) -> Result<HashMap<String, ApiVersion>> {
    let resp = client
        .post(format!("{API}/version_files"))
        .json(&serde_json::json!({ "hashes": hashes, "algorithm": "sha1" }))
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json().await?)
}

async fn lookup_updates(client: &reqwest::Client, inst: &Instance, kind: Content, hashes: &[String]) -> Result<HashMap<String, ApiVersion>> {
    let Some(loaders) = update_loaders(inst, kind).filter(|l| !l.is_empty()) else { return Ok(HashMap::new()) };
    let resp = client
        .post(format!("{API}/version_files/update"))
        .json(&serde_json::json!({ "hashes": hashes, "algorithm": "sha1", "loaders": loaders, "game_versions": [inst.mc_version] }))
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json().await?)
}

async fn fetch_projects(client: &reqwest::Client, ids: &[&str]) -> HashMap<String, ApiProject> {
    let mut out = HashMap::new();
    for chunk in ids.chunks(100) {
        let Ok(query) = serde_json::to_string(chunk) else { continue };
        let resp = client.get(format!("{API}/projects")).query(&[("ids", query)]).send().await.and_then(|r| r.error_for_status());
        if let Ok(r) = resp {
            for p in r.json::<Vec<ApiProject>>().await.unwrap_or_default() {
                out.insert(p.id.clone(), p);
            }
        }
    }
    out
}

fn meta_of(p: &ApiProject) -> ProjectMeta {
    ProjectMeta {
        id: p.id.clone(),
        slug: p.slug.clone(),
        title: p.title.clone(),
        description: p.description.clone(),
        icon_url: p.icon_url.clone().filter(|u| u.starts_with("https://cdn.modrinth.com/")),
    }
}

pub async fn content_details(client: &reqwest::Client, inst: &Instance, kind: Content) -> Result<Vec<ContentEntry>> {
    let files = list_content(&inst.id, kind)?;
    let dir = paths::game_dir(&inst.id).join(kind.folder());
    let names: Vec<String> = files.iter().map(|f| f.file_name.clone()).collect();
    let hashes: Vec<(String, String)> = tokio::task::spawn_blocking(move || {
        names.into_iter().filter_map(|n| sha1_of(&dir.join(&n)).ok().map(|h| (n, h))).collect()
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?;
    let unique: Vec<String> = hashes.iter().map(|(_, h)| h.clone()).collect::<HashSet<_>>().into_iter().collect();

    let (versions, updates, projects) = if unique.is_empty() {
        (HashMap::new(), HashMap::new(), HashMap::new())
    } else {
        let versions = lookup_versions(client, &unique).await.unwrap_or_default();
        let updates = lookup_updates(client, inst, kind, &unique).await.unwrap_or_default();
        let ids: Vec<&str> = versions.values().map(|v| v.project_id.as_str()).collect::<HashSet<_>>().into_iter().collect();
        let projects = fetch_projects(client, &ids).await;
        (versions, updates, projects)
    };

    let hash_of: HashMap<&str, &str> = hashes.iter().map(|(n, h)| (n.as_str(), h.as_str())).collect();
    Ok(files
        .into_iter()
        .map(|f| {
            let h = hash_of.get(f.file_name.as_str()).copied().unwrap_or("");
            let version = versions.get(h);
            let latest = updates.get(h).filter(|u| version.is_some_and(|v| v.id != u.id));
            ContentEntry {
                project: version.and_then(|v| projects.get(&v.project_id)).map(|p| ProjectMeta {
                    id: p.id.clone(),
                    slug: p.slug.clone(),
                    title: p.title.clone(),
                    description: p.description.clone(),
                    icon_url: p.icon_url.clone().filter(|u| u.starts_with("https://cdn.modrinth.com/")),
                }),
                version: version.map(|v| VersionMeta { id: v.id.clone(), number: v.version_number.clone() }),
                update: latest.map(|u| VersionMeta { id: u.id.clone(), number: u.version_number.clone() }),
                file_name: f.file_name,
                size: f.size,
                enabled: f.enabled,
            }
        })
        .collect())
}

pub async fn update_content(client: &reqwest::Client, inst: &Instance, kind: Content, file_name: &str) -> Result<String> {
    let old = content_path(&inst.id, kind, file_name)?;
    let hash = sha1_of(&old)?;
    let latest = lookup_updates(client, inst, kind, std::slice::from_ref(&hash))
        .await?
        .remove(&hash)
        .ok_or_else(|| Error::msg("no update available"))?;
    let file = latest
        .files
        .iter()
        .find(|f| f.primary)
        .or(latest.files.first())
        .ok_or_else(|| Error::msg("the new version has no files"))?;
    allowed_url(&file.url)?;
    let new_name = plain_file_name(&file.filename)?;
    if !kind.accepts(new_name) {
        return Err(Error::msg(format!("unexpected format for {}: {new_name}", kind.folder())));
    }
    let dir = paths::game_dir(&inst.id).join(kind.folder());
    net::download(
        client,
        &Job { url: file.url.clone(), path: dir.join(new_name), check: Check::sha1(file.hashes.sha1.clone()), size: file.size },
    )
    .await?;

    let was_disabled = file_name.ends_with(".disabled");
    let final_name = if was_disabled { format!("{new_name}.disabled") } else { new_name.to_string() };
    if was_disabled {
        std::fs::rename(dir.join(new_name), dir.join(&final_name))?;
    }
    if old != dir.join(&final_name) {
        let _ = std::fs::remove_file(old);
    }
    Ok(final_name)
}


#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    name: String,
    files: Vec<IndexFile>,
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
    path: String,
    #[serde(default)]
    hashes: Hashes,
    #[serde(default)]
    env: Option<Env>,
    downloads: Vec<String>,
    #[serde(default)]
    file_size: Option<u64>,
}

#[derive(Deserialize)]
struct Env {
    #[serde(default)]
    client: Option<String>,
}

fn relative_safe(path: &str) -> Option<PathBuf> {
    let p = Path::new(path);
    (!path.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)))).then(|| p.to_path_buf())
}

fn read_index(archive: &Path) -> Result<Index> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
    let entry = zip
        .by_name("modrinth.index.json")
        .map_err(|_| Error::msg("Not a valid Modrinth modpack (modrinth.index.json is missing)"))?;
    let mut text = String::new();
    entry.take(32 << 20).read_to_string(&mut text)?;
    Ok(serde_json::from_str(&text)?)
}

fn extract_overrides(archive: &Path, game_dir: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
    let mut total = 0u64;
    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let Ok(name) = entry.name().map(|n| n.into_owned()) else { continue };
            let Some(rest) = name.strip_prefix(prefix) else { continue };
            let Some(rel) = relative_safe(rest) else { continue };
            let out = game_dir.join(rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let written = std::io::copy(&mut (&mut entry).take(MAX_EXTRACT_BYTES - total), &mut std::fs::File::create(out)?)?;
            total += written;
            if total >= MAX_EXTRACT_BYTES {
                return Err(Error::msg("The modpack is too large to extract"));
            }
        }
    }
    Ok(())
}

pub async fn install_modpack(
    client: &reqwest::Client,
    project_id: &str,
    version_id: Option<&str>,
    mc_version: Option<&str>,
    progress: Progress<'_>,
) -> Result<Instance> {
    progress("Finding the latest version", 0, 0);
    let version = pick_version(versions_of(client, project_id, None, &[]).await?, version_id, mc_version)?;
    let file = version
        .files
        .iter()
        .find(|f| f.filename.ends_with(".mrpack"))
        .ok_or_else(|| Error::msg("The version doesn't contain a .mrpack file"))?;
    allowed_url(&file.url)?;

    let archive = paths::root().join("tmp").join(format!("{}.mrpack", plain_file_name(&file.filename)?));
    let _ = tokio::fs::remove_file(&archive).await;
    progress("Downloading the modpack", 0, 0);
    net::download(
        client,
        &Job { url: file.url.clone(), path: archive.clone(), check: Check::sha1(file.hashes.sha1.clone()), size: None },
    )
    .await?;

    let path = archive.clone();
    let index = tokio::task::spawn_blocking(move || read_index(&path))
        .await
        .map_err(|e| Error::msg(e.to_string()))??;

    let mc = index
        .dependencies
        .get("minecraft")
        .cloned()
        .ok_or_else(|| Error::msg("The modpack doesn't specify a Minecraft version"))?;
    let (loader, loader_version) = if let Some(v) = index.dependencies.get("fabric-loader") {
        (Loader::Fabric, v.clone())
    } else if let Some(v) = index.dependencies.get("quilt-loader") {
        (Loader::Quilt, v.clone())
    } else if let Some(v) = index.dependencies.get("neoforge") {
        (Loader::NeoForge, v.clone())
    } else if let Some(v) = index.dependencies.get("forge") {
        (Loader::Forge, v.clone())
    } else {
        (Loader::Vanilla, String::new())
    };

    progress("Installing the loader", 0, 0);
    let (loader_version, launch_version) = match loader {
        Loader::Vanilla => (None, None),
        _ => {
            let i = loaders::install(client, loader, &mc, Some(loader_version), progress).await?;
            (Some(i.loader_version), Some(i.id))
        }
    };

    let mut inst = instance::create(&index.name, &mc, loader, loader_version, launch_version)?;
    inst.memory_max_mb = 4096;
    inst.memory_min_mb = inst.memory_min_mb.min(inst.memory_max_mb);
    if let Some((icon, banner)) = project_images(client, project_id).await {
        inst.icon_url = icon;
        inst.banner_url = banner;
    }
    instance::save(&inst)?;

    let result = fill_instance(client, &inst, &index, &archive, progress).await;
    let _ = tokio::fs::remove_file(&archive).await;
    match result {
        Ok(()) => Ok(inst),
        Err(e) => {
            let _ = instance::delete(&inst.id);
            Err(e)
        }
    }
}

async fn project_images(client: &reqwest::Client, project_id: &str) -> Option<(Option<String>, Option<String>)> {
    #[derive(Deserialize)]
    struct Info {
        #[serde(default)]
        icon_url: Option<String>,
        #[serde(default)]
        gallery: Vec<Shot>,
    }
    #[derive(Deserialize)]
    struct Shot {
        url: String,
        #[serde(default)]
        featured: bool,
    }
    if !safe_id(project_id) {
        return None;
    }
    let text = net::get_text(client, &format!("{API}/project/{project_id}")).await.ok()?;
    let info: Info = serde_json::from_str(&text).ok()?;
    let trusted = |u: String| u.starts_with("https://cdn.modrinth.com/").then_some(u);
    let banner = info
        .gallery
        .iter()
        .find(|g| g.featured)
        .or(info.gallery.first())
        .map(|g| g.url.clone())
        .and_then(trusted);
    Some((info.icon_url.and_then(trusted), banner))
}


#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    pub path: String,
    pub file_name: String,
    pub kind: String,
    pub size: u64,
    pub optional: bool,
    pub project: Option<ProjectMeta>,
    pub version: Option<VersionMeta>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackDetails {
    pub version: String,
    pub mc_version: Option<String>,
    pub loader: Option<String>,
    pub files: Vec<PackFile>,
    pub overrides: u32,
}

static PACK_CACHE: std::sync::Mutex<Option<HashMap<String, PackDetails>>> = std::sync::Mutex::new(None);

fn count_overrides(archive: &Path) -> u32 {
    let Ok(file) = std::fs::File::open(archive) else { return 0 };
    let Ok(mut zip) = zip::ZipArchive::new(file) else { return 0 };
    (0..zip.len())
        .filter(|&i| {
            zip.by_index(i).is_ok_and(|e| {
                !e.is_dir() && e.name().is_ok_and(|n| n.starts_with("overrides/") || n.starts_with("client-overrides/"))
            })
        })
        .count() as u32
}

pub async fn modpack_details(client: &reqwest::Client, project_id: &str, version_id: Option<&str>) -> Result<PackDetails> {
    let version = pick_version(versions_of(client, project_id, None, &[]).await?, version_id, None)?;
    if let Some(hit) = PACK_CACHE.lock().unwrap().as_ref().and_then(|m| m.get(&version.id)) {
        return Ok(hit.clone());
    }
    let file = version
        .files
        .iter()
        .find(|f| f.filename.ends_with(".mrpack"))
        .ok_or_else(|| Error::msg("The version doesn't contain a .mrpack file"))?;
    allowed_url(&file.url)?;

    let archive = paths::root().join("tmp").join(format!("peek-{}.mrpack", plain_file_name(&file.filename)?));
    let _ = tokio::fs::remove_file(&archive).await;
    net::download(
        client,
        &Job { url: file.url.clone(), path: archive.clone(), check: Check::sha1(file.hashes.sha1.clone()), size: None },
    )
    .await?;
    let path = archive.clone();
    let parsed = tokio::task::spawn_blocking(move || read_index(&path).map(|i| (i, count_overrides(&path))))
        .await
        .map_err(|e| Error::msg(e.to_string()))?;
    let _ = tokio::fs::remove_file(&archive).await;
    let (index, overrides) = parsed?;

    let hashes: Vec<String> = index.files.iter().filter_map(|f| f.hashes.sha1.clone()).collect::<HashSet<_>>().into_iter().collect();
    let versions = if hashes.is_empty() { HashMap::new() } else { lookup_versions(client, &hashes).await.unwrap_or_default() };
    let ids: Vec<&str> = versions.values().map(|v| v.project_id.as_str()).collect::<HashSet<_>>().into_iter().collect();
    let projects = fetch_projects(client, &ids).await;

    let mut files: Vec<PackFile> = index
        .files
        .iter()
        .filter(|f| f.env.as_ref().and_then(|e| e.client.as_deref()) != Some("unsupported"))
        .map(|f| {
            let ver = f.hashes.sha1.as_deref().and_then(|h| versions.get(h));
            let file_name = f.path.rsplit('/').next().unwrap_or(&f.path).to_string();
            let kind = match f.path.split('/').next().unwrap_or("") {
                "mods" => "mod",
                "resourcepacks" => "resourcepack",
                "shaderpacks" => "shader",
                _ => "other",
            };
            PackFile {
                path: f.path.clone(),
                file_name,
                kind: kind.into(),
                size: f.file_size.unwrap_or(0),
                optional: f.env.as_ref().and_then(|e| e.client.as_deref()) == Some("optional"),
                project: ver.and_then(|v| projects.get(&v.project_id)).map(meta_of),
                version: ver.map(|v| VersionMeta { id: v.id.clone(), number: v.version_number.clone() }),
            }
        })
        .collect();
    files.sort_by(|a, b| {
        let title = |f: &PackFile| f.project.as_ref().map_or_else(|| f.file_name.to_lowercase(), |p| p.title.to_lowercase());
        title(a).cmp(&title(b))
    });

    let loader = ["fabric-loader", "quilt-loader", "neoforge", "forge"]
        .iter()
        .find(|k| index.dependencies.contains_key(**k))
        .map(|k| k.trim_end_matches("-loader").to_string());
    let details = PackDetails {
        version: version.version_number.clone(),
        mc_version: index.dependencies.get("minecraft").cloned(),
        loader,
        files,
        overrides,
    };
    PACK_CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(version.id, details.clone());
    Ok(details)
}


#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackVersion {
    pub id: String,
    pub number: String,
    pub name: String,
    pub kind: String,
    pub mc_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date: String,
    pub downloads: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryImage {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPage {
    pub body: String,
    pub gallery: Vec<GalleryImage>,
    pub versions: Vec<PackVersion>,
}

pub async fn project_page(client: &reqwest::Client, project_id: &str) -> Result<ProjectPage> {
    #[derive(Deserialize)]
    struct Info {
        #[serde(default)]
        body: String,
        #[serde(default)]
        gallery: Vec<Shot>,
    }
    #[derive(Deserialize)]
    struct Shot {
        url: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        ordering: i64,
    }
    if !safe_id(project_id) {
        return Err(Error::msg("invalid project id"));
    }
    let url = format!("{API}/project/{project_id}");
    let (info, versions) = tokio::join!(net::get_text(client, &url), versions_of(client, project_id, None, &[]));
    let info: Info = serde_json::from_str(&info?)?;
    let mut shots = info.gallery;
    shots.sort_by_key(|g| g.ordering);
    Ok(ProjectPage {
        body: info.body.chars().take(60_000).collect(),
        gallery: shots
            .into_iter()
            .filter(|g| g.url.starts_with("https://cdn.modrinth.com/"))
            .take(12)
            .map(|g| GalleryImage { url: g.url, title: g.title.filter(|t| !t.is_empty()) })
            .collect(),
        versions: versions?
            .into_iter()
            .take(60)
            .map(|v| PackVersion {
                id: v.id,
                number: v.version_number,
                name: v.name,
                kind: v.version_type,
                mc_versions: v.game_versions,
                loaders: v.loaders,
                date: v.date_published,
                downloads: v.downloads,
            })
            .collect(),
    })
}

fn pick_version(versions: Vec<Version>, version_id: Option<&str>, mc_version: Option<&str>) -> Result<Version> {
    if let Some(id) = version_id {
        return versions.into_iter().find(|v| v.id == id).ok_or_else(|| Error::msg("That version doesn't exist"));
    }
    let versions = match mc_version {
        Some(mc) if versions.iter().any(|v| v.game_versions.iter().any(|g| g == mc)) => {
            versions.into_iter().filter(|v| v.game_versions.iter().any(|g| g == mc)).collect()
        }
        _ => versions,
    };
    best(versions).ok_or_else(|| Error::msg("The modpack has no versions"))
}

async fn fill_instance(
    client: &reqwest::Client,
    inst: &Instance,
    index: &Index,
    archive: &Path,
    progress: Progress<'_>,
) -> Result<()> {
    let game_dir = paths::game_dir(&inst.id);
    let mut jobs = vec![];
    for f in &index.files {
        if f.env.as_ref().and_then(|e| e.client.as_deref()) == Some("unsupported") {
            continue;
        }
        let rel = relative_safe(&f.path).ok_or_else(|| Error::msg(format!("invalid path in modpack: {}", f.path)))?;
        let url = f.downloads.iter().find(|u| allowed_url(u).is_ok()).ok_or_else(|| {
            Error::msg(format!("no allowed download link for {}", f.path))
        })?;
        jobs.push(Job {
            url: url.clone(),
            path: game_dir.join(rel),
            check: Check::sha1(f.hashes.sha1.clone()),
            size: f.file_size,
        });
    }
    net::download_all(client, jobs, &|d, t| progress("Downloading mods", d, t)).await?;

    progress("Applying configuration", 0, 0);
    let (archive, dir) = (archive.to_path_buf(), game_dir);
    tokio::task::spawn_blocking(move || extract_overrides(&archive, &dir))
        .await
        .map_err(|e| Error::msg(e.to_string()))??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_cannot_escape() {
        assert!(relative_safe("mods/a.jar").is_some());
        assert!(relative_safe("../x").is_none());
        assert!(relative_safe("/etc/passwd").is_none());
        assert!(relative_safe("a/../../b").is_none());
        assert!(relative_safe("").is_none());
    }

    #[test]
    fn only_trusted_hosts() {
        assert!(allowed_url("https://cdn.modrinth.com/data/x/a.jar").is_ok());
        assert!(allowed_url("http://cdn.modrinth.com/a.jar").is_err());
        assert!(allowed_url("https://evil.example/a.jar").is_err());
        assert!(allowed_url("https://cdn.modrinth.com.evil.example/a.jar").is_err());
    }

    #[test]
    fn toggling_a_mod_renames_the_file() {
        let id = format!("test-toggle-{}", std::process::id());
        let dir = paths::game_dir(&id).join("mods");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.jar"), b"x").unwrap();

        set_mod_enabled(&id, "a.jar", false).unwrap();
        let mods = list_content(&id, Content::Mods).unwrap();
        assert_eq!((mods[0].file_name.as_str(), mods[0].enabled), ("a.jar.disabled", false));

        set_mod_enabled(&id, "a.jar.disabled", true).unwrap();
        assert_eq!(list_content(&id, Content::Mods).unwrap()[0].file_name, "a.jar");

        assert!(set_mod_enabled(&id, "../a.jar", false).is_err());
        assert!(remove_content(&id, Content::Mods, "notes.txt").is_err());
        assert!(remove_content(&id, Content::Resourcepacks, "a.jar").is_err());
        remove_content(&id, Content::Mods, "a.jar").unwrap();
        let _ = std::fs::remove_dir_all(paths::instance_dir(&id));
    }

    #[test]
    fn file_names_are_plain() {
        assert!(plain_file_name("sodium-0.6.jar").is_ok());
        assert!(plain_file_name("../a.jar").is_err());
        assert!(plain_file_name("a/b.jar").is_err());
        assert!(plain_file_name(".hidden").is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn modrinth_e2e() {
        let client = net::client();
        let found = search(&client, "fabulously optimized", "modpack", None, &[], &[], "relevance", 0).await.unwrap();
        assert!(found.total > 0);
        let pack = found.hits.iter().find(|h| h.slug == "fabulously-optimized").expect("modpack atteso");
        eprintln!("trovato: {} ({} download)", pack.title, pack.downloads);

        let progress = |s: &str, d: u32, t: u32| {
            if t == 0 || d == t || d % 20 == 0 {
                eprintln!("{s} {d}/{t}");
            }
        };
        let mut inst = install_modpack(&client, &pack.project_id, None, None, &progress).await.unwrap();
        eprintln!("instance: {} mc={} {:?} {:?}", inst.id, inst.mc_version, inst.loader, inst.loader_version);
        let mods = list_content(&inst.id, Content::Mods).unwrap();
        eprintln!("{} mod installate, es. {:?}", mods.len(), mods.iter().take(3).map(|m| &m.file_name).collect::<Vec<_>>());
        assert!(!mods.is_empty());

        let added = install_content(&client, &inst, "mOgUt4GM", Content::Mods, &progress).await.unwrap();
        eprintln!("aggiunte: {added:?}");
        assert!(added.iter().any(|n| n.to_lowercase().contains("modmenu")));

        inst.flag_profile = crate::flags::FlagProfile::Low;
        inst.memory_max_mb = 3072;
        let log = crate::launch::tests::run_and_capture(&client, &inst, 60).await;
        for l in log.iter().filter(|l| l.contains("Loading") || l.contains("Setting user") || l.contains("ERROR")).take(20) {
            eprintln!("[mc] {}", l.chars().take(220).collect::<String>());
        }
        let _ = instance::delete(&inst.id);
        assert!(log.iter().any(|l| l.contains("Loading Minecraft")), "il loader non è partito");
    }

    #[tokio::test]
    #[ignore]
    async fn modpack_details_e2e() {
        let d = modpack_details(&net::client(), "1KVo5zza", None).await.unwrap();
        println!("{} mc={:?} loader={:?} files={} overrides={}", d.version, d.mc_version, d.loader, d.files.len(), d.overrides);
        for f in d.files.iter().take(8) {
            println!("{} | {:?} | {}", f.kind, f.project.as_ref().map(|p| &p.title), f.file_name);
        }
        assert!(!d.files.is_empty());
        assert!(d.files.iter().any(|f| f.project.is_some()), "no recognized files");
    }

    #[tokio::test]
    #[ignore]
    async fn content_details_e2e() {
        let client = net::client();
        let id = format!("test-details-{}", std::process::id());
        let inst = Instance {
            id: id.clone(),
            name: "t".into(),
            mc_version: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: None,
            launch_version: None,
            memory_max_mb: 1024,
            memory_min_mb: 256,
            flag_profile: crate::flags::FlagProfile::Low,
            java_path: None,
            extra_jvm_args: String::new(),
            icon_url: None,
            banner_url: None,
        };
        let mut versions = versions_of(&client, "AANobbMI", Some("1.21.1"), &["fabric"]).await.unwrap();
        let old = versions.pop().expect("Sodium versions");
        let file = primary(&old).unwrap();
        let dir = paths::game_dir(&id).join("mods");
        std::fs::create_dir_all(&dir).unwrap();
        net::download(&client, &Job { url: file.url.clone(), path: dir.join(&file.filename), check: Check::sha1(file.hashes.sha1.clone()), size: file.size })
            .await
            .unwrap();
        std::fs::write(dir.join("handmade.jar"), b"not from modrinth").unwrap();

        let entries = content_details(&client, &inst, Content::Mods).await.unwrap();
        for e in &entries {
            eprintln!("{} -> {:?} {:?} update={:?}", e.file_name, e.project.as_ref().map(|p| &p.title), e.version.as_ref().map(|v| &v.number), e.update.as_ref().map(|v| &v.number));
        }
        let sodium = entries.iter().find(|e| e.file_name == file.filename).unwrap();
        assert_eq!(sodium.project.as_ref().unwrap().title, "Sodium");
        assert!(sodium.update.is_some(), "una versione vecchia dovrebbe avere un aggiornamento");
        assert!(entries.iter().find(|e| e.file_name == "handmade.jar").unwrap().project.is_none());

        let new_name = update_content(&client, &inst, Content::Mods, &file.filename).await.unwrap();
        eprintln!("aggiornato: {} -> {new_name}", file.filename);
        assert!(dir.join(&new_name).is_file() && !dir.join(&file.filename).exists());
        let after = content_details(&client, &inst, Content::Mods).await.unwrap();
        assert!(after.iter().find(|e| e.file_name == new_name).unwrap().update.is_none());
        let _ = std::fs::remove_dir_all(paths::instance_dir(&id));
    }
}
