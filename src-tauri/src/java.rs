
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tokio::process::Command;

use crate::paths;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstall {
    pub path: String,
    pub major: u32,
    pub version: String,
    pub is_graal: bool,
}

const EXE: &str = if cfg!(windows) { "java.exe" } else { "java" };

fn java_in(home: &Path) -> PathBuf {
    home.join("bin").join(EXE)
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

fn candidates() -> Vec<PathBuf> {
    let mut homes: Vec<PathBuf> = subdirs(&paths::runtimes())
        .into_iter()
        .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .collect();

    if let Some(h) = std::env::var_os("JAVA_HOME") {
        homes.push(PathBuf::from(h));
    }

    let mut roots: Vec<PathBuf> = vec![];
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(var) {
                let base = PathBuf::from(base);
                for vendor in ["Java", "Eclipse Adoptium", "Microsoft", "Zulu", "GraalVM", "BellSoft", "Amazon Corretto", "Programs\\Eclipse Adoptium"] {
                    roots.push(base.join(vendor));
                }
            }
        }
    } else {
        roots.push("/usr/lib/jvm".into());
        roots.push("/usr/lib64/jvm".into());
        roots.push("/opt".into());
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".sdkman/candidates/java"));
            roots.push(home.join(".jdks"));
        }
    }
    for r in roots {
        homes.extend(subdirs(&r));
    }

    let mut out: Vec<PathBuf> = homes.iter().map(|h| java_in(h)).collect();
    out.push(PathBuf::from(EXE));
    out.into_iter().filter(|p| p.components().count() == 1 || p.is_file()).collect()
}

fn parse_major(version: &str) -> Option<u32> {
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

pub async fn probe(path: &Path) -> Option<JavaInstall> {
    let child = Command::new(path)
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(Duration::from_secs(8), child).await.ok()?.ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    let version = text.split('"').nth(1)?.to_string();
    let major = parse_major(&version)?;
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Some(JavaInstall {
        path: real.to_string_lossy().into_owned(),
        major,
        is_graal: text.contains("GraalVM"),
        version,
    })
}

pub async fn detect() -> Vec<JavaInstall> {
    let probes = candidates().into_iter().map(|p| async move { probe(&p).await });
    let mut seen = HashSet::new();
    let mut found: Vec<JavaInstall> = futures_util::future::join_all(probes)
        .await
        .into_iter()
        .flatten()
        .filter(|j| seen.insert(j.path.clone()))
        .collect();
    found.sort_by(|a, b| b.major.cmp(&a.major));
    found
}

pub fn pick(installs: &[JavaInstall], required: u32) -> Option<&JavaInstall> {
    installs
        .iter()
        .filter(|j| j.major >= required)
        .min_by_key(|j| (j.major != required, !j.is_graal, j.major))
}
