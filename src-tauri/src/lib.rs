mod accounts;
mod cli;
mod commands;
mod console;
mod crash;
mod error;
mod flags;
mod install;
mod instance;
mod java;
mod launch;
mod library;
mod loaders;
mod microsoft;
mod minecraft;
mod modrinth;
mod nbt;
mod net;
mod news;
mod optimizer;
mod paths;
mod presence;
mod runtime;
mod servers;
mod settings;
mod share;
mod shortcut;
mod skin;
mod stats;
mod storage;
mod thumb;
mod update;

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::Manager;
use tokio::sync::oneshot;

#[derive(Default)]
pub struct Running(pub Mutex<HashMap<String, oneshot::Sender<()>>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let request = cli::parse(std::env::args().skip(1));
    if cli::forward(&request) {
        return;
    }

    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 2);
    }

    tauri::Builder::default()
        .manage(Running::default())
        .setup(|app| {
            if let Some(w) = app.get_webview_window("main") {
                if let Ok(Some(m)) = w.primary_monitor() {
                    let scale = m.scale_factor();
                    let (sw, sh) = (m.size().width as f64 / scale, m.size().height as f64 / scale);
                    let width = (sw * 0.82).clamp(1100.0, 1560.0).min(sw);
                    let height = (sh * 0.86).clamp(700.0, 980.0).min(sh);
                    let _ = w.set_size(tauri::LogicalSize::new(width, height));
                    let _ = w.center();
                }
            }
            presence::launcher();
            cli::serve(app.handle().clone(), request);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_mc_versions,
            commands::list_instances,
            commands::create_instance,
            commands::update_instance,
            commands::delete_instance,
            commands::list_accounts,
            commands::add_offline_account,
            commands::microsoft_login_start,
            commands::microsoft_login_wait,
            commands::microsoft_login_cancel,
            commands::remove_account,
            commands::set_active_account,
            commands::detect_java,
            commands::system_info,
            commands::recommend_performance,
            commands::list_runtimes,
            commands::install_runtime,
            commands::launch_instance,
            commands::stop_instance,
            commands::running_instances,
            commands::modrinth_search,
            commands::install_modpack,
            commands::modpack_details,
            commands::project_page,
            commands::open_url,
            commands::install_content,
            commands::list_content,
            commands::remove_content,
            commands::transfer_content,
            commands::content_details,
            commands::update_content,
            commands::modrinth_categories,
            commands::sessions,
            commands::installed_versions,
            commands::minecraft_news,
            commands::get_settings,
            commands::fetch_skin,
            commands::thumbnail,
            commands::check_update,
            commands::save_settings,
            commands::storage_info,
            commands::delete_runtime,
            commands::open_data_dir,
            commands::reset_data,
            commands::restart_app,
            commands::version_images,
            commands::bench_info,
            commands::bench_done,
            commands::set_language,
            commands::set_mod_enabled,
            commands::list_worlds,
            commands::open_folder,
            commands::duplicate_instance,
            commands::console_history,
            commands::open_console,
            commands::apply_crash_fix,
            commands::open_crash_report,
            commands::list_servers,
            commands::add_server,
            commands::remove_server,
            commands::ping_server,
            commands::recent_worlds,
            commands::take_cli_request,
            commands::create_shortcut,
            commands::export_mrpack,
            commands::share_code,
            commands::import_share_code,
            commands::import_modrinth_link,
            commands::import_mrpack_file,
            commands::import_mrpack_bytes,
            commands::reveal_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while starting PiumaLauncher");
}
