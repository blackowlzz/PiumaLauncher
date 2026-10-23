use std::io::{Read, Write};
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::{settings, stats};

const CLIENT_ID: &str = "1559113688605261854";
const DOWNLOAD_URL: &str = "https://www.piumalauncher.it/download";
const RETRY: Duration = Duration::from_secs(20);
const IO_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, PartialEq, Debug)]
enum Place {
    Launcher,
    Menu,
    Single,
    Server(String),
    Private,
}

#[derive(Clone)]
struct Current {
    id: String,
    player: String,
    version: String,
    since: u64,
    place: Place,
}

enum Msg {
    Show(Current),
    Hide,
}

static TX: OnceLock<Sender<Msg>> = OnceLock::new();
static CURRENT: Mutex<Option<Current>> = Mutex::new(None);
static GEN: AtomicU64 = AtomicU64::new(0);
static LAUNCHER: Mutex<Option<Current>> = Mutex::new(None);

fn send(msg: Msg) {
    let tx = TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new().name("presence".into()).spawn(move || worker(rx)).ok();
        tx
    });
    let _ = tx.send(msg);
}

fn publish() {
    let shown = if enabled() { CURRENT.lock().unwrap().clone().or_else(|| LAUNCHER.lock().unwrap().clone()) } else { None };
    send(shown.map_or(Msg::Hide, Msg::Show));
}

fn enabled() -> bool {
    !CLIENT_ID.is_empty() && settings::load().discord_presence
}

pub fn refresh() {
    if !CLIENT_ID.is_empty() {
        publish();
    }
}

pub fn launcher() {
    if CLIENT_ID.is_empty() {
        return;
    }
    let store = crate::accounts::load();
    let player = store.active.and_then(|id| store.accounts.into_iter().find(|a| a.id == id)).map(|a| a.name).unwrap_or_default();
    *LAUNCHER.lock().unwrap() = Some(Current { id: String::new(), player, version: String::new(), since: stats::now_ms() / 1000, place: Place::Launcher });
    publish();
}

pub fn start(id: &str, player: &str, version: &str) {
    if CLIENT_ID.is_empty() {
        return;
    }
    GEN.fetch_add(1, Ordering::Relaxed);
    *CURRENT.lock().unwrap() = Some(Current { id: id.into(), player: player.into(), version: version.into(), since: stats::now_ms() / 1000, place: Place::Menu });
    publish();
}

pub fn observe(id: &str, line: &str) {
    if CLIENT_ID.is_empty() {
        return;
    }
    let Some((place, port)) = parse(line) else { return };
    let mut guard = CURRENT.lock().unwrap();
    let Some(cur) = guard.as_mut().filter(|c| c.id == id && c.place != place) else { return };
    cur.place = place.clone();
    let gen = GEN.fetch_add(1, Ordering::Relaxed) + 1;
    drop(guard);
    publish();
    if let (Some(port), Place::Server(_) | Place::Private) = (port, place) {
        watch(id.to_string(), port, gen);
    }
}

pub fn stop(id: &str) {
    let mut guard = CURRENT.lock().unwrap();
    if guard.as_ref().is_some_and(|c| c.id == id) {
        *guard = None;
        GEN.fetch_add(1, Ordering::Relaxed);
        drop(guard);
        publish();
    }
}

pub(crate) fn log_message(line: &str) -> Option<&str> {
    message(line).map(|(m, _)| m)
}

fn message(line: &str) -> Option<(&str, bool)> {
    if let Some(i) = line.find("<![CDATA[") {
        let rest = &line[i + 9..];
        return Some((rest.find("]]>").map_or(rest, |e| &rest[..e]), true));
    }
    let at = line.find("/INFO]").or_else(|| line.find("/WARN]"))?;
    let mut rest = line[at + 6..].trim_start_matches(':').trim_start();
    let mut stdout = false;
    if rest.starts_with('(') {
        let (logger, tail) = rest.split_once(") ")?;
        stdout = logger == "(STDOUT";
        rest = tail;
    }
    match rest.strip_prefix("[STDOUT]:") {
        Some(r) => Some((r.trim_start(), true)),
        None => Some((rest, stdout)),
    }
}

