
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::minecraft::{self, AssetIndex, Resolved, RESOURCES_BASE};
use crate::net::{self, Check, Job};
use crate::paths;

pub type Progress<'a> = &'a (dyn Fn(&str, u32, u32) + Sync);

pub struct Prepared {
    pub resolved: Resolved,
    pub classpath: Vec<PathBuf>,
    pub natives_dir: PathBuf,
    pub assets_index_id: String,
    pub game_assets: PathBuf,
    pub logging_arg: Option<String>,
}

pub async fn prepare(
    client: &reqwest::Client,
    version_id: &str,
    instance_id: &str,
    game_dir: PathBuf,
    progress: Progress<'_>,
) -> Result<Prepared> {
    progress("Resolving version", 0, 0);
    let resolved = minecraft::resolve(client, version_id).await?;
    let v = &resolved.version;

    let client_dl = v
        .downloads
        .as_ref()
        .and_then(|d| d.client.as_ref())
        .ok_or_else(|| Error::msg("this version has no client jar"))?;
    let mut jobs = vec![Job {
        url: client_dl.url.clone(),
        path: resolved.client_jar(),
        check: Check::sha1(client_dl.sha1.clone()),
        size: client_dl.size,
    }];

    let lib_files = resolved.classpath_files();
    let native_files = resolved.native_files();
    let to_job = |f: &minecraft::LibFile| Job {
        url: f.url.clone(),
        path: f.local_path(),
        check: Check::sha1(f.sha1.clone()),
        size: f.size,
    };
    jobs.extend(lib_files.iter().map(to_job));
    jobs.extend(native_files.iter().map(|(f, _)| to_job(f)));
    net::download_all(client, jobs, &|d, t| progress("Downloading libraries", d, t)).await?;

    let natives_dir = paths::natives(instance_id);
    let jars: Vec<(PathBuf, Vec<String>)> =
        native_files.iter().map(|(f, ex)| (f.local_path(), ex.clone())).collect();
    let dest = natives_dir.clone();
    tokio::task::spawn_blocking(move || extract_natives(jars, dest))
        .await
        .map_err(|e| Error::msg(e.to_string()))??;

    let index_ref = v.asset_index.as_ref().ok_or_else(|| Error::msg("asset index missing"))?;
    let assets_root = paths::assets();
    let index_path = assets_root.join("indexes").join(format!("{}.json", index_ref.id));
    net::download(
        client,
        &Job {
            url: index_ref.url.clone(),
            path: index_path.clone(),
            check: Check::sha1(index_ref.sha1.clone()),
            size: index_ref.size,
        },
    )
    .await?;
    let index: AssetIndex = serde_json::from_str(&tokio::fs::read_to_string(&index_path).await?)?;

    let asset_jobs: Vec<Job> = index
        .objects
        .values()
        .map(|o| {
            let prefix = &o.hash[..2];
            Job {
                url: format!("{RESOURCES_BASE}{prefix}/{}", o.hash),
                path: assets_root.join("objects").join(prefix).join(&o.hash),
                check: Check::sha1(Some(o.hash.clone())),
                size: Some(o.size),
            }
        })
        .collect();
    net::download_all(client, asset_jobs, &|d, t| progress("Downloading assets", d, t)).await?;

    let mut game_assets = assets_root.clone();
    if index.virtual_ || index.map_to_resources {
        let target = if index.map_to_resources {
            game_dir.join("resources")
        } else {
            assets_root.join("virtual").join("legacy")
        };
        if index.virtual_ {
            game_assets = target.clone();
        }
        let root = assets_root.clone();
        tokio::task::spawn_blocking(move || copy_virtual_assets(&index, &root, &target))
            .await
            .map_err(|e| Error::msg(e.to_string()))??;
    }

    let mut logging_arg = None;
    if let Some(lc) = v.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let path = assets_root.join("log_configs").join(&lc.file.id);
        net::download(
            client,
            &Job { url: lc.file.url.clone(), path: path.clone(), check: Check::sha1(lc.file.sha1.clone()), size: lc.file.size },
        )
        .await?;
        logging_arg = Some(lc.argument.replace("${path}", &path.to_string_lossy()));
    }

    let mut classpath: Vec<PathBuf> = lib_files.iter().map(|f| f.local_path()).collect();
    classpath.push(resolved.client_jar());

    Ok(Prepared {
        assets_index_id: index_ref.id.clone(),
        resolved,
        classpath,
        natives_dir,
        game_assets,
        logging_arg,
    })
}

fn extract_natives(jars: Vec<(PathBuf, Vec<String>)>, dest: PathBuf) -> Result<()> {
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::create_dir_all(&dest)?;
    for (jar, exclude) in jars {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&jar)?)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let Ok(name) = entry.name().map(|n| n.into_owned()) else { continue };
            if entry.is_dir() || exclude.iter().any(|x| name.starts_with(x.as_str())) {
                continue;
            }
            let Some(rel) = entry.enclosed_name() else { continue };
            let out = dest.join(rel);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::io::copy(&mut entry, &mut std::fs::File::create(out)?)?;
        }
    }
    Ok(())
}

fn copy_virtual_assets(index: &AssetIndex, root: &std::path::Path, target: &std::path::Path) -> Result<()> {
    for (name, obj) in &index.objects {
        let dst = target.join(name);
        if dst.exists() {
            continue;
        }
        if name.split('/').any(|p| p == ".." || p.is_empty()) {
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(root.join("objects").join(&obj.hash[..2]).join(&obj.hash), dst)?;
    }
    Ok(())
}
