use std::cmp::Ordering;
use std::time::Duration;

use serde::{Deserialize, Serialize};

const REPO: &str = "blackowlzz/PiumaLauncher";

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
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
    is_newer(&release.tag_name, env!("CARGO_PKG_VERSION")).then(|| UpdateInfo {
        version: release.tag_name.trim_start_matches('v').to_string(),
        url: release.html_url,
    })
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
