
use std::collections::HashMap;

use serde::Deserialize;
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
    #[serde(default)]
    version: String,
    #[serde(default)]
    image: Option<Image>,
}

#[derive(Deserialize)]
struct Image {
    url: String,
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
