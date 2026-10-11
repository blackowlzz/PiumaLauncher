use std::io::Cursor;
use std::sync::Once;
use std::time::{Duration, SystemTime};

use base64::{engine::general_purpose::STANDARD, Engine};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, ImageReader, Limits};
use sha1::{Digest, Sha1};
use tokio::sync::Semaphore;

use crate::error::{Error, Result};
use crate::paths;

const HOSTS: [&str; 2] = ["cdn.modrinth.com", "launchercontent.mojang.com"];
const MAX_DOWNLOAD: usize = 20 * 1024 * 1024;
const MAX_SIDE: u32 = 8192;
const JPEG_QUALITY: u8 = 82;
const CACHE_MAX_AGE: Duration = Duration::from_secs(21 * 24 * 3600);

static PRUNE: Once = Once::new();
static SLOTS: Semaphore = Semaphore::const_new(2);

fn cache_dir() -> std::path::PathBuf {
    paths::root().join("imgcache")
}

fn prune_old() {
    let Ok(entries) = std::fs::read_dir(cache_dir()) else { return };
    for e in entries.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > CACHE_MAX_AGE);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

fn check_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|_| Error::msg("invalid image URL"))?;
    if parsed.scheme() == "https" && parsed.host_str().is_some_and(|h| HOSTS.contains(&h)) {
        Ok(())
    } else {
        Err(Error::msg("image from a disallowed host"))
    }
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

fn shrink(raw: &[u8], width: u32) -> Result<(&'static str, Vec<u8>)> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(512 * 1024 * 1024);
    let mut reader = ImageReader::new(Cursor::new(raw)).with_guessed_format()?;
    reader.limits(limits);
    let mut img = reader.decode().map_err(|e| Error::msg(format!("unreadable image: {e}")))?;

    if img.width() > width {
        let height = ((img.height() as u64 * width as u64) / img.width() as u64).max(1) as u32;
        img = img.resize_exact(width, height, FilterType::Triangle);
    }

    let mut out = Vec::new();
    if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        PngEncoder::new(&mut out)
            .write_image(&rgba, rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
            .map_err(|e| Error::msg(e.to_string()))?;
        Ok(("image/png", out))
    } else {
        let rgb = img.to_rgb8();
        JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY)
            .write_image(&rgb, rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
            .map_err(|e| Error::msg(e.to_string()))?;
        Ok(("image/jpeg", out))
    }
}

pub async fn get(client: &reqwest::Client, url: &str, width: u32) -> Result<String> {
    check_url(url)?;
    let width = width.clamp(32, 1600);
    PRUNE.call_once(|| {
        std::thread::spawn(prune_old);
    });

    let key = hex::encode(Sha1::digest(format!("{url}|{width}").as_bytes()));
    let dir = cache_dir();
    for (ext, mime) in [("jpg", "image/jpeg"), ("png", "image/png")] {
        if let Ok(bytes) = tokio::fs::read(dir.join(format!("{key}.{ext}"))).await {
            return Ok(data_url(mime, &bytes));
        }
    }

    let _slot = SLOTS.acquire().await.map_err(|e| Error::msg(e.to_string()))?;
    let resp = client.get(url).send().await?.error_for_status()?;
    if resp.content_length().is_some_and(|n| n as usize > MAX_DOWNLOAD) {
        return Err(Error::msg("image too large"));
    }
    let raw = resp.bytes().await?;
    if raw.len() > MAX_DOWNLOAD {
        return Err(Error::msg("image too large"));
    }

    let (mime, out) = tokio::task::spawn_blocking(move || shrink(&raw, width))
        .await
        .map_err(|e| Error::msg(e.to_string()))??;

    let ext = if mime == "image/png" { "png" } else { "jpg" };
    if tokio::fs::create_dir_all(&dir).await.is_ok() {
        let tmp = dir.join(format!("{key}.tmp"));
        if tokio::fs::write(&tmp, &out).await.is_ok() {
            let _ = tokio::fs::rename(&tmp, dir.join(format!("{key}.{ext}"))).await;
        }
    }
    Ok(data_url(mime, &out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, alpha: bool) -> Vec<u8> {
        let mut out = Vec::new();
        if alpha {
            let img = image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 128]));
            PngEncoder::new(&mut out).write_image(&img, w, h, ExtendedColorType::Rgba8).unwrap();
        } else {
            let img = image::RgbImage::from_pixel(w, h, image::Rgb([10, 20, 30]));
            PngEncoder::new(&mut out).write_image(&img, w, h, ExtendedColorType::Rgb8).unwrap();
        }
        out
    }

    #[test]
    fn big_opaque_image_becomes_a_small_jpeg() {
        let (mime, out) = shrink(&png(1920, 1080, false), 640).unwrap();
        assert_eq!(mime, "image/jpeg");
        let back = image::load_from_memory(&out).unwrap();
        assert_eq!((back.width(), back.height()), (640, 360));
    }

    #[test]
    fn transparent_icon_stays_png_and_is_not_enlarged() {
        let (mime, out) = shrink(&png(64, 64, true), 96).unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(image::load_from_memory(&out).unwrap().width(), 64);
    }

    #[test]
    fn only_known_hosts_are_fetched() {
        assert!(check_url("https://cdn.modrinth.com/data/x/y.png").is_ok());
        assert!(check_url("https://evil.example/y.png").is_err());
        assert!(check_url("http://cdn.modrinth.com/y.png").is_err());
        assert!(check_url("https://cdn.modrinth.com.evil.example/y.png").is_err());
    }
}
