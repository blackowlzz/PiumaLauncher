use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

use crate::error::{Error, Result};
use crate::instance::Instance;
use crate::loaders::Loader;
use crate::modrinth::{self, ApiVersion, Content, Progress};
use crate::paths;

const API: &str = "https://api.modrinth.com/v2";
const CODE_PREFIX: &str = "PIUMA1-";
const MAX_CODE: usize = 64 * 1024;
const MAX_PAYLOAD: u64 = 256 * 1024;
const MAX_FILES: usize = 2000;

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub resourcepacks: bool,
    pub shaderpacks: bool,
    pub config: bool,
    pub options: bool,
    pub servers: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    path: String,
    linked: u32,
    bundled: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareCode {
    code: String,
    included: u32,
    excluded: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    instance: Instance,
    skipped: u32,
    server: Option<String>,
}

struct Local {
    kind: Content,
    file_name: String,
    path: PathBuf,
    sha1: String,
}

fn loader_key(loader: Loader) -> Option<&'static str> {
    match loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("fabric-loader"),
        Loader::Quilt => Some("quilt-loader"),
        Loader::Forge => Some("forge"),
        Loader::NeoForge => Some("neoforge"),
    }
}

fn kind_letter(kind: Content) -> char {
    match kind {
        Content::Mods => 'm',
        Content::Resourcepacks => 'r',
        Content::Shaderpacks => 's',
    }
}

fn letter_kind(c: char) -> Option<Content> {
    match c {
        'm' => Some(Content::Mods),
        'r' => Some(Content::Resourcepacks),
        's' => Some(Content::Shaderpacks),
        _ => None,
    }
}

fn dependencies(inst: &Instance) -> Value {
    let mut deps = serde_json::Map::new();
    deps.insert("minecraft".into(), json!(inst.mc_version));
    if let (Some(key), Some(v)) = (loader_key(inst.loader), inst.loader_version.as_deref()) {
        deps.insert(key.into(), json!(v));
    }
    Value::Object(deps)
}

async fn collect(inst: &Instance, kinds: &[Content], progress: Progress<'_>) -> Result<Vec<Local>> {
    progress("Reading files", 0, 0);
    let id = inst.id.clone();
    let kinds = kinds.to_vec();
    tokio::task::spawn_blocking(move || {
        let mut out = vec![];
        for kind in kinds {
            let dir = paths::game_dir(&id).join(kind.folder());
            for f in modrinth::list_content(&id, kind)?.into_iter().filter(|f| f.enabled) {
                let path = dir.join(&f.file_name);
                if !path.is_file() {
                    continue;
                }
                let sha1 = modrinth::sha1_of(&path)?;
                out.push(Local { kind, file_name: f.file_name, path, sha1 });
            }
        }
        Ok(out)
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

async fn identify(client: &reqwest::Client, files: &[Local], progress: Progress<'_>) -> Result<HashMap<String, ApiVersion>> {
    progress("Looking up Modrinth", 0, 0);
    let hashes: Vec<String> = files.iter().map(|f| f.sha1.clone()).collect();
    let mut found = HashMap::new();
    for chunk in hashes.chunks(500) {
        found.extend(modrinth::lookup_versions(client, chunk).await?);
    }
    Ok(found)
}

fn sha512_of(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha512};
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha512::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn safe_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' }).collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() { "modpack".into() } else { s.chars().take(64).collect() }
}

fn free_path(dir: &Path, base: &str) -> PathBuf {
    let first = dir.join(format!("{base}.mrpack"));
    if !first.exists() {
        return first;
    }
    (2..1000).map(|n| dir.join(format!("{base} ({n}).mrpack"))).find(|p| !p.exists()).unwrap_or(first)
}

fn options_for(path: &Path) -> SimpleFileOptions {
    let packed = path.extension().is_some_and(|e| e == "jar" || e == "zip" || e == "png");
    SimpleFileOptions::default()
        .compression_method(if packed { CompressionMethod::Stored } else { CompressionMethod::Deflated })
        .large_file(std::fs::metadata(path).is_ok_and(|m| m.len() >= u32::MAX as u64))
}

fn add_file<W: Write + std::io::Seek>(zip: &mut zip::ZipWriter<W>, name: &str, path: &Path) -> Result<()> {
    zip.start_file(name, options_for(path))?;
    std::io::copy(&mut std::fs::File::open(path)?, zip)?;
    Ok(())
}

fn add_dir<W: Write + std::io::Seek>(zip: &mut zip::ZipWriter<W>, prefix: &str, dir: &Path) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(()) };
    for e in entries.flatten() {
        let Ok(t) = e.file_type() else { continue };
        let name = format!("{prefix}/{}", e.file_name().to_string_lossy());
        if t.is_dir() {
            add_dir(zip, &name, &e.path())?;
        } else if t.is_file() {
            add_file(zip, &name, &e.path())?;
        }
    }
    Ok(())
}

