use std::future::Future;
use std::io;
use std::time::Duration;

const ATTEMPTS: u32 = if cfg!(windows) { 8 } else { 1 };

fn transient(e: &io::Error) -> bool {
    cfg!(windows) && (e.kind() == io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32))
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(150 * attempt as u64)
}

pub fn retry<T>(mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut attempt = 1;
    loop {
        match op() {
            Err(e) if attempt < ATTEMPTS && transient(&e) => {
                std::thread::sleep(backoff(attempt));
                attempt += 1;
            }
            r => return r,
        }
    }
}

pub async fn retry_async<T, F, Fut>(mut op: F) -> io::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = io::Result<T>>,
{
    let mut attempt = 1;
    loop {
        match op().await {
            Err(e) if attempt < ATTEMPTS && transient(&e) => {
                tokio::time::sleep(backoff(attempt)).await;
                attempt += 1;
            }
            r => return r,
        }
    }
}
