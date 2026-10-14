use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::{Error, Result};
use crate::nbt::{self, Tag};
use crate::{instance, paths};

const DEFAULT_PORT: u16 = 25565;
const TIMEOUT: Duration = Duration::from_secs(4);
const MAX_PACKET: usize = 2 * 1024 * 1024;
const ICON_CAP: u64 = 256 * 1024;

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum QuickPlay {
    Server { address: String },
    World { folder: String },
}

impl QuickPlay {
    pub fn validate(&self, instance_id: &str) -> Result<()> {
        match self {
            QuickPlay::Server { address } => check_address(address).map(|_| ()),
            QuickPlay::World { folder } => {
                let plain = !folder.is_empty() && !folder.starts_with('.') && !folder.contains(['/', '\\', '\0']);
                if plain && paths::game_dir(instance_id).join("saves").join(folder).join("level.dat").is_file() {
                    Ok(())
                } else {
                    Err(Error::msg("World not found"))
                }
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    name: String,
    address: String,
    icon: Option<String>,
    instances: Vec<String>,
}

pub fn check_address(address: &str) -> Result<&str> {
    let a = address.trim();
    if a.is_empty() || a.len() > 260 || a.chars().any(|c| c.is_whitespace() || c.is_control() || c == '/') {
        return Err(Error::msg("Enter a valid server address"));
    }
    Ok(a)
}

fn key(address: &str) -> String {
    let a = address.trim().to_ascii_lowercase();
    a.strip_suffix(":25565").map(String::from).unwrap_or(a)
}

fn file(id: &str) -> PathBuf {
    paths::game_dir(id).join("servers.dat")
}

fn load(id: &str) -> Result<Tag> {
    instance::check_id(id)?;
    match std::fs::File::open(file(id)) {
        Ok(f) => Ok(nbt::read(std::io::BufReader::new(f))?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Tag::Compound(vec![])),
        Err(e) => Err(e.into()),
    }
}

fn store(id: &str, root: &Tag) -> Result<()> {
    let path = file(id);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut buf = Vec::with_capacity(4096);
    nbt::write(&mut buf, root)?;
    let tmp = path.with_extension("dat.piuma");
    std::fs::write(&tmp, buf)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

fn entries(root: &mut Tag) -> &mut Vec<Tag> {
    if root.get("servers").is_none() {
        if let Tag::Compound(v) = root {
            v.push(("servers".into(), Tag::List(10, vec![])));
        }
    }
    match root.get_mut("servers") {
        Some(Tag::List(_, v)) => v,
        _ => unreachable!(),
    }
}

fn visible(t: &Tag) -> bool {
    t.int("hidden").unwrap_or(0) == 0 && t.str("ip").is_some_and(|ip| !ip.trim().is_empty())
}

fn read_list(id: &str) -> Vec<(String, String, Option<String>)> {
    let Ok(mut root) = load(id) else { return vec![] };
    entries(&mut root)
        .iter()
        .filter(|t| visible(t))
        .map(|t| {
            let ip = t.str("ip").unwrap_or_default().to_string();
            let name = t.str("name").filter(|n| !n.trim().is_empty()).map_or_else(|| ip.clone(), String::from);
            let icon = t.str("icon").filter(|i| !i.is_empty()).map(|i| format!("data:image/png;base64,{i}"));
            (name, ip, icon)
        })
        .collect()
}

pub fn list(instance_id: Option<&str>) -> Result<Vec<Server>> {
    let ids: Vec<String> = match instance_id {
        Some(id) => {
            instance::check_id(id)?;
            vec![id.to_string()]
        }
        None => instance::list().into_iter().map(|i| i.id).collect(),
    };
    let mut out: Vec<Server> = vec![];
    let mut index: HashMap<String, usize> = HashMap::new();
    for id in ids {
        for (name, address, icon) in read_list(&id) {
            match index.get(&key(&address)) {
                Some(&i) => {
                    let s = &mut out[i];
                    if !s.instances.contains(&id) {
                        s.instances.push(id.clone());
                    }
                    if s.icon.is_none() {
                        s.icon = icon;
                    }
                }
                None => {
                    index.insert(key(&address), out.len());
                    out.push(Server { name, address, icon, instances: vec![id.clone()] });
                }
            }
        }
    }
    Ok(out)
}

pub fn add(instance_ids: &[String], name: &str, address: &str) -> Result<()> {
    let address = check_address(address)?;
    let name = match name.trim() {
        "" => address,
        n => n,
    };
    let name: String = name.chars().take(64).collect();
    if instance_ids.is_empty() {
        return Err(Error::msg("Choose at least one instance"));
    }
    for id in instance_ids {
        let mut root = load(id)?;
        let list = entries(&mut root);
        match list.iter_mut().find(|t| t.str("ip").is_some_and(|ip| key(ip) == key(address))) {
            Some(Tag::Compound(fields)) => {
                fields.retain(|(k, _)| k != "hidden" && k != "name");
                fields.push(("name".into(), Tag::String(name.clone())));
            }
            _ => list.push(Tag::Compound(vec![
                ("name".into(), Tag::String(name.clone())),
                ("ip".into(), Tag::String(address.to_string())),
            ])),
        }
        store(id, &root)?;
    }
    Ok(())
}

pub fn remove(instance_ids: &[String], address: &str) -> Result<()> {
    for id in instance_ids {
        let mut root = load(id)?;
        let list = entries(&mut root);
        let before = list.len();
        list.retain(|t| !t.str("ip").is_some_and(|ip| key(ip) == key(address)));
        if list.len() != before {
            store(id, &root)?;
        }
    }
    Ok(())
}

pub fn split(address: &str) -> (String, Option<u16>) {
    let a = address.trim();
    if let Some(rest) = a.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            return (host.to_string(), tail.strip_prefix(':').and_then(|p| p.parse().ok()));
        }
    }
    match a.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => (host.to_string(), port.parse().ok()),
        _ => (a.to_string(), None),
    }
}

fn put_varint(buf: &mut Vec<u8>, mut v: u32) {
    loop {
        if v & !0x7F == 0 {
            buf.push(v as u8);
            return;
        }
        buf.push((v & 0x7F | 0x80) as u8);
        v >>= 7;
    }
}

async fn get_varint(s: &mut TcpStream) -> Result<u32> {
    let mut out = 0u32;
    for i in 0..5 {
        let b = s.read_u8().await?;
        out |= ((b & 0x7F) as u32) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(out);
        }
    }
    Err(Error::msg("bad response from the server"))
}

