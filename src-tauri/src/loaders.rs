
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::minecraft::{self, VersionJson};
use crate::net::{self, Check, Job};
use crate::{java, paths, runtime};

pub type Progress<'a> = &'a (dyn Fn(&str, u32, u32) + Sync);

const FORGE_MAVEN: &str = "https://maven.minecraftforge.net";
const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases";

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    #[default]
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

struct Artifact {
    base: &'static str,
    group: &'static str,
    name: &'static str,
    prefix: String,
    full: bool,
}

impl Artifact {
    fn metadata_url(&self) -> String {
        format!("{}/{}/{}/maven-metadata.xml", self.base, self.group, self.name)
    }

    fn version(&self, v: &str) -> String {
        if self.full { format!("{}{v}", self.prefix) } else { v.to_string() }
    }

    fn installer_url(&self, v: &str) -> String {
        let full = self.version(v);
        format!("{}/{}/{}/{full}/{}-{full}-installer.jar", self.base, self.group, self.name, self.name)
    }
}

fn neoforge_prefix(mc: &str) -> String {
    let stripped = mc.strip_prefix("1.");
    let want = if stripped.is_some() { 2 } else { 3 };
    let mut parts: Vec<&str> = stripped.unwrap_or(mc).split('.').collect();
    while parts.len() < want {
        parts.push("0");
    }
    format!("{}.", parts.join("."))
}

fn artifact(loader: Loader, mc: &str) -> Option<Artifact> {
    match loader {
        Loader::Forge => Some(Artifact {
            base: FORGE_MAVEN,
            group: "net/minecraftforge",
            name: "forge",
            prefix: format!("{mc}-"),
            full: true,
        }),
        Loader::NeoForge if mc == "1.20.1" => Some(Artifact {
            base: NEOFORGE_MAVEN,
            group: "net/neoforged",
            name: "forge",
            prefix: format!("{mc}-"),
            full: true,
        }),
        Loader::NeoForge => Some(Artifact {
            base: NEOFORGE_MAVEN,
            group: "net/neoforged",
            name: "neoforge",
            prefix: neoforge_prefix(mc),
            full: false,
        }),
        _ => None,
    }
}

impl Loader {
    fn meta_base(self) -> Option<&'static str> {
        match self {
            Loader::Vanilla => None,
            Loader::Fabric => Some("https://meta.fabricmc.net/v2"),
            Loader::Quilt => Some("https://meta.quiltmc.org/v3"),
            Loader::Forge | Loader::NeoForge => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Loader::Vanilla => "Vanilla",
            Loader::Fabric => "Fabric",
            Loader::Quilt => "Quilt",
            Loader::Forge => "Forge",
            Loader::NeoForge => "NeoForge",
        }
    }
}

#[derive(Deserialize)]
struct Entry {
    loader: Info,
}

#[derive(Deserialize)]
struct Info {
    version: String,
    #[serde(default)]
    stable: Option<bool>,
}

pub struct Installed {
    pub id: String,
    pub loader_version: String,
}

fn safe(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
}

fn is_stable(i: &Info) -> bool {
    match i.stable {
        Some(s) => s,
        None => !["alpha", "beta", "rc", "pre"].iter().any(|t| i.version.contains(t)),
    }
}

fn maven_versions(xml: &str, art: &Artifact) -> Vec<String> {
    let mut out: Vec<String> = xml
        .split("<version>")
        .skip(1)
        .filter_map(|chunk| chunk.split("</version>").next())
        .filter(|v| v.starts_with(art.prefix.as_str()) && v.len() > art.prefix.len())
        .map(|v| if art.full { v[art.prefix.len()..].to_string() } else { v.to_string() })
        .collect();
    out.reverse();
    let (stable, other): (Vec<_>, Vec<_>) = out
        .into_iter()
        .partition(|v| !["alpha", "beta", "rc", "pre", "snapshot"].iter().any(|t| v.to_lowercase().contains(t)));
    stable.into_iter().chain(other).collect()
}

pub async fn versions(client: &reqwest::Client, loader: Loader, mc: &str) -> Result<Vec<String>> {
    if !safe(mc) {
        return Err(Error::msg(format!("{} doesn't support version {mc}", loader.label())));
    }
    if let Some(art) = artifact(loader, mc) {
        let xml = net::get_text(client, &art.metadata_url()).await?;
        return Ok(maven_versions(&xml, &art));
    }
    let base = loader.meta_base().ok_or_else(|| Error::msg("Vanilla has no loader"))?;
    let entries: Vec<Entry> =
        serde_json::from_str(&net::get_text(client, &format!("{base}/versions/loader/{mc}")).await?)?;
    let (stable, other): (Vec<_>, Vec<_>) = entries.into_iter().map(|e| e.loader).partition(is_stable);
    Ok(stable.into_iter().chain(other).map(|i| i.version).collect())
}

pub async fn install(
    client: &reqwest::Client,
    loader: Loader,
    mc: &str,
    requested: Option<String>,
    progress: Progress<'_>,
) -> Result<Installed> {
    if loader == Loader::Vanilla {
        return Err(Error::msg("Vanilla has no loader"));
    }
    let available = versions(client, loader, mc).await?;
    let loader_version = match requested {
        Some(v) if available.contains(&v) => v,
        _ => available
            .into_iter()
            .next()
            .ok_or_else(|| Error::msg(format!("{} doesn't support Minecraft {mc} yet", loader.label())))?,
    };
    if !safe(&loader_version) {
        return Err(Error::msg("invalid loader version"));
    }

    if let Some(art) = artifact(loader, mc) {
        let id = install_forge_like(client, &art, mc, &loader_version, progress).await?;
        return Ok(Installed { id, loader_version });
    }

    let base = loader.meta_base().ok_or_else(|| Error::msg("Vanilla has no loader"))?;
    let text = net::get_text(client, &format!("{base}/versions/loader/{mc}/{loader_version}/profile/json")).await?;
    let profile: VersionJson = serde_json::from_str(&text)?;
    if !safe(&profile.id) {
        return Err(Error::msg("invalid profile id"));
    }
    let dir = paths::versions().join(&profile.id);
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join(format!("{}.json", profile.id)), text).await?;
    Ok(Installed { id: profile.id, loader_version })
}