pub async fn export(client: &reqwest::Client, inst: &Instance, opts: ExportOptions, progress: Progress<'_>) -> Result<Exported> {
    let dir = dirs::download_dir().or_else(dirs::home_dir).ok_or_else(|| Error::msg("Downloads folder not found"))?;
    export_to(client, inst, opts, &dir, progress).await
}

async fn export_to(client: &reqwest::Client, inst: &Instance, opts: ExportOptions, dir: &Path, progress: Progress<'_>) -> Result<Exported> {
    let mut kinds = vec![Content::Mods];
    if opts.resourcepacks {
        kinds.push(Content::Resourcepacks);
    }
    if opts.shaderpacks {
        kinds.push(Content::Shaderpacks);
    }
    let files = collect(inst, &kinds, progress).await?;
    let found = identify(client, &files, progress).await?;

    progress("Packing", 0, 0);
    let out = free_path(dir, &safe_name(&inst.name));
    let inst = inst.clone();
    let target = out.clone();
    let (linked, bundled) = tokio::task::spawn_blocking(move || -> Result<(u32, u32)> {
        let game = paths::game_dir(&inst.id);
        let mut entries = vec![];
        let mut loose = vec![];
        for f in &files {
            let remote = found.get(&f.sha1).and_then(|v| v.files.iter().find(|vf| vf.hashes.sha1.as_deref() == Some(f.sha1.as_str())));
            match remote.filter(|vf| modrinth::allowed_url(&vf.url).is_ok()) {
                Some(vf) => {
                    let sha512 = match &vf.hashes.sha512 {
                        Some(h) => h.clone(),
                        None => sha512_of(&f.path)?,
                    };
                    entries.push(json!({
                        "path": format!("{}/{}", f.kind.folder(), f.file_name),
                        "hashes": { "sha1": f.sha1, "sha512": sha512 },
                        "downloads": [vf.url],
                        "fileSize": std::fs::metadata(&f.path)?.len(),
                    }));
                }
                None => loose.push(f),
            }
        }
        let index = json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": env!("CARGO_PKG_VERSION"),
            "name": inst.name,
            "summary": "Exported from PiumaLauncher",
            "files": entries,
            "dependencies": dependencies(&inst),
        });
        let tmp = target.with_extension("mrpack.part");
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(std::fs::File::create(&tmp)?));
        zip.start_file("modrinth.index.json", SimpleFileOptions::default())?;
        zip.write_all(serde_json::to_string_pretty(&index)?.as_bytes())?;
        for f in &loose {
            add_file(&mut zip, &format!("overrides/{}/{}", f.kind.folder(), f.file_name), &f.path)?;
        }
        if opts.config {
            add_dir(&mut zip, "overrides/config", &game.join("config"))?;
        }
        for (wanted, name) in [(opts.options, "options.txt"), (opts.servers, "servers.dat")] {
            let p = game.join(name);
            if wanted && p.is_file() {
                add_file(&mut zip, &format!("overrides/{name}"), &p)?;
            }
        }
        zip.finish()?.flush()?;
        std::fs::rename(&tmp, &target)?;
        Ok((entries.len() as u32, loose.len() as u32))
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
    Ok(Exported { path: out.to_string_lossy().into_owned(), linked, bundled })
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Payload {
    n: String,
    m: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    l: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    v: String,
    f: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    s: Option<String>,
}

