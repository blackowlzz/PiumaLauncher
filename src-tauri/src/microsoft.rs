use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::accounts::{self, Account};
use crate::error::{Error, Result};
use crate::{net, paths};

const CLIENT_ID: &str = "00000000441cc96b";
const SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";
const DEVICE_URL: &str = "https://login.live.com/oauth20_connect.srf";
const TOKEN_URL: &str = "https://login.live.com/oauth20_token.srf";
const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const EXPIRY_MARGIN_SECS: u64 = 120;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLogin {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

#[derive(Deserialize)]
struct DeviceResponse {
    user_code: String,
    device_code: String,
    verification_uri: String,
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

fn default_interval() -> u64 {
    5
}

#[derive(Clone)]
struct Pending {
    id: u64,
    device_code: String,
    interval: u64,
    deadline: Instant,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Serialize, Deserialize, Clone)]
struct Tokens {
    refresh_token: String,
    mc_token: String,
    mc_expires_at: u64,
}

pub struct Session {
    pub token: String,
}

struct Profile {
    name: String,
    uuid: String,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn load_tokens() -> HashMap<String, Tokens> {
    std::fs::read_to_string(paths::microsoft_file())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    {
        use std::io::Write;
        opts.open(&tmp)?.write_all(bytes)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn save_tokens(map: &HashMap<String, Tokens>) -> Result<()> {
    write_private(&paths::microsoft_file(), serde_json::to_string(map)?.as_bytes())
}

pub fn forget(id: &str) {
    let mut map = load_tokens();
    if map.remove(id).is_some() {
        let _ = save_tokens(&map);
    }
}

pub async fn start() -> Result<DeviceLogin> {
    let res: DeviceResponse = net::client()
        .post(DEVICE_URL)
        .form(&[("scope", SCOPE), ("client_id", CLIENT_ID), ("response_type", "device_code")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    *PENDING.lock().unwrap() = Some(Pending {
        id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        device_code: res.device_code,
        interval: res.interval.max(1),
        deadline: Instant::now() + Duration::from_secs(res.expires_in),
    });
    Ok(DeviceLogin { user_code: res.user_code, verification_uri: res.verification_uri, expires_in: res.expires_in })
}

pub fn cancel() {
    *PENDING.lock().unwrap() = None;
}

fn still_current(id: u64) -> bool {
    PENDING.lock().unwrap().as_ref().is_some_and(|p| p.id == id)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    error: Option<String>,
}

pub async fn finish() -> Result<Account> {
    let mut pending = PENDING.lock().unwrap().clone().ok_or_else(|| Error::msg("Start the login first"))?;
    let client = net::client();
    let (access, refresh) = loop {
        tokio::time::sleep(Duration::from_secs(pending.interval)).await;
        if !still_current(pending.id) {
            return Err(Error::msg("Login cancelled"));
        }
        if Instant::now() > pending.deadline {
            cancel();
            return Err(Error::msg("The code expired. Try again"));
        }
        let res: TokenResponse = client
            .post(TOKEN_URL)
            .query(&[("client_id", CLIENT_ID)])
            .form(&[
                ("client_id", CLIENT_ID),
                ("device_code", pending.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?
            .json()
            .await?;
        match (res.access_token, res.refresh_token, res.error.as_deref()) {
            (Some(a), Some(r), _) => break (a, r),
            (_, _, Some("authorization_pending")) | (_, _, None) => {}
            (_, _, Some("slow_down")) => pending.interval += 5,
            (_, _, Some("authorization_declined")) => {
                cancel();
                return Err(Error::msg("Login declined"));
            }
            (_, _, Some(_)) => {
                cancel();
                return Err(Error::msg("The code expired. Try again"));
            }
        }
    };
    if !still_current(pending.id) {
        return Err(Error::msg("Login cancelled"));
    }
    cancel();

    let (tokens, profile) = exchange(&client, &access, refresh).await?;
    let account = accounts::upsert_microsoft(&profile.name, &profile.uuid)?;
    let mut map = load_tokens();
    map.insert(account.id.clone(), tokens);
    save_tokens(&map)?;
    Ok(account)
}

pub async fn session(account_id: &str) -> Result<Session> {
    let mut map = load_tokens();
    let saved = map.get(account_id).cloned().ok_or_else(expired)?;
    if saved.mc_expires_at > now() + EXPIRY_MARGIN_SECS {
        return Ok(Session { token: saved.mc_token });
    }

    let client = net::client();
    let res: TokenResponse = client
        .post(TOKEN_URL)
        .form(&[
            ("scope", SCOPE),
            ("client_id", CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", saved.refresh_token.as_str()),
        ])
        .send()
        .await?
        .json()
        .await?;
    let (access, refresh) = match (res.access_token, res.refresh_token) {
        (Some(a), Some(r)) => (a, r),
        _ => return Err(expired()),
    };
    let (tokens, profile) = exchange(&client, &access, refresh).await?;
    let session = Session { token: tokens.mc_token.clone() };
    map.insert(account_id.to_string(), tokens);
    save_tokens(&map)?;
    accounts::refresh_microsoft(account_id, &profile.name, &profile.uuid)?;
    Ok(session)
}

fn expired() -> Error {
    Error::msg("The Microsoft session expired. Sign in again from Accounts")
}

async fn exchange(client: &reqwest::Client, ms_access: &str, refresh_token: String) -> Result<(Tokens, Profile)> {
    let xbl: Value = post_json(
        client,
        XBL_URL,
        json!({
            "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": ms_access },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }),
    )
    .await?;
    let xbl_token = str_at(&xbl, "/Token")?;

    let xsts: Value = post_json(
        client,
        XSTS_URL,
        json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }),
    )
    .await?;
    let xsts_token = str_at(&xsts, "/Token")?;
    let user_hash = str_at(&xsts, "/DisplayClaims/xui/0/uhs")?;

    let mc: Value = post_json(
        client,
        MC_LOGIN_URL,
        json!({ "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}") }),
    )
    .await?;
    let mc_token = str_at(&mc, "/access_token")?;
    let lifetime = mc.pointer("/expires_in").and_then(Value::as_u64).unwrap_or(3600);

    let resp = client.get(MC_PROFILE_URL).bearer_auth(&mc_token).send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(Error::msg("This Microsoft account doesn't own Minecraft: Java Edition"));
    }
    let profile: Value = resp.error_for_status()?.json().await?;
    let profile = Profile { name: str_at(&profile, "/name")?, uuid: str_at(&profile, "/id")? };

    Ok((Tokens { refresh_token, mc_token, mc_expires_at: now() + lifetime }, profile))
}

async fn post_json(client: &reqwest::Client, url: &str, body: Value) -> Result<Value> {
    let resp = client
        .post(url)
        .header("Accept", "application/json")
        .header("x-xbl-contract-version", "1")
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let value: Value = resp.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(value);
    }
    if let Some(code) = value.get("XErr").and_then(Value::as_u64) {
        return Err(Error::msg(match code {
            2148916233 => "This Microsoft account has no Xbox profile. Create one at xbox.com first",
            2148916235 => "Xbox Live isn't available in your country",
            2148916236 | 2148916237 => "This account needs adult verification on the Xbox site",
            2148916238 => "This is a child account: an adult must add it to a Microsoft family first",
            _ => "Xbox Live refused the sign-in",
        }));
    }
    Err(Error::msg(format!("Sign-in failed ({status})")))
}

fn str_at(v: &Value, pointer: &str) -> Result<String> {
    v.pointer(pointer)
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| Error::msg("Unexpected reply from Microsoft"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_file_is_private_and_replaced_atomically() {
        let dir = std::env::temp_dir().join(format!("piuma-test-{}", std::process::id()));
        let path = dir.join("microsoft.json");
        write_private(&path, b"first").unwrap();
        write_private(&path, b"second-longer").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second-longer");
        assert!(!path.with_extension("json.tmp").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stale_loose_temp_file_does_not_leak_permissions() {
        let dir = std::env::temp_dir().join(format!("piuma-test-stale-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("microsoft.json");
        let stale = path.with_extension("json.tmp");
        std::fs::write(&stale, b"old").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stale, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        write_private(&path, b"secret").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
