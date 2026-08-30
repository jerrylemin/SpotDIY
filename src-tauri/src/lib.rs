pub mod db;
pub mod domain;
pub mod ipc;
pub mod settings;

use db::{standard_database_path, Database};
use ipc::{app_status, source_capabilities, AppStatus, ProviderCapabilities};
use settings::{SettingValue, SettingsRepository, SettingsSnapshot};
use tauri::{Manager, State};

struct AppState {
    database: Database,
}

#[tauri::command]
fn get_app_status(state: State<'_, AppState>) -> Result<AppStatus, String> {
    app_status(env!("CARGO_PKG_VERSION"), &state.database).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_source_capabilities() -> Vec<ProviderCapabilities> {
    source_capabilities()
}

#[tauri::command]
fn get_settings_snapshot(state: State<'_, AppState>) -> Result<SettingsSnapshot, String> {
    SettingsRepository::new(&state.database)
        .get_snapshot()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_setting(
    setting: SettingValue,
    state: State<'_, AppState>,
) -> Result<SettingsSnapshot, String> {
    SettingsRepository::new(&state.database)
        .set_setting(setting)
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let local_data_root = app.path().local_data_dir()?;
            let database = Database::open(standard_database_path(local_data_root))?;
            app.manage(AppState { database });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_status,
            get_source_capabilities,
            get_settings_snapshot,
            set_setting
        ])
        .run(tauri::generate_context!())
        .expect("error while running SpotDIY");
}
