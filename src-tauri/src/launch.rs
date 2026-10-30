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
use crate::{aot, backup, console, crash, diag, flags, options_sync, procs, startup};
use crate::install::{self, Prepared};
use crate::instance::Instance;
use crate::java::{self, JavaInstall};
use crate::loaders::Loader;
use crate::microsoft::{self, Session};
use crate::minecraft::{rules_allow, Arg, OneOrMany};
use crate::servers::{self, QuickPlay};
use crate::{net, paths, power, presence, runtime, settings, stats, Running};

const SHIM_CLASS: &[u8] = include_bytes!("../shim/PiumaShim.class");
const TOKEN_PLACEHOLDER: &str = "@@PIUMA_TOKEN@@";

fn shim_jar_bytes() -> Result<Vec<u8>> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default());
    zip.start_file("PiumaShim.class", opts)?;
    zip.write_all(SHIM_CLASS)?;
    Ok(zip.finish()?.into_inner())
}

fn ensure_shim() -> Result<()> {
    let dir = paths::shim_dir();
    let jar = paths::shim_jar();
    let bytes = shim_jar_bytes()?;
    if std::fs::read(&jar).ok() != Some(bytes.clone()) {
        std::fs::create_dir_all(&dir).map_err(Error::at(&dir))?;
        crate::fsx::retry(|| std::fs::write(&jar, &bytes)).map_err(Error::at(&jar))?;
    }
    let _ = std::fs::remove_file(dir.join("PiumaShim.class"));
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
    let shim = session.is_some().then(paths::shim_jar);
    let classpath = shim
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
    args.extend(flags::launch_flags(eff.flag_profile, &eff.tuning, java.major, java.is_graal));
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
    args.extend(inst.effective_launch().game_args());
    Ok(args)
}

fn java_target(required: u32, lwjgl3: bool, loader: Loader) -> u32 {
    if required <= 8 && lwjgl3 && !matches!(loader, Loader::Forge | Loader::NeoForge) { 21 } else { required }
}

fn game_command(launch: &crate::instance::LaunchOptions, java: &str, args: Vec<String>, game_dir: &std::path::Path) -> Command {
    let wrapper = launch.wrapper_command();
    let mut cmd = match wrapper.split_first() {
        Some((program, rest)) => {
            let mut c = Command::new(program);
            c.args(rest).arg(java);
            c
        }
        None => Command::new(java),
    };
    cmd.args(args).envs(launch.env_vars()).current_dir(game_dir);
    cmd
}

async fn start(app: &AppHandle, inst: &Instance, acc: &Account, quick: Option<&QuickPlay>, close_leftovers: bool) -> Result<(Child, Option<String>, bool, Option<aot::Training>)> {
    let client = net::client();
    let progress = |stage: &str, done: u32, total: u32| {
        emit_state(app, &inst.id, "preparing", Some(stage), done, total, None, None)
    };
    emit_state(app, &inst.id, "preparing", Some("Preparing"), 0, 0, None, None);

    if close_leftovers {
        progress("Closing the old game", 0, 0);
        let id = inst.id.clone();
        let closed = tokio::task::spawn_blocking(move || procs::close_leftovers(&id))
            .await
            .map_err(|e| Error::msg(e.to_string()))??;
        diag::log(format!("Closed {closed} leftover game process(es) for {}", inst.id));
    }

    let session = match acc.kind {
        AccountKind::Microsoft => {
            progress("Signing in", 0, 0);
            let s = microsoft::launch_session(&acc.id).await?;
            if s.offline {
                diag::log(format!("{}: Microsoft unreachable, playing offline with the saved session", inst.id));
            }
            Some(s)
        }
        AccountKind::Offline => None,
    };
    let acc = &accounts::get(&acc.id).unwrap_or_else(|| acc.clone());

    let game_dir = paths::game_dir(&inst.id);
    tokio::fs::create_dir_all(&game_dir).await.map_err(Error::at(&game_dir))?;
    {
        let id = inst.id.clone();
        let app = app.clone();
        let _ = tokio::task::spawn_blocking(move || {
            backup::auto(&id, "launch", &|done, total| emit_state(&app, &id, "preparing", Some("Backing up worlds"), done, total, None, None))
        })
        .await;
    }
    let prep = install::prepare(&client, inst.launch_id(), &inst.id, game_dir.clone(), &progress).await?;

    progress("Looking for Java", 0, 0);
    let required = java_target(prep.resolved.required_java(), prep.resolved.uses_lwjgl3(), inst.loader);
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
    diag::log(format!("{}: Java {} at {}", inst.id, java.major, java.path));
    if java.major < required {
        return Err(Error::msg(format!("This version requires Java {required}, but you have Java {}", java.major)));
    }

    if session.is_some() {
        ensure_shim()?;
    }
    options_sync::apply(inst);
    let mut args = build_args(&prep, inst, acc, session.as_ref(), &java, quick)?;
    let main_class = prep.resolved.version.main_class.clone().unwrap_or_default();
    let classpath: Vec<std::path::PathBuf> = session.is_some().then(paths::shim_jar).into_iter().chain(prep.classpath.iter().cloned()).collect();
    let training = if settings::load().aot_cache {
        aot::apply(&inst.id, &java, &mut args, &main_class, &classpath)
    } else {
        aot::clear(&inst.id);
        None
    };
    let launch = inst.effective_launch();
    let wrapper = launch.wrapper_command();
    if !wrapper.is_empty() {
        diag::log(format!("{}: launching through {}", inst.id, wrapper.join(" ")));
    }
    let mut cmd = game_command(&launch, &java.path, args, &game_dir);
    cmd.stdin(if session.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        _ if !wrapper.is_empty() => Error::msg(format!("The launch wrapper \"{}\" couldn't be started: {e}", wrapper[0])),
        std::io::ErrorKind::PermissionDenied => Error::msg(format!(
            "Windows refused to start Java ({}). Your antivirus may be blocking it, or pick another Java in the instance settings",
            java.path
        )),
        _ => Error::at(std::path::Path::new(&java.path))(e),
    })?;
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
    let offline = session.as_ref().is_some_and(|s| s.offline);
    Ok((child, session.map(|s| s.token), offline, training))
}

