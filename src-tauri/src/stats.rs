
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::paths;

const MAX_SESSIONS: usize = 3000;
const MIN_SECS: u64 = 5;
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub instance_id: String,
    pub start_ms: u64,
    pub secs: u64,
}

fn file() -> std::path::PathBuf {
    paths::root().join("sessions.json")
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub fn load() -> Vec<Session> {
    std::fs::read_to_string(file())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn record(instance_id: &str, start_ms: u64, secs: u64) {
    if secs < MIN_SECS {
        return;
    }
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = load();
    all.push(Session { instance_id: instance_id.to_string(), start_ms, secs });
    if all.len() > MAX_SESSIONS {
        all.drain(..all.len() - MAX_SESSIONS);
    }
    if let Ok(text) = serde_json::to_string(&all) {
        let _ = std::fs::create_dir_all(paths::root());
        let _ = std::fs::write(file(), text);
    }
}
