use std::collections::HashSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{paths, servers};

const URL: &str = "https://raw.githubusercontent.com/blackowlzz/PiumaLauncher/main/sponsored-servers.json";
const MAX_SERVERS: usize = 24;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sponsored {
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Feed {
    List(Vec<serde_json::Value>),
    Wrapped { servers: Vec<serde_json::Value> },
}

fn clean(text: &str, max: usize) -> String {
    text.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

fn parse(text: &str) -> Vec<Sponsored> {
    let entries = match serde_json::from_str::<Feed>(text) {
        Ok(Feed::List(v)) | Ok(Feed::Wrapped { servers: v }) => v,
        Err(_) => return vec![],
    };
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .filter_map(|v| serde_json::from_value::<Sponsored>(v).ok())
        .filter_map(|s| {
            let address = servers::check_address(&s.address).ok()?.to_ascii_lowercase();
            let name = clean(&s.name, 48);
            (!name.is_empty() && seen.insert(address.clone())).then(|| Sponsored {
                name,
                address,
                description: clean(&s.description, 200),
                version: s.version.map(|v| clean(&v, 24)).filter(|v| !v.is_empty()),
            })
        })
        .take(MAX_SERVERS)
        .collect()
}

fn cache_file() -> std::path::PathBuf {
    paths::root().join("sponsored-cache.json")
}

pub async fn list(client: &reqwest::Client) -> Vec<Sponsored> {
    let fetched = async {
        let resp = client.get(URL).timeout(Duration::from_secs(8)).send().await.ok()?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Some(String::from("[]"));
        }
        resp.error_for_status().ok()?.text().await.ok()
    }
    .await;
    match fetched {
        Some(text) => {
            let list = parse(&text);
            let _ = std::fs::write(cache_file(), serde_json::to_string(&list).unwrap_or_default());
            list
        }
        None => std::fs::read_to_string(cache_file()).map(|t| parse(&t)).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_is_validated() {
        let feed = r#"{"servers": [
            {"name": "Good", "address": "play.example.net", "description": "A nice server", "version": "1.21.x"},
            {"name": "Dup", "address": "PLAY.example.net"},
            {"name": "", "address": "empty.example.net"},
            {"name": "Bad address", "address": "bad host"},
            {"name": "No address"},
            {"name": "Long\u0007name that keeps going and going and going and going and going", "address": "long.example.net"}
        ]}"#;
        let list = parse(feed);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], Sponsored { name: "Good".into(), address: "play.example.net".into(), description: "A nice server".into(), version: Some("1.21.x".into()) });
        assert_eq!(list[1].name.chars().count(), 48);
        assert!(!list[1].name.contains('\u{7}'));
        assert_eq!(parse(r#"[{"name": "Plain", "address": "plain.example.net:25570"}]"#).len(), 1);
        assert!(parse("not json").is_empty());
        let many: Vec<String> = (0..40).map(|i| format!(r#"{{"name":"S{i}","address":"s{i}.example.net"}}"#)).collect();
        assert_eq!(parse(&format!("[{}]", many.join(","))).len(), MAX_SERVERS);
    }
}