fn take_varint(b: &[u8]) -> Option<(u32, &[u8])> {
    let mut out = 0u32;
    for (i, &x) in b.iter().take(5).enumerate() {
        out |= ((x & 0x7F) as u32) << (7 * i);
        if x & 0x80 == 0 {
            return Some((out, &b[i + 1..]));
        }
    }
    None
}

fn packet(id: u8, body: &[u8]) -> Vec<u8> {
    let mut inner = vec![id];
    inner.extend_from_slice(body);
    let mut out = Vec::with_capacity(inner.len() + 5);
    put_varint(&mut out, inner.len() as u32);
    out.extend(inner);
    out
}

async fn read_packet(s: &mut TcpStream) -> Result<Vec<u8>> {
    let len = get_varint(s).await? as usize;
    if len == 0 || len > MAX_PACKET {
        return Err(Error::msg("bad response from the server"));
    }
    let mut buf = vec![0; len];
    s.read_exact(&mut buf).await?;
    Ok(buf)
}

const COLORS: &[(&str, char)] = &[
    ("black", '0'), ("dark_blue", '1'), ("dark_green", '2'), ("dark_aqua", '3'), ("dark_red", '4'), ("dark_purple", '5'), ("gold", '6'), ("gray", '7'),
    ("dark_gray", '8'), ("blue", '9'), ("green", 'a'), ("aqua", 'b'), ("red", 'c'), ("light_purple", 'd'), ("yellow", 'e'), ("white", 'f'),
];

fn flatten(v: &Value, style: &str, out: &mut String) {
    match v {
        Value::String(s) => {
            out.push_str(style);
            out.push_str(s);
        }
        Value::Array(a) => a.iter().for_each(|c| flatten(c, style, out)),
        Value::Object(o) => {
            let mut own = style.to_string();
            if let Some(c) = o.get("color").and_then(Value::as_str) {
                own = match (c.strip_prefix('#'), COLORS.iter().find(|(n, _)| *n == c)) {
                    (Some(hex), _) if hex.len() == 6 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) => format!("§#{hex}"),
                    (_, Some((_, code))) => format!("§{code}"),
                    _ => own,
                };
            }
            for (k, code) in [("bold", 'l'), ("italic", 'o'), ("underlined", 'n'), ("strikethrough", 'm')] {
                if o.get(k).and_then(Value::as_bool) == Some(true) {
                    own.push('§');
                    own.push(code);
                }
            }
            let text = o.get("text").or_else(|| o.get("translate")).and_then(Value::as_str).unwrap_or_default();
            if !text.is_empty() {
                out.push_str("§r");
                out.push_str(&own);
                out.push_str(text);
            }
            if let Some(extra) = o.get("extra") {
                flatten(extra, &own, out);
            }
        }
        _ => {}
    }
}

