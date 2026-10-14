use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::{Error, Result};
use crate::instance::{self, Instance};
use crate::modrinth::{self, Content};
use crate::{library, optimizer, paths, settings};

const TAIL: usize = 400;
const EXCERPT: usize = 10;
const READ_CAP: u64 = 512 * 1024;
const BASE_MODS: &[&str] = &["minecraft", "java", "fabricloader", "fabric-loader", "quilt_loader", "forge", "neoforge", "fml", "mixinextras"];

#[derive(Clone, Default)]
pub struct Tail(Arc<Mutex<VecDeque<String>>>);

impl Tail {
    pub fn push(&self, line: &str) {
        let mut q = self.0.lock().unwrap();
        if q.len() == TAIL {
            q.pop_front();
        }
        q.push_back(line.to_string());
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap()).into()
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Fix {
    DisableMods { files: Vec<String> },
    InstallMods { projects: Vec<String> },
    Memory { mb: u32 },
    AutoJava,
    ClearJvmArgs,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    instance_id: String,
    cause: &'static str,
    subject: Option<String>,
    fix: Option<Fix>,
    excerpt: Vec<String>,
    has_report: bool,
    code: Option<i32>,
    #[serde(skip)]
    file: Option<PathBuf>,
}

static LAST: LazyLock<Mutex<HashMap<String, Report>>> = LazyLock::new(Default::default);

pub fn forget(id: &str) {
    LAST.lock().unwrap().remove(id);
}

struct Jar {
    file: String,
    ids: Vec<String>,
    mixins: Vec<String>,
    modified: SystemTime,
}

struct Mods {
    dir: PathBuf,
    jars: Option<Vec<Jar>>,
}

impl Mods {
    fn jars(&mut self) -> &[Jar] {
        let dir = &self.dir;
        self.jars.get_or_insert_with(|| scan_mods(dir))
    }

    fn files_for(&mut self, keys: &[String]) -> Vec<String> {
        let mut out: Vec<String> = vec![];
        for jar in self.jars() {
            let hit = keys.iter().any(|k| jar.ids.iter().any(|id| id.eq_ignore_ascii_case(k)) || jar.mixins.iter().any(|m| m == k));
            if hit && !out.contains(&jar.file) {
                out.push(jar.file.clone());
            }
        }
        out
    }
}

fn read_entry<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Option<String> {
    let mut s = String::new();
    zip.by_name(name).ok()?.take(READ_CAP).read_to_string(&mut s).ok()?;
    Some(s)
}

fn quoted(s: &str) -> Option<&str> {
    let start = s.find(['"', '\''])?;
    let q = s[start..].chars().next()?;
    let rest = &s[start + 1..];
    Some(&rest[..rest.find(q)?])
}

fn scan_jar(path: &Path) -> Option<(Vec<String>, Vec<String>)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mixins = zip
        .file_names()
        .filter_map(|n| n.ok().filter(|n| !n.contains('/') && n.ends_with(".json") && n.contains("mixins")).map(|n| n.into_owned()))
        .collect();
    let mut ids = vec![];
    let json = |s: String| serde_json::from_str::<serde_json::Value>(&s).ok();
    if let Some(v) = read_entry(&mut zip, "fabric.mod.json").and_then(json) {
        ids.extend(v["id"].as_str().map(String::from));
    }
    if let Some(v) = read_entry(&mut zip, "quilt.mod.json").and_then(json) {
        ids.extend(v["quilt_loader"]["id"].as_str().map(String::from));
    }
    for name in ["META-INF/mods.toml", "META-INF/neoforge.mods.toml"] {
        if let Some(s) = read_entry(&mut zip, name) {
            ids.extend(s.lines().filter_map(|l| {
                let rest = l.trim_start().strip_prefix("modId")?.trim_start().strip_prefix('=')?;
                quoted(rest).filter(|id| !id.is_empty()).map(String::from)
            }));
        }
    }
    Some((ids, mixins))
}

fn scan_mods(dir: &Path) -> Vec<Jar> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().into_owned();
            if !file.ends_with(".jar") {
                return None;
            }
            let modified = e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
            let (ids, mixins) = scan_jar(&e.path())?;
            Some(Jar { file, ids, mixins, modified })
        })
        .collect()
}

