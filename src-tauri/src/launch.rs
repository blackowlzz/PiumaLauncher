use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;

use crate::accounts::{self, Account, AccountKind};
use crate::error::{Error, Result};
use crate::{console, crash, flags};
use crate::install::{self, Prepared};
use crate::instance::Instance;
use crate::java::{self, JavaInstall};
use crate::loaders::Loader;
use crate::microsoft::{self, Session};
use crate::minecraft::{rules_allow, Arg, OneOrMany};
use crate::servers::{self, QuickPlay};
use crate::{net, paths, presence, runtime, settings, stats, Running};

const SHIM_CLASS: &[u8] = include_bytes!("../shim/PiumaShim.class");
const TOKEN_PLACEHOLDER: &str = "@@PIUMA_TOKEN@@";

fn ensure_shim() -> Result<()> {
    let dir = paths::shim_dir();
    let file = dir.join("PiumaShim.class");
    if std::fs::read(&file).ok().as_deref() != Some(SHIM_CLASS) {
        std::fs::create_dir_all(&dir)?;
        std::fs::write(&file, SHIM_CLASS)?;
    }
    Ok(())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StateEvent<'a> {
    instance_id: &'a str,
    state: &'a str,
    stage: Option<&'a str>,
    done: u32,
    total: u32,
    message: Option<String>,
    code: Option<i32>,
}

fn emit_state(app: &AppHandle, id: &str, state: &str, stage: Option<&str>, done: u32, total: u32, message: Option<String>, code: Option<i32>) {
    let _ = app.emit("game-state", StateEvent { instance_id: id, state, stage, done, total, message, code });
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StatsEvent<'a> {
    instance_id: &'a str,
    rss_mb: u64,
}