fn strip_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '§' {
            it.next();
        } else {
            out.push(c);
        }
    }
    out
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    motd: String,
    version: String,
    protocol: i64,
    online: i64,
    max: i64,
    latency_ms: u64,
    favicon: Option<String>,
    sample: Vec<String>,
}

fn parse_status(json: &str, latency_ms: u64) -> Result<Status> {
    let v: Value = serde_json::from_str(json)?;
    let mut motd = String::new();
    flatten(&v["description"], "", &mut motd);
    let sample = v["players"]["sample"]
        .as_array()
        .map(|a| a.iter().filter_map(|p| p["name"].as_str()).map(strip_codes).filter(|n| !n.trim().is_empty()).take(12).collect())
        .unwrap_or_default();
    Ok(Status {
        motd: motd.trim_end().to_string(),
        version: strip_codes(v["version"]["name"].as_str().unwrap_or_default()),
        protocol: v["version"]["protocol"].as_i64().unwrap_or(-1),
        online: v["players"]["online"].as_i64().unwrap_or(0),
        max: v["players"]["max"].as_i64().unwrap_or(0),
        latency_ms,
        favicon: v["favicon"].as_str().filter(|f| f.starts_with("data:image/png;base64,")).map(String::from),
        sample,
    })
}

async fn query(host: &str, target: &str, port: u16) -> Result<Status> {
    let started = Instant::now();
    let mut s = TcpStream::connect((target, port)).await?;
    s.set_nodelay(true)?;
    let mut hs = vec![];
    put_varint(&mut hs, u32::MAX);
    put_varint(&mut hs, host.len() as u32);
    hs.extend_from_slice(host.as_bytes());
    hs.extend_from_slice(&port.to_be_bytes());
    put_varint(&mut hs, 1);
    let mut out = packet(0, &hs);
    out.extend(packet(0, &[]));
    s.write_all(&out).await?;
    let body = read_packet(&mut s).await?;
    let mut latency = started.elapsed().as_millis() as u64;
    let (id, rest) = take_varint(&body).ok_or_else(|| Error::msg("bad response from the server"))?;
    let (len, rest) = take_varint(rest).ok_or_else(|| Error::msg("bad response from the server"))?;
    if id != 0 || rest.len() < len as usize {
        return Err(Error::msg("bad response from the server"));
    }
    let json = String::from_utf8_lossy(&rest[..len as usize]).into_owned();
    let sent = Instant::now();
    if s.write_all(&packet(1, &0x0050_6975_6d61_i64.to_be_bytes())).await.is_ok() {
        if let Ok(Ok(_)) = tokio::time::timeout(Duration::from_millis(1500), read_packet(&mut s)).await {
            latency = sent.elapsed().as_millis() as u64;
        }
    }
    parse_status(&json, latency)
}

pub async fn ping(address: &str) -> Result<Status> {
    let address = check_address(address)?;
    let (host, port) = split(address);
    let (target, port) = match port {
        Some(p) => (host.clone(), p),
        None if host.parse::<std::net::IpAddr>().is_ok() => (host.clone(), DEFAULT_PORT),
        None => srv(&host).await.unwrap_or_else(|| (host.clone(), DEFAULT_PORT)),
    };
    tokio::time::timeout(TIMEOUT, query(&host, &target, port))
        .await
        .map_err(|_| Error::msg("The server didn't answer"))?
}

#[cfg(not(windows))]
async fn srv(host: &str) -> Option<(String, u16)> {
    let conf = tokio::fs::read_to_string("/etc/resolv.conf").await.ok()?;
    let ns: std::net::IpAddr = conf.lines().find_map(|l| l.trim().strip_prefix("nameserver")?.trim().split('%').next()?.parse().ok())?;
    let sock = tokio::net::UdpSocket::bind(if ns.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }).await.ok()?;
    let nanos = std::time::SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let id = (nanos as u16) ^ ((nanos >> 16) as u16) ^ (std::process::id() as u16);
    let q = dns_query(id, &format!("_minecraft._tcp.{host}"))?;
    sock.send_to(&q, (ns, 53)).await.ok()?;
    let mut buf = [0u8; 1500];
    let n = tokio::time::timeout(Duration::from_secs(2), sock.recv(&mut buf)).await.ok()?.ok()?;
    dns_srv(&buf[..n], id)
}