fn encode(p: &Payload) -> Result<String> {
    let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    enc.write_all(serde_json::to_string(p)?.as_bytes())?;
    Ok(format!("{CODE_PREFIX}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(enc.finish()?)))
}

fn decode(code: &str) -> Result<Payload> {
    let bad = || Error::msg("This share code isn't valid");
    let compact: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() > MAX_CODE {
        return Err(bad());
    }
    let data = compact.find(CODE_PREFIX).map(|i| &compact[i + CODE_PREFIX.len()..]).ok_or_else(bad)?;
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(data.trim_end_matches('=')).map_err(|_| bad())?;
    let mut text = String::new();
    flate2::read::DeflateDecoder::new(raw.as_slice()).take(MAX_PAYLOAD).read_to_string(&mut text).map_err(|_| bad())?;
    serde_json::from_str(&text).map_err(|_| bad())
}

fn parse_entries(f: &str) -> Result<Vec<(Content, String)>> {
    let bad = || Error::msg("This share code isn't valid");
    let mut out = vec![];
    for item in f.split(',').filter(|s| !s.is_empty()).take(MAX_FILES) {
        let mut chars = item.chars();
        let kind = chars.next().and_then(letter_kind).ok_or_else(bad)?;
        let id = chars.as_str();
        if id.is_empty() || id.len() > 16 || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(bad());
        }
        out.push((kind, id.to_string()));
    }
    Ok(out)
}

pub async fn share_code(client: &reqwest::Client, inst: &Instance, server: Option<&str>, progress: Progress<'_>) -> Result<ShareCode> {
    let server = server.map(crate::servers::check_address).transpose()?.map(String::from);
    let files = collect(inst, &[Content::Mods, Content::Resourcepacks, Content::Shaderpacks], progress).await?;
    let found = identify(client, &files, progress).await?;
    let mut items = vec![];
    let mut excluded = vec![];
    for f in &files {
        match found.get(&f.sha1) {
            Some(v) => items.push(format!("{}{}", kind_letter(f.kind), v.id)),
            None => excluded.push(f.file_name.clone()),
        }
    }
    if items.is_empty() && server.is_none() {
        return Err(Error::msg("None of the files in this instance come from Modrinth"));
    }
    let payload = Payload {
        n: inst.name.clone(),
        m: inst.mc_version.clone(),
        l: loader_key(inst.loader).unwrap_or_default().to_string(),
        v: inst.loader_version.clone().unwrap_or_default(),
        f: items.join(","),
        s: server,
    };
    Ok(ShareCode { code: encode(&payload)?, included: items.len() as u32, excluded })
}

async fn resolve(client: &reqwest::Client, ids: &[String]) -> Result<HashMap<String, ApiVersion>> {
    let mut out = HashMap::new();
    for chunk in ids.chunks(100) {
        let query = serde_json::to_string(chunk)?;
        let list: Vec<ApiVersion> = client.get(format!("{API}/versions")).query(&[("ids", query)]).send().await?.error_for_status()?.json().await?;
        out.extend(list.into_iter().map(|v| (v.id.clone(), v)));
    }
    Ok(out)
}

fn write_index_pack(path: &Path, index: &Value) -> Result<()> {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path)?);
    zip.start_file("modrinth.index.json", SimpleFileOptions::default())?;
    zip.write_all(serde_json::to_string(index)?.as_bytes())?;
    zip.finish()?;
    Ok(())
}

fn temp_pack() -> Result<PathBuf> {
    let dir = paths::root().join("tmp");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("import-{}.mrpack", uuid::Uuid::new_v4().simple())))
}

async fn install_temp(client: &reqwest::Client, archive: &Path, progress: Progress<'_>) -> Result<Instance> {
    let hash = modrinth::sha1_of(archive).ok();
    let result = modrinth::install_archive(client, archive, async { None }, progress).await;
    let _ = tokio::fs::remove_file(archive).await;
    let mut inst = result?;
    if let Some(hash) = hash {
        modrinth::link_pack(client, &mut inst, &hash).await;
    }
    Ok(inst)
}