fn installer_version_id(jar: &std::path::Path) -> Result<String> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::fs::File::open(jar)?)?;
    let mut read = |name: &str| -> Option<serde_json::Value> {
        let mut entry = archive.by_name(name).ok()?;
        let mut text = String::new();
        entry.read_to_string(&mut text).ok()?;
        serde_json::from_str(&text).ok()
    };
    let id = read("version.json")
        .and_then(|v| v.get("id").and_then(|i| i.as_str().map(String::from)))
        .or_else(|| {
            read("install_profile.json").and_then(|v| {
                v.pointer("/versionInfo/id").or_else(|| v.get("version")).and_then(|i| i.as_str().map(String::from))
            })
        })
        .ok_or_else(|| Error::msg("The installer doesn't describe a version"))?;
    if safe(&id) { Ok(id) } else { Err(Error::msg("invalid profile id")) }
}

async fn installer_java(client: &reqwest::Client, mc: &str, progress: Progress<'_>) -> Result<java::JavaInstall> {
    let required = minecraft::resolve(client, mc).await?.required_java().max(17);
    match runtime::ensure(client, required, progress).await {
        Ok(Some(j)) => Ok(j),
        other => {
            let all = java::detect().await;
            java::pick(&all, required).cloned().ok_or_else(|| {
                let reason = other.err().map(|e| format!(" ({e})")).unwrap_or_default();
                Error::msg(format!("Java {required} or newer is required and couldn't be obtained{reason}"))
            })
        }
    }
}

async fn install_forge_like(
    client: &reqwest::Client,
    art: &Artifact,
    mc: &str,
    loader_version: &str,
    progress: Progress<'_>,
) -> Result<String> {
    let full = art.version(loader_version);
    if !safe(&full) {
        return Err(Error::msg("invalid loader version"));
    }
    let dir = paths::root().join("installers");
    tokio::fs::create_dir_all(&dir).await?;
    let jar = dir.join(format!("{}-{full}-installer.jar", art.name));
    progress("Downloading the loader installer", 0, 0);
    net::download(
        client,
        &Job { url: art.installer_url(loader_version), path: jar.clone(), check: Check::None, size: None },
    )
    .await?;

    let reader = jar.clone();
    let id = tokio::task::spawn_blocking(move || installer_version_id(&reader))
        .await
        .map_err(|e| Error::msg(e.to_string()))??;

    let version_dir = paths::versions().join(&id);
    let marker = version_dir.join(".piuma-installed");
    if marker.is_file() && version_dir.join(format!("{id}.json")).is_file() {
        return Ok(id);
    }

    let java = installer_java(client, mc, progress).await?;
    minecraft::resolve(client, mc).await?;
    let root = paths::root();
    let profiles = root.join("launcher_profiles.json");
    if !profiles.is_file() {
        tokio::fs::write(&profiles, r#"{"profiles":{},"version":3}"#).await?;
    }

    progress("Installing the loader", 0, 0);
    let output = tokio::process::Command::new(&java.path)
        .arg("-jar")
        .arg(&jar)
        .arg("--installClient")
        .arg(&root)
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await?;
    if !output.status.success() || !version_dir.join(format!("{id}.json")).is_file() {
        let log = format!("{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        let tail: Vec<&str> = log.lines().rev().filter(|l| !l.trim().is_empty()).take(3).collect();
        let tail = tail.into_iter().rev().collect::<Vec<_>>().join(" | ");
        return Err(Error::msg(format!("The loader installer failed: {tail}")));
    }
    tokio::fs::write(&marker, b"").await?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_rejects_path_tricks() {
        assert!(safe("fabric-loader-0.19.5-1.21.1"));
        assert!(!safe("../x"));
        assert!(!safe("a/b"));
        assert!(!safe(""));
    }

    #[test]
    fn neoforge_prefix_follows_minecraft_version() {
        assert_eq!(neoforge_prefix("1.21.1"), "21.1.");
        assert_eq!(neoforge_prefix("1.21"), "21.0.");
        assert_eq!(neoforge_prefix("26.1.2"), "26.1.2.");
        assert_eq!(neoforge_prefix("26.3"), "26.3.0.");
    }

    #[test]
    fn maven_versions_filter_and_order() {
        let art = artifact(Loader::Forge, "1.21.1").unwrap();
        let xml = "<version>1.20.1-47.0.1</version><version>1.21.1-52.0.1</version><version>1.21.1-52.0.9</version><version>1.21.1-52.1.0-beta</version>";
        assert_eq!(maven_versions(xml, &art), vec!["52.0.9", "52.0.1", "52.1.0-beta"]);
    }

    #[test]
    fn quilt_betas_are_not_stable() {
        let beta = Info { version: "0.20.0-beta.9".into(), stable: None };
        let rel = Info { version: "0.27.1".into(), stable: None };
        assert!(!is_stable(&beta));
        assert!(is_stable(&rel));
    }
}
