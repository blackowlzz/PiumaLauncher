
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::{net, paths};

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const LIBRARIES_BASE: &str = "https://libraries.minecraft.net/";
pub const RESOURCES_BASE: &str = "https://resources.download.minecraft.net/";


#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub versions: Vec<ManifestEntry>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub release_time: String,
}

pub async fn manifest(client: &reqwest::Client) -> Result<Manifest> {
    match net::get_text(client, MANIFEST_URL).await {
        Ok(text) => {
            let parsed: Manifest = serde_json::from_str(&text)?;
            let cache = paths::manifest_cache();
            if let Some(dir) = cache.parent() {
                let _ = tokio::fs::create_dir_all(dir).await;
            }
            let _ = tokio::fs::write(cache, text).await;
            Ok(parsed)
        }
        Err(e) => match tokio::fs::read_to_string(paths::manifest_cache()).await {
            Ok(text) => Ok(serde_json::from_str(&text)?),
            Err(_) => Err(e),
        },
    }
}


#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VersionJson {
    pub id: String,
    pub inherits_from: Option<String>,
    pub main_class: Option<String>,
    pub minecraft_arguments: Option<String>,
    pub arguments: Option<Arguments>,
    pub asset_index: Option<AssetIndexRef>,
    pub assets: Option<String>,
    pub downloads: Option<VersionDownloads>,
    pub libraries: Vec<Library>,
    pub java_version: Option<JavaVersion>,
    pub logging: Option<Logging>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Arguments {
    pub game: Vec<Arg>,
    pub jvm: Vec<Arg>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Arg {
    Plain(String),
    Ruled { rules: Vec<Rule>, value: OneOrMany },
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Deserialize, Clone)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct VersionDownloads {
    pub client: Option<Artifact>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Artifact {
    pub path: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub major_version: u32,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Logging {
    pub client: Option<LoggingClient>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    pub virtual_: bool,
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Debug, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}


pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn arch_name() -> &'static str {
    if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

fn rule_matches(r: &Rule) -> bool {
    if let Some(os) = &r.os {
        if os.name.as_deref().is_some_and(|n| n != os_name()) {
            return false;
        }
        if os.arch.as_deref().is_some_and(|a| a != arch_name()) {
            return false;
        }
    }
    if let Some(f) = &r.features {
        if f.values().any(|v| *v) {
            return false;
        }
    }
    true
}

pub fn rules_allow(rules: &[Rule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for r in rules {
        if rule_matches(r) {
            allowed = r.action == "allow";
        }
    }
    allowed
}


#[derive(Debug, Deserialize, Clone)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibDownloads>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub extract: Option<Extract>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct LibDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Extract {
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LibFile {
    pub path: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

impl LibFile {
    pub fn local_path(&self) -> PathBuf {
        paths::libraries().join(&self.path)
    }
}

fn maven_path(name: &str) -> Option<String> {
    let (coords, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let p: Vec<&str> = coords.split(':').collect();
    if p.len() < 3 {
        return None;
    }
    let file = if p.len() >= 4 {
        format!("{}-{}-{}.{ext}", p[1], p[2], p[3])
    } else {
        format!("{}-{}.{ext}", p[1], p[2])
    };
    Some(format!("{}/{}/{}/{file}", p[0].replace('.', "/"), p[1], p[2]))
}

fn lib_file_from(a: &Artifact, fallback_path: Option<String>) -> Option<LibFile> {
    let path = a.path.clone().or(fallback_path)?;
    Some(LibFile {
        url: if a.url.is_empty() { format!("{LIBRARIES_BASE}{path}") } else { a.url.clone() },
        path,
        sha1: a.sha1.clone(),
        size: a.size,
    })
}

impl Library {
    pub fn applies(&self) -> bool {
        rules_allow(&self.rules)
    }

    fn key(&self) -> String {
        let p: Vec<&str> = self.name.split('@').next().unwrap_or("").split(':').collect();
        match p.len() {
            0..=2 => self.name.clone(),
            3 => format!("{}:{}", p[0], p[1]),
            _ => format!("{}:{}:{}", p[0], p[1], p[3]),
        }
    }

    pub fn main_file(&self) -> Option<LibFile> {
        if let Some(a) = self.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            return lib_file_from(a, maven_path(&self.name));
        }
        if self.natives.is_some() {
            return None;
        }
        let path = maven_path(&self.name)?;
        let base = self.url.as_deref().unwrap_or(LIBRARIES_BASE);
        Some(LibFile {
            url: format!("{}/{}", base.trim_end_matches('/'), path),
            path,
            sha1: self.sha1.clone(),
            size: None,
        })
    }

    pub fn native_file(&self) -> Option<LibFile> {
        let classifier = self.natives.as_ref()?.get(os_name())?;
        let classifier = classifier.replace("${arch}", if cfg!(target_pointer_width = "32") { "32" } else { "64" });
        let a = self.downloads.as_ref()?.classifiers.as_ref()?.get(&classifier)?;
        lib_file_from(a, None)
    }
}


#[derive(Debug, Clone)]
pub struct Resolved {
    pub version: VersionJson,
    pub jar_id: String,
}

impl VersionJson {
    fn merge_over(mut self, parent: VersionJson) -> VersionJson {
        let mut libraries = std::mem::take(&mut self.libraries);
        libraries.extend(parent.libraries);

        let arguments = match (parent.arguments, self.arguments.take()) {
            (Some(mut p), Some(c)) => {
                p.game.extend(c.game);
                p.jvm.extend(c.jvm);
                Some(p)
            }
            (p, c) => c.or(p),
        };

        VersionJson {
            id: self.id,
            inherits_from: None,
            main_class: self.main_class.or(parent.main_class),
            minecraft_arguments: self.minecraft_arguments.or(parent.minecraft_arguments),
            arguments,
            asset_index: self.asset_index.or(parent.asset_index),
            assets: self.assets.or(parent.assets),
            downloads: self.downloads.or(parent.downloads),
            libraries,
            java_version: self.java_version.or(parent.java_version),
            logging: self.logging.or(parent.logging),
            kind: self.kind.or(parent.kind),
        }
    }
}

fn version_json_path(id: &str) -> PathBuf {
    paths::versions().join(id).join(format!("{id}.json"))
}

async fn load_version_json(client: &reqwest::Client, id: &str) -> Result<VersionJson> {
    let path = version_json_path(id);
    if let Ok(text) = tokio::fs::read_to_string(&path).await {
        return Ok(serde_json::from_str(&text)?);
    }
    let entry = manifest(client)
        .await?
        .versions
        .into_iter()
        .find(|v| v.id == id)
        .ok_or_else(|| Error::msg(format!("unknown Minecraft version: {id}")))?;
    let text = net::get_text(client, &entry.url).await?;
    let parsed: VersionJson = serde_json::from_str(&text)?;
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    tokio::fs::write(&path, text).await?;
    Ok(parsed)
}

pub async fn resolve(client: &reqwest::Client, id: &str) -> Result<Resolved> {
    let mut chain = Vec::new();
    let mut current = id.to_string();
    loop {
        let v = load_version_json(client, &current).await?;
        let parent = v.inherits_from.clone();
        chain.push(v);
        match parent {
            Some(p) if chain.len() < 6 => current = p,
            Some(_) => return Err(Error::msg("inheritsFrom chain too long")),
            None => break,
        }
    }
    let jar_id = chain.last().map(|v| v.id.clone()).unwrap_or_else(|| id.to_string());
    let mut iter = chain.into_iter().rev();
    let mut acc = iter.next().expect("catena non vuota");
    for child in iter {
        acc = child.merge_over(acc);
    }
    Ok(Resolved { version: acc, jar_id })
}

impl Resolved {
    pub fn client_jar(&self) -> PathBuf {
        paths::versions().join(&self.jar_id).join(format!("{}.jar", self.jar_id))
    }

    pub fn required_java(&self) -> u32 {
        self.version.java_version.as_ref().map_or(8, |j| j.major_version)
    }

    pub fn classpath_files(&self) -> Vec<LibFile> {
        let mut seen = HashSet::new();
        self.version
            .libraries
            .iter()
            .filter(|l| l.applies())
            .filter(|l| seen.insert(l.key()))
            .filter_map(|l| l.main_file())
            .collect()
    }

    pub fn native_files(&self) -> Vec<(LibFile, Vec<String>)> {
        self.version
            .libraries
            .iter()
            .filter(|l| l.applies())
            .filter_map(|l| {
                let exclude = l.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default();
                l.native_file().map(|f| (f, exclude))
            })
            .collect()
    }
}


#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionEntry {
    pub id: String,
    pub kind: String,
    pub release_time: String,
}
