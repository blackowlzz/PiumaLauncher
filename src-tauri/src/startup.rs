use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::{instance, paths, stats};

const MARKER: &str = "Sound engine started";
const QUIET: Duration = Duration::from_secs(20);
const KEEP: usize = 20;
const MAX_MS: u64 = 30 * 60 * 1000;
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    pub at_ms: u64,
    pub ms: u64,
    pub mods: Vec<String>,
}

#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StartupReport {
    pub history: Vec<u64>,
    pub last_ms: Option<u64>,
    pub usual_ms: Option<u64>,
    pub slower: Option<Slower>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Slower {
    pub before_ms: u64,
    pub now_ms: u64,
    pub added: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Measured {
    instance_id: String,
    ms: u64,
}

fn file(id: &str) -> std::path::PathBuf {
    paths::instance_dir(id).join("startup.json")
}

fn load(id: &str) -> Vec<Launch> {
    if instance::check_id(id).is_err() {
        return vec![];
    }
    std::fs::read_to_string(file(id)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn enabled_mods(id: &str) -> Vec<String> {
    let mut mods: Vec<String> = std::fs::read_dir(paths::game_dir(id).join("mods"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .filter(|n| n.ends_with(".jar"))
        .collect();
    mods.sort();
    mods
}

fn record(id: &str, at_ms: u64, ms: u64, mods: Vec<String>) {
    if !(1000..MAX_MS).contains(&ms) || instance::check_id(id).is_err() {
        return;
    }
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(id);
    list.retain(|l| l.at_ms != at_ms);
    list.push(Launch { at_ms, ms, mods });
    if list.len() > KEEP {
        list.drain(..list.len() - KEEP);
    }
    if let Ok(text) = serde_json::to_string(&list) {
        let _ = std::fs::write(file(id), text);
    }
}

pub struct Probe {
    app: AppHandle,
    id: String,
    started: Instant,
    mods: Vec<String>,
    state: Mutex<Watch>,
}

#[derive(Default)]
struct Watch {
    closed: bool,
    last: Option<Instant>,
    at_ms: Option<u64>,
}

fn is_loading(message: &str) -> bool {
    message.starts_with(MARKER) || (message.starts_with("Created: ") && message.contains("atlas"))
}

fn is_playing(message: &str) -> bool {
    message.starts_with("Connecting to ") || message.starts_with("Starting integrated minecraft server")
}

impl Watch {
    fn step(&mut self, message: &str, now: Instant) -> bool {
        if self.closed {
            return false;
        }
        if is_playing(message) || self.last.is_some_and(|l| now.duration_since(l) > QUIET) {
            self.closed = true;
            return false;
        }
        if !is_loading(message) {
            return false;
        }
        self.last = Some(now);
        true
    }
}

impl Probe {
    pub fn new(app: AppHandle, id: &str) -> Arc<Probe> {
        Arc::new(Probe { app, id: id.to_string(), started: Instant::now(), mods: enabled_mods(id), state: Mutex::new(Watch::default()) })
    }

    pub fn observe(&self, message: &str) {
        let at_ms = {
            let mut watch = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if !watch.step(message, Instant::now()) {
                return;
            }
            *watch.at_ms.get_or_insert_with(stats::now_ms)
        };
        let ms = self.started.elapsed().as_millis() as u64;
        record(&self.id, at_ms, ms, self.mods.clone());
        let _ = self.app.emit("startup-measured", Measured { instance_id: self.id.clone(), ms });
    }
}

fn median(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn pretty(name: &str) -> String {
    name.strip_suffix(".jar").unwrap_or(name).to_string()
}

fn analyze(list: &[Launch]) -> StartupReport {
    let last_ms = list.last().map(|l| l.ms);
    let mut recent: Vec<u64> = list.iter().rev().take(5).map(|l| l.ms).collect();
    let usual_ms = (recent.len() >= 2).then(|| median(&mut recent));
    let slower = (|| {
        if list.len() < 4 {
            return None;
        }
        let (earlier, latest) = list.split_at(list.len() - 2);
        let mut base: Vec<u64> = earlier.iter().rev().take(5).map(|l| l.ms).collect();
        let before_ms = median(&mut base);
        let now_ms = latest.iter().map(|l| l.ms).min()?;
        if now_ms * 10 < before_ms * 13 || now_ms < before_ms + 8000 {
            return None;
        }
        let reference = earlier.last()?;
        let added = latest
            .last()?
            .mods
            .iter()
            .filter(|m| !reference.mods.contains(m))
            .map(|m| pretty(m))
            .collect();
        Some(Slower { before_ms, now_ms, added })
    })();
    StartupReport { history: list.iter().map(|l| l.ms).collect(), last_ms, usual_ms, slower }
}

pub fn report(id: &str) -> StartupReport {
    analyze(&load(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(ms: u64, mods: &[&str]) -> Launch {
        Launch { at_ms: 0, ms, mods: mods.iter().map(|m| m.to_string()).collect() }
    }

    #[test]
    fn the_marker_is_found_in_both_log_formats() {
        let found = |line: &str| crate::presence::log_message(line).is_some_and(|m| m.starts_with(MARKER));
        assert!(found("[11:10:31] [Render thread/INFO]: Sound engine started"));
        assert!(found("[15:09:41] [Sound engine/INFO]: Sound engine started"));
        assert!(found("    <log4j:Message><![CDATA[Sound engine started]]></log4j:Message>"));
        assert!(!found("[11:10:30] [Render thread/INFO]: Reloading ResourceManager: vanilla"));
        assert!(!found("[11:10:30] [Render thread/INFO]: [chat] Sound engine started"));
    }

    #[test]
    fn the_measure_follows_loading_until_the_player_joins() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut w = Watch::default();
        assert!(!w.step("Setting user: Tester", at(1)));
        assert!(w.step("Sound engine started", at(2)));
        assert!(w.step("Created: 1024x512 textures-atlas", at(9)));
        assert!(w.step("Created: 2048x1024x2 minecraft:textures/atlas/blocks.png-atlas", at(10)));
        assert!(!w.step("Connecting to mc.example.net, 25565", at(14)));
        assert!(!w.step("Created: 1024x512 textures-atlas", at(15)));

        let mut idle = Watch::default();
        assert!(idle.step("Sound engine started", at(2)));
        assert!(!idle.step("Created: 1024x512 textures-atlas", at(40)));
        assert!(idle.closed);

        let mut world = Watch::default();
        assert!(world.step("Created: 1024x512 textures-atlas", at(3)));
        assert!(!world.step("Starting integrated minecraft server version 1.12.2", at(5)));
        assert!(!world.step("Sound engine started", at(6)));
    }

    #[test]
    fn flags_a_slowdown_only_when_it_repeats() {
        let base = ["a.jar", "b.jar"];
        let heavy = ["a.jar", "b.jar", "heavy-1.0.jar"];
        let steady: Vec<Launch> = [20_000, 21_000, 19_000, 20_500].iter().map(|&ms| launch(ms, &base)).collect();
        let r = analyze(&steady);
        assert_eq!(r.last_ms, Some(20_500));
        assert!(r.slower.is_none());

        let mut once = steady.clone();
        once.push(launch(55_000, &heavy));
        once.push(launch(21_000, &heavy));
        assert!(analyze(&once).slower.is_none());

        let mut twice = steady.clone();
        twice.push(launch(60_000, &heavy));
        twice.push(launch(48_000, &heavy));
        let s = analyze(&twice).slower.unwrap();
        assert_eq!((s.before_ms, s.now_ms), (20_500, 48_000));
        assert_eq!(s.added, ["heavy-1.0"]);

        let mut small = steady.clone();
        small.push(launch(26_000, &base));
        small.push(launch(27_000, &base));
        assert!(analyze(&small).slower.is_none());

        assert_eq!(analyze(&[]), StartupReport::default());
        assert!(analyze(&steady[..3]).slower.is_none());
    }
}
