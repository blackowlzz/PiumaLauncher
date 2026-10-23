use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::oneshot;

use crate::accounts::{self, Account, AccountStore};
use crate::backup;
use crate::console;
use crate::cli;
use crate::crash;
use crate::diag;
use crate::shortcut;
use crate::share;
use crate::servers::{self, QuickPlay};
use crate::error::{Error, Result};
use crate::instance::{self, Instance};
use crate::java::{self, JavaInstall};
use crate::runtime;
use crate::microsoft;
use crate::migrate;
use crate::presence;
use crate::minecraft::{self, VersionEntry};
use crate::modrinth::{self, Content, ModFile, SearchResult};
use crate::news::{self, NewsItem};
use crate::settings;
use crate::storage;
use crate::skin;
use crate::wardrobe;
use crate::thumb;
use crate::update;
use crate::stats::{self, Session};
use crate::library;
use crate::loaders::{self, Loader};
use crate::{launch, net, paths, procs, Running};

#[tauri::command]
pub async fn list_mc_versions() -> Result<Vec<VersionEntry>> {
    let manifest = minecraft::manifest(&net::client()).await?;
    Ok(manifest
        .versions
        .into_iter()
        .map(|v| VersionEntry { id: v.id, kind: v.kind, release_time: v.release_time })
        .collect())
}

#[tauri::command(async)]
pub fn list_instances() -> Vec<Instance> {
    instance::list()
}

#[tauri::command]
pub async fn create_instance(app: AppHandle, name: String, mc_version: String, loader: Loader) -> Result<Instance> {
    if name.trim().is_empty() {
        return Err(Error::msg("Give the instance a name"));
    }
    let (loader_version, launch_version) = match loader {
        Loader::Vanilla => (None, None),
        _ => {
            let installed = net::cancellable(loaders::install(&net::client(), loader, &mc_version, None, &progress_emitter(&app))).await?;
            (Some(installed.loader_version), Some(installed.id))
        }
    };
    instance::create(&name, &mc_version, loader, loader_version, launch_version)
}

#[tauri::command]
pub fn update_instance(mut instance: Instance) -> Result<()> {
    instance.pack = instance::get(&instance.id)?.pack;
    instance::save(&instance)
}

#[tauri::command(async)]
pub fn delete_instance(running: State<'_, Running>, id: String) -> Result<()> {
    if running.0.lock().unwrap().contains_key(&id) {
        return Err(Error::msg("Close the game before deleting the instance"));
    }
    instance::delete(&id)?;
    shortcut::forget(&id);
    Ok(())
}

#[tauri::command]
pub fn list_accounts() -> AccountStore {
    accounts::load()
}

#[tauri::command]
pub fn add_offline_account(name: String) -> Result<Account> {
    accounts::add_offline(&name)
}

#[tauri::command]
pub async fn microsoft_login_start() -> Result<microsoft::DeviceLogin> {
    microsoft::start().await
}

#[tauri::command]
pub async fn microsoft_login_wait() -> Result<Account> {
    microsoft::finish().await
}

#[tauri::command]
pub fn microsoft_login_cancel() {
    microsoft::cancel()
}

#[tauri::command]
pub fn remove_account(id: String) -> Result<()> {
    accounts::remove(&id)
}

#[tauri::command]
pub fn set_active_account(id: String) -> Result<()> {
    accounts::set_active(&id)
}

#[tauri::command]
pub async fn detect_java() -> Vec<JavaInstall> {
    java::detect().await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    total_memory_mb: u64,
    cpu_cores: usize,
}

#[tauri::command(async)]
pub fn system_info() -> SystemInfo {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    SystemInfo {
        total_memory_mb: sys.total_memory() / 1_048_576,
        cpu_cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
    }
}

#[tauri::command(async)]
pub fn effective_launch(instance_id: String) -> Result<crate::optimizer::Effective> {
    let inst = instance::get(&instance_id)?;
    Ok(crate::optimizer::effective(&inst, &crate::settings::load()))
}

#[tauri::command(async)]
pub fn recommend_performance() -> crate::optimizer::Recommendation {
    crate::optimizer::recommend(&crate::optimizer::hardware(), 0)
}

