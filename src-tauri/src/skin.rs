
use std::time::{Duration, SystemTime};

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::paths;

const CACHE_TTL: Duration = Duration::from_secs(24 * 3600);
const MAX_PNG: usize = 256 * 1024;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkinInfo {
    pub png: String,
    pub slim: bool,
}

#[derive(Deserialize)]
struct Profile {
    id: String,
}

#[derive(Deserialize)]
struct Session {
    properties: Vec<Prop>,
}

#[derive(Deserialize)]
struct Prop {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct Textures {
    textures: TexMap,
}

#[derive(Deserialize)]
struct TexMap {
    #[serde(rename = "SKIN")]
    skin: Option<Tex>,
}

#[derive(Deserialize)]
struct Tex {
    url: String,
    metadata: Option<Meta>,
}

#[derive(Deserialize)]
struct Meta {
    model: Option<String>,
}

pub fn valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn parse_textures(session: &Session) -> Option<(String, bool)> {
    let value = &session.properties.iter().find(|p| p.name == "textures")?.value;
    let json = STANDARD.decode(value).ok()?;
    let tex: Textures = serde_json::from_slice(&json).ok()?;
    let skin = tex.textures.skin?;
    let slim = skin.metadata.and_then(|m| m.model).is_some_and(|m| m == "slim");
    Some((skin.url, slim))
}

pub fn trusted_texture_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://"))?;
    rest.starts_with("textures.minecraft.net/").then(|| format!("https://{rest}"))
}

fn cache_file(name: &str) -> std::path::PathBuf {
    paths::root().join("skins").join(format!("{}.json", name.to_ascii_lowercase()))
}

fn read_cache(name: &str, ignore_age: bool) -> Option<Option<SkinInfo>> {
    let path = cache_file(name);
    let age = SystemTime::now().duration_since(std::fs::metadata(&path).ok()?.modified().ok()?).ok()?;
    if !ignore_age && age > CACHE_TTL {
        return None;
    }
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn write_cache(name: &str, info: &Option<SkinInfo>) {
    let path = cache_file(name);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(info) {
        let _ = std::fs::write(path, text);
    }
}

async fn download(client: &reqwest::Client, name: &str) -> Result<Option<SkinInfo>> {
    let resp = client
        .get(format!("https://api.mojang.com/users/profiles/minecraft/{name}"))
        .send()
        .await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND || resp.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(None);
    }
    let profile: Profile = resp.error_for_status()?.json().await?;
    if !profile.id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::msg("unexpected profile id"));
    }

    let session: Session = client
        .get(format!("https://sessionserver.mojang.com/session/minecraft/profile/{}", profile.id))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let Some((url, slim)) = parse_textures(&session) else { return Ok(None) };
    Ok(Some(SkinInfo { png: texture(client, &url).await?, slim }))
}

pub async fn texture(client: &reqwest::Client, url: &str) -> Result<String> {
    let url = trusted_texture_url(url).ok_or_else(|| Error::msg("unexpected skin host"))?;
    let bytes = client.get(url).send().await?.error_for_status()?.bytes().await?;
    if bytes.len() > MAX_PNG || !bytes.starts_with(b"\x89PNG") {
        return Err(Error::msg("unexpected skin file"));
    }
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(&bytes)))
}

pub fn remember(name: &str, info: SkinInfo) {
    write_cache(name, &Some(info));
}

pub async fn fetch(client: &reqwest::Client, name: &str) -> Result<Option<SkinInfo>> {
    if !valid_name(name) {
        return Err(Error::msg("invalid player name"));
    }
    if !crate::settings::load().lookup_skins {
        return Ok(None);
    }
    if let Some(cached) = read_cache(name, false) {
        return Ok(cached);
    }
    match download(client, name).await {
        Ok(info) => {
            write_cache(name, &info);
            Ok(info)
        }
        Err(e) => read_cache(name, true).ok_or(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net;

    #[test]
    fn names_are_validated() {
        assert!(valid_name("Notch"));
        assert!(valid_name("a_b_1"));
        assert!(!valid_name("ab"));
        assert!(!valid_name("../x"));
        assert!(!valid_name("seventeen_chars__"));
    }

    #[test]
    fn only_mojang_textures_are_trusted() {
        assert_eq!(
            trusted_texture_url("http://textures.minecraft.net/texture/abc").as_deref(),
            Some("https://textures.minecraft.net/texture/abc")
        );
        assert!(trusted_texture_url("https://evil.example/texture/abc").is_none());
        assert!(trusted_texture_url("https://textures.minecraft.net.evil.example/x").is_none());
    }

    #[test]
    fn slim_model_is_detected() {
        let json = r#"{"textures":{"SKIN":{"url":"http://textures.minecraft.net/texture/x","metadata":{"model":"slim"}}}}"#;
        let session = Session { properties: vec![Prop { name: "textures".into(), value: STANDARD.encode(json) }] };
        assert_eq!(parse_textures(&session), Some(("http://textures.minecraft.net/texture/x".into(), true)));
    }

    #[tokio::test]
    #[ignore]
    async fn real_skin() {
        let info = fetch(&net::client(), "Notch").await.unwrap().expect("Notch esiste");
        assert!(info.png.starts_with("data:image/png;base64,"));
        assert!(fetch(&net::client(), "zzqxk_nobody_77").await.unwrap().is_none());
    }
}
