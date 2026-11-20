use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

const URL: &str = "https://raw.githubusercontent.com/blackowlzz/PiumaLauncher/main/notice.txt";
const KEYS: [&str; 3] = ["TITLE", "DESCRIPTION", "LINK"];

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub id: String,
    pub title: String,
    pub description: String,
    pub link: Option<String>,
}

fn key_of(line: &str) -> Option<(String, &str)> {
    let (key, rest) = line.split_once(':')?;
    let key = key.trim();
    let (base, lang) = key.split_once('_').unwrap_or((key, ""));
    let valid = KEYS.contains(&base) && lang.len() <= 3 && lang.chars().all(|c| c.is_ascii_uppercase());
    valid.then(|| (key.to_string(), rest.trim()))
}

fn clean(text: &str, max: usize) -> String {
    let kept: String = text.chars().filter(|c| *c == '\n' || !c.is_control()).take(max).collect();
    kept.trim().to_string()
}

fn parse(text: &str, lang: &str) -> Option<Notice> {
    let mut fields: HashMap<String, String> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some((key, rest)) = key_of(line) {
            fields.insert(key.clone(), rest.to_string());
            current = Some(key);
        } else if let Some(field) = current.as_ref().and_then(|k| fields.get_mut(k)) {
            field.push('\n');
            field.push_str(line.trim());
        }
    }
    let suffix = lang.to_ascii_uppercase();
    let pick = |base: &str, max: usize| {
        let local = fields.get(&format!("{base}_{suffix}")).map(|v| clean(v, max)).filter(|v| !v.is_empty());
        local.or_else(|| fields.get(base).map(|v| clean(v, max))).unwrap_or_default()
    };
    let title = pick("TITLE", 120);
    if title.is_empty() {
        return None;
    }
    let link = fields
        .get("LINK")
        .and_then(|l| reqwest::Url::parse(l.trim()).ok())
        .filter(|u| u.scheme() == "https" && u.host_str().is_some())
        .map(|u| u.to_string());
    let base = [fields.get("TITLE"), fields.get("DESCRIPTION"), fields.get("LINK")].map(|v| v.map_or("", |s| s.as_str())).join("\n");
    Some(Notice {
        id: hex::encode(&Sha256::digest(base.as_bytes())[..8]),
        title,
        description: pick("DESCRIPTION", 600),
        link,
    })
}

pub async fn fetch(client: &reqwest::Client, lang: &str) -> Option<Notice> {
    let resp = client.get(URL).timeout(Duration::from_secs(8)).send().await.ok()?;
    let text = resp.error_for_status().ok()?.text().await.ok()?;
    parse(&text, lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_is_parsed() {
        let text = "TITLE: Java warning\nDESCRIPTION: First line\nsecond line\nTITLE_IT: Avviso Java\nLINK: https://example.com/a\n";
        let en = parse(text, "en").unwrap();
        assert_eq!(en.title, "Java warning");
        assert_eq!(en.description, "First line\nsecond line");
        assert_eq!(en.link.as_deref(), Some("https://example.com/a"));
        let it = parse(text, "it").unwrap();
        assert_eq!(it.title, "Avviso Java");
        assert_eq!(it.description, "First line\nsecond line");
        assert_eq!(it.id, en.id);
        assert!(parse("", "en").is_none());
        assert!(parse("DESCRIPTION: no title", "en").is_none());
        assert!(parse("TITLE: x\nLINK: http://insecure.example", "en").unwrap().link.is_none());
        assert_ne!(parse("TITLE: a", "en").unwrap().id, parse("TITLE: b", "en").unwrap().id);
        assert_eq!(parse("TITLE: Note: keep colons", "en").unwrap().title, "Note: keep colons");
        let blocks = parse("TITLE: A\nDESCRIPTION: B\nLINK: https://example.com\n\nTITLE_DE: C\nDESCRIPTION_DE: D\n", "de").unwrap();
        assert_eq!((blocks.title.as_str(), blocks.description.as_str()), ("C", "D"));
        assert_eq!(blocks.link.as_deref(), Some("https://example.com/"));
    }
}
