
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::stream::{self, StreamExt};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncWriteExt, BufWriter};

use crate::error::{Error, Result};

const PARALLEL_DOWNLOADS: usize = 12;
const ATTEMPTS: u32 = 3;
const WRITE_BUFFER: usize = 256 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(80);

static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

pub const CANCELLED: &str = "Cancelled";

tokio::task_local! {
    static CANCEL: Arc<AtomicBool>;
}

static CURRENT: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

pub async fn cancellable<F: std::future::Future>(fut: F) -> F::Output {
    let flag = Arc::new(AtomicBool::new(false));
    *CURRENT.lock().unwrap() = Some(flag.clone());
    let out = CANCEL.scope(flag.clone(), fut).await;
    let mut current = CURRENT.lock().unwrap();
    if current.as_ref().is_some_and(|c| Arc::ptr_eq(c, &flag)) {
        *current = None;
    }
    out
}

pub fn cancel() -> bool {
    match CURRENT.lock().unwrap().as_ref() {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

pub fn cancelled() -> bool {
    CANCEL.try_with(|c| c.load(Ordering::Relaxed)).unwrap_or(false)
}

pub fn check_cancelled() -> Result<()> {
    if cancelled() { Err(Error::msg(CANCELLED)) } else { Ok(()) }
}

pub fn client() -> reqwest::Client {
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(concat!("PiumaLauncher/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(300))
                .pool_idle_timeout(Duration::from_secs(60))
                .build()
                .expect("client HTTP")
        })
        .clone()
}

pub async fn get_text(client: &reqwest::Client, url: &str) -> Result<String> {
    Ok(client.get(url).send().await?.error_for_status()?.text().await?)
}

#[derive(Debug, Clone)]
pub enum Check {
    None,
    Sha1(String),
    Sha256(String),
}

impl Check {
    pub fn sha1(hash: Option<String>) -> Self {
        hash.map_or(Check::None, Check::Sha1)
    }
}

#[derive(Debug, Clone)]
pub struct Job {
    pub url: String,
    pub path: PathBuf,
    pub check: Check,
    pub size: Option<u64>,
}

enum Hasher {
    None,
    Sha1(Sha1, String),
    Sha256(Sha256, String),
}

impl Hasher {
    fn new(check: &Check) -> Self {
        match check {
            Check::None => Hasher::None,
            Check::Sha1(h) => Hasher::Sha1(Sha1::new(), h.clone()),
            Check::Sha256(h) => Hasher::Sha256(Sha256::new(), h.clone()),
        }
    }

    fn update(&mut self, data: &[u8]) {
        match self {
            Hasher::None => {}
            Hasher::Sha1(h, _) => h.update(data),
            Hasher::Sha256(h, _) => h.update(data),
        }
    }

    fn verify(self) -> std::result::Result<(), (String, String)> {
        let (got, want) = match self {
            Hasher::None => return Ok(()),
            Hasher::Sha1(h, w) => (hex::encode(h.finalize()), w),
            Hasher::Sha256(h, w) => (hex::encode(h.finalize()), w),
        };
        if got.eq_ignore_ascii_case(&want) { Ok(()) } else { Err((want, got)) }
    }
}

pub type ByteProgress<'a> = &'a (dyn Fn(u64, Option<u64>) + Sync);

fn already_valid(job: &Job) -> bool {
    match std::fs::metadata(&job.path) {
        Ok(m) if m.is_file() => job.size.map_or(true, |s| s == m.len()),
        _ => false,
    }
}

async fn download_once(client: &reqwest::Client, job: &Job, progress: Option<ByteProgress<'_>>) -> Result<()> {
    if let Some(parent) = job.path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(Error::at(parent))?;
    }
    let mut tmp_name = job.path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".part");
    let tmp = job.path.with_file_name(tmp_name);

    let mut resp = client.get(&job.url).send().await?.error_for_status()?;
    let mut file = BufWriter::with_capacity(WRITE_BUFFER, crate::fsx::retry_async(|| tokio::fs::File::create(&tmp)).await.map_err(Error::at(&tmp))?);
    let total = resp.content_length();
    let mut received = 0u64;
    let mut hasher = Hasher::new(&job.check);
    while let Some(chunk) = resp.chunk().await? {
        if cancelled() {
            drop(file);
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(Error::msg(CANCELLED));
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        received += chunk.len() as u64;
        if let Some(p) = progress {
            p(received, total);
        }
    }
    file.flush().await?;
    drop(file);

    if let Err((want, got)) = hasher.verify() {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(Error::msg(format!("checksum mismatch for {} (expected {want}, got {got})", job.url)));
    }
    crate::fsx::retry_async(|| tokio::fs::rename(&tmp, &job.path)).await.map_err(Error::at(&job.path))?;
    Ok(())
}

pub async fn download(client: &reqwest::Client, job: &Job) -> Result<()> {
    download_with_progress(client, job, None).await
}

pub async fn download_with_progress(client: &reqwest::Client, job: &Job, progress: Option<ByteProgress<'_>>) -> Result<()> {
    if already_valid(job) {
        return Ok(());
    }
    let mut last_err = None;
    for attempt in 0..ATTEMPTS {
        check_cancelled()?;
        match download_once(client, job, progress).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(Duration::from_millis(400 * (attempt as u64 + 1))).await;
            }
        }
    }
    Err(last_err.unwrap())
}

pub async fn download_all(
    client: &reqwest::Client,
    jobs: Vec<Job>,
    progress: &(dyn Fn(u32, u32) + Sync),
) -> Result<()> {
    progress(0, jobs.len() as u32);
    let (missing, present): (Vec<Job>, Vec<Job>) = tokio::task::spawn_blocking(move || {
        let mut seen = HashSet::new();
        jobs.into_iter().filter(|j| seen.insert(j.path.clone())).partition(|j| !already_valid(j))
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?;
    let total = (missing.len() + present.len()) as u32;
    let mut done = present.len() as u32;
    progress(done, total);

    let mut results = stream::iter(missing)
        .map(|job| async move { download(client, &job).await })
        .buffer_unordered(PARALLEL_DOWNLOADS);

    let mut last = Instant::now();
    let mut stopped = false;
    while let Some(r) = results.next().await {
        if cancelled() {
            stopped = true;
            continue;
        }
        r?;
        done += 1;
        if done == total || last.elapsed() >= PROGRESS_INTERVAL {
            last = Instant::now();
            progress(done, total);
        }
    }
    if stopped { Err(Error::msg(CANCELLED)) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelling_stops_only_the_running_job() {
        assert!(!cancelled());
        let first = cancellable(async {
            assert!(check_cancelled().is_ok());
            assert!(cancel());
            check_cancelled()
        })
        .await;
        assert_eq!(first.unwrap_err().to_string(), CANCELLED);
        assert!(!cancelled());
        assert!(!cancel());
        assert!(cancellable(async { check_cancelled() }).await.is_ok());
    }
}
