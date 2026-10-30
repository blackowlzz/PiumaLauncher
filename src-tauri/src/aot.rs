use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::java::JavaInstall;
use crate::{diag, paths};

const KEEP: usize = 2;
const MIN_JAVA: u32 = 25;

pub struct Training {
    partial: PathBuf,
    cache: PathBuf,
}

fn dir(instance_id: &str) -> PathBuf {
    paths::instance_dir(instance_id).join("aot")
}

pub fn clear(instance_id: &str) {
    let dir = dir(instance_id);
    if dir.is_dir() && std::fs::remove_dir_all(&dir).is_ok() {
        diag::log(format!("{instance_id}: AOT cache turned off, removed the saved cache"));
    }
}

fn user_managed(jvm: &[String]) -> bool {
    jvm.iter().any(|a| a.starts_with("-XX:AOT") || a.starts_with("-XX:SharedArchiveFile") || a.starts_with("-Xshare:"))
}

fn key(java: &JavaInstall, jvm: &[String], classpath: &[PathBuf]) -> String {
    let mut h = Sha256::new();
    h.update(java.path.as_bytes());
    h.update([0]);
    h.update(java.version.as_bytes());
    for a in jvm.iter().filter(|a| !a.starts_with("-Xms") && !a.starts_with("-Xmx")) {
        h.update([0]);
        h.update(a.as_bytes());
    }
    for p in classpath {
        let meta = std::fs::metadata(p).ok();
        let len = meta.as_ref().map_or(0, |m| m.len());
        let modified = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        h.update([0]);
        h.update(p.to_string_lossy().as_bytes());
        h.update(len.to_le_bytes());
        h.update(modified.to_le_bytes());
    }
    hex::encode(&h.finalize()[..12])
}

pub fn apply(instance_id: &str, java: &JavaInstall, args: &mut Vec<String>, main_class: &str, classpath: &[PathBuf]) -> Option<Training> {
    let end = args.iter().position(|a| a == main_class)?;
    if java.major < MIN_JAVA || user_managed(&args[..end]) {
        return None;
    }
    let dir = dir(instance_id);
    let key = key(java, &args[..end], classpath);
    let cache = dir.join(format!("{key}.aot"));
    if cache.is_file() {
        args.insert(0, format!("-XX:AOTCache={}", cache.display()));
        diag::log(format!("{instance_id}: launching with the AOT cache {key}"));
        return None;
    }
    std::fs::create_dir_all(&dir).ok()?;
    let partial = dir.join(format!("{key}.training.aot"));
    let _ = std::fs::remove_file(&partial);
    args.insert(0, format!("-XX:AOTCacheOutput={}", partial.display()));
    diag::log(format!("{instance_id}: recording an AOT cache for faster launches"));
    Some(Training { partial, cache })
}

impl Training {
    pub fn finish(self, instance_id: &str) {
        let written = std::fs::metadata(&self.partial).is_ok_and(|m| m.len() > 0);
        if !written || std::fs::rename(&self.partial, &self.cache).is_err() {
            let _ = std::fs::remove_file(&self.partial);
            diag::log(format!("{instance_id}: no AOT cache was written, the next launch records again"));
            return;
        }
        if let Some(dir) = self.cache.parent() {
            prune(dir);
        }
        let mb = std::fs::metadata(&self.cache).map_or(0, |m| m.len() / 1_048_576);
        diag::log(format!("{instance_id}: AOT cache ready ({mb} MB)"));
    }
}

fn prune(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut caches: Vec<(SystemTime, PathBuf)> = vec![];
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.contains(".training.") {
            let _ = std::fs::remove_file(&p);
        } else if name.ends_with(".aot") {
            caches.push((e.metadata().and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH), p));
        }
    }
    caches.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in caches.into_iter().skip(KEEP) {
        let _ = std::fs::remove_file(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn java(major: u32) -> JavaInstall {
        JavaInstall { path: "/j/bin/java".into(), major, version: format!("{major}.0.1"), is_graal: false }
    }

    fn args() -> Vec<String> {
        ["-Xms512M", "-Xmx4096M", "-XX:+UseG1GC", "-cp", "a.jar", "net.minecraft.Main", "--username", "x"].map(String::from).to_vec()
    }

    #[test]
    fn old_java_and_user_flags_are_left_alone() {
        let mut a = args();
        assert!(apply("aot-probe-old", &java(21), &mut a, "net.minecraft.Main", &[]).is_none());
        assert_eq!(a, args());
        let mut a = args();
        a.insert(2, "-XX:AOTCache=/mine.aot".into());
        let before = a.clone();
        assert!(apply("aot-probe-user", &java(25), &mut a, "net.minecraft.Main", &[]).is_none());
        assert_eq!(a, before);
    }

    #[test]
    fn heap_size_does_not_change_the_key() {
        let mut other = args();
        other[1] = "-Xmx8192M".into();
        assert_eq!(key(&java(25), &args()[..5], &[]), key(&java(25), &other[..5], &[]));
        let mut gc = args();
        gc[2] = "-XX:+UseZGC".into();
        assert_ne!(key(&java(25), &args()[..5], &[]), key(&java(25), &gc[..5], &[]));
    }

    #[test]
    fn records_then_uses_the_cache() {
        let id = "aot-probe";
        let _ = std::fs::remove_dir_all(paths::instance_dir(id));
        let mut a = args();
        let t = apply(id, &java(25), &mut a, "net.minecraft.Main", &[]).expect("records on the first launch");
        assert!(a[0].starts_with("-XX:AOTCacheOutput="));
        std::fs::write(&t.partial, b"cache").unwrap();
        t.finish(id);
        let mut a = args();
        assert!(apply(id, &java(25), &mut a, "net.minecraft.Main", &[]).is_none());
        assert!(a[0].starts_with("-XX:AOTCache=") && a[0].ends_with(".aot"));
        let _ = std::fs::remove_dir_all(paths::instance_dir(id));
    }
}