struct Ctx {
    inst: Instance,
    effective_max: u32,
    ceiling: u32,
    auto: bool,
    mods: Mods,
}

#[derive(Debug, PartialEq)]
struct Found {
    cause: &'static str,
    subject: Option<String>,
    fix: Option<Fix>,
    lines: Vec<String>,
}

impl Found {
    fn new(cause: &'static str, subject: Option<String>, fix: Option<Fix>, lines: Vec<String>) -> Self {
        Found { cause, subject, fix, lines }
    }
}

fn round_down(mb: u32) -> u32 {
    mb / 512 * 512
}

fn after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.find(marker).map(|i| &line[i + marker.len()..])
}

fn token(s: &str) -> Option<String> {
    let t: String = s
        .trim_start_matches(['\'', '"', '`', '(', '[', ' '])
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        .collect();
    let t = t.trim_end_matches('.');
    (!t.is_empty()).then(|| t.to_string())
}

fn paren_id(s: &str) -> Option<String> {
    let open = s.find('(')?;
    let close = s[open..].find(')')? + open;
    token(&s[open + 1..close])
}

fn paren_ids(s: &str) -> Vec<String> {
    s.split('(').skip(1).filter_map(|p| p.split_once(')').and_then(|(id, _)| token(id))).collect()
}

fn fabric_mod(line: &str) -> Option<String> {
    after(line, "Mod '").and_then(paren_id)
}

fn missing_dep(line: &str) -> Option<String> {
    if let Some(rest) = after(line, "requires {") {
        return token(rest);
    }
    let head = &line[..line.find(", which is missing")?];
    let last = head.rsplit(' ').next()?;
    if last.ends_with(')') {
        paren_id(&head[head.rfind('(')?..])
    } else {
        token(last)
    }
}

fn slug(id: &str) -> String {
    match id {
        "fabric" => "fabric-api".into(),
        "qsl" | "quilted_fabric_api" => "qsl".into(),
        other => other.replace('_', "-"),
    }
}

fn push_unique(v: &mut Vec<String>, s: String) {
    if !v.contains(&s) {
        v.push(s);
    }
}

fn class_version(text: &[String]) -> Option<(u32, String)> {
    text.iter().find_map(|l| {
        let rest = after(l, "class file version ")?;
        let n: u32 = rest.split(['.', ')', ' ']).next()?.parse().ok()?;
        Some((n.checked_sub(44)?, l.clone()))
    })
}

fn has(text: &[String], needles: &[&str]) -> Vec<String> {
    text.iter().filter(|l| needles.iter().any(|n| l.contains(n))).take(EXCERPT).cloned().collect()
}

fn java_fix(ctx: &Ctx) -> Option<Fix> {
    ctx.inst.java_path.as_deref().is_some_and(|p| !p.trim().is_empty()).then_some(Fix::AutoJava)
}

fn jvm_args(text: &[String], ctx: &Ctx) -> Option<Found> {
    let lines = has(text, &["Unrecognized VM option", "Unrecognized option:"]);
    if lines.is_empty() {
        return None;
    }
    let option = lines.iter().find_map(|l| {
        after(l, "Unrecognized VM option '")
            .and_then(|r| r.split('\'').next())
            .or_else(|| after(l, "Unrecognized option: ").map(str::trim))
            .map(String::from)
    });
    let custom = option.as_deref().is_some_and(|o| !o.is_empty() && ctx.inst.extra_jvm_args.contains(o.trim_start_matches(['+', '-'])));
    let fix = if custom { Some(Fix::ClearJvmArgs) } else { java_fix(ctx) };
    Some(Found::new("jvmArgs", option, fix, lines))
}

fn memory(text: &[String], ctx: &Ctx) -> Option<Found> {
    let mismatch = has(text, &["Initial heap size set to a larger value"]);
    if !mismatch.is_empty() {
        let mb = ctx.inst.memory_max_mb.max(ctx.inst.memory_min_mb);
        return Some(Found::new("heapTooLarge", None, Some(Fix::Memory { mb }), mismatch));
    }
    let big = has(text, &["Could not reserve enough space", "Invalid maximum heap size", "insufficient memory for the Java Runtime", "Cannot allocate memory"]);
    if !big.is_empty() {
        let mb = round_down(ctx.effective_max / 2).max(1024);
        let fix = (!ctx.auto && mb < ctx.effective_max).then_some(Fix::Memory { mb });
        return Some(Found::new("heapTooLarge", None, fix, big));
    }
    let oom = has(text, &["java.lang.OutOfMemoryError"]);
    if oom.is_empty() {
        return None;
    }
    let mb = round_down(ctx.effective_max + (ctx.effective_max / 2).max(1024)).min(round_down(ctx.ceiling));
    let fix = (mb > ctx.effective_max).then_some(Fix::Memory { mb });
    Some(Found::new("outOfMemory", fix.as_ref().map(|_| format!("{mb}")), fix, oom))
}