#[cfg(test)]
fn place_of(line: &str) -> Option<Place> {
    parse(line).map(|p| p.0)
}

fn parse(line: &str) -> Option<(Place, Option<u16>)> {
    let (msg, stdout) = message(line)?;
    if msg.starts_with("Starting integrated minecraft server") {
        Some((Place::Single, None))
    } else if let Some(rest) = msg.strip_prefix("Connecting to ") {
        let (host, port) = rest.trim_end().rsplit_once(", ")?;
        let port = port.parse::<u16>().ok()?;
        Some((classify(host), Some(port)))
    } else if msg.starts_with("Stopping server") || msg.starts_with("Client disconnected") || (stdout && msg.starts_with("Disconnected")) {
        Some((Place::Menu, None))
    } else {
        None
    }
}

fn classify(host: &str) -> Place {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let plain = host.trim_start_matches('[').trim_end_matches(']');
    if plain.parse::<IpAddr>().is_ok() || !plain.contains('.') {
        return Place::Private;
    }
    let tld = plain.rsplit('.').next().unwrap_or("");
    let public = tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_lowercase()) && !matches!(tld, "local" | "lan" | "home" | "internal" | "localdomain" | "arpa");
    if public && plain.len() <= 128 {
        Place::Server(plain.to_string())
    } else {
        Place::Private
    }
}

fn watch(id: String, port: u16, gen: u64) {
    std::thread::Builder::new()
        .name("presence-watch".into())
        .spawn(move || {
            let started = Instant::now();
            let mut seen = false;
            let mut misses = 0;
            loop {
                std::thread::sleep(Duration::from_secs(4));
                if GEN.load(Ordering::Relaxed) != gen {
                    return;
                }
                match established(port) {
                    None => return,
                    Some(true) => {
                        seen = true;
                        misses = 0;
                    }
                    Some(false) if seen || started.elapsed() > Duration::from_secs(25) => {
                        misses += 1;
                        if misses >= 2 {
                            leave(&id, gen);
                            return;
                        }
                    }
                    Some(false) => {}
                }
            }
        })
        .ok();
}

fn leave(id: &str, gen: u64) {
    let mut guard = CURRENT.lock().unwrap();
    let Some(cur) = guard.as_mut().filter(|c| c.id == id && GEN.load(Ordering::Relaxed) == gen) else { return };
    cur.place = Place::Menu;
    GEN.fetch_add(1, Ordering::Relaxed);
    drop(guard);
    publish();
}

#[cfg(unix)]
fn established(port: u16) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata("/proc/self").ok()?.uid();
    let mut readable = false;
    for file in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        readable = true;
        if text.lines().skip(1).any(|l| connection_matches(l, port, uid)) {
            return Some(true);
        }
    }
    readable.then_some(false)
}

#[cfg(not(unix))]
fn established(_port: u16) -> Option<bool> {
    None
}

#[cfg(unix)]
fn connection_matches(row: &str, port: u16, uid: u32) -> bool {
    let cols: Vec<&str> = row.split_whitespace().collect();
    cols.len() > 7
        && cols[3] == "01"
        && cols[7].parse::<u32>().ok() == Some(uid)
        && cols[2].rsplit(':').next().and_then(|p| u16::from_str_radix(p, 16).ok()) == Some(port)
}