#[tauri::command]
pub async fn list_runtimes() -> Vec<runtime::RuntimeEntry> {
    runtime::catalog().await
}

#[tauri::command]
pub async fn install_runtime(app: AppHandle, flavor: String, major: u32) -> Result<JavaInstall> {
    runtime::install_by_id(&net::client(), &flavor, major, &progress_emitter(&app)).await
}

#[tauri::command]
pub fn launch_instance(app: AppHandle, running: State<'_, Running>, id: String, quick_play: Option<QuickPlay>, close_leftovers: Option<bool>) -> Result<()> {
    let inst = instance::get(&id)?;
    if let Some(q) = &quick_play {
        q.validate(&id)?;
    }
    let account = accounts::active().ok_or_else(|| Error::msg("Create or select an account before playing"))?;

    let (tx, rx) = oneshot::channel();
    {
        let mut map = running.0.lock().unwrap();
        if map.contains_key(&id) {
            return Err(Error::msg("The instance is already running"));
        }
        map.insert(id, tx);
    }
    tauri::async_runtime::spawn(launch::run(app, inst, account, quick_play, close_leftovers.unwrap_or(false), rx));
    Ok(())
}

#[tauri::command(async)]
pub fn leftover_processes(running: State<'_, Running>, instance_id: String) -> Result<usize> {
    instance::check_id(&instance_id)?;
    if running.0.lock().unwrap().contains_key(&instance_id) {
        return Ok(0);
    }
    Ok(procs::leftovers(&instance_id).len())
}

#[tauri::command(async)]
pub fn diagnostics(running: State<'_, Running>) -> String {
    let ids: Vec<String> = running.0.lock().unwrap().keys().cloned().collect();
    diag::report(&ids)
}

#[tauri::command]
pub fn diag_log(line: String) {
    diag::log(line.chars().take(2000).collect::<String>());
}

#[tauri::command(async)]
pub fn open_logs() -> Result<()> {
    library::open_path(&paths::logs())
}

#[tauri::command]
pub async fn scan_launchers(folder: Option<String>) -> Vec<migrate::Found> {
    migrate::scan(&net::client(), folder).await
}

#[tauri::command]
pub async fn import_from_launcher(app: AppHandle, choice: migrate::Choice, folder: Option<String>) -> Result<Instance> {
    migrate::import(&net::client(), choice, folder, &progress_emitter(&app)).await
}

#[tauri::command]
pub fn stop_instance(running: State<'_, Running>, id: String) {
    if let Some(tx) = running.0.lock().unwrap().remove(&id) {
        let _ = tx.send(());
    }
}

#[tauri::command(async)]
pub fn console_history(instance_id: String) -> Result<Vec<console::Line>> {
    instance::check_id(&instance_id)?;
    Ok(console::history(&instance_id))
}

#[tauri::command]
pub fn open_console(app: AppHandle, instance_id: String) -> Result<()> {
    if !settings::load().live_console {
        return Err(Error::msg("The live console is turned off in Settings"));
    }
    let inst = instance::get(&instance_id)?;
    console::show(&app, &inst);
    Ok(())
}

#[tauri::command]
pub async fn apply_crash_fix(app: AppHandle, instance_id: String) -> Result<()> {
    crash::apply(&net::client(), &instance_id, &progress_emitter(&app)).await
}

#[tauri::command(async)]
pub fn open_crash_report(instance_id: String) -> Result<()> {
    crash::open_report(&instance_id)
}

#[tauri::command(async)]
pub fn list_servers(instance_id: Option<String>) -> Result<Vec<servers::Server>> {
    servers::list(instance_id.as_deref())
}

fn ensure_closed(running: &Running, ids: &[String]) -> Result<()> {
    let map = running.0.lock().unwrap();
    if ids.iter().any(|id| map.contains_key(id)) {
        return Err(Error::msg("Close the game before editing its servers"));
    }
    Ok(())
}

#[tauri::command(async)]
pub fn add_server(running: State<'_, Running>, instance_ids: Vec<String>, name: String, address: String) -> Result<()> {
    ensure_closed(&running, &instance_ids)?;
    servers::add(&instance_ids, &name, &address)
}