#[cfg(windows)]
async fn srv(host: &str) -> Option<(String, u16)> {
    let mut cmd = tokio::process::Command::new("nslookup");
    cmd.args(["-type=SRV", &format!("_minecraft._tcp.{host}")]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.creation_flags(0x0800_0000);
    let out = tokio::time::timeout(Duration::from_secs(3), cmd.output()).await.ok()?.ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let field = |name: &str| text.lines().find_map(|l| l.trim().strip_prefix(name)?.trim_start().strip_prefix('=').map(|v| v.trim().to_string()));
    let port = field("port")?.parse().ok()?;
    let target = field("svr hostname")?;
    Some((target.trim_end_matches('.').to_string(), port))
}

#[cfg_attr(windows, allow(dead_code))]
fn dns_query(id: u16, name: &str) -> Option<Vec<u8>> {
    let mut q = Vec::with_capacity(name.len() + 18);
    q.extend_from_slice(&id.to_be_bytes());
    q.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]);
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() || label.len() > 63 {
            return None;
        }
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.extend_from_slice(&[0, 0, 33, 0, 1]);
    Some(q)
}

#[cfg_attr(windows, allow(dead_code))]
fn dns_name(msg: &[u8], mut pos: usize) -> Option<(String, usize)> {
    let mut labels: Vec<String> = vec![];
    let mut end = None;
    for _ in 0..64 {
        let len = *msg.get(pos)? as usize;
        if len == 0 {
            return Some((labels.join("."), end.unwrap_or(pos + 1)));
        }
        if len & 0xC0 == 0xC0 {
            end.get_or_insert(pos + 2);
            pos = ((len & 0x3F) << 8) | *msg.get(pos + 1)? as usize;
            continue;
        }
        labels.push(String::from_utf8_lossy(msg.get(pos + 1..pos + 1 + len)?).into_owned());
        pos += 1 + len;
    }
    None
}

#[cfg_attr(windows, allow(dead_code))]
fn dns_srv(msg: &[u8], id: u16) -> Option<(String, u16)> {
    let u16_at = |p: usize| Some(u16::from_be_bytes([*msg.get(p)?, *msg.get(p + 1)?]));
    if u16_at(0)? != id || msg.get(3)? & 0x0F != 0 {
        return None;
    }
    let (qd, an) = (u16_at(4)?, u16_at(6)?);
    let mut pos = 12;
    for _ in 0..qd {
        pos = dns_name(msg, pos)?.1 + 4;
    }
    let mut best: Option<(u16, String, u16)> = None;
    for _ in 0..an {
        pos = dns_name(msg, pos)?.1;
        let (kind, len) = (u16_at(pos)?, u16_at(pos + 8)? as usize);
        let data = pos + 10;
        if kind == 33 {
            let (prio, port) = (u16_at(data)?, u16_at(data + 4)?);
            let target = dns_name(msg, data + 6)?.0;
            if !target.is_empty() && best.as_ref().is_none_or(|b| prio < b.0) {
                best = Some((prio, target, port));
            }
        }
        pos = data + len;
    }
    best.map(|(_, t, p)| (t, p))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentWorld {
    instance_id: String,
    folder: String,
    name: String,
    modified_ms: u64,
    mode: Option<i64>,
    hardcore: bool,
    icon: Option<String>,
}

pub fn level_info(dir: &Path) -> Option<(String, Option<i64>, bool)> {
    let f = std::fs::File::open(dir.join("level.dat")).ok()?;
    let root = nbt::read(flate2::read::GzDecoder::new(std::io::BufReader::new(f)).take(16 * 1024 * 1024)).ok()?;
    let data = root.get("Data")?;
    let name = data.str("LevelName").map(strip_codes).filter(|n| !n.trim().is_empty())?;
    Some((name, data.int("GameType"), data.int("hardcore").unwrap_or(0) != 0))
}

fn icon(dir: &Path) -> Option<String> {
    let mut buf = vec![];
    std::fs::File::open(dir.join("icon.png")).ok()?.take(ICON_CAP).read_to_end(&mut buf).ok()?;
    buf.starts_with(b"\x89PNG").then(|| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(buf)))
}