fn worker(rx: Receiver<Msg>) {
    let mut conn: Option<Conn> = None;
    let mut want: Option<Current> = None;
    let mut dirty = false;
    loop {
        let msg = if want.is_some() {
            match rx.recv_timeout(RETRY) {
                Ok(m) => Some(m),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => return,
            }
        };
        match msg {
            Some(Msg::Show(c)) => {
                want = Some(c);
                dirty = true;
            }
            Some(Msg::Hide) => {
                want = None;
                dirty = false;
                conn = None;
                continue;
            }
            None => {}
        }
        let Some(c) = &want else { continue };
        if conn.is_none() {
            conn = Conn::open();
            dirty = true;
        }
        if dirty {
            if let Some(k) = conn.as_mut() {
                if k.set(c).is_ok() {
                    dirty = false;
                } else {
                    conn = None;
                }
            }
        }
    }
}

#[cfg(unix)]
type Stream = std::os::unix::net::UnixStream;
#[cfg(windows)]
type Stream = std::fs::File;

struct Conn {
    stream: Stream,
    nonce: u64,
}

impl Conn {
    #[cfg(unix)]
    fn open() -> Option<Conn> {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        for var in ["XDG_RUNTIME_DIR", "TMPDIR"] {
            if let Some(d) = std::env::var_os(var) {
                let d = std::path::PathBuf::from(d);
                dirs.push(d.join("app/com.discordapp.Discord"));
                dirs.push(d.join("snap.discord"));
                dirs.push(d);
            }
        }
        dirs.push("/tmp".into());
        for dir in dirs {
            for i in 0..10 {
                if let Ok(s) = Stream::connect(dir.join(format!("discord-ipc-{i}"))) {
                    let _ = s.set_read_timeout(Some(IO_TIMEOUT));
                    let _ = s.set_write_timeout(Some(IO_TIMEOUT));
                    if let Some(c) = Conn::handshake(s) {
                        return Some(c);
                    }
                }
            }
        }
        None
    }

    #[cfg(windows)]
    fn open() -> Option<Conn> {
        for i in 0..10 {
            let path = format!(r"\\.\pipe\discord-ipc-{i}");
            if let Ok(s) = std::fs::OpenOptions::new().read(true).write(true).open(path) {
                if let Some(c) = Conn::handshake(s) {
                    return Some(c);
                }
            }
        }
        None
    }

    fn handshake(stream: Stream) -> Option<Conn> {
        let mut c = Conn { stream, nonce: 0 };
        c.frame(0, &json!({ "v": 1, "client_id": CLIENT_ID })).ok()?;
        Some(c)
    }