#[tauri::command(async)]
pub fn remove_server(running: State<'_, Running>, instance_ids: Vec<String>, address: String) -> Result<()> {
    ensure_closed(&running, &instance_ids)?;
    servers::remove(&instance_ids, &address)
}

#[tauri::command]
pub async fn ping_server(address: String) -> Result<servers::Status> {
    servers::ping(&address).await
}

#[tauri::command(async)]
pub fn recent_worlds(limit: usize) -> Vec<servers::RecentWorld> {
    servers::recent_worlds(limit.min(24))
}

#[tauri::command]
pub fn take_cli_request() -> Option<cli::Request> {
    cli::take()
}

#[tauri::command]
pub async fn create_shortcut(instance_id: String) -> Result<()> {
    let inst = instance::get(&instance_id)?;
    shortcut::create(&net::client(), &inst).await
}

#[tauri::command]
pub async fn export_mrpack(app: AppHandle, instance_id: String, options: share::ExportOptions) -> Result<share::Exported> {
    let inst = instance::get(&instance_id)?;
    share::export(&net::client(), &inst, options, &progress_emitter(&app)).await
}

#[tauri::command]
pub async fn share_code(app: AppHandle, instance_id: String, server: Option<String>) -> Result<share::ShareCode> {
    let inst = instance::get(&instance_id)?;
    share::share_code(&net::client(), &inst, server.as_deref(), &progress_emitter(&app)).await
}

#[tauri::command]
pub async fn import_share_code(app: AppHandle, code: String) -> Result<share::Imported> {
    net::cancellable(share::import_code(&net::client(), &code, &progress_emitter(&app))).await
}

#[tauri::command]
pub async fn import_modrinth_link(app: AppHandle, link: String) -> Result<share::Imported> {
    net::cancellable(share::import_link(&net::client(), &link, &progress_emitter(&app))).await
}

#[tauri::command]
pub async fn import_mrpack_file(app: AppHandle, path: String) -> Result<share::Imported> {
    net::cancellable(share::import_file(&net::client(), std::path::Path::new(&path), &progress_emitter(&app))).await
}

#[tauri::command]
pub async fn import_mrpack_bytes(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<share::Imported> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(Error::msg("Choose a .mrpack file"));
    };
    net::cancellable(share::import_bytes(&net::client(), bytes, &progress_emitter(&app))).await
}

#[tauri::command(async)]
pub fn reveal_file(path: String) -> Result<()> {
    let p = std::path::Path::new(&path);
    if !p.is_file() || !p.extension().is_some_and(|e| e == "mrpack") {
        return Err(Error::msg("File not found"));
    }
    library::open_path(p.parent().ok_or_else(|| Error::msg("File not found"))?)
}

#[tauri::command]
pub fn running_instances(running: State<'_, Running>) -> Vec<String> {
    running.0.lock().unwrap().keys().cloned().collect()
}

#[derive(Serialize, Clone)]
struct InstallProgress<'a> {
    stage: &'a str,
    done: u32,
    total: u32,
}

fn progress_emitter(app: &AppHandle) -> impl Fn(&str, u32, u32) + Sync + '_ {
    move |stage, done, total| {
        let _ = app.emit("install-progress", InstallProgress { stage, done, total });
    }
}

#[tauri::command]
pub async fn modrinth_search(
    query: String,
    project_type: String,
    mc_version: Option<String>,
    loaders: Vec<String>,
    categories: Vec<String>,
    sort: String,
    offset: u32,
) -> Result<SearchResult> {
    modrinth::search(&net::client(), &query, &project_type, mc_version.as_deref(), &loaders, &categories, &sort, offset).await
}

#[tauri::command]
pub async fn modrinth_categories(project_type: String) -> Result<Vec<String>> {
    modrinth::categories(&net::client(), &project_type).await
}

#[tauri::command]
pub async fn modpack_details(project_id: String, version_id: Option<String>) -> Result<modrinth::PackDetails> {
    modrinth::modpack_details(&net::client(), &project_id, version_id.as_deref()).await
}

#[tauri::command]
pub async fn project_page(project_id: String) -> Result<modrinth::ProjectPage> {
    modrinth::project_page(&net::client(), &project_id).await
}

#[tauri::command(async)]
pub fn open_url(url: String) -> Result<()> {
    library::open_url(&url)
}

