pub mod credentials;
pub mod db;
pub mod domain;
pub mod downloads;
pub mod fusion;
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
use downloads::{DownloadMode, DownloadService, DownloadSnapshot, DownloadTask, DownloadTaskId};
use fusion::{FusionEvaluation, FusionOverride, FusionOverrideDecision, SourceFusionService};
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

use crate::db::repository::TrackRepository;
use crate::domain::{ProviderKind, TrackId};
use crate::search::{SearchEvent, SearchEventSink, SearchRequest, SearchResult, SearchStarted};
use crate::sources::{
    LocalSourceAdapter, SoundcloudSourceAdapter, SourceAdapter, SourceResolution, SourceResolver,
    SpotifySourceAdapter, YoutubeSourceAdapter,
};

pub const SEARCH_PROVIDER_UPDATE_EVENT: &str = "search://provider-update";
pub const SEARCH_COMPLETED_EVENT: &str = "search://complete";
pub const SPOTIFY_AUTH_STATE_EVENT: &str = "spotify://auth-state";

struct AppState {
    database: Database,
    library: LibraryService,
    media_tools: MediaToolManager,
    downloads: DownloadService,
    playback: PlaybackService,
    search: search::SearchService,
    spotify_auth: sources::spotify::SpotifyAuthService,
    fusion: SourceFusionService,
    source_resolver: SourceResolver,
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
fn evaluate_fusion_candidate(
    candidate: SearchResult,
    target_track_id: TrackId,
    state: State<'_, AppState>,
) -> Result<FusionEvaluation, String> {
    state
        .fusion
        .evaluate_candidate(&candidate, target_track_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn accept_fusion_candidate(
    candidate: SearchResult,
    target_track_id: TrackId,
    state: State<'_, AppState>,
) -> Result<FusionEvaluation, String> {
    state
        .fusion
        .accept_match(&candidate, target_track_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_fusion_override(
    provider_kind: ProviderKind,
    provider_item_id: String,
    target_track_id: TrackId,
    decision: FusionOverrideDecision,
    state: State<'_, AppState>,
) -> Result<FusionOverride, String> {
    state
        .fusion
        .set_override(provider_kind, provider_item_id, target_track_id, decision)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn clear_fusion_override(
    provider_kind: ProviderKind,
    provider_item_id: String,
    target_track_id: TrackId,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .fusion
        .clear_override(provider_kind, &provider_item_id, target_track_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_source_resolution(
    track_id: TrackId,
    state: State<'_, AppState>,
) -> Result<SourceResolution, String> {
    let track = TrackRepository::new(&state.database)
        .get(track_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("track {track_id} was not found"))?;
    state
        .source_resolver
        .resolve(&track)
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
fn get_download_snapshot(state: State<'_, AppState>) -> Result<DownloadSnapshot, String> {
    state
        .downloads
        .snapshot()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn queue_search_result_download(
    result: SearchResult,
    mode: DownloadMode,
    state: State<'_, AppState>,
) -> Result<DownloadTask, String> {
    state
        .downloads
        .queue_search_result_download(result, mode)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn queue_source_download(
    track_id: TrackId,
    source_id: crate::domain::SourceId,
    mode: DownloadMode,
    state: State<'_, AppState>,
) -> Result<DownloadTask, String> {
    state
        .downloads
        .queue_source_download(track_id, source_id, mode)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_download(
    task_id: DownloadTaskId,
    state: State<'_, AppState>,
) -> Result<DownloadTask, String> {
    state
        .downloads
        .cancel_download(task_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn retry_download(
    task_id: DownloadTaskId,
    state: State<'_, AppState>,
) -> Result<DownloadTask, String> {
    state
        .downloads
        .retry_download(task_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_download_concurrency(
    max_concurrent: u8,
    state: State<'_, AppState>,
) -> Result<DownloadSnapshot, String> {
    state
        .downloads
        .set_download_concurrency(max_concurrent)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_download_location(
    task_id: DownloadTaskId,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let path = state
        .downloads
        .trusted_destination_path(task_id)
        .map_err(|error| error.to_string())?;
    app.opener()
        .reveal_item_in_dir(path)
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

fn download_snapshot_sink(app: &AppHandle) -> downloads::DownloadSnapshotSink {
    let app = app.clone();
    Arc::new(move |snapshot| {
        let _ = app.emit(downloads::DOWNLOAD_STATE_EVENT, snapshot);
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
            let download_cache_root = local_data_root
                .join(db::APPLICATION_DATA_DIRECTORY)
                .join("cache")
                .join("downloads");
            let downloads = DownloadService::with_task_root(
                database.clone(),
                media_tools.clone(),
                download_cache_root,
                Some(download_snapshot_sink(app.handle())),
            )?;
            downloads.start()?;
            let spotify_auth = sources::spotify::SpotifyAuthService::production();
            let adapters: Vec<Arc<dyn SourceAdapter>> = vec![
                Arc::new(LocalSourceAdapter::new(database.clone())),
                Arc::new(YoutubeSourceAdapter::new(media_tools.clone())),
                Arc::new(SoundcloudSourceAdapter::new(media_tools.clone())),
                Arc::new(SpotifySourceAdapter::new(spotify_auth.clone())),
            ];
            let search = search::SearchService::new(adapters);
            let fusion = SourceFusionService::new(database.clone());
            let source_resolver = SourceResolver::new(library.clone());
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
                downloads,
                playback,
                search,
                spotify_auth,
                fusion,
                source_resolver,
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
            evaluate_fusion_candidate,
            accept_fusion_candidate,
            set_fusion_override,
            clear_fusion_override,
            get_source_resolution,
            get_settings_snapshot,
            set_setting,
            get_download_snapshot,
            queue_search_result_download,
            queue_source_download,
            cancel_download,
            retry_download,
            set_download_concurrency,
            open_download_location,
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
            clear_playback_queue,
        ])
        .build(tauri::generate_context!())
        .expect("error while building SpotDIY")
        .run(|app_handle, event| {
            if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    let _ = state.search.cancel_search();
                    let _ = state.downloads.shutdown();
                    let _ = state.playback.shutdown();
                }
            }
        });
}