fn java_version(text: &[String], ctx: &Ctx) -> Option<Found> {
    let (major, line) = class_version(text).filter(|_| text.iter().any(|l| l.contains("UnsupportedClassVersionError")))?;
    Some(Found::new("javaVersion", Some(major.to_string()), java_fix(ctx), vec![line]))
}

fn dependencies(text: &[String], ctx: &mut Ctx) -> Option<Found> {
    let mut missing: Vec<String> = vec![];
    let mut broken: Vec<String> = vec![];
    let mut lines: Vec<String> = vec![];
    for l in text {
        if l.contains(", which is missing") || l.contains("requires {") {
            let Some(dep) = missing_dep(l) else { continue };
            match dep.as_str() {
                "java" => {
                    let major = after(l, "version ").and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()?.parse::<u32>().ok());
                    return Some(Found::new("javaVersion", major.map(|m| m.to_string()), java_fix(ctx), vec![l.clone()]));
                }
                d if BASE_MODS.contains(&d) => broken.extend(fabric_mod(l)),
                _ => push_unique(&mut missing, dep),
            }
            lines.push(l.clone());
        } else if l.contains("but only the wrong version is present") || l.contains("is incompatible with") {
            if let Some(m) = fabric_mod(l) {
                push_unique(&mut broken, m);
                lines.push(l.clone());
            }
        } else if let Some(rest) = after(l, "Mod ID: '") {
            let Some(id) = rest.split('\'').next().map(String::from) else { continue };
            if l.contains("[MISSING]") {
                push_unique(&mut missing, id);
            } else if let Some(by) = after(l, "Requested by: '").and_then(|r| r.split('\'').next()) {
                push_unique(&mut broken, by.to_string());
            }
            lines.push(l.clone());
        }
    }
    lines.truncate(EXCERPT);
    if !missing.is_empty() {
        let projects = missing.iter().map(|m| slug(m)).collect();
        return Some(Found::new("missingDeps", Some(missing.join(", ")), Some(Fix::InstallMods { projects }), lines));
    }
    if broken.is_empty() {
        return None;
    }
    let files = ctx.mods.files_for(&broken);
    let fix = (!files.is_empty()).then_some(Fix::DisableMods { files });
    Some(Found::new("incompatibleMods", Some(broken.join(", ")), fix, lines))
}

fn duplicates(text: &[String], ctx: &mut Ctx) -> Option<Found> {
    let lines = has(text, &["Duplicate mods found", "Found duplicate mods", "present multiple times", "DuplicateModsFoundException", "duplicate mods"]);
    if lines.is_empty() {
        return None;
    }
    let mut by_id: HashMap<&str, Vec<&Jar>> = HashMap::new();
    for jar in ctx.mods.jars() {
        for id in &jar.ids {
            by_id.entry(id.as_str()).or_default().push(jar);
        }
    }
    let mut ids: Vec<String> = vec![];
    let mut files: Vec<String> = vec![];
    for (id, mut jars) in by_id.into_iter().filter(|(_, j)| j.len() > 1) {
        jars.sort_by_key(|j| std::cmp::Reverse(j.modified));
        ids.push(id.to_string());
        for j in &jars[1..] {
            push_unique(&mut files, j.file.clone());
        }
    }
    ids.sort();
    let fix = (!files.is_empty()).then_some(Fix::DisableMods { files });
    Some(Found::new("duplicateMods", (!ids.is_empty()).then(|| ids.join(", ")), fix, lines))
}