#[tauri::command]
pub async fn install_modpack(app: AppHandle, project_id: String, version_id: Option<String>, mc_version: Option<String>) -> Result<Instance> {
    net::cancellable(modrinth::install_modpack(&net::client(), &project_id, version_id.as_deref(), mc_version.as_deref(), &progress_emitter(&app))).await
}

#[tauri::command(async)]
pub fn all_worlds() -> Vec<crate::worlds::WorldEntry> {
    crate::worlds::all()
}

fn closed_for_world(running: &Running, ids: &[&str]) -> Result<()> {
    let map = running.0.lock().unwrap();
    if ids.iter().any(|id| map.contains_key(*id)) {
        return Err(Error::msg("Close the game first"));
    }
    Ok(())
}

#[tauri::command(async)]
pub fn transfer_world(running: State<'_, Running>, from_instance: String, folder: String, to_instance: String, remove_source: bool) -> Result<String> {
    closed_for_world(&running, &[&from_instance, &to_instance])?;
    crate::worlds::transfer(&from_instance, &folder, &to_instance, remove_source)
}

#[tauri::command(async)]
pub fn export_world(running: State<'_, Running>, instance_id: String, folder: String) -> Result<String> {
    closed_for_world(&running, &[&instance_id])?;
    crate::worlds::export(&instance_id, &folder).map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn import_world_bytes(running: State<'_, Running>, request: tauri::ipc::Request<'_>) -> Result<String> {
    let instance_id = request.headers().get("x-instance").and_then(|v| v.to_str().ok()).unwrap_or_default().to_string();
    closed_for_world(&running, &[&instance_id])?;
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(Error::msg("This file isn't a Minecraft world"));
    };
    let bytes = bytes.clone();
    tokio::task::spawn_blocking(move || crate::worlds::import_bytes(&instance_id, &bytes))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
}

#[tauri::command]
pub async fn sponsored_servers() -> Vec<crate::sponsored::Sponsored> {
    crate::sponsored::list(&net::client()).await
}

#[tauri::command(async)]
pub fn crash_history() -> Vec<crate::crash::CrashEntry> {
    crate::crash::history()
}

#[tauri::command(async)]
pub fn startup_report(instance_id: String) -> crate::startup::StartupReport {
    crate::startup::report(&instance_id)
}

#[tauri::command(async)]
pub fn gpu_info() -> Option<crate::gpu::Dedicated> {
    crate::gpu::dedicated()
}

#[tauri::command]
pub fn cancel_install() -> bool {
    net::cancel()
}

#[tauri::command]
pub async fn modpack_update(instance_id: String) -> Result<Option<modrinth::PackUpdate>> {
    let inst = instance::get(&instance_id)?;
    modrinth::pack_update(&net::client(), &inst).await
}

#[tauri::command]
pub async fn modpack_diff(instance_id: String, version_id: String) -> Result<modrinth::PackDiff> {
    let inst = instance::get(&instance_id)?;
    modrinth::pack_diff(&net::client(), &inst, &version_id).await
}

#[tauri::command]
pub async fn modpack_updates() -> std::collections::HashMap<String, modrinth::PackUpdate> {
    use futures_util::stream::{self, StreamExt};
    let client = net::client();
    let linked: Vec<Instance> = instance::list().into_iter().filter(|i| i.pack.is_some()).collect();
    stream::iter(linked)
        .map(|inst| {
            let client = client.clone();
            async move { modrinth::pack_update(&client, &inst).await.ok().flatten().map(|u| (inst.id, u)) }
        })
        .buffer_unordered(4)
        .filter_map(|found| async move { found })
        .collect()
        .await
}

#[tauri::command]
pub async fn update_modpack(app: AppHandle, running: State<'_, Running>, instance_id: String, version_id: Option<String>) -> Result<Instance> {
    not_running(&running, &instance_id)?;
    let inst = instance::get(&instance_id)?;
    backup_before_change(&app, &running, &inst.id, Content::Mods).await;
    net::cancellable(modrinth::update_modpack(&net::client(), &inst, version_id.as_deref(), &progress_emitter(&app))).await
}