pub fn recent_worlds(limit: usize) -> Vec<RecentWorld> {
    let mut all: Vec<(u64, String, PathBuf)> = vec![];
    for inst in instance::list() {
        let saves = paths::game_dir(&inst.id).join("saves");
        for e in std::fs::read_dir(saves).into_iter().flatten().flatten() {
            let dat = e.path().join("level.dat");
            let Ok(m) = std::fs::metadata(&dat).and_then(|m| m.modified()) else { continue };
            let ms = m.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
            all.push((ms, inst.id.clone(), e.path()));
        }
    }
    all.sort_by_key(|w| std::cmp::Reverse(w.0));
    all.into_iter()
        .take(limit)
        .map(|(modified_ms, instance_id, dir)| {
            let folder = dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            let (name, mode, hardcore) = level_info(&dir).unwrap_or_else(|| (folder.clone(), None, false));
            RecentWorld { icon: icon(&dir), instance_id, folder, name, modified_ms, mode, hardcore }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_split_and_normalize() {
        assert_eq!(split("play.example.net"), ("play.example.net".into(), None));
        assert_eq!(split("1.2.3.4:25570"), ("1.2.3.4".into(), Some(25570)));
        assert_eq!(split("[::1]:25566"), ("::1".into(), Some(25566)));
        assert_eq!(split("::1"), ("::1".into(), None));
        assert_eq!(key("Play.Example.NET:25565"), "play.example.net");
        assert!(check_address("bad host").is_err());
        assert!(check_address(" mc.example.org ").is_ok());
    }

    #[test]
    fn motd_components_become_codes() {
        let s = parse_status(
            r##"{"description":{"text":"","extra":[{"text":"Hello ","color":"gold","bold":true},{"text":"world","color":"#12ab9f"}]},"players":{"online":3,"max":20,"sample":[{"name":"§aSteve"}]},"version":{"name":"Paper 1.21.4","protocol":769}}"##,
            10,
        )
        .unwrap();
        assert_eq!(s.motd, "§r§6§lHello §r§#12ab9fworld");
        assert_eq!((s.online, s.max, s.protocol), (3, 20, 769));
        assert_eq!(s.sample, vec!["Steve".to_string()]);
        assert_eq!(parse_status(r#"{"description":"§aPlain"}"#, 0).unwrap().motd, "§aPlain");
    }

    #[test]
    fn srv_answers_are_parsed() {
        let q = dns_query(7, "_minecraft._tcp.example.net").unwrap();
        let mut r = q.clone();
        r[2] = 0x81;
        r[3] = 0x80;
        r[7] = 1;
        r.extend_from_slice(&[0xC0, 12, 0, 33, 0, 1, 0, 0, 0, 60]);
        let mut data = vec![0, 5, 0, 0, 0x63, 0xDD];
        data.extend_from_slice(&[2, b'm', b'c', 0xC0, 28]);
        r.extend_from_slice(&(data.len() as u16).to_be_bytes());
        r.extend(data);
        assert_eq!(dns_srv(&r, 7), Some(("mc.example.net".into(), 25565)));
        assert_eq!(dns_srv(&r, 8), None);
    }

    #[tokio::test]
    #[ignore]
    async fn ping_real_servers() {
        for a in ["mc.hypixel.net", "play.cubecraft.net", "2b2t.org"] {
            match ping(a).await {
                Ok(s) => eprintln!("{a}: {} {}/{} {}ms motd={:?} icon={}", s.version, s.online, s.max, s.latency_ms, s.motd, s.favicon.is_some()),
                Err(e) => eprintln!("{a}: {e}"),
            }
        }
        eprintln!("srv 2b2t: {:?}", srv("2b2t.org").await);
    }

    #[test]
    fn servers_dat_add_remove_keeps_other_fields() {
        let id = format!("servers-test-{}", std::process::id());
        let dir = paths::game_dir(&id);
        std::fs::create_dir_all(&dir).unwrap();
        let root = Tag::Compound(vec![(
            "servers".into(),
            Tag::List(10, vec![
                Tag::Compound(vec![("name".into(), Tag::String("Old".into())), ("ip".into(), Tag::String("a.net".into())), ("acceptTextures".into(), Tag::Byte(1))]),
                Tag::Compound(vec![("ip".into(), Tag::String("b.net".into())), ("hidden".into(), Tag::Byte(1))]),
            ]),
        )]);
        store(&id, &root).unwrap();
        assert_eq!(read_list(&id).len(), 1);
        add(std::slice::from_ref(&id), "", "b.net").unwrap();
        add(std::slice::from_ref(&id), "New", "c.net:25565").unwrap();
        let names: Vec<_> = read_list(&id).into_iter().map(|s| s.0).collect();
        assert_eq!(names, vec!["Old", "b.net", "New"]);
        remove(std::slice::from_ref(&id), "C.NET").unwrap();
        let mut root = load(&id).unwrap();
        assert_eq!(entries(&mut root)[0].int("acceptTextures"), Some(1));
        assert_eq!(read_list(&id).len(), 2);
        let _ = std::fs::remove_dir_all(paths::instance_dir(&id));
    }
}
