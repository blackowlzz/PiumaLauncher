
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::error::Result;
use crate::net;

const FEED: &str = "https://launchercontent.mojang.com/v2/javaPatchNotes.json";
const IMAGE_BASE: &str = "https://launchercontent.mojang.com";

#[derive(Deserialize)]
struct Feed {
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    title: String,
    #[serde(default)]
    version: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    date: String,
    #[serde(default, rename = "shortText")]
    short_text: String,
    #[serde(default)]
    image: Option<Image>,
}

#[derive(Deserialize)]
struct Image {
    url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub title: String,
    pub version: String,
    pub kind: String,
    pub date: String,
    pub summary: String,
    pub image_url: Option<String>,
}

pub async fn latest(client: &reqwest::Client) -> Result<Vec<NewsItem>> {
    let feed: Feed = serde_json::from_str(&net::get_text(client, FEED).await?)?;
    Ok(feed
        .entries
        .into_iter()
        .take(8)
        .map(|e| NewsItem {
            image_url: e
                .image
                .map(|i| i.url)
                .filter(|u| u.starts_with('/'))
                .map(|u| format!("{IMAGE_BASE}{u}")),
            title: e.title,
            version: e.version,
            kind: e.kind,
            date: e.date,
            summary: e.short_text,
        })
        .collect())
}

static IMAGES: OnceCell<HashMap<String, String>> = OnceCell::const_new();

pub async fn version_images(client: &reqwest::Client) -> Result<HashMap<String, String>> {
    let map = IMAGES
        .get_or_try_init(|| async {
            let feed: Feed = serde_json::from_str(&net::get_text(client, FEED).await?)?;
            Ok::<_, crate::error::Error>(
                feed.entries
                    .into_iter()
                    .filter(|e| !e.version.is_empty())
                    .filter_map(|e| {
                        let url = e.image?.url;
                        url.starts_with('/').then(|| (e.version, format!("{IMAGE_BASE}{url}")))
                    })
                    .collect(),
            )
        })
        .await?;
    Ok(map.clone())
}
