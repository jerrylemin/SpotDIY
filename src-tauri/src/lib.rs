pub mod db;
pub mod domain;
pub mod ipc;
pub mod library;
pub mod media_tools;
pub mod playback {
    pub mod backend;
}
pub mod queue;
pub mod settings;

use db::{standard_database_path, Database};
use ipc::{app_status, source_capabilities, AppStatus, ProviderCapabilities};
use library::{LibraryService, ProgressSink, LIBRARY_PROGRESS_EVENT};
use settings::{SettingValue, SettingsRepository, SettingsSnapshot};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    database: Database,
    library: LibraryService,
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

#[tauri::command]
fn get_library_folders(
    state: State<'_, AppState>,
) -> Result<Vec<crate::domain::LibraryFolder>, String> {
    state
        .library
        .list_folders()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn add_library_folders(
    paths: Vec<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<crate::domain::LibraryFolder>, String> {
    state
        .library
        .add_folders_and_start(
            paths.into_iter().map(PathBuf::from).collect(),
            Some(progress_sink(&app)),
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_library_folder(
    folder_id: crate::domain::LibraryFolderId,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .library
        .remove_folder(folder_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_library_status(state: State<'_, AppState>) -> Result<crate::domain::LibraryStatus, String> {
    state.library.status().map_err(|error| error.to_string())
}

#[tauri::command]
fn rescan_library_folder(
    folder_id: crate::domain::LibraryFolderId,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .library
        .rescan_folder(folder_id, Some(progress_sink(&app)))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rescan_all_library_folders(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state
        .library
        .rescan_all(Some(progress_sink(&app)))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_library_page(
    request: crate::domain::LibraryPageRequest,
    state: State<'_, AppState>,
) -> Result<crate::domain::LibraryPage, String> {
    state
        .library
        .page(request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn reveal_local_file(
    source_id: crate::domain::SourceId,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let path = state
        .library
        .reveal_path(source_id)
        .map_err(|error| error.to_string())?;
    tauri_plugin_opener::OpenerExt::opener(&app)
        .reveal_item_in_dir(path)
        .map_err(|error| error.to_string())
}

fn progress_sink(app: &AppHandle) -> ProgressSink {
    let app = app.clone();
    Arc::new(move |progress| {
        let _ = app.emit(LIBRARY_PROGRESS_EVENT, progress);
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let local_data_root = app.path().local_data_dir()?;
            let database = Database::open(standard_database_path(&local_data_root))?;
            let artwork_root = local_data_root
                .join(db::APPLICATION_DATA_DIRECTORY)
                .join("cache")
                .join("artwork");
            let library = LibraryService::new(database.clone(), artwork_root)?;
            let sink = Some(progress_sink(app.handle()));
            library.register_watchers(sink.clone())?;
            app.manage(AppState {
                database,
                library: library.clone(),
            });
            library.start_all_scans(sink)?;
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_app_status,
            get_source_capabilities,
            get_settings_snapshot,
            set_setting,
            get_library_folders,
            add_library_folders,
            remove_library_folder,
            get_library_status,
            rescan_library_folder,
            rescan_all_library_folders,
            get_library_page,
            reveal_local_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running SpotDIY");
}