fn mixin(text: &[String], ctx: &mut Ctx) -> Option<Found> {
    let mut keys: Vec<String> = vec![];
    let mut lines = vec![];
    for l in text.iter().filter(|l| l.contains("Mixin") || l.contains("mixin")) {
        let key = after(l, "from mod ")
            .or_else(|| after(l, "for mod "))
            .and_then(token)
            .or_else(|| {
                let rest = after(l, "[")?;
                let name = rest.split([':', ']']).next()?;
                name.ends_with(".json").then(|| name.to_string())
            });
        let failed = ["MixinApplyError", "MixinTransformerError", "InvalidInjectionException", "InvalidMixinException", "failed injection check", "FAILED during"]
            .iter()
            .any(|n| l.contains(n));
        if let Some(k) = key.filter(|k| failed && !BASE_MODS.contains(&k.as_str())) {
            push_unique(&mut keys, k);
            lines.push(l.clone());
        }
    }
    if keys.is_empty() {
        return None;
    }
    lines.truncate(EXCERPT);
    let files = ctx.mods.files_for(&keys);
    let subject = keys.iter().map(|k| k.trim_end_matches(".json").trim_end_matches(".mixins")).collect::<Vec<_>>().join(", ");
    let fix = (!files.is_empty()).then_some(Fix::DisableMods { files });
    Some(Found::new("mixin", Some(subject), fix, lines))
}

const DRIVERS: &[&str] = &["nvoglv", "atio6axx", "atioglxx", "ig9icd", "ig7icd", "ig75icd", "ig8icd", "libnvidia", "radeonsi", "iris_dri", "amdgpu_dri", "libGL", "opengl32", "amdxc"];

fn graphics(text: &[String], native: Option<&str>) -> Option<Found> {
    let mut lines = has(
        text,
        &["Pixel format not accelerated", "GLFW error 65542", "GLFW error 65543", "does not appear to support OpenGL", "No OpenGL context", "GLX: Failed to create context", "Failed to create window"],
    );
    if let Some(frame) = native.and_then(problematic_frame).filter(|f| DRIVERS.iter().any(|d| f.contains(d))) {
        lines.push(frame);
    }
    (!lines.is_empty()).then(|| Found::new("graphics", None, None, lines))
}

fn problematic_frame(log: &str) -> Option<String> {
    let mut it = log.lines();
    it.find(|l| l.contains("Problematic frame"))?;
    it.next().map(|l| l.trim_start_matches(['#', ' ']).trim().to_string())
}

fn suspected(text: &[String], ctx: &mut Ctx) -> Option<Found> {
    let mut ids: Vec<String> = vec![];
    let mut lines = vec![];
    let mut it = text.iter().peekable();
    while let Some(l) = it.next() {
        let Some(rest) = after(l, "Suspected Mod") else { continue };
        let rest = rest.trim_start_matches(['s', '(', ')']).strip_prefix(':').unwrap_or_default();
        lines.push(l.clone());
        for id in paren_ids(rest) {
            push_unique(&mut ids, id);
        }
        while let Some(next) = it.peek().filter(|n| n.starts_with('\t') && !n.trim_start().starts_with("at ") && !n.trim_start().starts_with("Issue")) {
            if let Some(id) = paren_id(next) {
                lines.push(next.to_string());
                push_unique(&mut ids, id);
            }
            it.next();
        }
    }
    if ids.is_empty() {
        let mut counts: Vec<(String, usize)> = vec![];
        for l in text.iter().filter(|l| l.trim_start().starts_with("at ")) {
            let Some(rest) = after(l, "handler$") else { continue };
            let Some(id) = rest.split('$').nth(1).filter(|s| !s.is_empty()) else { continue };
            match counts.iter_mut().find(|(k, _)| k == id) {
                Some((_, n)) => *n += 1,
                None => {
                    counts.push((id.to_string(), 1));
                    lines.push(l.clone());
                }
            }
        }
        counts.sort_by_key(|c| std::cmp::Reverse(c.1));
        ids.extend(counts.into_iter().take(1).map(|(k, _)| k));
    }
    ids.retain(|i| !BASE_MODS.contains(&i.as_str()));
    if ids.is_empty() {
        return None;
    }
    lines.truncate(EXCERPT);
    let files = ctx.mods.files_for(&ids);
    let fix = (!files.is_empty()).then_some(Fix::DisableMods { files });
    Some(Found::new("suspectedMod", Some(ids.join(", ")), fix, lines))
}

