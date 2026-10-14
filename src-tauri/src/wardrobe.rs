use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};

use crate::accounts::{self, AccountKind};
use crate::error::{Error, Result};
use crate::skin::{self, SkinInfo};
use crate::{microsoft, net};

const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const MAX_UPLOAD: usize = 64 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Wardrobe {
    pub skin: Option<SkinInfo>,
    pub capes: Vec<Cape>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cape {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub png: Option<String>,
}

#[derive(Deserialize)]
struct Profile {
    #[serde(default)]
    skins: Vec<SkinEntry>,
    #[serde(default)]
    capes: Vec<CapeEntry>,
}

#[derive(Deserialize)]
struct SkinEntry {
    state: String,
    url: String,
    variant: Option<String>,
}

#[derive(Deserialize)]
struct CapeEntry {
    id: String,
    state: String,
    url: String,
    alias: Option<String>,
}

async fn token(account_id: &str) -> Result<(String, String)> {
    let account = accounts::get(account_id).ok_or_else(|| Error::msg("Account not found"))?;
    if account.kind != AccountKind::Microsoft {
        return Err(Error::msg("Only Microsoft accounts can change skin and cape"));
    }
    Ok((microsoft::session(account_id).await?.token, account.name))
}

fn refused(status: reqwest::StatusCode) -> Error {
    Error::msg(match status.as_u16() {
        400 => "Mojang refused the file. Use a 64x64 or 64x32 PNG skin",
        401 | 403 => "The Microsoft session expired. Sign in again from Accounts",
        429 => "Too many changes. Wait a minute and try again",
        _ => "Mojang could not apply the change. Try again later",
    })
}

pub async fn load(account_id: &str) -> Result<Wardrobe> {
    let (token, _) = token(account_id).await?;
    let client = net::client();
    let resp = client.get(PROFILE_URL).bearer_auth(&token).send().await?;
    if !resp.status().is_success() {
        return Err(refused(resp.status()));
    }
    let profile: Profile = resp.json().await?;

    let skin = match profile.skins.iter().find(|s| s.state == "ACTIVE") {
        Some(s) => skin::texture(&client, &s.url)
            .await
            .ok()
            .map(|png| SkinInfo { png, slim: s.variant.as_deref().is_some_and(|v| v.eq_ignore_ascii_case("slim")) }),
        None => None,
    };
    let capes = join_all(profile.capes.iter().map(|c| {
        let client = client.clone();
        async move {
            Cape {
                id: c.id.clone(),
                name: c.alias.clone().unwrap_or_else(|| "Cape".into()),
                active: c.state == "ACTIVE",
                png: skin::texture(&client, &c.url).await.ok(),
            }
        }
    }))
    .await;
    Ok(Wardrobe { skin, capes })
}

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

fn multipart(boundary: &str, slim: bool, png: &[u8]) -> Vec<u8> {
    let variant = if slim { "slim" } else { "classic" };
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{variant}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(png);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

pub async fn set_skin(account_id: &str, png_base64: &str, slim: bool) -> Result<()> {
    let png = STANDARD.decode(png_base64.trim()).map_err(|_| Error::msg("This is not a valid image"))?;
    if png.len() > MAX_UPLOAD {
        return Err(Error::msg("The skin file is too big"));
    }
    if !matches!(png_size(&png), Some((64, 64)) | Some((64, 32))) {
        return Err(Error::msg("A skin must be a 64x64 or 64x32 PNG"));
    }
    let (token, name) = token(account_id).await?;
    let boundary = format!("piuma{}", uuid::Uuid::new_v4().simple());
    let resp = net::client()
        .post(format!("{PROFILE_URL}/skins"))
        .bearer_auth(&token)
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .body(multipart(&boundary, slim, &png))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(refused(resp.status()));
    }
    skin::remember(&name, SkinInfo { png: format!("data:image/png;base64,{}", STANDARD.encode(&png)), slim });
    Ok(())
}

pub async fn set_cape(account_id: &str, cape_id: Option<&str>) -> Result<()> {
    let (token, _) = token(account_id).await?;
    let client = net::client();
    let url = format!("{PROFILE_URL}/capes/active");
    let resp = match cape_id {
        Some(id) => {
            if !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
                return Err(Error::msg("Unknown cape"));
            }
            client.put(url).bearer_auth(&token).json(&serde_json::json!({ "capeId": id })).send().await?
        }
        None => client.delete(url).bearer_auth(&token).send().await?,
    };
    if !resp.status().is_success() {
        return Err(refused(resp.status()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b
    }

    #[test]
    fn png_dimensions_are_read() {
        assert_eq!(png_size(&header(64, 64)), Some((64, 64)));
        assert_eq!(png_size(&header(64, 32)), Some((64, 32)));
        assert_eq!(png_size(b"not a png at all, really not"), None);
    }

    #[test]
    fn multipart_wraps_the_file() {
        let body = multipart("B", true, b"PNGDATA");
        let text = String::from_utf8_lossy(&body);
        assert!(text.starts_with("--B\r\n"));
        assert!(text.contains("name=\"variant\"\r\n\r\nslim\r\n"));
        assert!(text.contains("PNGDATA\r\n--B--\r\n"));
    }
}