#[tauri::command]
pub async fn install_content(app: AppHandle, running: State<'_, Running>, instance_id: String, project_id: String, kind: Content, version_id: Option<String>) -> Result<Vec<String>> {
    let inst = instance::get(&instance_id)?;
    backup_before_change(&app, &running, &inst.id, kind).await;
    net::cancellable(modrinth::install_content(&net::client(), &inst, &project_id, kind, version_id.as_deref(), &progress_emitter(&app))).await
}

#[tauri::command(async)]
pub fn list_content(instance_id: String, kind: Content) -> Result<Vec<ModFile>> {
    modrinth::list_content(&instance_id, kind)
}

#[tauri::command]
pub async fn content_details(instance_id: String, kind: Content) -> Result<Vec<modrinth::ContentEntry>> {
    let inst = instance::get(&instance_id)?;
    modrinth::content_details(&net::client(), &inst, kind).await
}

#[tauri::command]
pub async fn installed_projects(instance_id: String, kind: Content, updates: bool) -> Result<Vec<modrinth::InstalledProject>> {
    let inst = instance::get(&instance_id)?;
    modrinth::installed_projects(&net::client(), &inst, kind, updates).await
}

#[tauri::command]
pub async fn update_content(app: AppHandle, running: State<'_, Running>, instance_id: String, kind: Content, file_name: String) -> Result<String> {
    let inst = instance::get(&instance_id)?;
    backup_before_change(&app, &running, &inst.id, kind).await;
    net::cancellable(modrinth::update_content(&net::client(), &inst, kind, &file_name)).await
}

#[tauri::command(async)]
pub fn transfer_content(from_id: String, to_id: String, kind: Content, files: Vec<String>, remove_source: bool) -> Result<modrinth::TransferResult> {
    modrinth::transfer_content(&from_id, &to_id, kind, &files, remove_source)
}

#[tauri::command(async)]
pub fn import_content(instance_id: String, kind: Content, files: Vec<String>) -> Result<modrinth::TransferResult> {
    modrinth::import_content(&instance_id, kind, &files)
}

#[tauri::command(async)]
pub fn remove_content(running: State<'_, Running>, instance_id: String, kind: Content, file_name: String) -> Result<()> {
    if kind == Content::Mods && !running.0.lock().unwrap().contains_key(&instance_id) {
        backup::auto(&instance_id, "update", &|_, _| {});
    }
    modrinth::remove_content(&instance_id, kind, &file_name)
}

async fn backup_before_change(app: &AppHandle, running: &State<'_, Running>, instance_id: &str, kind: Content) {
    if kind != Content::Mods || running.0.lock().unwrap().contains_key(instance_id) {
        return;
    }
    let id = instance_id.to_string();
    let app = app.clone();
    let _ = tokio::task::spawn_blocking(move || {
        let emit = progress_emitter(&app);
        backup::auto(&id, "update", &|done, total| emit("Backing up worlds", done, total))
    })
    .await;
}

fn not_running(running: &State<'_, Running>, instance_id: &str) -> Result<()> {
    if running.0.lock().unwrap().contains_key(instance_id) {
        return Err(Error::msg("Close the game first"));
    }
    Ok(())
}

#[tauri::command(async)]
pub fn list_backups(instance_id: String) -> Result<Vec<backup::Backup>> {
    backup::list(&instance_id)
}

#[tauri::command(async)]
pub fn backup_world(running: State<'_, Running>, instance_id: String, world: String) -> Result<backup::Backup> {
    not_running(&running, &instance_id)?;
    backup::create(&instance_id, &world, "manual")
}

#[tauri::command(async)]
pub fn restore_backup(running: State<'_, Running>, instance_id: String, world: String, id: String, as_copy: bool) -> Result<String> {
    not_running(&running, &instance_id)?;
    backup::restore(&instance_id, &world, &id, as_copy)
}

#[tauri::command(async)]
pub fn delete_backup(instance_id: String, world: String, id: String) -> Result<()> {
    backup::delete(&instance_id, &world, &id)
}

#[tauri::command(async)]
pub fn open_backups(instance_id: String) -> Result<()> {
    library::open_path(&backup::folder(&instance_id)?)
}

#[tauri::command]
pub fn set_mod_enabled(instance_id: String, file_name: String, enabled: bool) -> Result<()> {
    modrinth::set_mod_enabled(&instance_id, &file_name, enabled)
}