fn fallback(text: &[String], native: Option<&str>) -> Found {
    if let Some(frame) = native.and_then(problematic_frame) {
        return Found::new("nativeCrash", Some(frame.clone()), None, vec![frame]);
    }
    let description = text.iter().find_map(|l| l.strip_prefix("Description: ")).map(String::from);
    let error = text.iter().position(|l| {
        let t = l.trim_start();
        !t.starts_with("at ") && (t.contains("Exception") || t.contains("Error:") || t.starts_with("Caused by"))
    });
    let subject = description.or_else(|| error.map(|i| text[i].trim().chars().take(200).collect()));
    let lines = match error {
        Some(i) => text[i..].iter().take(EXCERPT).cloned().collect(),
        None => {
            let mut last: Vec<String> = text.iter().rev().filter(|l| !l.trim().is_empty()).take(EXCERPT).cloned().collect();
            last.reverse();
            last
        }
    };
    Found::new("unknown", subject, None, lines)
}

fn diagnose(text: &[String], native: Option<&str>, ctx: &mut Ctx) -> Found {
    jvm_args(text, ctx)
        .or_else(|| memory(text, ctx))
        .or_else(|| java_version(text, ctx))
        .or_else(|| dependencies(text, ctx))
        .or_else(|| duplicates(text, ctx))
        .or_else(|| mixin(text, ctx))
        .or_else(|| graphics(text, native))
        .or_else(|| suspected(text, ctx))
        .unwrap_or_else(|| fallback(text, native))
}

fn newest(dir: &Path, since: SystemTime, keep: impl Fn(&str) -> bool) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| keep(&e.file_name().to_string_lossy()))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .filter(|(t, _)| *t >= since)
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

fn read_capped(path: &Path) -> Option<String> {
    let mut s = String::new();
    std::fs::File::open(path).ok()?.take(READ_CAP).read_to_string(&mut s).ok()?;
    Some(s)
}

fn analyze(inst: Instance, code: Option<i32>, since: SystemTime, tail: Vec<String>) -> Option<Report> {
    let game = paths::game_dir(&inst.id);
    let report_file = newest(&game.join("crash-reports"), since, |n| n.ends_with(".txt"));
    let native_file = newest(&game, since, |n| n.starts_with("hs_err_pid") && n.ends_with(".log"));
    if code == Some(0) && report_file.is_none() && native_file.is_none() {
        return None;
    }
    let mut text = tail;
    if let Some(s) = report_file.as_deref().and_then(read_capped) {
        text.extend(s.lines().map(String::from));
    }
    let native = native_file.as_deref().and_then(read_capped);
    let prefs = settings::load();
    let eff = optimizer::effective(&inst, &prefs);
    let ceiling = (optimizer::hardware().total_mb * 75 / 100).max(1024) as u32;
    let mut ctx = Ctx {
        effective_max: eff.memory_max_mb,
        ceiling,
        auto: prefs.auto_optimize,
        mods: Mods { dir: game.join("mods"), jars: None },
        inst,
    };
    let found = diagnose(&text, native.as_deref(), &mut ctx);
    Some(Report {
        instance_id: ctx.inst.id,
        cause: found.cause,
        subject: found.subject,
        fix: found.fix,
        excerpt: found.lines.into_iter().map(|l| l.chars().take(300).collect()).collect(),
        has_report: report_file.is_some() || native_file.is_some(),
        code,
        file: report_file.or(native_file),
    })
}

pub async fn inspect(app: &AppHandle, inst: Instance, code: Option<i32>, since: SystemTime, tail: Tail) {
    let lines = tail.take();
    let Ok(Some(report)) = tokio::task::spawn_blocking(move || analyze(inst, code, since, lines)).await else { return };
    LAST.lock().unwrap().insert(report.instance_id.clone(), report.clone());
    let _ = app.emit("game-crash", report);
}

fn last(id: &str) -> Result<Report> {
    instance::check_id(id)?;
    LAST.lock().unwrap().get(id).cloned().ok_or_else(|| Error::msg("No crash to look at"))
}

