use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::oneshot;

use crate::accounts::{self, Account, AccountStore};
use crate::console;
use crate::cli;
use crate::crash;
use crate::shortcut;
use crate::share;
use crate::servers::{self, QuickPlay};
use crate::error::{Error, Result};
use crate::instance::{self, Instance};
use crate::java::{self, JavaInstall};
use crate::runtime;
use crate::microsoft;
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
use crate::{launch, net, Running};

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
            let installed = loaders::install(&net::client(), loader, &mc_version, None, &progress_emitter(&app)).await?;
            (Some(installed.loader_version), Some(installed.id))
        }
    };
    instance::create(&name, &mc_version, loader, loader_version, launch_version)
}

#[tauri::command]
pub fn update_instance(instance: Instance) -> Result<()> {
    instance::get(&instance.id)?;
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
pub fn launch_instance(app: AppHandle, running: State<'_, Running>, id: String, quick_play: Option<QuickPlay>) -> Result<()> {
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
    tauri::async_runtime::spawn(launch::run(app, inst, account, quick_play, rx));
    Ok(())
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
pub async fn share_code(app: AppHandle, instance_id: String) -> Result<share::ShareCode> {
    let inst = instance::get(&instance_id)?;
    share::share_code(&net::client(), &inst, &progress_emitter(&app)).await
}

#[tauri::command]
pub async fn import_share_code(app: AppHandle, code: String) -> Result<share::Imported> {
    share::import_code(&net::client(), &code, &progress_emitter(&app)).await
}

#[tauri::command]
pub async fn import_modrinth_link(app: AppHandle, link: String) -> Result<share::Imported> {
    share::import_link(&net::client(), &link, &progress_emitter(&app)).await
}

#[tauri::command]
pub async fn import_mrpack_file(app: AppHandle, path: String) -> Result<share::Imported> {
    share::import_file(&net::client(), std::path::Path::new(&path), &progress_emitter(&app)).await
}

#[tauri::command]
pub async fn import_mrpack_bytes(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<share::Imported> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(Error::msg("Choose a .mrpack file"));
    };
    share::import_bytes(&net::client(), bytes, &progress_emitter(&app)).await
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
    modrinth::install_modpack(&net::client(), &project_id, version_id.as_deref(), mc_version.as_deref(), &progress_emitter(&app)).await
}

#[tauri::command]
pub async fn install_content(app: AppHandle, instance_id: String, project_id: String, kind: Content, version_id: Option<String>) -> Result<Vec<String>> {
    let inst = instance::get(&instance_id)?;
    modrinth::install_content(&net::client(), &inst, &project_id, kind, version_id.as_deref(), &progress_emitter(&app)).await
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
pub async fn update_content(instance_id: String, kind: Content, file_name: String) -> Result<String> {
    let inst = instance::get(&instance_id)?;
    modrinth::update_content(&net::client(), &inst, kind, &file_name).await
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
pub fn remove_content(instance_id: String, kind: Content, file_name: String) -> Result<()> {
    modrinth::remove_content(&instance_id, kind, &file_name)
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
pub async fn duplicate_instance(running: State<'_, Running>, instance_id: String) -> Result<Instance> {
    if running.0.lock().unwrap().contains_key(&instance_id) {
        return Err(Error::msg("Close the game before duplicating the instance"));
    }
    tokio::task::spawn_blocking(move || library::duplicate(&instance_id))
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
    settings::save(settings)?;
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
pub async fn check_update() -> Option<update::UpdateInfo> {
    if !settings::load().check_updates {
        return None;
    }
    update::check(&net::client()).await
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