fn sample_memory(app: AppHandle, id: String, pid: u32, done: Arc<AtomicBool>) {
    tauri::async_runtime::spawn(async move {
        let pid = Pid::from_u32(pid);
        let mut sys = System::new();
        while !done.load(Ordering::Relaxed) {
            sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing().with_memory());
            let Some(p) = sys.process(pid) else { break };
            let _ = app.emit("game-stats", StatsEvent { instance_id: &id, rss_mb: p.memory() / 1_048_576 });
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}

fn expand(s: &str, vars: &HashMap<&'static str, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        match tail.find('}').and_then(|end| vars.get(&tail[..end]).map(|v| (end, v))) {
            Some((end, v)) => {
                out.push_str(v);
                rest = &tail[end + 1..];
            }
            None => {
                out.push_str("${");
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

fn collect(args: &[Arg], vars: &HashMap<&'static str, String>) -> Vec<String> {
    let mut out = vec![];
    for a in args {
        match a {
            Arg::Plain(s) => out.push(expand(s, vars)),
            Arg::Ruled { rules, value } if rules_allow(rules) => match value {
                OneOrMany::One(s) => out.push(expand(s, vars)),
                OneOrMany::Many(v) => out.extend(v.iter().map(|s| expand(s, vars))),
            },
            Arg::Ruled { .. } => {}
        }
    }
    out
}

fn has_game_flag(args: &[Arg], flag: &str) -> bool {
    args.iter().any(|a| match a {
        Arg::Plain(s) => s == flag,
        Arg::Ruled { value: OneOrMany::One(s), .. } => s == flag,
        Arg::Ruled { value: OneOrMany::Many(v), .. } => v.iter().any(|s| s == flag),
    })
}

fn quick_play_args(prep: &Prepared, quick: &QuickPlay) -> Result<Vec<String>> {
    let modern = prep.resolved.version.arguments.as_ref().is_some_and(|a| has_game_flag(&a.game, "--quickPlayMultiplayer"));
    Ok(match (quick, modern) {
        (QuickPlay::Server { address }, true) => vec!["--quickPlayMultiplayer".into(), address.trim().into()],
        (QuickPlay::Server { address }, false) => {
            let (host, port) = servers::split(address);
            vec!["--server".into(), host, "--port".into(), port.unwrap_or(25565).to_string()]
        }
        (QuickPlay::World { folder }, true) => vec!["--quickPlaySingleplayer".into(), folder.clone()],
        (QuickPlay::World { .. }, false) => return Err(Error::msg("Opening a world directly needs Minecraft 1.20 or newer")),
    })
}

pub fn build_args(prep: &Prepared, inst: &Instance, acc: &Account, session: Option<&Session>, java: &JavaInstall, quick: Option<&QuickPlay>) -> Result<Vec<String>> {
    let v = &prep.resolved.version;
    let sep = if cfg!(windows) { ";" } else { ":" };
    let shim_dir = session.is_some().then(paths::shim_dir);
    let classpath = shim_dir
        .iter()
        .chain(prep.classpath.iter())
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(sep);

    let mut vars: HashMap<&'static str, String> = HashMap::new();
    vars.insert("auth_player_name", acc.name.clone());
    vars.insert("auth_uuid", acc.uuid.clone());
    let (token, user_type) = match session {
        Some(_) => (TOKEN_PLACEHOLDER, "msa"),
        None => ("0", "legacy"),
    };
    vars.insert("auth_access_token", token.into());
    vars.insert("auth_session", token.into());
    vars.insert("user_type", user_type.into());
    vars.insert("user_properties", "{}".into());
    vars.insert("clientid", String::new());
    vars.insert("auth_xuid", String::new());
    vars.insert("version_name", inst.launch_id().to_string());
    vars.insert("version_type", v.kind.clone().unwrap_or_else(|| "release".into()));
    vars.insert("game_directory", paths::game_dir(&inst.id).to_string_lossy().into_owned());
    vars.insert("assets_root", paths::assets().to_string_lossy().into_owned());
    vars.insert("game_assets", prep.game_assets.to_string_lossy().into_owned());
    vars.insert("assets_index_name", prep.assets_index_id.clone());
    vars.insert("natives_directory", prep.natives_dir.to_string_lossy().into_owned());
    vars.insert("library_directory", paths::libraries().to_string_lossy().into_owned());
    vars.insert("classpath_separator", sep.into());
    vars.insert("classpath", classpath);
    vars.insert("launcher_name", "PiumaLauncher".into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());

    let main_class = v.main_class.clone().ok_or_else(|| Error::msg("mainClass missing"))?;

    let (version_jvm, game) = match (&v.arguments, &v.minecraft_arguments) {
        (Some(a), _) => {
            let mut jvm_vars = vars.clone();
            if matches!(inst.loader, Loader::Forge | Loader::NeoForge) {
                jvm_vars.insert("version_name", prep.resolved.jar_id.clone());
            }
            (collect(&a.jvm, &jvm_vars), collect(&a.game, &vars))
        }
        (None, Some(legacy)) => (
            vec![
                expand("-Djava.library.path=${natives_directory}", &vars),
                "-cp".into(),
                vars["classpath"].clone(),
            ],
            legacy.split_whitespace().map(|s| expand(s, &vars)).collect(),
        ),
        _ => return Err(Error::msg("this version has no launch arguments")),
    };

    let eff = crate::optimizer::effective(inst, &crate::settings::load());
    let mut args = vec![format!("-Xms{}M", eff.memory_min_mb), format!("-Xmx{}M", eff.memory_max_mb)];
    args.extend(flags::launch_flags(eff.flag_profile, &eff.tuning, java.major));
    args.extend(inst.extra_jvm_args.split_whitespace().map(String::from));
    args.extend(prep.logging_arg.clone());
    args.extend(version_jvm);
    if session.is_some() {
        args.push("PiumaShim".into());
    }
    args.push(main_class);
    args.extend(game);
    if let Some(q) = quick {
        args.extend(quick_play_args(prep, q)?);
    }
    Ok(args)
}

async fn start(app: &AppHandle, inst: &Instance, acc: &Account, quick: Option<&QuickPlay>) -> Result<(Child, Option<String>)> {
    let client = net::client();
    let progress = |stage: &str, done: u32, total: u32| {
        emit_state(app, &inst.id, "preparing", Some(stage), done, total, None, None)
    };
    emit_state(app, &inst.id, "preparing", Some("Preparing"), 0, 0, None, None);

    let session = match acc.kind {
        AccountKind::Microsoft => {
            progress("Signing in", 0, 0);
            Some(microsoft::session(&acc.id).await?)
        }
        AccountKind::Offline => None,
    };
    let acc = &accounts::get(&acc.id).unwrap_or_else(|| acc.clone());

    let game_dir = paths::game_dir(&inst.id);
    tokio::fs::create_dir_all(&game_dir).await?;
    let prep = install::prepare(&client, inst.launch_id(), &inst.id, game_dir.clone(), &progress).await?;

    progress("Looking for Java", 0, 0);
    let required = prep.resolved.required_java();
    let java = match inst.java_path.as_deref().filter(|p| !p.trim().is_empty()) {
        Some(p) => java::probe(std::path::Path::new(p))
            .await
            .ok_or_else(|| Error::msg(format!("The selected Java doesn't work: {p}")))?,
        None => {
            let chosen = match settings::load().preferred_java {
                Some(id) => runtime::preferred(&id, required).await,
                None => None,
            };
            let managed = match chosen {
                Some(j) => Ok(Some(j)),
                None => runtime::ensure(&client, required, &progress).await,
            };
            match managed {
                Ok(Some(j)) => j,
                other => {
                    let all = java::detect().await;
                    match java::pick(&all, required) {
                        Some(j) => j.clone(),
                        None => {
                            let reason = other.err().map(|e| format!(" ({e})")).unwrap_or_default();
                            return Err(Error::msg(format!(
                                "Java {required} or newer is required and couldn't be obtained{reason}"
                            )));
                        }
                    }
                }
            }
        }
    };
    if java.major < required {
        return Err(Error::msg(format!("This version requires Java {required}, but you have Java {}", java.major)));
    }

    if session.is_some() {
        ensure_shim()?;
    }
    let args = build_args(&prep, inst, acc, session.as_ref(), &java, quick)?;
    let mut cmd = Command::new(&java.path);
    cmd.args(args)
        .current_dir(&game_dir)
        .stdin(if session.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let mut child = cmd.spawn()?;
    if let Some(s) = &session {
        let sent = match child.stdin.take() {
            Some(mut stdin) => {
                use tokio::io::AsyncWriteExt;
                let line = format!("{}\n", s.token);
                stdin.write_all(line.as_bytes()).await.is_ok()
            }
            None => false,
        };
        if !sent {
            let _ = child.kill().await;
            return Err(Error::msg("Couldn't hand the Microsoft session to the game"));
        }
    }
    Ok((child, session.map(|s| s.token)))
}

fn forward_lines<R: AsyncRead + Unpin + Send + 'static>(app: AppHandle, id: String, reader: R, secret: Option<String>, console: bool, tail: crash::Tail) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            presence::observe(&id, &line);
            let line = match &secret {
                Some(s) if line.contains(s.as_str()) => line.replace(s.as_str(), "***"),
                _ => line,
            };
            tail.push(&line);
            if console {
                console::push(&app, &id, &line);
            }
        }
    })
}

