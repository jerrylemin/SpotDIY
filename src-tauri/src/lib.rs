pub mod credentials;
pub mod db;
pub mod domain;
pub mod ipc;
pub mod library;
pub mod media_tools;
pub mod playback;
pub mod queue;
pub mod settings;
pub mod search {
    pub mod sort;
    pub mod types;

    pub use sort::all_provider_kinds_for_lens;
    pub use types::*;
}
pub mod sources;

use db::{standard_database_path, Database};
use ipc::{app_status_with_runtime, source_capabilities, AppStatus, ProviderCapabilities};
use library::{LibraryService, ProgressSink, LIBRARY_PROGRESS_EVENT};
use media_tools::MediaToolManager;
use playback::{
    AudioDevice, PlaybackErrorDto, PlaybackService, PlaybackSnapshot, RepeatMode,
    TrackPlaybackRequest, PLAYBACK_STATE_EVENT,
};
use settings::{SettingValue, SettingsRepository, SettingsSnapshot};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use tauri_plugin_opener::OpenerExt;

use crate::search::{SearchEvent, SearchEventSink, SearchRequest, SearchStarted};
use crate::sources::{
    LocalSourceAdapter, SoundcloudSourceAdapter, SourceAdapter, SpotifySourceAdapter,
    YoutubeSourceAdapter,
};

pub const SEARCH_PROVIDER_UPDATE_EVENT: &str = "search://provider-update";
pub const SEARCH_COMPLETED_EVENT: &str = "search://complete";
pub const SPOTIFY_AUTH_STATE_EVENT: &str = "spotify://auth-state";

struct AppState {
    database: Database,
    library: LibraryService,
    media_tools: MediaToolManager,
    playback: PlaybackService,
    search: search::SearchService,
    spotify_auth: sources::spotify::SpotifyAuthService,
}