pub async fn apply(client: &reqwest::Client, id: &str, progress: modrinth::Progress<'_>) -> Result<()> {
    let fix = last(id)?.fix.ok_or_else(|| Error::msg("There is no automatic fix for this crash"))?;
    let mut inst = instance::get(id)?;
    match fix {
        Fix::DisableMods { files } => {
            for f in &files {
                modrinth::set_mod_enabled(id, f, false)?;
            }
        }
        Fix::InstallMods { projects } => {
            for p in &projects {
                modrinth::install_content(client, &inst, p, Content::Mods, progress).await?;
            }
        }
        Fix::Memory { mb } => {
            inst.custom_performance = true;
            inst.memory_max_mb = mb;
            inst.memory_min_mb = inst.memory_min_mb.min(mb);
            instance::save(&inst)?;
        }
        Fix::AutoJava => {
            inst.java_path = None;
            instance::save(&inst)?;
        }
        Fix::ClearJvmArgs => {
            inst.extra_jvm_args.clear();
            instance::save(&inst)?;
        }
    }
    forget(id);
    Ok(())
}

pub fn open_report(id: &str) -> Result<()> {
    match last(id)?.file {
        Some(f) => library::open_path(&f),
        None => library::open_folder(id, "game"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::FlagProfile;
    use crate::loaders::Loader;

    fn ctx(java: Option<&str>, args: &str) -> Ctx {
        Ctx {
            inst: Instance {
                id: "crash-test".into(),
                name: "t".into(),
                mc_version: "1.21.1".into(),
                loader: Loader::Fabric,
                loader_version: None,
                launch_version: None,
                custom_performance: true,
                memory_max_mb: 4096,
                memory_min_mb: 1024,
                flag_profile: FlagProfile::Balanced,
                java_path: java.map(String::from),
                extra_jvm_args: args.into(),
                icon_url: None,
                banner_url: None,
            },
            effective_max: 4096,
            ceiling: 12288,
            auto: false,
            mods: Mods { dir: PathBuf::from("/nonexistent"), jars: Some(vec![]) },
        }
    }

    fn run(log: &str, c: &mut Ctx) -> Found {
        let text: Vec<String> = log.lines().map(String::from).collect();
        diagnose(&text, None, c)
    }

    #[test]
    fn fabric_missing_dependency() {
        let f = run(
            "Incompatible mods found!\n\t - Mod 'Iris' (iris) 1.8.0 requires any version of fabric-api, which is missing!\n\t - Mod 'Sodium Extra' (sodium-extra) 0.6 requires version 0.6 or later of mod 'Sodium' (sodium), which is missing!",
            &mut ctx(None, ""),
        );
        assert_eq!(f.cause, "missingDeps");
        assert_eq!(f.fix, Some(Fix::InstallMods { projects: vec!["fabric-api".into(), "sodium".into()] }));
    }

    #[test]
    fn old_fabric_and_forge_dependencies() {
        let f = run(" - Mod sodium requires {fabric @ [*]}, which is missing!", &mut ctx(None, ""));
        assert_eq!(f.fix, Some(Fix::InstallMods { projects: vec!["fabric-api".into()] }));
        let f = run("\tMod ID: 'architectury', Requested by: 'rei', Expected range: '[9,)', Actual version: '[MISSING]'", &mut ctx(None, ""));
        assert_eq!(f.subject.as_deref(), Some("architectury"));
    }

    #[test]
    fn fabric_wrong_minecraft_is_incompatible() {
        let f = run("\t - Mod 'Old Mod' (oldmod) 1.0 requires version 1.20.1 of minecraft, which is missing!", &mut ctx(None, ""));
        assert_eq!((f.cause, f.subject.as_deref()), ("incompatibleMods", Some("oldmod")));
    }

    #[test]
    fn java_too_old() {
        let log = "Exception in thread \"main\" java.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
        let f = run(log, &mut ctx(Some("/usr/bin/java"), ""));
        assert_eq!((f.cause, f.subject.as_deref(), f.fix), ("javaVersion", Some("21"), Some(Fix::AutoJava)));
        assert_eq!(run(log, &mut ctx(None, "")).fix, None);
    }

    #[test]
    fn memory_problems() {
        let f = run("java.lang.OutOfMemoryError: Java heap space", &mut ctx(None, ""));
        assert_eq!(f.fix, Some(Fix::Memory { mb: 6144 }));
        let f = run("Error occurred during initialization of VM\nCould not reserve enough space for 8388608KB object heap", &mut ctx(None, ""));
        assert_eq!((f.cause, f.fix), ("heapTooLarge", Some(Fix::Memory { mb: 2048 })));
    }

    #[test]
    fn bad_custom_jvm_option() {
        let f = run("Unrecognized VM option 'UseFooGC'\nError: Could not create the Java Virtual Machine.", &mut ctx(None, "-XX:+UseFooGC"));
        assert_eq!((f.cause, f.subject.as_deref(), f.fix), ("jvmArgs", Some("UseFooGC"), Some(Fix::ClearJvmArgs)));
    }

    #[test]
    fn mixin_failure_names_the_mod() {
        let f = run(
            "org.spongepowered.asm.mixin.transformer.throwables.MixinTransformerError: An unexpected critical error was encountered\nCaused by: org.spongepowered.asm.mixin.injection.throwables.InvalidInjectionException: Critical injection failure: Redirector foo in badmod.mixins.json:BarMixin from mod badmod failed injection check",
            &mut ctx(None, ""),
        );
        assert_eq!((f.cause, f.subject.as_deref()), ("mixin", Some("badmod")));
    }

    #[test]
    fn suspected_mods_from_report_and_stack() {
        let f = run("Suspected Mods: Sodium (sodium), Iris (iris)\n\tat foo.Bar.baz(Bar.java:1)", &mut ctx(None, ""));
        assert_eq!((f.cause, f.subject.as_deref()), ("suspectedMod", Some("sodium, iris")));
        let f = run("Suspected Mod: \n\tCool Mod (coolmod), Version: 1.0\n\t\tIssue tracker URL: x", &mut ctx(None, ""));
        assert_eq!(f.subject.as_deref(), Some("coolmod"));
        let f = run("java.lang.NullPointerException\n\tat net.minecraft.class_1.handler$zab000$fancymod$onTick(class_1.java:9)", &mut ctx(None, ""));
        assert_eq!(f.subject.as_deref(), Some("fancymod"));
    }

    #[test]
    fn graphics_and_unknown() {
        let f = run("[LWJGL] GLFW error 65542: WGL: The driver does not appear to support OpenGL", &mut ctx(None, ""));
        assert_eq!(f.cause, "graphics");
        let native = "# Problematic frame:\n# C  [nvoglv64.dll+0x1234]\n";
        let f = diagnose(&[], Some(native), &mut ctx(None, ""));
        assert_eq!(f.cause, "graphics");
        let f = run("Description: Ticking entity\njava.lang.IllegalStateException: boom\n\tat a.b(c)", &mut ctx(None, ""));
        assert_eq!((f.cause, f.subject.as_deref()), ("unknown", Some("Ticking entity")));
    }

    #[test]
    fn jars_expose_ids_and_mixin_configs() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("piuma-crash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, entries: &[(&str, &str)]| {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join(name)).unwrap());
            for (path, body) in entries {
                zip.start_file(*path, zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(body.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        };
        write("fab.jar", &[("fabric.mod.json", r#"{"id": "fab"}"#), ("fab.mixins.json", "{}")]);
        write("forge.jar", &[("META-INF/mods.toml", "[[mods]]\nmodId = \"forgy\" # main\n")]);
        write("notes.jar.disabled", &[("fabric.mod.json", r#"{"id": "off"}"#)]);
        let mut mods = Mods { dir: dir.clone(), jars: None };
        assert_eq!(mods.files_for(&["forgy".into()]), vec!["forge.jar".to_string()]);
        assert_eq!(mods.files_for(&["fab.mixins.json".into()]), vec!["fab.jar".to_string()]);
        assert!(mods.files_for(&["off".into()]).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicates_keep_the_newest() {
        let mut c = ctx(None, "");
        let t = |s| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(s);
        c.mods.jars = Some(vec![
            Jar { file: "sodium-0.5.jar".into(), ids: vec!["sodium".into()], mixins: vec![], modified: t(1) },
            Jar { file: "sodium-0.6.jar".into(), ids: vec!["sodium".into()], mixins: vec![], modified: t(2) },
        ]);
        let f = run("Duplicate mods found!", &mut c);
        assert_eq!(f.fix, Some(Fix::DisableMods { files: vec!["sodium-0.5.jar".into()] }));
    }
}
