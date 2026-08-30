mod domain;
mod ipc;

use ipc::{app_status, source_capabilities, AppStatus, ProviderCapabilities};

#[tauri::command]
fn get_app_status() -> AppStatus {
    app_status(env!("CARGO_PKG_VERSION"))
}

#[tauri::command]
fn get_source_capabilities() -> Vec<ProviderCapabilities> {
    source_capabilities()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_app_status,
            get_source_capabilities
        ])
        .run(tauri::generate_context!())
        .expect("error while running SpotDIY");
}