pub async fn import_code(client: &reqwest::Client, code: &str, progress: Progress<'_>) -> Result<Imported> {
    let payload = decode(code)?;
    let entries = parse_entries(&payload.f)?;
    let server = payload.s.as_deref().and_then(|a| crate::servers::check_address(a).ok()).map(String::from);
    if (entries.is_empty() && server.is_none()) || payload.m.trim().is_empty() {
        return Err(Error::msg("This share code isn't valid"));
    }
    progress("Looking up Modrinth", 0, 0);
    let ids: Vec<String> = entries.iter().map(|(_, id)| id.clone()).collect();
    let versions = resolve(client, &ids).await?;
    let mut files = vec![];
    let mut skipped = 0u32;
    for (kind, id) in &entries {
        let file = versions.get(id).and_then(|v| v.files.iter().find(|f| f.primary).or(v.files.first()));
        let Some(f) = file.filter(|f| modrinth::allowed_url(&f.url).is_ok() && modrinth::plain_file_name(&f.filename).is_ok()) else {
            skipped += 1;
            continue;
        };
        files.push(json!({
            "path": format!("{}/{}", kind.folder(), f.filename),
            "hashes": { "sha1": f.hashes.sha1, "sha512": f.hashes.sha512 },
            "downloads": [f.url],
            "fileSize": f.size,
        }));
    }
    let mut deps = serde_json::Map::new();
    deps.insert("minecraft".into(), json!(payload.m));
    if ["fabric-loader", "quilt-loader", "forge", "neoforge"].contains(&payload.l.as_str()) && !payload.v.is_empty() {
        deps.insert(payload.l.clone(), json!(payload.v));
    }
    let name: String = payload.n.chars().filter(|c| !c.is_control()).take(64).collect();
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "share",
        "name": if name.trim().is_empty() { "Shared instance".to_string() } else { name },
        "files": files,
        "dependencies": deps,
    });
    let archive = temp_pack()?;
    write_index_pack(&archive, &index)?;
    let instance = install_temp(client, &archive, progress).await?;
    Ok(Imported { instance, skipped, server })
}

fn parse_link(link: &str) -> Option<(String, Option<String>)> {
    let link = link.trim();
    let rest = link.strip_prefix("https://").or_else(|| link.strip_prefix("http://")).unwrap_or(link);
    let rest = rest.split(['?', '#']).next()?;
    let mut parts = rest.split('/').filter(|p| !p.is_empty());
    let host = parts.next()?;
    if !host.eq_ignore_ascii_case("modrinth.com") && !host.eq_ignore_ascii_case("www.modrinth.com") {
        return None;
    }
    if !matches!(parts.next()?, "modpack" | "project") {
        return None;
    }
    let slug = parts.next()?.to_string();
    let version = match parts.next() {
        Some("version") => parts.next().map(str::to_string),
        _ => None,
    };
    Some((slug, version))
}

pub async fn import_link(client: &reqwest::Client, link: &str, progress: Progress<'_>) -> Result<Imported> {
    let (slug, version) = parse_link(link).ok_or_else(|| Error::msg("This link isn't a Modrinth modpack"))?;
    progress("Finding the latest version", 0, 0);
    let (project, version) = modrinth::resolve_modpack(client, &slug, version.as_deref()).await?;
    let instance = modrinth::install_modpack(client, &project, version.as_deref(), None, progress).await?;
    Ok(Imported { instance, skipped: 0, server: None })
}

pub async fn import_file(client: &reqwest::Client, path: &Path, progress: Progress<'_>) -> Result<Imported> {
    let meta = std::fs::metadata(path).map_err(|_| Error::msg("File not found"))?;
    if !meta.is_file() || !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("mrpack") || e.eq_ignore_ascii_case("zip")) {
        return Err(Error::msg("Choose a .mrpack file"));
    }
    let archive = temp_pack()?;
    std::fs::copy(path, &archive)?;
    Ok(Imported { instance: install_temp(client, &archive, progress).await?, skipped: 0, server: None })
}