    fn frame(&mut self, op: u32, body: &serde_json::Value) -> std::io::Result<()> {
        let data = serde_json::to_vec(body)?;
        let mut buf = Vec::with_capacity(8 + data.len());
        buf.extend_from_slice(&op.to_le_bytes());
        buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&data);
        self.stream.write_all(&buf)?;
        let mut head = [0u8; 8];
        self.stream.read_exact(&mut head)?;
        let len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
        if len > 65_536 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        let mut reply = vec![0u8; len];
        self.stream.read_exact(&mut reply)?;
        if u32::from_le_bytes([head[0], head[1], head[2], head[3]]) == 2 {
            return Err(std::io::ErrorKind::ConnectionAborted.into());
        }
        Ok(())
    }

    fn set(&mut self, c: &Current) -> std::io::Result<()> {
        self.nonce += 1;
        let state = match &c.place {
            Place::Launcher => "In the launcher".to_string(),
            Place::Menu => "In the menus".to_string(),
            Place::Single => "Singleplayer".to_string(),
            Place::Server(host) => host.clone(),
            Place::Private => "Private server".to_string(),
        };
        let details = if c.place == Place::Launcher {
            if c.player.is_empty() { "PiumaLauncher".to_string() } else { format!("Player: {}", c.player) }
        } else {
            format!("Playing as {}", c.player)
        };
        let body = json!({
            "cmd": "SET_ACTIVITY",
            "nonce": self.nonce.to_string(),
            "args": {
                "pid": std::process::id(),
                "activity": {
                    "details": details,
                    "state": state,
                    "timestamps": { "start": c.since },
                    "assets": { "large_image": "piuma", "large_text": if c.version.is_empty() { "PiumaLauncher".to_string() } else { format!("Minecraft {}", c.version) } },
                    "buttons": [{ "label": "Get PiumaLauncher", "url": DOWNLOAD_URL }],
                },
            },
        });
        self.frame(1, &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn matches_established_connections_to_the_server_port() {
        let row = "   5: 0100A8C0:9C40 8E3D0A22:63DD 01 00000000:00000000 00:00000000 00000000  1000        0 98765 1 0000000000000000 20 4 30 10 -1";
        assert!(connection_matches(row, 25565, 1000));
        assert!(!connection_matches(row, 25566, 1000));
        assert!(!connection_matches(row, 25565, 1001));
        assert!(!connection_matches(&row.replace(" 01 ", " 06 "), 25565, 1000));
    }

    #[test]
    fn domains_are_shown_and_addresses_are_private() {
        assert_eq!(classify("play.Hypixel.net"), Place::Server("play.hypixel.net".into()));
        assert_eq!(classify("localhost"), Place::Private);
        assert_eq!(classify("192.168.1.10"), Place::Private);
        assert_eq!(classify("[::1]"), Place::Private);
        assert_eq!(classify("mybox.local"), Place::Private);
        assert_eq!(classify("server"), Place::Private);
    }

    #[test]
    fn reads_places_from_game_log() {
        let l = |m: &str| place_of(&format!("[22:43:55] [Render thread/INFO]: {m}"));
        assert_eq!(l("Starting integrated minecraft server version 1.21.1"), Some(Place::Single));
        assert_eq!(l("Connecting to mc.example.com, 25565"), Some(Place::Server("mc.example.com".into())));
        assert_eq!(l("Connecting to localhost, 25565"), Some(Place::Private));
        assert_eq!(l("Connecting to Modrinth, now"), None);
        assert_eq!(l("[STDOUT]: Disconnected: literal{Server closed}"), Some(Place::Menu));
        assert_eq!(l("Stopping server"), Some(Place::Menu));
        assert_eq!(l("Loaded 12 mods"), None);
        let f = |m: &str| place_of(&format!("[10:26:05] [Render thread/INFO] (Minecraft) {m}"));
        assert_eq!(f("Connecting to hypixel.net, 25565"), Some(Place::Server("hypixel.net".into())));
        assert_eq!(f("Starting integrated minecraft server version 26.2"), Some(Place::Single));
        assert_eq!(f("Stopping server"), Some(Place::Menu));
        assert_eq!(place_of("[10:26:05] [Render thread/INFO] (STDOUT) Disconnected: x"), Some(Place::Menu));
        assert_eq!(place_of("[10:26:05] [Render thread/INFO] (Mod) Disconnected: x"), None);
        assert_eq!(place_of("[10:26:05] [Render thread/INFO] [STDOUT]: Disconnected: x"), Some(Place::Menu));
        assert_eq!(place_of("Connecting to hypixel.net, 25565"), None);
        let x = |m: &str| place_of(&format!("<log4j:Message><![CDATA[{m}]]></log4j:Message>"));
        assert_eq!(x("Connecting to hypixel.net, 25565"), Some(Place::Server("hypixel.net".into())));
        assert_eq!(x("Connecting to 10.0.0.2, 25565"), Some(Place::Private));
        assert_eq!(x("Starting integrated minecraft server version 26.2"), Some(Place::Single));
        assert_eq!(x("Stopping server"), Some(Place::Menu));
        assert_eq!(x("Loaded 3 mods"), None);
        assert_eq!(place_of("<log4j:Event logger=\"net.minecraft.client.multiplayer.ClientHandshakePacketListenerImpl\" level=\"INFO\" thread=\"Render thread\"><log4j:Message><![CDATA[Connecting to play.example.org, 25565]]></log4j:Message>"), Some(Place::Server("play.example.org".into())));
    }
}
