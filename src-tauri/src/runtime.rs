
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::java::{self, JavaInstall};
use crate::net::{self, Check, Job};
use crate::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flavor {
    Graal,
    Temurin,
}

impl Flavor {
    fn slug(self) -> &'static str {
        match self {
            Flavor::Graal => "graalvm",
            Flavor::Temurin => "temurin",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Flavor::Graal => "GraalVM",
            Flavor::Temurin => "Temurin",
        }
    }
}

fn plan(required: u32) -> Option<(Flavor, u32)> {
    match required {
        0..=8 => Some((Flavor::Temurin, 8)),
        9..=17 => Some((Flavor::Temurin, 17)),
        18..=21 => Some((Flavor::Graal, 21)),
        22..=25 => Some((Flavor::Graal, 25)),
        _ => None,
    }
}

fn target() -> Option<(&'static str, &'static str)> {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return None;
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return None;
    };
    Some((os, arch))
}

fn home_dir(flavor: Flavor, major: u32) -> PathBuf {
    paths::runtimes().join(format!("{}-{major}", flavor.slug()))
}

struct Download {
    url: String,
    sha256: String,
    size: Option<u64>,
}

#[derive(Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}
#[derive(Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}
#[derive(Deserialize)]
struct AdoptiumPackage {
    link: String,
    checksum: String,
    size: Option<u64>,
}

async fn resolve(client: &reqwest::Client, flavor: Flavor, major: u32) -> Result<Download> {
    let (os, arch) = target().ok_or_else(|| Error::msg("this system isn't supported for automatic Java download"))?;
    match flavor {
        Flavor::Graal => {
            if os == "windows" && arch != "x64" {
                return Err(Error::msg("GraalVM for Windows is only available for x64"));
            }
            let ext = if os == "windows" { "zip" } else { "tar.gz" };
            let url = format!(
                "https://download.oracle.com/graalvm/{major}/latest/graalvm-jdk-{major}_{os}-{arch}_bin.{ext}"
            );
            let sum = net::get_text(client, &format!("{url}.sha256")).await?;
            let sha256 = sum.split_whitespace().next().unwrap_or_default().to_lowercase();
            if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(Error::msg("invalid GraalVM checksum"));
            }
            Ok(Download { url, sha256, size: None })
        }
        Flavor::Temurin => {
            let api = format!(
                "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jre&os={os}"
            );
            let assets: Vec<AdoptiumAsset> = serde_json::from_str(&net::get_text(client, &api).await?)?;
            let pkg = assets
                .into_iter()
                .next()
                .ok_or_else(|| Error::msg(format!("Temurin {major} isn't available for this system")))?
                .binary
                .package;
            Ok(Download { url: pkg.link, sha256: pkg.checksum, size: pkg.size })
        }
    }
}

fn java_bin(home: &Path) -> PathBuf {
    home.join("bin").join(if cfg!(windows) { "java.exe" } else { "java" })
}

pub async fn ensure(
    client: &reqwest::Client,
    required: u32,
    progress: &(dyn Fn(&str, u32, u32) + Sync),
) -> Result<Option<JavaInstall>> {
    let Some((flavor, major)) = plan(required) else { return Ok(None) };
    if target().is_none() {
        return Ok(None);
    }
    let home = home_dir(flavor, major);
    if java_bin(&home).is_file() {
        if let Some(found) = java::probe(&java_bin(&home)).await {
            return Ok(Some(found));
        }
    }

    let stage = format!("Downloading {} {major}", flavor.label());
    progress(&stage, 0, 100);
    let dl = resolve(client, flavor, major).await?;
    let ext = if dl.url.ends_with(".zip") { "zip" } else { "tar.gz" };
    let archive = paths::runtimes().join(".download").join(format!("{}-{major}.{ext}", flavor.slug()));
    let job = Job { url: dl.url, path: archive.clone(), check: Check::Sha256(dl.sha256), size: dl.size };
    let _ = tokio::fs::remove_file(&archive).await;
    net::download_with_progress(client, &job, Some(&|got, total| {
        if let Some(t) = total.filter(|t| *t > 0) {
            progress(&stage, (got * 100 / t) as u32, 100);
        }
    }))
    .await?;

    progress(&format!("Installing {} {major}", flavor.label()), 0, 0);
    let staging = paths::runtimes().join(format!(".{}-{major}.tmp", flavor.slug()));
    let (src, dst, final_home) = (archive.clone(), staging.clone(), home.clone());
    tokio::task::spawn_blocking(move || -> Result<()> {
        let _ = std::fs::remove_dir_all(&dst);
        std::fs::create_dir_all(&dst)?;
        if src.extension().is_some_and(|e| e == "zip") {
            unzip_stripped(&src, &dst)?;
        } else {
            untar_stripped(&src, &dst)?;
        }
        let _ = std::fs::remove_dir_all(&final_home);
        std::fs::rename(&dst, &final_home)?;
        let _ = std::fs::remove_file(&src);
        Ok(())
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;

    java::probe(&java_bin(&home))
        .await
        .map(Some)
        .ok_or_else(|| Error::msg("The downloaded Java doesn't start"))
}

fn stripped(path: &Path) -> Option<PathBuf> {
    let mut parts = path.components();
    parts.next()?;
    let rest: PathBuf = parts.as_path().into();
    if rest.as_os_str().is_empty() || !rest.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(rest)
}

fn untar_stripped(archive: &Path, dest: &Path) -> Result<()> {
    let gz = flate2::read::GzDecoder::new(std::fs::File::open(archive)?);
    let mut tar = tar::Archive::new(gz);
    for entry in tar.entries()? {
        let mut entry = entry?;
        let Some(rel) = stripped(&entry.path()?) else { continue };
        let out = dest.join(rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry.unpack(out)?;
    }
    Ok(())
}

fn unzip_stripped(archive: &Path, dest: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(rel) = entry.enclosed_name().and_then(|p| stripped(&p)) else { continue };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::io::copy(&mut entry, &mut std::fs::File::create(out)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_maps_mojang_versions() {
        assert_eq!(plan(8), Some((Flavor::Temurin, 8)));
        assert_eq!(plan(16), Some((Flavor::Temurin, 17)));
        assert_eq!(plan(21), Some((Flavor::Graal, 21)));
        assert_eq!(plan(25), Some((Flavor::Graal, 25)));
        assert_eq!(plan(30), None);
    }

    #[test]
    fn strip_rejects_traversal() {
        assert_eq!(stripped(Path::new("jdk-21/bin/java")), Some(PathBuf::from("bin/java")));
        assert_eq!(stripped(Path::new("jdk-21")), None);
        assert_eq!(stripped(Path::new("jdk-21/../evil")), None);
    }

    #[tokio::test]
    #[ignore]
    async fn runtime_download() {
        let client = net::client();
        let progress = |s: &str, d: u32, t: u32| {
            if d % 25 == 0 {
                eprintln!("{s} {d}/{t}");
            }
        };
        let j8 = ensure(&client, 8, &progress).await.unwrap().unwrap();
        eprintln!("{j8:?}");
        assert_eq!(j8.major, 8);
        let j21 = ensure(&client, 21, &progress).await.unwrap().unwrap();
        eprintln!("{j21:?}");
        assert_eq!(j21.major, 21);
        assert!(j21.is_graal, "GraalVM atteso");
    }
}