fn forward_lines<R: AsyncRead + Unpin + Send + 'static>(app: AppHandle, id: String, reader: R, secret: Option<String>, console: bool, tail: crash::Tail, probe: Arc<startup::Probe>) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            presence::observe(&id, &line);
            if let Some(message) = presence::log_message(&line) {
                probe.observe(message);
            }
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

pub async fn run(app: AppHandle, inst: Instance, acc: Account, quick: Option<QuickPlay>, close_leftovers: bool, mut kill_rx: oneshot::Receiver<()>) {
    let id = inst.id.clone();
    crash::forget(&id);
    let kind = match acc.kind {
        AccountKind::Microsoft => "Microsoft",
        AccountKind::Offline => "offline",
    };
    diag::log(format!(
        "Launching {id}: Minecraft {} {:?} {} with {kind} account",
        inst.mc_version,
        inst.loader,
        inst.loader_version.as_deref().unwrap_or("")
    ));
    let wrapped = !inst.effective_launch().wrapper_command().is_empty();
    match start(&app, &inst, &acc, quick.as_ref(), close_leftovers).await {
        Err(e) => {
            diag::log(format!("Launch of {id} failed: {e}"));
            emit_state(&app, &id, "error", None, 0, 0, Some(e.to_string()), None)
        }
        Ok((mut child, secret, offline, training)) => {
            diag::log(format!("{id} started (pid {})", child.id().map_or("?".into(), |p| p.to_string())));
            let notice = offline.then(|| "Playing offline: singleplayer works, online servers will be back when you reconnect".to_string());
            emit_state(&app, &id, "running", None, 0, 0, notice, None);
            let prefs = settings::load();
            let console = prefs.live_console;
            if console {
                console::start(&app, &inst);
            }
            presence::start(&id, &acc.name, &inst.mc_version);
            let boost = if prefs.performance_power { power::hold_performance().await } else { None };
            if boost.is_some() {
                diag::log(format!("{id}: holding the performance power profile"));
            }
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
            let probe = startup::Probe::new(app.clone(), &id);
            let mut readers = vec![];
            let done = Arc::new(AtomicBool::new(false));
            if let Some(pid) = child.id() {
                sample_memory(app.clone(), id.clone(), pid, done.clone());
            }
            if let Some(out) = child.stdout.take() {
                readers.push(forward_lines(app.clone(), id.clone(), out, secret.clone(), console, tail.clone(), probe.clone()));
            }
            if let Some(err) = child.stderr.take() {
                readers.push(forward_lines(app.clone(), id.clone(), err, secret.clone(), console, tail.clone(), probe.clone()));
            }
            let (code, stopped) = tokio::select! {
                status = child.wait() => (status.ok().and_then(|s| s.code()), false),
                Ok(()) = &mut kill_rx => {
                    let _ = child.kill().await;
                    if wrapped {
                        let id = id.clone();
                        let _ = tokio::task::spawn_blocking(move || procs::close_leftovers(&id)).await;
                    }
                    (None, true)
                }
            };
            done.store(true, Ordering::Relaxed);
            presence::stop(&id);
            if let Some(hold) = boost {
                power::release(hold).await;
            }
            if minimized {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
            stats::record(&id, started_ms, started.elapsed().as_secs());
            options_sync::capture(&inst);
            diag::log(match (stopped, code) {
                (true, _) => format!("{id} stopped from the launcher after {}s", started.elapsed().as_secs()),
                (false, Some(c)) => format!("{id} exited with code {c} after {}s", started.elapsed().as_secs()),
                (false, None) => format!("{id} ended without an exit code after {}s", started.elapsed().as_secs()),
            });
            emit_state(&app, &id, "exited", None, 0, 0, None, code);
            if let Some(t) = training {
                let id = id.clone();
                let _ = tokio::task::spawn_blocking(move || t.finish(&id)).await;
            }
            let hung = !stopped && code != Some(0) && crash::shutdown_hang(&id, since);
            if hung {
                diag::log(format!("{id}: a mod kept Java open after Minecraft stopped, so its shutdown watchdog closed it (code {code:?}); not treated as a crash"));
            }
            if !stopped && (code == Some(0) || hung) {
                let target = id.clone();
                if let Ok(Ok(n @ 1..)) = tokio::task::spawn_blocking(move || procs::close_leftovers(&target)).await {
                    diag::log(format!("{id}: closed {n} helper process(es) left behind by mods"));
                }
            }
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

        let mut cmd = game_command(&inst.effective_launch(), &java.path, args, &game_dir);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
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

    const OLD_AIKAR: &str = "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 -XX:G1RSetUpdatingPauseTimePercent=5 -XX:SurvivorRatio=32 -XX:MaxTenuringThreshold=1 -XX:-UsePerfData -XX:ConcGCThreads=2 -XX:+UseStringDeduplication";

    struct Sample {
        ready_ms: u64,
        pauses: usize,
        pause_total_ms: f64,
        pause_max_ms: f64,
        rss_mb: u64,
    }

    fn safepoints(log: &std::path::Path) -> (usize, f64, f64) {
        let text = std::fs::read_to_string(log).unwrap_or_default();
        let ns: Vec<f64> = text
            .lines()
            .filter_map(|l| l.split("Total: ").nth(1))
            .filter_map(|t| t.split_whitespace().next()?.parse::<f64>().ok())
            .collect();
        let total = ns.iter().sum::<f64>() / 1e6;
        let max = ns.iter().cloned().fold(0.0, f64::max) / 1e6;
        (ns.len(), total, max)
    }

    fn rss_mb(pid: u32) -> u64 {
        std::fs::read_to_string(format!("/proc/{pid}/status"))
            .unwrap_or_default()
            .lines()
            .find_map(|l| l.strip_prefix("VmRSS:"))
            .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
            .map_or(0, |kb| kb / 1024)
    }

    async fn measure_once(prep: &Prepared, inst: &Instance, java: &JavaInstall, gc_log: &std::path::Path) -> Option<Sample> {
        let acc = Account { id: "x".into(), name: "Bench".into(), uuid: accounts::offline_uuid("Bench"), kind: AccountKind::Offline };
        let _ = std::fs::remove_file(gc_log);
        let mut args = vec![format!("-Xlog:safepoint:file={}", gc_log.display())];
        args.extend(build_args(prep, inst, &acc, None, java, None).unwrap());
        let game_dir = paths::game_dir(&inst.id);
        let mut cmd = game_command(&inst.effective_launch(), &java.path, args, &game_dir);
        cmd.stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
        let started = std::time::Instant::now();
        let mut child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let hard = tokio::time::Instant::now() + std::time::Duration::from_secs(240);
        let mut ready: Option<u64> = None;
        let mut until = hard;
        loop {
            match tokio::time::timeout_at(until, lines.next_line()).await {
                Ok(Ok(Some(line))) => {
                    if line.contains("Sound engine started") || (ready.is_some() && line.contains("Created: ") && line.contains("atlas")) {
                        ready = Some(started.elapsed().as_millis() as u64);
                        until = (tokio::time::Instant::now() + std::time::Duration::from_secs(8)).min(hard);
                    }
                }
                Ok(Ok(None)) | Ok(Err(_)) => break,
                Err(_) => break,
            }
        }
        let rss = rss_mb(pid);
        let _ = child.kill().await;
        let (pauses, pause_total_ms, pause_max_ms) = safepoints(gc_log);
        ready.map(|ready_ms| Sample { ready_ms, pauses, pause_total_ms, pause_max_ms, rss_mb: rss })
    }

    fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
        let Ok(rd) = std::fs::read_dir(from) else { return };
        std::fs::create_dir_all(to).unwrap();
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                copy_tree(&p, &to.join(e.file_name()));
            } else {
                let _ = std::fs::copy(&p, to.join(e.file_name()));
            }
        }
    }

    async fn bench_base(id: &str, muted: bool) -> (Instance, Prepared) {
        let client = net::client();
        let from = crate::instance::get(&std::env::var("BENCH_FROM").expect("BENCH_FROM=<instance id>")).unwrap();
        let game_dir = paths::game_dir(id);
        let _ = std::fs::remove_dir_all(paths::instance_dir(id));
        let src = paths::game_dir(&from.id);
        for sub in ["mods", "config"] {
            copy_tree(&src.join(sub), &game_dir.join(sub));
        }
        let skip = std::env::var("BENCH_SKIP_MODS").unwrap_or_default().to_lowercase();
        for e in std::fs::read_dir(game_dir.join("mods")).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if skip.split(',').any(|s| !s.trim().is_empty() && name.contains(s.trim())) {
                let _ = std::fs::remove_file(e.path());
            }
        }
        let options = std::fs::read_to_string(src.join("options.txt")).unwrap_or_default();
        let kept: Vec<&str> = options.lines().filter(|l| !l.starts_with("inactivityFpsLimit:")).collect();
        let mut options = format!("{}\ninactivityFpsLimit:\"minimized\"\n", kept.join("\n"));
        if muted {
            let kept: Vec<&str> = options.lines().filter(|l| !l.starts_with("soundCategory_master:")).collect();
            options = format!("{}\nsoundCategory_master:0.0\n", kept.join("\n"));
        }
        std::fs::write(game_dir.join("options.txt"), options).unwrap();
        let base = Instance { id: id.into(), custom_performance: true, java_path: None, extra_jvm_args: String::new(), pack: None, ..from.clone() };
        let prep = install::prepare(&client, base.launch_id(), id, game_dir, &|_: &str, _: u32, _: u32| {}).await.unwrap();
        (base, prep)
    }

    async fn bench_variants(base: &Instance) -> Vec<(String, Instance, JavaInstall)> {
        let runtimes = std::env::var("BENCH_JAVA").unwrap_or_else(|_| "graalvm-25".into());
        let only = std::env::var("BENCH_CASES").ok();
        let mut variants = vec![];
        for rt in runtimes.split(',') {
            let bin = runtime::home_of(rt).unwrap().join("bin").join("java");
            let java = java::probe(&bin).await.expect("runtime not installed");
            let old_zgc = if (21..=22).contains(&java.major) { "-XX:+UseZGC -XX:+ZGenerational" } else { "-XX:+UseZGC" };
            let old_perf = format!("{old_zgc} -XX:+DisableExplicitGC -XX:-UsePerfData -XX:ConcGCThreads=2");
            let balanced_without = |flag: &str| {
                flags::jvm_flags(FlagProfile::Balanced, java.major, java.is_graal).into_iter().filter(|f| f != flag).collect::<Vec<_>>().join(" ")
            };
            let cases = [
                ("old balanced", FlagProfile::None, OLD_AIKAR.to_string()),
                ("new balanced", FlagProfile::Balanced, String::new()),
                ("old performance", FlagProfile::None, old_perf),
                ("new performance", FlagProfile::Performance, String::new()),
                ("new balanced -THP", FlagProfile::None, balanced_without("-XX:+UseTransparentHugePages")),
                ("new balanced -COH", FlagProfile::None, balanced_without("-XX:+UseCompactObjectHeaders")),
            ];
            for (name, profile, extra) in cases {
                let wanted = match &only {
                    Some(o) => o.split(',').any(|c| c.trim() == name),
                    None => !name.contains(" -"),
                };
                if wanted {
                    let inst = Instance { flag_profile: profile, extra_jvm_args: extra, ..base.clone() };
                    variants.push((format!("{rt} {name}"), inst, java.clone()));
                }
            }
        }
        variants
    }

    #[tokio::test]
    #[ignore]
    async fn bench_jvm_profiles() {
        let reps: usize = std::env::var("BENCH_REPS").ok().and_then(|r| r.parse().ok()).unwrap_or(3);
        let id = "bench-jvm";
        let (base, prep) = bench_base(id, true).await;
        let gc_log = paths::instance_dir(id).join("safepoints.log");
        let variants = bench_variants(&base).await;

        eprintln!("warm-up...");
        let _ = measure_once(&prep, &variants[0].1, &variants[0].2, &gc_log).await;
        let mut results: Vec<Vec<Sample>> = variants.iter().map(|_| vec![]).collect();
        for rep in 0..reps {
            for (i, (name, inst, java)) in variants.iter().enumerate() {
                let s = measure_once(&prep, inst, java, &gc_log).await;
                match &s {
                    Some(s) => eprintln!("[{}/{reps}] {name}: ready {} ms, {} pauses, {:.0} ms total, max {:.1} ms, rss {} MB", rep + 1, s.ready_ms, s.pauses, s.pause_total_ms, s.pause_max_ms, s.rss_mb),
                    None => eprintln!("[{}/{reps}] {name}: did not reach the title screen", rep + 1),
                }
                results[i].extend(s);
            }
        }

        let rows: Vec<(String, Vec<Sample>)> = variants.iter().map(|(n, _, _)| n.clone()).zip(results).collect();
        print_samples(&rows);
        let _ = std::fs::remove_dir_all(paths::instance_dir(id));
        let _ = std::fs::remove_dir_all(paths::natives(id));
    }

    fn median(mut v: Vec<f64>) -> f64 {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if v.is_empty() { 0.0 } else { v[v.len() / 2] }
    }

    fn print_samples(rows: &[(String, Vec<Sample>)]) {
        eprintln!("\n{:<44} {:>9} {:>8} {:>11} {:>9} {:>8}", "variant (median)", "ready ms", "pauses", "pause tot", "pause max", "rss MB");
        for (name, r) in rows {
            eprintln!(
                "{:<44} {:>9.0} {:>8.0} {:>9.0}ms {:>7.1}ms {:>8.0}   ({} ok)",
                name,
                median(r.iter().map(|s| s.ready_ms as f64).collect()),
                median(r.iter().map(|s| s.pauses as f64).collect()),
                median(r.iter().map(|s| s.pause_total_ms).collect()),
                median(r.iter().map(|s| s.pause_max_ms).collect()),
                median(r.iter().map(|s| s.rss_mb as f64).collect()),
                r.len()
            );
        }
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    fn aot_bench_procs() -> Vec<u32> {
        std::fs::read_dir("/proc")
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
            .filter(|p| std::fs::read_link(format!("/proc/{p}/cwd")).is_ok_and(|c| c.to_string_lossy().contains("bench-aot")))
            .collect()
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    async fn train_aot(prep: &Prepared, inst: &Instance, java: &JavaInstall, out: &std::path::Path, log: &std::path::Path) -> bool {
        let acc = Account { id: "x".into(), name: "Bench".into(), uuid: accounts::offline_uuid("Bench"), kind: AccountKind::Offline };
        let _ = std::fs::remove_file(out);
        let inst = Instance { extra_jvm_args: format!("{} -XX:AOTCacheOutput={}", inst.extra_jvm_args, out.display()), ..inst.clone() };
        let args = build_args(prep, &inst, &acc, None, java, None).unwrap();
        launch_to_title(&inst, java, args, true, log).await && out.is_file()
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    async fn launch_to_title(inst: &Instance, java: &JavaInstall, args: Vec<String>, graceful: bool, log: &std::path::Path) -> bool {
        let mut cmd = game_command(&inst.effective_launch(), &java.path, args, &paths::game_dir(&inst.id));
        cmd.stdout(Stdio::piped()).stderr(std::fs::File::create(log).unwrap()).kill_on_drop(true);
        let mut child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let (ready_tx, ready_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let mut ready_tx = Some(ready_tx);
            while let Ok(Some(line)) = lines.next_line().await {
                if line.contains("Sound engine started") {
                    if let Some(tx) = ready_tx.take() {
                        let _ = tx.send(());
                    }
                }
            }
        });
        if !matches!(tokio::time::timeout(std::time::Duration::from_secs(240), ready_rx).await, Ok(Ok(()))) {
            let _ = child.kill().await;
            return false;
        }
        if !graceful {
            let _ = child.kill().await;
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        let exited = tokio::time::timeout(std::time::Duration::from_secs(300), child.wait()).await.is_ok();
        if !exited {
            let _ = child.kill().await;
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let left = aot_bench_procs();
        if !left.is_empty() {
            eprintln!("after a clean exit {} process(es) are still running: {left:?}", left.len());
        }
        true
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[tokio::test]
    #[ignore]
    async fn aot_cache_roundtrip() {
        let id = "bench-aot";
        let (base, prep) = bench_base(id, true).await;
        let rt = std::env::var("BENCH_JAVA").unwrap_or_else(|_| "graalvm-25".into());
        let java = java::probe(&runtime::home_of(&rt).unwrap().join("bin").join("java")).await.unwrap();
        let acc = Account { id: "x".into(), name: "Bench".into(), uuid: accounts::offline_uuid("Bench"), kind: AccountKind::Offline };
        let main = prep.resolved.version.main_class.clone().unwrap();
        let log = paths::instance_dir(id).join("roundtrip.log");
        let mut args = build_args(&prep, &base, &acc, None, &java, None).unwrap();
        let training = aot::apply(id, &java, &mut args, &main, &prep.classpath).expect("the first launch records a cache");
        let t = std::time::Instant::now();
        assert!(launch_to_title(&base, &java, args, true, &log).await, "training launch never reached the title screen");
        eprintln!("training launch with a clean exit took {} s", t.elapsed().as_secs());
        training.finish(id);
        let mut args = build_args(&prep, &base, &acc, None, &java, None).unwrap();
        assert!(aot::apply(id, &java, &mut args, &main, &prep.classpath).is_none());
        assert!(args[0].starts_with("-XX:AOTCache="), "the second launch did not pick up the cache");
        args.insert(1, "-XX:AOTMode=on".into());
        let ok = launch_to_title(&base, &java, args, false, &log).await;
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let aot_lines: Vec<&str> = text.lines().filter(|l| l.contains("AOT") || l.contains("aot")).take(10).collect();
        assert!(ok, "the JVM refused the cache:\n{}", aot_lines.join("\n"));
        let _ = std::fs::remove_dir_all(paths::instance_dir(id));
        let _ = std::fs::remove_dir_all(paths::natives(id));
    }

    #[tokio::test]
    #[ignore]
    async fn old_version_on_other_java() {
        let id = "bench-oldjava";
        let (base, prep) = bench_base(id, true).await;
        let rt = std::env::var("BENCH_JAVA").unwrap_or_else(|_| "graalvm-25".into());
        let java = java::probe(&runtime::home_of(&rt).unwrap().join("bin").join("java")).await.expect("runtime not installed");
        let acc = Account { id: "x".into(), name: "Bench".into(), uuid: accounts::offline_uuid("Bench"), kind: AccountKind::Offline };
        let args = build_args(&prep, &base, &acc, None, &java, None).unwrap();
        let mut cmd = game_command(&base.effective_launch(), &java.path, args, &paths::game_dir(id));
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        let mut child = cmd.spawn().unwrap();
        async fn drain<R: AsyncRead + Unpin>(r: R) -> Vec<String> {
            let mut lines = BufReader::new(r).lines();
            let mut v = vec![];
            while let Ok(Some(x)) = lines.next_line().await {
                v.push(x);
            }
            v
        }
        let out = child.stdout.take().unwrap();
        let err = child.stderr.take().unwrap();
        let started = std::time::Instant::now();
        let run = async { tokio::join!(drain(out), drain(err), child.wait()) };
        let res = tokio::time::timeout(std::time::Duration::from_secs(90), run).await;
        match res {
            Ok((o, e, status)) => {
                eprintln!("{} on Java {}: exited after {} s with {:?}", base.mc_version, java.major, started.elapsed().as_secs(), status.ok().and_then(|s| s.code()));
                let lines: Vec<&String> = o.iter().chain(e.iter()).filter(|l| l.contains("Exception") || l.contains("Error") || l.contains("Caused by")).take(8).collect();
                for l in lines {
                    eprintln!("  {}", l.chars().take(220).collect::<String>());
                }
            }
            Err(_) => eprintln!("{} on Java {}: still running after 90 s, so it got past startup", base.mc_version, java.major),
        }
        let _ = std::fs::remove_dir_all(paths::instance_dir(id));
        let _ = std::fs::remove_dir_all(paths::natives(id));
    }

    async fn reaches_title(inst: &Instance, prep: &Prepared, java: &JavaInstall) -> (bool, Option<i32>, Vec<String>) {
        let acc = Account { id: "x".into(), name: "Bench".into(), uuid: accounts::offline_uuid("Bench"), kind: AccountKind::Offline };
        let args = build_args(prep, inst, &acc, None, java, None).unwrap();
        let log = paths::instance_dir(&inst.id).join("stderr.log");
        let mut cmd = game_command(&inst.effective_launch(), &java.path, args, &paths::game_dir(&inst.id));
        cmd.stdout(Stdio::piped()).stderr(std::fs::File::create(&log).unwrap()).kill_on_drop(true);
        let mut child = cmd.spawn().unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(240);
        let mut errors = vec![];
        let mut ready = false;
        while let Ok(Ok(Some(line))) = tokio::time::timeout_at(deadline, lines.next_line()).await {
            if line.contains("Sound engine started") {
                ready = true;
                break;
            }
            if errors.len() < 6 && (line.contains("Exception") || line.contains("Caused by") || line.contains("Unsupported class file")) {
                errors.push(line.chars().take(600).collect());
            }
        }
        let code = if ready {
            let _ = child.kill().await;
            None
        } else {
            match tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await {
                Ok(Ok(st)) => st.code(),
                _ => {
                    let _ = child.kill().await;
                    None
                }
            }
        };
        if !ready {
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            errors.extend(text.lines().filter(|l| l.contains("Exception") || l.contains("Caused by") || l.contains("Error:")).take(6).map(|l| l.chars().take(600).collect::<String>()));
        }
        (ready, code, errors)
    }

    #[tokio::test]
    #[ignore]
    async fn java_matrix() {
        let client = net::client();
        let cases: Vec<(String, Loader)> = std::env::var("MATRIX").unwrap_or_else(|_| "1.17.1:fabric,1.17.1:forge,1.18.2:fabric,1.18.2:forge,1.19.4:fabric,1.19.4:forge,1.20.1:fabric,1.20.1:forge,1.20.4:forge,1.20.4:neoforge".into())
            .split(',')
            .map(|c| {
                let (v, l) = c.split_once(':').unwrap();
                let loader = match l { "fabric" => Loader::Fabric, "forge" => Loader::Forge, "neoforge" => Loader::NeoForge, "quilt" => Loader::Quilt, _ => Loader::Vanilla };
                (v.to_string(), loader)
            })
            .collect();
        let probe = |id: String| {
            let client = client.clone();
            async move {
                let bin = runtime::home_of(&id).unwrap().join("bin").join("java");
                if let Some(j) = java::probe(&bin).await {
                    return j;
                }
                let major: u32 = id.rsplit('-').next().unwrap().parse().unwrap();
                runtime::ensure_temurin(&client, major, &|_, _, _| {}).await.unwrap_or_else(|e| panic!("{id}: {e}"))
            }
        };
        let tested = std::env::var("MATRIX_JAVA").unwrap_or_else(|_| "graalvm-21,graalvm-25".into());
        let mut javas = vec![];
        for id in tested.split(',') {
            javas.push(probe(id.trim().to_string()).await);
        }
        let base_id = std::env::var("MATRIX_BASE").unwrap_or_else(|_| "temurin-17".into());
        let t17 = probe(base_id.clone()).await;
        let mut rows = vec![];
        for (mc, loader) in cases {
            let id = format!("jtest-{}-{:?}", mc.replace('.', "_"), loader).to_lowercase();
            let _ = std::fs::remove_dir_all(paths::instance_dir(&id));
            let (loader_version, launch_version) = match loader {
                Loader::Vanilla => (None, None),
                _ => match loaders::install(&client, loader, &mc, None, &|_, _, _| {}).await {
                    Ok(i) => (Some(i.loader_version), Some(i.id)),
                    Err(e) => {
                        eprintln!("{mc} {loader:?}: loader install failed: {e}");
                        rows.push(format!("{mc:<8} {loader:<9?} install failed"));
                        continue;
                    }
                },
            };
            let inst = Instance {
                id: id.clone(),
                name: id.clone(),
                mc_version: mc.clone(),
                loader,
                loader_version,
                launch_version,
                custom_performance: true,
                memory_max_mb: 2048,
                memory_min_mb: 512,
                flag_profile: FlagProfile::Balanced,
                java_path: None,
                extra_jvm_args: String::new(),
                icon_url: None,
                banner_url: None,
                pack: None,
                launch: crate::instance::LaunchOptions::default(),
            };
            let game_dir = paths::game_dir(&id);
            std::fs::create_dir_all(&game_dir).unwrap();
            std::fs::write(game_dir.join("options.txt"), "soundCategory_master:0.0\n").unwrap();
            let prep = install::prepare(&client, inst.launch_id(), &id, game_dir, &|_: &str, _: u32, _: u32| {}).await.unwrap();
            eprintln!("{mc} {loader:?}: the launcher would pick Java {}", java_target(prep.resolved.required_java(), prep.resolved.uses_lwjgl3(), loader));
            let mut cells = vec![];
            let mut failed = false;
            for java in &javas {
                let (ok, code, errors) = reaches_title(&inst, &prep, java).await;
                eprintln!("{mc} {loader:?} on {} {}: {}", if java.is_graal { "GraalVM" } else { "Temurin" }, java.major, if ok { "title screen".to_string() } else { format!("FAILED (exit {code:?})") });
                for e in &errors {
                    eprintln!("    {e}");
                }
                failed |= !ok;
                cells.push(if ok { "ok" } else { "FAIL" });
            }
            let baseline = if failed {
                let (ok, code, errors) = reaches_title(&inst, &prep, &t17).await;
                eprintln!("{mc} {loader:?} on {base_id}: {}", if ok { "title screen".to_string() } else { format!("FAILED (exit {code:?})") });
                for e in &errors {
                    eprintln!("    {e}");
                }
                if ok { "ok" } else { "FAIL" }
            } else {
                "-"
            };
            rows.push(format!("{mc:<8} {loader:<9?} {} {}", cells.iter().map(|c| format!("{c:<10}")).collect::<String>(), baseline));
            let _ = std::fs::remove_dir_all(paths::instance_dir(&id));
            let _ = std::fs::remove_dir_all(paths::natives(&id));
        }
        eprintln!("\n{:<8} {:<9} {} {base_id} (only if needed)", "mc", "loader", tested.split(',').map(|j| format!("{:<10}", j.trim())).collect::<String>());
        for r in rows {
            eprintln!("{r}");
        }
    }

    #[tokio::test]
    #[ignore]
    async fn print_jvm_args() {
        let client = net::client();
        let mc = std::env::var("ARGS_MC").unwrap_or_else(|_| "1.8.9".into());
        let rt = std::env::var("ARGS_JAVA").unwrap_or_else(|_| "temurin-8".into());
        let java = java::probe(&runtime::home_of(&rt).unwrap().join("bin").join("java")).await.unwrap();
        let id = "jtest-args";
        let inst = Instance {
            id: id.into(), name: id.into(), mc_version: mc.clone(), loader: Loader::Vanilla, loader_version: None, launch_version: None,
            custom_performance: true, memory_max_mb: 2048, memory_min_mb: 512, flag_profile: FlagProfile::Balanced, java_path: None,
            extra_jvm_args: String::new(), icon_url: None, banner_url: None, pack: None, launch: crate::instance::LaunchOptions::default(),
        };
        let game_dir = paths::game_dir(id);
        std::fs::create_dir_all(&game_dir).unwrap();
        let prep = install::prepare(&client, inst.launch_id(), id, game_dir, &|_: &str, _: u32, _: u32| {}).await.unwrap();
        let acc = Account { id: "x".into(), name: "Bench".into(), uuid: accounts::offline_uuid("Bench"), kind: AccountKind::Offline };
        let args = build_args(&prep, &inst, &acc, None, &java, None).unwrap();
        let main = prep.resolved.version.main_class.clone().unwrap();
        for a in args.iter().take_while(|a| **a != main) {
            if a.starts_with("-cp") || a.contains(".jar:") { eprintln!("  <classpath>"); } else { eprintln!("  {a}"); }
        }
        let out = std::process::Command::new(&java.path).args(args.iter().take_while(|a| **a != main).filter(|a| !a.contains(".jar:") && *a != "-cp")).arg("-version").output().unwrap();
        eprintln!("java -version with these flags: {}", String::from_utf8_lossy(&out.stderr).lines().take(4).collect::<Vec<_>>().join(" | "));
        let _ = std::fs::remove_dir_all(paths::instance_dir(id));
        let _ = std::fs::remove_dir_all(paths::natives(id));
    }

    #[test]
    #[ignore]
    fn write_shim_jar() {
        let out = std::env::var("SHIM_OUT").expect("SHIM_OUT=<path>");
        std::fs::write(out, shim_jar_bytes().unwrap()).unwrap();
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[tokio::test]
    #[ignore]
    async fn bench_aot() {
        let reps: usize = std::env::var("BENCH_REPS").ok().and_then(|r| r.parse().ok()).unwrap_or(3);
        let id = "bench-aot";
        let (base, prep) = bench_base(id, true).await;
        let dir = paths::instance_dir(id);
        let gc_log = dir.join("safepoints.log");
        let variants = bench_variants(&base).await;
        let mut rows: Vec<(String, Vec<Sample>)> = vec![];
        for (name, inst, java) in &variants {
            let cache = dir.join(format!("{}.aot", name.replace(' ', "-")));
            let log = dir.join(format!("{}.train.log", name.replace(' ', "-")));
            let t = std::time::Instant::now();
            let ok = train_aot(&prep, inst, java, &cache, &log).await;
            let mb = std::fs::metadata(&cache).map_or(0, |m| m.len() / 1_048_576);
            eprintln!("{name}: training took {} s, cache {mb} MB", t.elapsed().as_secs());
            if !ok {
                let text = std::fs::read_to_string(&log).unwrap_or_default();
                let aot: Vec<&str> = text.lines().filter(|l| l.contains("AOT") || l.contains("aot")).take(15).collect();
                eprintln!("{name}: no cache was written\n{}", aot.join("\n"));
                continue;
            }
            let with = Instance { extra_jvm_args: format!("{} -XX:AOTCache={} -XX:AOTMode=on", inst.extra_jvm_args, cache.display()), ..inst.clone() };
            let _ = measure_once(&prep, inst, java, &gc_log).await;
            let (mut plain, mut cached) = (vec![], vec![]);
            for rep in 0..reps {
                for (label, target, out) in [("plain", inst, &mut plain), ("aot", &with, &mut cached)] {
                    let s = measure_once(&prep, target, java, &gc_log).await;
                    match &s {
                        Some(s) => eprintln!("[{}/{reps}] {name} {label}: ready {} ms, rss {} MB", rep + 1, s.ready_ms, s.rss_mb),
                        None => eprintln!("[{}/{reps}] {name} {label}: did not reach the title screen", rep + 1),
                    }
                    out.extend(s);
                }
            }
            rows.push((name.clone(), plain));
            rows.push((format!("{name} + AOT cache"), cached));
        }
        print_samples(&rows);
        let _ = std::fs::remove_dir_all(paths::instance_dir(id));
        let _ = std::fs::remove_dir_all(paths::natives(id));
    }

    const FPS_DELAY_S: u64 = 40;
    const FPS_LOG_S: u64 = 120;

    fn frame_log(dir: &std::path::Path, summary: bool) -> Option<std::path::PathBuf> {
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "csv") && p.to_string_lossy().contains("summary") == summary)
    }

    fn frame_stats(dir: &std::path::Path) -> Option<(f64, f64, f64, usize)> {
        let text = std::fs::read_to_string(frame_log(dir, false)?).ok()?;
        let mut lines = text.lines();
        let header: Vec<&str> = lines.by_ref().find(|l| l.starts_with("fps,"))?.split(',').collect();
        let col = header.iter().position(|h| *h == "frametime")?;
        let mut ft: Vec<f64> = lines.filter_map(|l| l.split(',').nth(col)?.parse().ok()).filter(|v: &f64| *v > 0.0).collect();
        if ft.is_empty() {
            return None;
        }
        let avg = 1000.0 * ft.len() as f64 / ft.iter().sum::<f64>();
        ft.sort_by(|a, b| b.partial_cmp(a).unwrap());
        let low = |share: f64| {
            let n = ((ft.len() as f64 * share).ceil() as usize).max(1);
            1000.0 * n as f64 / ft[..n].iter().sum::<f64>()
        };
        Some((avg, low(0.01), low(0.001), ft.len()))
    }

    async fn play_once(prep: &Prepared, inst: &Instance, java: &JavaInstall, quick: Option<&QuickPlay>, gc_log: &std::path::Path, fps_dir: Option<&std::path::Path>) -> u64 {
        let acc = Account { id: "x".into(), name: "Bench".into(), uuid: accounts::offline_uuid("Bench"), kind: AccountKind::Offline };
        let _ = std::fs::remove_file(gc_log);
        let mut args = vec![format!("-Xlog:safepoint:file={}", gc_log.display())];
        args.extend(build_args(prep, inst, &acc, None, java, quick).unwrap());
        let mut launch = inst.effective_launch();
        if let Some(dir) = fps_dir {
            std::fs::create_dir_all(dir).unwrap();
            launch.wrapper = "mangohud --dlsym".into();
            launch.env = format!("{}\nMANGOHUD_CONFIG=output_folder={},autostart_log={FPS_DELAY_S},log_duration={FPS_LOG_S},log_interval=0", launch.env, dir.display());
        }
        let started = std::time::Instant::now();
        let mut cmd = game_command(&launch, &java.path, args, &paths::game_dir(&inst.id));
        cmd.stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
        let mut child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();
        let mut peak = 0;
        let mut logging: Option<std::time::Instant> = None;
        loop {
            tokio::select! {
                _ = child.wait() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
            }
            peak = peak.max(rss_mb(pid));
            let Some(dir) = fps_dir else { continue };
            if logging.is_none() && frame_log(dir, false).is_some() {
                logging = Some(std::time::Instant::now());
            }
            let overdue = started.elapsed().as_secs() > FPS_DELAY_S + FPS_LOG_S + 20;
            let finished = overdue || frame_log(dir, true).is_some() || logging.is_some_and(|t| t.elapsed().as_secs() > FPS_LOG_S + 5);
            if finished {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let _ = child.kill().await;
                break;
            }
        }
        peak
    }

    #[tokio::test]
    #[ignore]
    async fn play_jvm_profiles() {
        let id = "bench-play";
        let (base, prep) = bench_base(id, false).await;
        let mut variants = bench_variants(&base).await;
        let dir = paths::instance_dir(id);
        let gc_log = dir.join("safepoints.log");
        let saves = paths::game_dir(id).join("saves");

        let kept_world = std::env::var("BENCH_WORLD_DIR").ok().map(std::path::PathBuf::from);
        match &kept_world {
            Some(w) if w.is_dir() => copy_tree(w, &saves),
            _ => {
                eprintln!("setup: create a world, walk around for a bit, then quit the game");
                play_once(&prep, &variants[0].1, &variants[0].2, None, &gc_log, None).await;
                if let Some(w) = &kept_world {
                    copy_tree(&saves, w);
                }
            }
        }
        let world = std::fs::read_dir(&saves)
            .expect("no world was created")
            .flatten()
            .filter(|e| e.path().is_dir())
            .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
            .expect("no world was created")
            .file_name()
            .to_string_lossy()
            .into_owned();
        let snapshot = dir.join("saves-snapshot");
        copy_tree(&saves, &snapshot);

        let mut seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos() as u64;
        for i in (1..variants.len()).rev() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            variants.swap(i, (seed >> 33) as usize % (i + 1));
        }

        let quick = QuickPlay::World { folder: world };
        let mut rows = vec![];
        for (i, (name, inst, java)) in variants.iter().enumerate() {
            let letter = (b'A' + i as u8) as char;
            let _ = std::fs::remove_dir_all(&saves);
            copy_tree(&snapshot, &saves);
            let fps_dir = dir.join("fps").join(letter.to_string());
            eprintln!("version {letter}: recording starts {FPS_DELAY_S}s after launch and lasts {FPS_LOG_S}s, the game closes by itself");
            let peak = play_once(&prep, inst, java, Some(&quick), &gc_log, Some(&fps_dir)).await;
            let (pauses, pause_total_ms, pause_max_ms) = safepoints(&gc_log);
            let stats = frame_stats(&fps_dir);
            match stats {
                Some((avg, low1, low01, _)) => eprintln!("version {letter}: {avg:.0} fps, 1% low {low1:.0}, 0.1% low {low01:.0}"),
                None => eprintln!("version {letter}: no frame log"),
            }
            rows.push((letter, name.clone(), stats, pauses, pause_total_ms, pause_max_ms, peak));
        }

        eprintln!("\n{:<3} {:<30} {:>7} {:>7} {:>8} {:>7} {:>7} {:>10} {:>9} {:>8}", "", "variant", "avg", "1% low", "0.1% low", "frames", "pauses", "pause tot", "pause max", "peak MB");
        for (letter, name, stats, pauses, total, max, peak) in rows {
            let (avg, low1, low01, frames) = stats.unwrap_or_default();
            eprintln!("{letter:<3} {name:<30} {avg:>7.0} {low1:>7.0} {low01:>8.0} {frames:>7} {pauses:>7} {total:>8.0}ms {max:>7.1}ms {peak:>8}");
        }
        if let Ok(out) = std::env::var("BENCH_OUT") {
            copy_tree(&dir.join("fps"), std::path::Path::new(&out));
        }
        let _ = std::fs::remove_dir_all(paths::instance_dir(id));
        let _ = std::fs::remove_dir_all(paths::natives(id));
    }

    #[test]
    fn modern_java_only_where_it_runs() {
        assert_eq!(java_target(8, true, Loader::Vanilla), 21);
        assert_eq!(java_target(8, true, Loader::Fabric), 21);
        assert_eq!(java_target(8, true, Loader::Forge), 8);
        assert_eq!(java_target(8, false, Loader::Vanilla), 8);
        assert_eq!(java_target(17, true, Loader::Forge), 17);
        assert_eq!(java_target(25, true, Loader::Fabric), 25);
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
            pack: None,
            launch: crate::instance::LaunchOptions {
                wrapper: std::env::var("SMOKE_WRAPPER").unwrap_or_default(),
                env: std::env::var("SMOKE_ENV").unwrap_or_default(),
                width: std::env::var("SMOKE_WIDTH").ok().and_then(|w| w.parse().ok()),
                height: std::env::var("SMOKE_HEIGHT").ok().and_then(|h| h.parse().ok()),
                fullscreen: false,
                skip_defaults: false,
            },
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
