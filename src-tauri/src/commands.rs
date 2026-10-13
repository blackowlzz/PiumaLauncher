use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::oneshot;

use crate::accounts::{self, Account, AccountStore};
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

#[tauri::command]
pub fn list_instances() -> Vec<Instance> {
    instance::list()
}

#[tauri::command]
pub async fn create_instance(name: String, mc_version: String, loader: Loader) -> Result<Instance> {
    if name.trim().is_empty() {
        return Err(Error::msg("Give the instance a name"));
    }
    let (loader_version, launch_version) = match loader {
        Loader::Vanilla => (None, None),
        _ => {
            let installed = loaders::install(&net::client(), loader, &mc_version, None, &|_, _, _| {}).await?;
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

#[tauri::command]
pub fn delete_instance(running: State<'_, Running>, id: String) -> Result<()> {
    if running.0.lock().unwrap().contains_key(&id) {
        return Err(Error::msg("Close the game before deleting the instance"));
    }
    instance::delete(&id)
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

#[tauri::command]
pub fn system_info() -> SystemInfo {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    SystemInfo {
        total_memory_mb: sys.total_memory() / 1_048_576,
        cpu_cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
    }
}

#[tauri::command]
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
pub fn launch_instance(app: AppHandle, running: State<'_, Running>, id: String) -> Result<()> {
    let inst = instance::get(&id)?;
    let account = accounts::active().ok_or_else(|| Error::msg("Create or select an account before playing"))?;

    let (tx, rx) = oneshot::channel();
    {
        let mut map = running.0.lock().unwrap();
        if map.contains_key(&id) {
            return Err(Error::msg("The instance is already running"));
        }
        map.insert(id, tx);
    }
    tauri::async_runtime::spawn(launch::run(app, inst, account, rx));
    Ok(())
}

#[tauri::command]
pub fn stop_instance(running: State<'_, Running>, id: String) {
    if let Some(tx) = running.0.lock().unwrap().remove(&id) {
        let _ = tx.send(());
    }
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

#[tauri::command]
pub fn open_url(url: String) -> Result<()> {
    library::open_url(&url)
}

#[tauri::command]
pub async fn install_modpack(app: AppHandle, project_id: String, version_id: Option<String>, mc_version: Option<String>) -> Result<Instance> {
    modrinth::install_modpack(&net::client(), &project_id, version_id.as_deref(), mc_version.as_deref(), &progress_emitter(&app)).await
}

#[tauri::command]
pub async fn install_content(app: AppHandle, instance_id: String, project_id: String, kind: Content) -> Result<Vec<String>> {
    let inst = instance::get(&instance_id)?;
    modrinth::install_content(&net::client(), &inst, &project_id, kind, &progress_emitter(&app)).await
}

#[tauri::command]
pub fn list_content(instance_id: String, kind: Content) -> Result<Vec<ModFile>> {
    modrinth::list_content(&instance_id, kind)
}

#[tauri::command]
pub async fn content_details(instance_id: String, kind: Content) -> Result<Vec<modrinth::ContentEntry>> {
    let inst = instance::get(&instance_id)?;
    modrinth::content_details(&net::client(), &inst, kind).await
}

#[tauri::command]
pub async fn update_content(instance_id: String, kind: Content, file_name: String) -> Result<String> {
    let inst = instance::get(&instance_id)?;
    modrinth::update_content(&net::client(), &inst, kind, &file_name).await
}

#[tauri::command]
pub fn transfer_content(from_id: String, to_id: String, kind: Content, files: Vec<String>, remove_source: bool) -> Result<modrinth::TransferResult> {
    modrinth::transfer_content(&from_id, &to_id, kind, &files, remove_source)
}

#[tauri::command]
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

#[tauri::command]
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

#[tauri::command]
pub fn sessions() -> Vec<Session> {
    stats::load()
}

#[tauri::command]
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

#[tauri::command]
pub fn delete_runtime(name: String) -> Result<()> {
    storage::delete_runtime(&name)
}

#[tauri::command]
pub fn open_data_dir() -> Result<()> {
    library::open_root()
}

#[tauri::command]
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
