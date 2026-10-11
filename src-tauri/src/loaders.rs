
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::minecraft::VersionJson;
use crate::{net, paths};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    #[default]
    Vanilla,
    Fabric,
    Quilt,
}

impl Loader {
    fn meta_base(self) -> Option<&'static str> {
        match self {
            Loader::Vanilla => None,
            Loader::Fabric => Some("https://meta.fabricmc.net/v2"),
            Loader::Quilt => Some("https://meta.quiltmc.org/v3"),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Loader::Vanilla => "Vanilla",
            Loader::Fabric => "Fabric",
            Loader::Quilt => "Quilt",
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

pub async fn versions(client: &reqwest::Client, loader: Loader, mc: &str) -> Result<Vec<String>> {
    let base = loader.meta_base().ok_or_else(|| Error::msg("Vanilla has no loader"))?;
    if !safe(mc) {
        return Err(Error::msg(format!("{} doesn't support version {mc}", loader.label())));
    }
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
) -> Result<Installed> {
    let base = loader.meta_base().ok_or_else(|| Error::msg("Vanilla has no loader"))?;
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
    fn quilt_betas_are_not_stable() {
        let beta = Info { version: "0.20.0-beta.9".into(), stable: None };
        let rel = Info { version: "0.27.1".into(), stable: None };
        assert!(!is_stable(&beta));
        assert!(is_stable(&rel));
    }
}