#[tauri::command]
pub async fn list_worlds(instance_id: String) -> Result<Vec<library::World>> {
    tokio::task::spawn_blocking(move || library::list_worlds(&instance_id))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
}

#[tauri::command(async)]
pub fn open_folder(instance_id: String, which: String) -> Result<()> {
    library::open_folder(&instance_id, &which)
}

#[tauri::command]
pub async fn duplicate_instance(running: State<'_, Running>, instance_id: String, name: Option<String>) -> Result<Instance> {
    if running.0.lock().unwrap().contains_key(&instance_id) {
        return Err(Error::msg("Close the game before duplicating the instance"));
    }
    tokio::task::spawn_blocking(move || library::duplicate(&instance_id, name.as_deref()))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
}

#[tauri::command(async)]
pub fn sessions() -> Vec<Session> {
    stats::load()
}

#[tauri::command(async)]
pub fn installed_versions() -> Vec<String> {
    std::fs::read_dir(crate::paths::versions())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            e.path().join(format!("{id}.jar")).is_file().then_some(id)
        })
        .collect()
}

#[tauri::command]
pub async fn minecraft_news() -> Result<Vec<NewsItem>> {
    news::latest(&net::client()).await
}

#[tauri::command]
pub fn get_settings() -> settings::Settings {
    settings::load()
}

#[tauri::command]
pub fn set_language(code: String) -> Result<()> {
    settings::set_language(&code)
}

#[tauri::command]
pub fn save_settings(settings: settings::Settings) -> Result<()> {
    let turned_on = settings.sync_options && !settings::load().sync_options;
    let last = settings.last_instance.clone();
    settings::save(settings)?;
    if turned_on {
        if let Some(inst) = last.and_then(|id| instance::get(&id).ok()) {
            crate::options_sync::seed(&inst);
        }
    }
    presence::refresh();
    Ok(())
}

#[tauri::command]
pub async fn storage_info() -> Result<storage::Storage> {
    tokio::task::spawn_blocking(storage::info).await.map_err(|e| Error::msg(e.to_string()))
}

#[tauri::command(async)]
pub fn delete_runtime(name: String) -> Result<()> {
    storage::delete_runtime(&name)
}

#[tauri::command(async)]
pub fn open_data_dir() -> Result<()> {
    library::open_root()
}

#[tauri::command(async)]
pub fn reset_data(running: State<'_, Running>, scope: Vec<String>) -> Result<()> {
    if !running.0.lock().unwrap().is_empty() {
        return Err(Error::msg("Close the game before resetting the launcher"));
    }
    storage::reset(&scope)
}

#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart()
}

#[tauri::command]
pub async fn version_images() -> Result<std::collections::HashMap<String, String>> {
    news::version_images(&net::client()).await
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Option<update::UpdateInfo> {
    if !settings::load().check_updates {
        return None;
    }
    update::check_app(&app, &net::client()).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<()> {
    update::install(&app).await
}

#[tauri::command]
pub async fn thumbnail(url: String, width: u32) -> Result<String> {
    thumb::get(&net::client(), &url, width).await
}

#[tauri::command]
pub async fn fetch_skin(name: String) -> Result<Option<skin::SkinInfo>> {
    skin::fetch(&net::client(), &name).await
}

#[tauri::command]
pub async fn wardrobe_load(account_id: String) -> Result<wardrobe::Wardrobe> {
    wardrobe::load(&account_id).await
}

#[tauri::command]
pub async fn wardrobe_set_skin(account_id: String, png: String, slim: bool) -> Result<()> {
    wardrobe::set_skin(&account_id, &png, slim).await
}

#[tauri::command]
pub async fn wardrobe_set_cape(account_id: String, cape_id: Option<String>) -> Result<()> {
    wardrobe::set_cape(&account_id, cape_id.as_deref()).await
}

#[tauri::command]
pub fn bench_info() -> Option<String> {
    std::env::var("PIUMA_BENCH_OUT").ok()
}

#[tauri::command]
pub fn bench_done(report: String) {
    if let Ok(path) = std::env::var("PIUMA_BENCH_OUT") {
        let _ = std::fs::write(path, report);
    }
    std::process::exit(0)
}