pub async fn import_bytes(client: &reqwest::Client, bytes: &[u8], progress: Progress<'_>) -> Result<Imported> {
    if !bytes.starts_with(b"PK") {
        return Err(Error::msg("Choose a .mrpack file"));
    }
    let archive = temp_pack()?;
    tokio::fs::write(&archive, bytes).await?;
    Ok(Imported { instance: install_temp(client, &archive, progress).await?, skipped: 0, server: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_stay_short() {
        let ids: Vec<String> = (0..120).map(|i| format!("m{:08}", i * 7919)).collect();
        let p = Payload { n: "Cobblemon".into(), m: "1.21.1".into(), l: "fabric-loader".into(), v: "0.16.9".into(), f: ids.join(","), s: None };
        let code = encode(&p).unwrap();
        assert!(code.starts_with(CODE_PREFIX));
        assert!(code.len() < 1400, "{}", code.len());
        assert_eq!(decode(&format!("  Look: {}\n", code)).unwrap(), p);
        assert_eq!(parse_entries(&p.f).unwrap().len(), 120);
    }

    #[test]
    fn invites_carry_the_server_and_old_codes_still_work() {
        let invite = Payload { n: "Vanilla".into(), m: "1.21.4".into(), l: String::new(), v: String::new(), f: String::new(), s: Some("mc.example.net".into()) };
        let back = decode(&encode(&invite).unwrap()).unwrap();
        assert_eq!(back.s.as_deref(), Some("mc.example.net"));
        assert!(parse_entries(&back.f).unwrap().is_empty());
        let old = Payload { s: None, ..invite.clone() };
        let code = encode(&old).unwrap();
        assert!(decode(&code).unwrap().s.is_none());
        let text = {
            let compact = code.trim_start_matches(CODE_PREFIX);
            let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(compact).unwrap();
            let mut t = String::new();
            flate2::read::DeflateDecoder::new(raw.as_slice()).read_to_string(&mut t).unwrap();
            t
        };
        assert!(!text.contains("\"s\""));
    }

    #[tokio::test]
    #[ignore]
    async fn vanilla_invite_imports_for_real() {
        let client = crate::net::client();
        let invite = Payload { n: "Invite test".into(), m: "1.21.4".into(), l: String::new(), v: String::new(), f: String::new(), s: Some("mc.example.net".into()) };
        let imported = import_code(&client, &encode(&invite).unwrap(), &|_, _, _| {}).await.unwrap();
        assert_eq!(imported.server.as_deref(), Some("mc.example.net"));
        assert_eq!(imported.instance.mc_version, "1.21.4");
        eprintln!("istanza {} creata, server {:?}", imported.instance.id, imported.server);
        let _ = crate::instance::delete(&imported.instance.id);
    }

    #[test]
    fn broken_codes_are_rejected() {
        assert!(decode("hello").is_err());
        assert!(decode("PIUMA1-!!!").is_err());
        assert!(decode(&format!("PIUMA1-{}", "A".repeat(MAX_CODE))).is_err());
        assert!(parse_entries("xAAAA").is_err());
        assert!(parse_entries("m../../x").is_err());
        assert!(parse_entries("mAbC123,rXyZ").is_ok());
    }

    #[test]
    fn links_are_parsed() {
        assert_eq!(parse_link("https://modrinth.com/modpack/fabulously-optimized"), Some(("fabulously-optimized".into(), None)));
        assert_eq!(parse_link(" modrinth.com/modpack/cobblemon/versions?x=1 "), Some(("cobblemon".into(), None)));
        assert_eq!(
            parse_link("https://www.modrinth.com/modpack/cobblemon/version/1.5.2#files"),
            Some(("cobblemon".into(), Some("1.5.2".into())))
        );
        assert!(parse_link("https://evil.com/modpack/x").is_none());
        assert!(parse_link("https://modrinth.com/mod/x").is_none());
        assert!(parse_link("https://modrinth.com/modpack").is_none());
    }

    #[test]
    fn export_names_are_safe() {
        assert_eq!(safe_name("My/Pack: 1"), "My_Pack_ 1");
        assert_eq!(safe_name("..."), "modpack");
    }

    #[tokio::test]
    #[ignore]
    async fn export_share_and_import_for_real() {
        let client = crate::net::client();
        let quiet = |_: &str, _: u32, _: u32| {};
        let inst = crate::instance::create("Share Test", "1.20.1", Loader::Fabric, Some("0.16.9".into()), None).unwrap();
        let mods = paths::game_dir(&inst.id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        let v = resolve(&client, &["OihdIimA".to_string()]).await.unwrap();
        let f = v["OihdIimA"].files.iter().find(|f| f.primary).unwrap();
        let jar = client.get(&f.url).send().await.unwrap().bytes().await.unwrap();
        std::fs::write(mods.join(&f.filename), &jar).unwrap();
        std::fs::write(mods.join("homemade.jar"), b"PK\x05\x06not on modrinth").unwrap();
        std::fs::write(mods.join("off.jar.disabled"), b"x").unwrap();
        std::fs::create_dir_all(paths::game_dir(&inst.id).join("config/sub")).unwrap();
        std::fs::write(paths::game_dir(&inst.id).join("config/sub/a.toml"), "x = 1").unwrap();

        let out = std::env::temp_dir().join(format!("piuma-share-{}", std::process::id()));
        std::fs::create_dir_all(&out).unwrap();
        let opts = ExportOptions { resourcepacks: true, shaderpacks: true, config: true, options: false, servers: false };
        let e = export_to(&client, &inst, opts, &out, &quiet).await.unwrap();
        eprintln!("exported {} linked={} bundled={}", e.path, e.linked, e.bundled);
        assert_eq!((e.linked, e.bundled), (1, 1));
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&e.path).unwrap()).unwrap();
        let names: Vec<String> = zip.file_names().filter_map(|n| n.ok().map(|n| n.into_owned())).collect();
        eprintln!("entries {names:?}");
        assert!(names.contains(&"overrides/mods/homemade.jar".to_string()));
        assert!(names.contains(&"overrides/config/sub/a.toml".to_string()));
        let mut idx = String::new();
        zip.by_name("modrinth.index.json").unwrap().read_to_string(&mut idx).unwrap();
        let idx: Value = serde_json::from_str(&idx).unwrap();
        assert_eq!(idx["dependencies"]["fabric-loader"], "0.16.9");
        assert!(idx["files"][0]["downloads"][0].as_str().unwrap().starts_with("https://cdn.modrinth.com/"));

        let code = share_code(&client, &inst, None, &quiet).await.unwrap();
        eprintln!("code {} included={} excluded={:?}", code.code, code.included, code.excluded);
        assert_eq!(code.excluded, vec!["homemade.jar".to_string()]);

        let a = import_file(&client, Path::new(&e.path), &quiet).await.unwrap();
        let b = import_code(&client, &code.code, &quiet).await.unwrap();
        for imported in [&a, &b] {
            let m: Vec<String> = modrinth::list_content(&imported.instance.id, Content::Mods).unwrap().into_iter().map(|f| f.file_name).collect();
            eprintln!("{} -> {:?} loader={:?}", imported.instance.name, m, imported.instance.loader_version);
            assert!(m.contains(&f.filename));
        }
        assert!(modrinth::list_content(&a.instance.id, Content::Mods).unwrap().iter().any(|f| f.file_name == "homemade.jar"));
        assert!(paths::game_dir(&a.instance.id).join("config/sub/a.toml").is_file());
        for id in [&inst.id, &a.instance.id, &b.instance.id] {
            let _ = crate::instance::delete(id);
        }
        let _ = std::fs::remove_dir_all(&out);
    }

    #[tokio::test]
    #[ignore]
    async fn real_code_import_resolves_versions() {
        let client = crate::net::client();
        let v = resolve(&client, &["OihdIimA".to_string()]).await.unwrap();
        let f = v["OihdIimA"].files.iter().find(|f| f.primary).unwrap();
        eprintln!("{} {} {:?} {:?}", f.filename, f.url, f.hashes.sha1, f.size);
        assert!(f.hashes.sha512.is_some());
    }
}