#[tauri::command]
fn get_app_status(state: State<'_, AppState>) -> Result<AppStatus, String> {
    app_status_with_runtime(
        env!("CARGO_PKG_VERSION"),
        &state.database,
        &state.media_tools,
        &state.spotify_auth,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_source_capabilities() -> Vec<ProviderCapabilities> {
    source_capabilities()
}

#[tauri::command]
fn start_search(
    request: SearchRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SearchStarted, String> {
    let sink = search_event_sink(&app);
    state
        .search
        .start_search(request, sink)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_search(state: State<'_, AppState>) -> Option<crate::search::SearchId> {
    state.search.cancel_search()
}

#[tauri::command]
fn get_spotify_setup_status(state: State<'_, AppState>) -> sources::spotify::SpotifySetupStatus {
    state.spotify_auth.setup_status()
}

#[tauri::command]
async fn begin_spotify_authorization(
    client_id: String,
    market: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<sources::spotify::SpotifyAuthorizationRequest, String> {
    let request = state
        .spotify_auth
        .begin_authorization(client_id, &market)
        .await
        .map_err(|error| error.to_string())?;
    app.opener()
        .open_url(request.authorization_url.clone(), None::<&str>)
        .map_err(|error| error.to_string())?;
    let auth = state.spotify_auth.clone();
    let app_handle = app.clone();
    tokio::spawn(async move {
        let status = match auth.complete_authorization().await {
            Ok(status) => status,
            Err(_) => auth.setup_status(),
        };
        let _ = app_handle.emit(SPOTIFY_AUTH_STATE_EVENT, status);
    });
    Ok(request)
}

#[tauri::command]
fn disconnect_spotify(
    state: State<'_, AppState>,
) -> Result<sources::spotify::SpotifySetupStatus, String> {
    state
        .spotify_auth
        .disconnect()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_provider_result(
    provider: crate::domain::ProviderKind,
    url: String,
    app: AppHandle,
) -> Result<(), String> {
    let safe = sources::validate_provider_url(provider, &url).map_err(|error| error.to_string())?;
    app.opener()
        .open_url(safe.as_url().as_str().to_owned(), None::<&str>)
        .map_err(|error| error.to_string())
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

#[tauri::command]
fn get_playback_snapshot(state: State<'_, AppState>) -> PlaybackSnapshot {
    state.playback.snapshot()
}

#[tauri::command]
fn play_track(
    track_id: crate::domain::TrackId,
    source_id: Option<crate::domain::SourceId>,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .play_track(TrackPlaybackRequest {
            track_id,
            source_id,
        })
        .map_err(|error| error.dto())
}

#[tauri::command]
fn enqueue_track(
    track_id: crate::domain::TrackId,
    source_id: Option<crate::domain::SourceId>,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .enqueue_track(TrackPlaybackRequest {
            track_id,
            source_id,
        })
        .map_err(|error| error.dto())
}

#[tauri::command]
fn play_track_next(
    track_id: crate::domain::TrackId,
    source_id: Option<crate::domain::SourceId>,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .play_track_next(TrackPlaybackRequest {
            track_id,
            source_id,
        })
        .map_err(|error| error.dto())
}

#[tauri::command]
fn toggle_play_pause(state: State<'_, AppState>) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .toggle_play_pause()
        .map_err(|error| error.dto())
}

#[tauri::command]
fn seek_playback(
    position_ms: u64,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .seek_playback(position_ms)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn next_track(state: State<'_, AppState>) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state.playback.next_track().map_err(|error| error.dto())
}

#[tauri::command]
fn previous_track(state: State<'_, AppState>) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state.playback.previous_track().map_err(|error| error.dto())
}

#[tauri::command]
fn set_playback_volume(
    volume_percent: u8,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .set_playback_volume(volume_percent)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn set_playback_muted(
    muted: bool,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .set_playback_muted(muted)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn set_repeat_mode(
    repeat_mode: RepeatMode,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .set_repeat_mode(repeat_mode)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn set_shuffle_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .set_shuffle_enabled(enabled)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn get_audio_devices(state: State<'_, AppState>) -> Result<Vec<AudioDevice>, PlaybackErrorDto> {
    state
        .playback
        .get_audio_devices()
        .map_err(|error| error.dto())
}

#[tauri::command]
fn set_audio_device(
    name: String,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .set_audio_device(name)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn switch_playback_source(
    track_id: crate::domain::TrackId,
    source_id: crate::domain::SourceId,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .switch_playback_source(TrackPlaybackRequest {
            track_id,
            source_id: Some(source_id),
        })
        .map_err(|error| error.dto())
}

#[tauri::command]
fn retry_playback_backend(
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    let _ = state.media_tools.refresh_mpv();
    state
        .playback
        .retry_playback_backend()
        .map_err(|error| error.dto())
}

#[tauri::command]
fn clear_playback_queue(state: State<'_, AppState>) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .clear_playback_queue()
        .map_err(|error| error.dto())
}

fn progress_sink(app: &AppHandle) -> ProgressSink {
    let app = app.clone();
    Arc::new(move |progress| {
        let _ = app.emit(LIBRARY_PROGRESS_EVENT, progress);
    })
}

fn search_event_sink(app: &AppHandle) -> SearchEventSink {
    let app = app.clone();
    Arc::new(move |event| match event {
        SearchEvent::ProviderSection(event) => {
            let _ = app.emit(SEARCH_PROVIDER_UPDATE_EVENT, event);
        }
        SearchEvent::Completed(event) => {
            let _ = app.emit(SEARCH_COMPLETED_EVENT, event);
        }
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let local_data_root =
                if std::env::var("SPOTDIY_PACKAGED_SMOKE").ok().as_deref() == Some("1") {
                    std::env::var_os("SPOTDIY_PACKAGED_DATA_ROOT")
                        .map(PathBuf::from)
                        .unwrap_or(app.path().local_data_dir()?)
                } else {
                    app.path().local_data_dir()?
                };
            let database = Database::open(standard_database_path(&local_data_root))?;
            let artwork_root = local_data_root
                .join(db::APPLICATION_DATA_DIRECTORY)
                .join("cache")
                .join("artwork");
            let library = LibraryService::new(database.clone(), artwork_root)?;
            let sink = Some(progress_sink(app.handle()));
            let media_tools = MediaToolManager::new();
            let spotify_auth = sources::spotify::SpotifyAuthService::production();
            let adapters: Vec<Arc<dyn SourceAdapter>> = vec![
                Arc::new(LocalSourceAdapter::new(database.clone())),
                Arc::new(YoutubeSourceAdapter::new(media_tools.clone())),
                Arc::new(SoundcloudSourceAdapter::new(media_tools.clone())),
                Arc::new(SpotifySourceAdapter::new(spotify_auth.clone())),
            ];
            let search = search::SearchService::new(adapters);
            let playback_sink = {
                let app_handle = app.handle().clone();
                Arc::new(move |snapshot: PlaybackSnapshot| {
                    let _ = app_handle.emit(PLAYBACK_STATE_EVENT, snapshot);
                })
            };
            let playback =
                PlaybackService::new(library.clone(), media_tools.clone(), playback_sink);
            library.register_watchers(sink.clone())?;
            app.manage(AppState {
                database,
                library: library.clone(),
                media_tools,
                playback,
                search,
                spotify_auth,
            });
            library.start_all_scans(sink)?;
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_app_status,
            get_source_capabilities,
            start_search,
            cancel_search,
            get_spotify_setup_status,
            begin_spotify_authorization,
            disconnect_spotify,
            open_provider_result,
            get_settings_snapshot,
            set_setting,
            get_library_folders,
            add_library_folders,
            remove_library_folder,
            get_library_status,
            rescan_library_folder,
            rescan_all_library_folders,
            get_library_page,
            reveal_local_file,
            get_playback_snapshot,
            play_track,
            enqueue_track,
            play_track_next,
            toggle_play_pause,
            seek_playback,
            next_track,
            previous_track,
            set_playback_volume,
            set_playback_muted,
            set_repeat_mode,
            set_shuffle_enabled,
            get_audio_devices,
            set_audio_device,
            switch_playback_source,
            retry_playback_backend,
            clear_playback_queue
        ])
        .build(tauri::generate_context!())
        .expect("error while building SpotDIY")
        .run(|app_handle, event| {
            if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    let _ = state.search.cancel_search();
                    let _ = state.playback.shutdown();
                }
            }
        });
}
