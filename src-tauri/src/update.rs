use std::cmp::Ordering;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::error::{Error, Result};
use crate::{diag, Running};

const REPO: &str = "blackowlzz/PiumaLauncher";
const PUBLISH_DELAY_SECS: u64 = 300;
const DOWNLOAD_URL: &str = "https://www.piumalauncher.it/download";

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
    pub auto: bool,
}

static FOUND: Mutex<Option<tauri_plugin_updater::Update>> = Mutex::new(None);
static INSTALLING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Clone)]
struct InstallProgress {
    done: u64,
    total: Option<u64>,
}

fn info(version: &str, auto: bool) -> UpdateInfo {
    UpdateInfo { version: version.trim_start_matches('v').to_string(), url: DOWNLOAD_URL.to_string(), auto }
}

pub async fn check_app(app: &AppHandle, client: &reqwest::Client) -> Option<UpdateInfo> {
    let found = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    match found {
        Ok(Some(update)) => {
            let out = info(&update.version, true);
            *FOUND.lock().unwrap() = Some(update);
            Some(out)
        }
        Ok(None) => None,
        Err(e) => {
            diag::log(format!("Updater unavailable ({e}), falling back to the release page"));
            check(client).await
        }
    }
}

fn games_running(app: &AppHandle) -> bool {
    app.try_state::<Running>().is_some_and(|r| !r.0.lock().unwrap().is_empty())
}

pub async fn install(app: &AppHandle) -> Result<()> {
    if games_running(app) {
        return Err(Error::msg("Close the game before updating the launcher"));
    }
    if INSTALLING.swap(true, AtomicOrdering::SeqCst) {
        return Err(Error::msg("The update is already being installed"));
    }
    let result = async {
        let update = FOUND.lock().unwrap().clone().ok_or_else(|| Error::msg("No update to install"))?;
        let mut done = 0u64;
        let bytes = update
            .download(
                |chunk, total| {
                    done += chunk as u64;
                    let _ = app.emit("update-progress", InstallProgress { done, total });
                },
                || {},
            )
            .await
            .map_err(|e| Error::msg(format!("Couldn't download the update: {e}")))?;
        diag::log(format!("Installing update {} ({} KB, signature verified)", update.version, bytes.len() / 1024));
        let _ = app.emit("update-installing", ());
        update.install(&bytes).map_err(|e| Error::msg(format!("Couldn't install the update: {e}")))
    }
    .await;
    if let Err(e) = result {
        INSTALLING.store(false, AtomicOrdering::SeqCst);
        diag::log(format!("Update failed: {e}"));
        return Err(e);
    }
    app.restart()
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    published_at: Option<String>,
}

fn unix_time(iso: &str) -> Option<u64> {
    let (date, time) = iso.trim_end_matches('Z').split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.split(':').map(|p| p.parse::<i64>().ok());
    let (h, mi, sec) = (t.next()??, t.next()??, t.next()??);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    u64::try_from(days * 86400 + h * 3600 + mi * 60 + sec).ok()
}

fn settled(published_at: Option<&str>, now: u64) -> bool {
    published_at
        .and_then(unix_time)
        .is_some_and(|t| now.saturating_sub(t) >= PUBLISH_DELAY_SECS)
}

fn parse(version: &str) -> Option<Vec<u64>> {
    let core = version.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let parts: Option<Vec<u64>> = core.split('.').map(|p| p.parse().ok()).collect();
    parts.filter(|p| !p.is_empty())
}

fn is_newer(latest: &str, current: &str) -> bool {
    let (Some(mut a), Some(mut b)) = (parse(latest), parse(current)) else {
        return false;
    };
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    a.cmp(&b) == Ordering::Greater
}

fn trusted_url(url: &str) -> bool {
    url.starts_with(&format!("https://github.com/{REPO}/"))
}

pub async fn check(client: &reqwest::Client) -> Option<UpdateInfo> {
    let release: Release = client
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    if release.draft || release.prerelease || !trusted_url(&release.html_url) {
        return None;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    if !settled(release.published_at.as_deref(), now) {
        return None;
    }
    is_newer(&release.tag_name, env!("CARGO_PKG_VERSION")).then(|| info(&release.tag_name, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions_are_detected() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.10.0", "0.9.0"));
    }

    #[test]
    fn same_or_older_versions_are_ignored() {
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.1", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn iso_timestamps_are_parsed() {
        assert_eq!(unix_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_time("2026-10-13T15:30:52Z"), Some(1791905452));
        assert_eq!(unix_time("nope"), None);
    }

    #[test]
    fn fresh_releases_wait_for_the_delay() {
        let t = unix_time("2026-10-13T15:30:52Z").unwrap();
        assert!(!settled(Some("2026-10-13T15:30:52Z"), t + 299));
        assert!(settled(Some("2026-10-13T15:30:52Z"), t + 300));
        assert!(!settled(None, t + 1000));
    }

    #[test]
    fn garbage_is_never_an_update() {
        assert!(!is_newer("latest", "0.1.0"));
        assert!(!is_newer("", "0.1.0"));
        assert!(!is_newer("v1.x", "0.1.0"));
    }

    #[test]
    fn prerelease_suffixes_are_ignored_for_ordering() {
        assert!(is_newer("v0.3.0-beta.1", "0.2.0"));
        assert!(!is_newer("v0.2.0-rc.1", "0.2.0"));
    }

    #[test]
    fn only_this_repository_links_are_trusted() {
        assert!(trusted_url("https://github.com/blackowlzz/PiumaLauncher/releases/tag/v0.2.0"));
        assert!(!trusted_url("https://github.com/blackowlzz/PiumaLauncher.evil/releases"));
        assert!(!trusted_url("https://evil.example/blackowlzz/PiumaLauncher/"));
        assert!(!trusted_url("http://github.com/blackowlzz/PiumaLauncher/releases"));
    }
}