pub async fn run(app: AppHandle, inst: Instance, acc: Account, quick: Option<QuickPlay>, mut kill_rx: oneshot::Receiver<()>) {
    let id = inst.id.clone();
    crash::forget(&id);
    match start(&app, &inst, &acc, quick.as_ref()).await {
        Err(e) => emit_state(&app, &id, "error", None, 0, 0, Some(e.to_string()), None),
        Ok((mut child, secret)) => {
            emit_state(&app, &id, "running", None, 0, 0, None, None);
            let prefs = settings::load();
            let console = prefs.live_console;
            if console {
                console::start(&app, &inst);
            }
            presence::start(&id, &acc.name, &inst.mc_version);
            let minimized = prefs.minimize_on_launch;
            if minimized {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
            let started = std::time::Instant::now();
            let started_ms = stats::now_ms();
            let since = std::time::SystemTime::now() - std::time::Duration::from_secs(2);
            let tail = crash::Tail::default();
            let mut readers = vec![];
            let done = Arc::new(AtomicBool::new(false));
            if let Some(pid) = child.id() {
                sample_memory(app.clone(), id.clone(), pid, done.clone());
            }
            if let Some(out) = child.stdout.take() {
                readers.push(forward_lines(app.clone(), id.clone(), out, secret.clone(), console, tail.clone()));
            }
            if let Some(err) = child.stderr.take() {
                readers.push(forward_lines(app.clone(), id.clone(), err, secret.clone(), console, tail.clone()));
            }
            let (code, stopped) = tokio::select! {
                status = child.wait() => (status.ok().and_then(|s| s.code()), false),
                Ok(()) = &mut kill_rx => {
                    let _ = child.kill().await;
                    (None, true)
                }
            };
            done.store(true, Ordering::Relaxed);
            presence::stop(&id);
            if minimized {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
            stats::record(&id, started_ms, started.elapsed().as_secs());
            emit_state(&app, &id, "exited", None, 0, 0, None, code);
            if !stopped {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), futures_util::future::join_all(readers)).await;
                crash::inspect(&app, inst, code, since, tail).await;
            }
        }
    }
    if let Some(running) = app.try_state::<Running>() {
        running.0.lock().unwrap().remove(&id);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::accounts::AccountKind;
    use crate::flags::FlagProfile;
    use crate::loaders::{self, Loader};
    use crate::minecraft;

    pub(crate) async fn run_and_capture(client: &reqwest::Client, inst: &Instance, secs: u64) -> Vec<String> {
        let acc = Account {
            id: "x".into(),
            name: "Tester".into(),
            uuid: accounts::offline_uuid("Tester"),
            kind: AccountKind::Offline,
        };
        let game_dir = paths::game_dir(&inst.id);
        std::fs::create_dir_all(&game_dir).unwrap();

        let last = std::sync::Mutex::new(String::new());
        let progress = |stage: &str, d: u32, t: u32| {
            let line = format!("{stage} {d}/{t}");
            if d == t || d % 500 == 0 {
                let mut l = last.lock().unwrap();
                if *l != line {
                    eprintln!("{line}");
                    *l = line;
                }
            }
        };
        let prep = install::prepare(client, inst.launch_id(), &inst.id, game_dir.clone(), &progress)
            .await
            .unwrap();
        assert!(prep.classpath.iter().all(|p| p.is_file()), "incomplete classpath");

        let required = prep.resolved.required_java();
        let java = runtime::ensure(client, required, &progress).await.unwrap().expect("no managed runtime");
        eprintln!("java: {java:?}");
        let args = build_args(&prep, inst, &acc, None, &java, None).unwrap();

        let mut cmd = Command::new(&java.path);
        cmd.args(args).current_dir(&game_dir).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        let mut child = cmd.spawn().unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(secs);
        let mut out = vec![];
        while let Ok(Ok(Some(line))) = tokio::time::timeout_at(deadline, lines.next_line()).await {
            out.push(line);
        }
        let _ = child.kill().await;
        out
    }

    #[test]
    fn expand_replaces_known_vars_only() {
        let vars = HashMap::from([("a", "1".to_string()), ("b", "${a}".to_string())]);
        assert_eq!(expand("x${a}y${b}z", &vars), "x1y${a}z");
        assert_eq!(expand("${missing}-${a", &vars), "${missing}-${a");
        assert_eq!(expand("${x${a}", &vars), "${x1");
    }

    #[tokio::test]
    #[ignore]
    async fn smoke_launch_latest_release() {
        let client = net::client();
        let manifest = minecraft::manifest(&client).await.unwrap();
        let version = std::env::var("SMOKE_MC")
            .unwrap_or_else(|_| manifest.versions.iter().find(|v| v.kind == "release").unwrap().id.clone());
        eprintln!("versione: {version}");

        let loader = match std::env::var("SMOKE_LOADER").as_deref() {
            Ok("fabric") => Loader::Fabric,
            Ok("quilt") => Loader::Quilt,
            Ok("forge") => Loader::Forge,
            Ok("neoforge") => Loader::NeoForge,
            _ => Loader::Vanilla,
        };
        let (loader_version, launch_version) = match loader {
            Loader::Vanilla => (None, None),
            _ => {
                let i = loaders::install(&client, loader, &version, None, &|_, _, _| {}).await.unwrap();
                eprintln!("loader: {:?} {}", loader, i.loader_version);
                (Some(i.loader_version), Some(i.id))
            }
        };
        let inst = Instance {
            id: "smoke-test".into(),
            name: "smoke".into(),
            mc_version: version,
            loader,
            loader_version,
            launch_version,
            custom_performance: true,
            memory_max_mb: 1024,
            memory_min_mb: 256,
            flag_profile: FlagProfile::Low,
            java_path: None,
            extra_jvm_args: String::new(),
            icon_url: None,
            banner_url: None,
        };
        let log = run_and_capture(&client, &inst, 25).await;
        for l in log.iter().take(400) {
            eprintln!("[mc] {l}");
        }
        let _ = std::fs::remove_dir_all(paths::instance_dir(&inst.id));
        let _ = std::fs::remove_dir_all(paths::natives(&inst.id));
        assert!(!log.is_empty(), "il gioco non ha prodotto output");
    }
}
