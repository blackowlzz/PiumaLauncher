use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::paths;

const REPLY: &str = "piuma-ok";
const MAX_LINE: usize = 4096;

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub instance: Option<String>,
    pub server: Option<String>,
    pub world: Option<String>,
    pub pack: Option<String>,
}

impl Request {
    fn is_empty(&self) -> bool {
        self.instance.is_none() && self.server.is_none() && self.world.is_none() && self.pack.is_none()
    }
}

static PENDING: Mutex<Option<Request>> = Mutex::new(None);

pub fn parse(args: impl IntoIterator<Item = String>) -> Request {
    let mut req = Request::default();
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (arg, None),
        };
        let slot = match flag.as_str() {
            "--instance" | "-i" => &mut req.instance,
            "--server" | "-s" => &mut req.server,
            "--world" | "-w" => &mut req.world,
            path if !path.starts_with('-') && path.to_ascii_lowercase().ends_with(".mrpack") => {
                req.pack = std::fs::canonicalize(path).ok().map(|p| p.to_string_lossy().into_owned());
                continue;
            }
            _ => continue,
        };
        *slot = inline.or_else(|| it.next()).map(|v| v.trim().to_string()).filter(|v| !v.is_empty() && v.len() <= 256);
    }
    req
}

fn lock_file() -> std::path::PathBuf {
    paths::root().join("launcher.lock")
}

pub fn forward(req: &Request) -> bool {
    forward_via(&lock_file(), req)
}

fn forward_via(lock: &std::path::Path, req: &Request) -> bool {
    let Ok(text) = std::fs::read_to_string(lock) else { return false };
    let Some((port, token)) = text.trim().split_once(' ') else { return false };
    let Ok(port) = port.parse::<u16>() else { return false };
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else { return false };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let Ok(body) = serde_json::to_string(req) else { return false };
    if s.write_all(format!("{token}\n{body}\n").as_bytes()).is_err() {
        return false;
    }
    let mut reply = String::new();
    BufReader::new(s).read_line(&mut reply).is_ok() && reply.trim() == REPLY
}

fn write_lock(port: u16, token: &str) -> std::io::Result<()> {
    let path = lock_file();
    std::fs::create_dir_all(paths::root())?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    opts.open(path)?.write_all(format!("{port} {token}").as_bytes())
}

fn deliver(app: &AppHandle, req: Request) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    if !req.is_empty() {
        *PENDING.lock().unwrap() = Some(req);
        let _ = app.emit("cli-request", ());
    }
}

fn read_request(s: TcpStream, token: &str) -> Option<Request> {
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let mut out = s.try_clone().ok()?;
    let mut lines = BufReader::new(s).take(MAX_LINE as u64 * 2).lines();
    if lines.next()?.ok()? != token {
        return None;
    }
    let req = serde_json::from_str::<Request>(&lines.next()?.ok()?).ok()?;
    out.write_all(format!("{REPLY}\n").as_bytes()).ok()?;
    Some(req)
}

pub fn serve(app: AppHandle, first: Request) {
    if !first.is_empty() {
        *PENDING.lock().unwrap() = Some(first);
    }
    let Ok(listener) = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) else { return };
    let Ok(port) = listener.local_addr().map(|a| a.port()) else { return };
    let token = uuid::Uuid::new_v4().simple().to_string();
    if write_lock(port, &token).is_err() {
        return;
    }
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
            if let Some(req) = read_request(s, &token) {
                deliver(&app, req);
            }
        }
    });
}

pub fn take() -> Option<Request> {
    PENDING.lock().unwrap().take()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Request {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn flags_are_read_in_any_form() {
        assert_eq!(
            p(&["--instance", "Cobblemon", "--server=mc.hypixel.net"]),
            Request { instance: Some("Cobblemon".into()), server: Some("mc.hypixel.net".into()), world: None, pack: None }
        );
        assert_eq!(p(&["-i", "abc", "-w", "My World"]).world.as_deref(), Some("My World"));
        assert!(p(&["--unknown", "x", "--instance"]).is_empty());
        assert!(p(&["--instance", "  "]).instance.is_none());
    }

    #[test]
    fn a_second_launch_reaches_the_first() {
        let lock = std::env::temp_dir().join(format!("piuma-lock-{}", std::process::id()));
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        std::fs::write(&lock, format!("{port} secret")).unwrap();
        let server = std::thread::spawn(move || {
            let mut got = vec![];
            for s in listener.incoming().take(2).flatten() {
                got.push(read_request(s, "secret"));
            }
            got
        });
        let req = p(&["--instance", "Cobblemon", "--server", "mc.hypixel.net"]);
        assert!(forward_via(&lock, &req));
        std::fs::write(&lock, format!("{port} wrong")).unwrap();
        assert!(!forward_via(&lock, &req));
        assert_eq!(server.join().unwrap(), vec![Some(req), None]);
        std::fs::write(&lock, "1 secret").unwrap();
        assert!(!forward_via(&lock, &Request::default()));
        let _ = std::fs::remove_file(&lock);
        assert!(!forward_via(&lock, &Request::default()));
    }

    #[test]
    fn requests_travel_as_one_line() {
        let r = p(&["--instance", "a b", "--world", "Mondo \"1\""]);
        let line = serde_json::to_string(&r).unwrap();
        assert!(!line.contains('\n'));
        assert_eq!(serde_json::from_str::<Request>(&line).unwrap(), r);
    }
}
