pub mod bookmarks;
pub mod credentials;
pub mod db;
pub mod domain;
pub mod downloads;
pub mod fusion;
pub mod inspector;
pub mod ipc;
pub mod library;
pub mod lyrics;
pub mod media_tools;
pub mod playback;
pub mod playlists;
pub mod queue;
pub mod settings;
pub mod windows;
pub mod search {
    pub mod sort;
    pub mod types;

    pub use sort::all_provider_kinds_for_lens;
    pub use types::*;
}
pub mod sources;

use bookmarks::{AbLoopPreset, Bookmark, BookmarkErrorDto, BookmarkService};
use db::{standard_database_path, Database};
use downloads::{DownloadMode, DownloadService, DownloadSnapshot, DownloadTask, DownloadTaskId};
use fusion::{FusionEvaluation, FusionOverride, FusionOverrideDecision, SourceFusionService};
use inspector::{TrackInspector, TrackInspectorService};
use ipc::{app_status_with_runtime, source_capabilities, AppStatus, ProviderCapabilities};
use library::{LibraryService, ProgressSink, LIBRARY_PROGRESS_EVENT};
use lyrics::{LyricsCandidate, LyricsDocument, LyricsErrorDto, LyricsService, ManualLyricsMode};
use media_tools::MediaToolManager;
use playback::{
    AudioDevice, PlaybackErrorDto, PlaybackService, PlaybackSnapshot, QueueSection, RepeatMode,
    TrackPlaybackRequest, PLAYBACK_STATE_EVENT, QUEUE_STATE_EVENT,
};
use playlists::{PlaylistErrorDto, PlaylistService};
use settings::{
    GlobalShortcutBinding, SettingValue, SettingsRepository, SettingsSnapshot,
    WindowsIntegrationSettings,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::db::repository::TrackRepository;
use crate::domain::{ProviderKind, TrackId};
use crate::search::{SearchEvent, SearchEventSink, SearchRequest, SearchResult, SearchStarted};
use crate::sources::{
    LocalSourceAdapter, SoundcloudSourceAdapter, SourceAdapter, SourceResolution, SourceResolver,
    SpotifySourceAdapter, YoutubeSourceAdapter,
};
use crate::windows::{
    GamingClickThroughError, WindowsIntegrationService, WindowsIntegrationSnapshot,
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
    lyrics: LyricsService,
    bookmarks: BookmarkService,
    playlists: PlaylistService,
    search: search::SearchService,
    spotify_auth: sources::spotify::SpotifyAuthService,
    fusion: SourceFusionService,
    source_resolver: SourceResolver,
    inspector: TrackInspectorService,
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
fn get_track_inspector(
    track_id: TrackId,
    state: State<'_, AppState>,
) -> Result<TrackInspector, String> {
    state
        .inspector
        .get_track_inspector(track_id)
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
fn get_windows_integration_snapshot(
    state: State<'_, WindowsIntegrationService>,
) -> WindowsIntegrationSnapshot {
    state.snapshot()
}

#[tauri::command]
fn set_windows_integration_settings(
    settings: WindowsIntegrationSettings,
    state: State<'_, WindowsIntegrationService>,
) -> Result<WindowsIntegrationSnapshot, String> {
    state.set_windows_integration_settings(settings)
}

#[tauri::command]
fn set_global_shortcuts_enabled(
    enabled: bool,
    state: State<'_, WindowsIntegrationService>,
) -> Result<WindowsIntegrationSnapshot, String> {
    state.set_global_shortcuts_enabled(enabled)
}

#[tauri::command]
fn update_global_shortcut(
    binding: GlobalShortcutBinding,
    state: State<'_, WindowsIntegrationService>,
) -> Result<WindowsIntegrationSnapshot, String> {
    state.update_global_shortcut(binding)
}

#[tauri::command]
fn reset_global_shortcuts(
    state: State<'_, WindowsIntegrationService>,
) -> Result<WindowsIntegrationSnapshot, String> {
    state.reset_global_shortcuts()
}

#[tauri::command]
async fn open_overlay(
    kind: windows::OverlayKind,
    state: State<'_, WindowsIntegrationService>,
) -> Result<WindowsIntegrationSnapshot, String> {
    state.open_overlay(kind)
}

#[tauri::command]
async fn close_overlay(
    kind: windows::OverlayKind,
    state: State<'_, WindowsIntegrationService>,
) -> Result<WindowsIntegrationSnapshot, String> {
    state.close_overlay(kind)
}

#[tauri::command]
async fn toggle_overlay(
    kind: windows::OverlayKind,
    state: State<'_, WindowsIntegrationService>,
) -> Result<WindowsIntegrationSnapshot, String> {
    state.toggle_overlay(kind)
}

#[tauri::command]
fn set_gaming_click_through(
    enabled: bool,
    state: State<'_, WindowsIntegrationService>,
) -> Result<WindowsIntegrationSnapshot, GamingClickThroughError> {
    state.set_gaming_click_through(enabled)
}

#[tauri::command]
fn list_output_profiles(
    state: State<'_, WindowsIntegrationService>,
) -> Vec<playback::OutputProfile> {
    state.list_output_profiles()
}

#[tauri::command]
fn create_output_profile(
    name: String,
    state: State<'_, WindowsIntegrationService>,
) -> Result<WindowsIntegrationSnapshot, String> {
    state.create_output_profile(name)
}

#[tauri::command]
fn update_output_profile(
    profile: playback::OutputProfile,
    state: State<'_, WindowsIntegrationService>,
) -> Result<WindowsIntegrationSnapshot, String> {
    state.update_output_profile(profile)
}

#[tauri::command]
fn delete_output_profile(
    id: String,
    state: State<'_, WindowsIntegrationService>,
) -> Result<WindowsIntegrationSnapshot, String> {
    state.delete_output_profile(&id)
}

#[tauri::command]
fn apply_output_profile(
    id: String,
    state: State<'_, WindowsIntegrationService>,
) -> Result<PlaybackSnapshot, playback::OutputProfileApplyError> {
    state.apply_output_profile(&id)
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
fn list_playlists(
    state: State<'_, AppState>,
) -> Result<Vec<playlists::Playlist>, PlaylistErrorDto> {
    state
        .playlists
        .list_playlists()
        .map_err(|error| error.dto())
}

#[tauri::command]
fn get_playlist(
    playlist_id: crate::domain::PlaylistId,
    state: State<'_, AppState>,
) -> Result<Option<playlists::Playlist>, PlaylistErrorDto> {
    state
        .playlists
        .get_playlist(playlist_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn create_playlist(
    name: String,
    state: State<'_, AppState>,
) -> Result<playlists::Playlist, PlaylistErrorDto> {
    state
        .playlists
        .create_playlist(name)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn rename_playlist(
    playlist_id: crate::domain::PlaylistId,
    name: String,
    state: State<'_, AppState>,
) -> Result<playlists::Playlist, PlaylistErrorDto> {
    state
        .playlists
        .rename_playlist(playlist_id, name)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn delete_playlist(
    playlist_id: crate::domain::PlaylistId,
    state: State<'_, AppState>,
) -> Result<(), PlaylistErrorDto> {
    state
        .playlists
        .delete_playlist(playlist_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn duplicate_playlist(
    playlist_id: crate::domain::PlaylistId,
    requested_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<playlists::Playlist, PlaylistErrorDto> {
    state
        .playlists
        .duplicate_playlist(playlist_id, requested_name)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn add_playlist_item(
    playlist_id: crate::domain::PlaylistId,
    track_id: crate::domain::TrackId,
    requested_source_id: Option<crate::domain::SourceId>,
    state: State<'_, AppState>,
) -> Result<playlists::PlaylistItem, PlaylistErrorDto> {
    state
        .playlists
        .add_playlist_item(playlist_id, track_id, requested_source_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn remove_playlist_item(
    playlist_id: crate::domain::PlaylistId,
    item_id: crate::domain::PlaylistItemId,
    state: State<'_, AppState>,
) -> Result<(), PlaylistErrorDto> {
    state
        .playlists
        .remove_playlist_item(playlist_id, item_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn reorder_playlist_item(
    playlist_id: crate::domain::PlaylistId,
    item_id: crate::domain::PlaylistItemId,
    target_position: u32,
    state: State<'_, AppState>,
) -> Result<playlists::Playlist, PlaylistErrorDto> {
    state
        .playlists
        .reorder_playlist_item(playlist_id, item_id, target_position)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn create_playlist_branch(
    parent_playlist_id: crate::domain::PlaylistId,
    name: String,
    state: State<'_, AppState>,
) -> Result<playlists::Playlist, PlaylistErrorDto> {
    state
        .playlists
        .create_playlist_branch(parent_playlist_id, name)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn get_branch_changes(
    branch_playlist_id: crate::domain::PlaylistId,
    state: State<'_, AppState>,
) -> Result<Vec<playlists::BranchChange>, PlaylistErrorDto> {
    state
        .playlists
        .get_branch_changes(branch_playlist_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn merge_branch_changes(
    branch_playlist_id: crate::domain::PlaylistId,
    selected_changes: Vec<playlists::BranchChange>,
    state: State<'_, AppState>,
) -> Result<playlists::BranchMergeResult, PlaylistErrorDto> {
    state
        .playlists
        .merge_branch_changes(branch_playlist_id, selected_changes)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn discard_playlist_branch(
    branch_playlist_id: crate::domain::PlaylistId,
    state: State<'_, AppState>,
) -> Result<(), PlaylistErrorDto> {
    state
        .playlists
        .discard_branch(branch_playlist_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn play_playlist(
    playlist_id: crate::domain::PlaylistId,
    item_ids: Vec<crate::domain::PlaylistItemId>,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .play_playlist(playlist_id, item_ids)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn queue_playlist(
    playlist_id: crate::domain::PlaylistId,
    item_ids: Vec<crate::domain::PlaylistItemId>,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .queue_playlist(playlist_id, item_ids)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn get_track_collection_states(
    track_ids: Vec<crate::domain::TrackId>,
    state: State<'_, AppState>,
) -> Result<Vec<playlists::TrackCollectionState>, PlaylistErrorDto> {
    state
        .playlists
        .get_track_collection_states(&track_ids)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn set_track_liked(
    track_id: crate::domain::TrackId,
    liked: bool,
    state: State<'_, AppState>,
) -> Result<bool, PlaylistErrorDto> {
    state
        .playlists
        .set_track_liked(track_id, liked)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn set_track_rating(
    track_id: crate::domain::TrackId,
    rating: Option<u8>,
    state: State<'_, AppState>,
) -> Result<Option<u8>, PlaylistErrorDto> {
    state
        .playlists
        .set_track_rating(track_id, rating)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn list_tags(state: State<'_, AppState>) -> Result<Vec<playlists::Tag>, PlaylistErrorDto> {
    state.playlists.list_tags().map_err(|error| error.dto())
}

#[tauri::command]
fn create_tag(
    name: String,
    state: State<'_, AppState>,
) -> Result<playlists::Tag, PlaylistErrorDto> {
    state
        .playlists
        .create_tag(name)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn rename_tag(
    tag_id: crate::domain::TagId,
    name: String,
    state: State<'_, AppState>,
) -> Result<playlists::Tag, PlaylistErrorDto> {
    state
        .playlists
        .rename_tag(tag_id, name)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn delete_tag(
    tag_id: crate::domain::TagId,
    state: State<'_, AppState>,
) -> Result<(), PlaylistErrorDto> {
    state
        .playlists
        .delete_tag(tag_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn add_track_tag(
    track_id: crate::domain::TrackId,
    tag_id: crate::domain::TagId,
    state: State<'_, AppState>,
) -> Result<(), PlaylistErrorDto> {
    state
        .playlists
        .add_track_tag(track_id, tag_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn remove_track_tag(
    track_id: crate::domain::TrackId,
    tag_id: crate::domain::TagId,
    state: State<'_, AppState>,
) -> Result<(), PlaylistErrorDto> {
    state
        .playlists
        .remove_track_tag(track_id, tag_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn add_track_to_inbox(
    track_id: crate::domain::TrackId,
    state: State<'_, AppState>,
) -> Result<playlists::PlaylistItem, PlaylistErrorDto> {
    state
        .playlists
        .add_track_to_inbox(track_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn get_lyrics(
    track_id: crate::domain::TrackId,
    current_source_id: Option<crate::domain::SourceId>,
    state: State<'_, AppState>,
) -> Result<Option<LyricsDocument>, LyricsErrorDto> {
    state
        .lyrics
        .get_lyrics(track_id, current_source_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn save_manual_lyrics(
    track_id: crate::domain::TrackId,
    mode: ManualLyricsMode,
    text: String,
    state: State<'_, AppState>,
) -> Result<LyricsDocument, LyricsErrorDto> {
    state
        .lyrics
        .save_manual_lyrics(track_id, mode, text)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn delete_manual_lyrics(
    track_id: crate::domain::TrackId,
    state: State<'_, AppState>,
) -> Result<(), LyricsErrorDto> {
    state
        .lyrics
        .delete_manual_lyrics(track_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn pick_and_import_lyrics_file(
    track_id: crate::domain::TrackId,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LyricsDocument, LyricsErrorDto> {
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("Lyrics", &["lrc", "txt"])
        .blocking_pick_file()
    else {
        return Err(lyrics::LyricsError::ImportCancelled.dto());
    };
    let path = file
        .into_path()
        .map_err(|_| lyrics::LyricsError::ImportRead)
        .map_err(|error| error.dto())?;
    state
        .lyrics
        .import_lyrics_file(track_id, path)
        .map_err(|error| error.dto())
}

#[tauri::command]
async fn find_lrclib_best(
    track_id: crate::domain::TrackId,
    state: State<'_, AppState>,
) -> Result<LyricsDocument, LyricsErrorDto> {
    state
        .lyrics
        .find_lrclib_best(track_id)
        .await
        .map_err(|error| error.dto())
}

#[tauri::command]
async fn search_lrclib(
    track_id: crate::domain::TrackId,
    state: State<'_, AppState>,
) -> Result<Vec<LyricsCandidate>, LyricsErrorDto> {
    state
        .lyrics
        .search_lrclib(track_id)
        .await
        .map_err(|error| error.dto())
}

#[tauri::command]
async fn select_lrclib_candidate(
    track_id: crate::domain::TrackId,
    provider_record_id: i64,
    state: State<'_, AppState>,
) -> Result<LyricsDocument, LyricsErrorDto> {
    state
        .lyrics
        .select_lrclib_candidate(track_id, provider_record_id)
        .await
        .map_err(|error| error.dto())
}

#[tauri::command]
fn clear_cached_lrclib(
    track_id: crate::domain::TrackId,
    state: State<'_, AppState>,
) -> Result<(), LyricsErrorDto> {
    state
        .lyrics
        .clear_cached_lrclib(track_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn list_bookmarks(
    track_id: crate::domain::TrackId,
    state: State<'_, AppState>,
) -> Result<Vec<Bookmark>, BookmarkErrorDto> {
    state
        .bookmarks
        .list_bookmarks(track_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn create_bookmark(
    track_id: crate::domain::TrackId,
    position_ms: u64,
    note: String,
    state: State<'_, AppState>,
) -> Result<Bookmark, BookmarkErrorDto> {
    state
        .bookmarks
        .create_bookmark(track_id, position_ms, note)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn update_bookmark(
    bookmark_id: crate::domain::BookmarkId,
    position_ms: u64,
    note: String,
    state: State<'_, AppState>,
) -> Result<Bookmark, BookmarkErrorDto> {
    state
        .bookmarks
        .update_bookmark(bookmark_id, position_ms, note)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn delete_bookmark(
    bookmark_id: crate::domain::BookmarkId,
    state: State<'_, AppState>,
) -> Result<(), BookmarkErrorDto> {
    state
        .bookmarks
        .delete_bookmark(bookmark_id)
        .map_err(|error| error.dto())
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
fn set_ab_loop_a(state: State<'_, AppState>) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state.playback.set_ab_loop_a().map_err(|error| error.dto())
}

#[tauri::command]
fn set_ab_loop_b(state: State<'_, AppState>) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state.playback.set_ab_loop_b().map_err(|error| error.dto())
}

#[tauri::command]
fn clear_ab_loop(state: State<'_, AppState>) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state.playback.clear_ab_loop().map_err(|error| error.dto())
}

#[tauri::command]
fn save_ab_loop_preset(
    track_id: crate::domain::TrackId,
    name: String,
    state: State<'_, AppState>,
) -> Result<AbLoopPreset, PlaybackErrorDto> {
    state
        .playback
        .save_ab_loop_preset(track_id, name)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn list_ab_loop_presets(
    track_id: crate::domain::TrackId,
    state: State<'_, AppState>,
) -> Result<Vec<AbLoopPreset>, PlaybackErrorDto> {
    state
        .playback
        .list_ab_loop_presets(track_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn apply_ab_loop_preset(
    preset_id: crate::domain::AbLoopPresetId,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .apply_ab_loop_preset(preset_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn delete_ab_loop_preset(
    preset_id: crate::domain::AbLoopPresetId,
    state: State<'_, AppState>,
) -> Result<(), PlaybackErrorDto> {
    state
        .playback
        .delete_ab_loop_preset(preset_id)
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

#[tauri::command]
fn get_queue_workspace(
    state: State<'_, AppState>,
) -> Result<playback::QueueWorkspace, PlaybackErrorDto> {
    state
        .playback
        .get_queue_workspace()
        .map_err(|error| error.dto())
}

#[tauri::command]
fn move_queue_entry(
    entry_id: crate::queue::QueueEntryId,
    section: QueueSection,
    target_index: usize,
    state: State<'_, AppState>,
) -> Result<playback::QueueWorkspace, PlaybackErrorDto> {
    state
        .playback
        .move_queue_entry(entry_id, section, target_index)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn remove_queue_entry(
    entry_id: crate::queue::QueueEntryId,
    state: State<'_, AppState>,
) -> Result<playback::QueueWorkspace, PlaybackErrorDto> {
    state
        .playback
        .remove_queue_entry(entry_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn set_queue_entry_pinned(
    entry_id: crate::queue::QueueEntryId,
    pinned: bool,
    state: State<'_, AppState>,
) -> Result<playback::QueueWorkspace, PlaybackErrorDto> {
    state
        .playback
        .set_queue_entry_pinned(entry_id, pinned)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn clear_queue_section(
    section: QueueSection,
    state: State<'_, AppState>,
) -> Result<playback::QueueWorkspace, PlaybackErrorDto> {
    state
        .playback
        .clear_queue_section(section)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn save_queue_snapshot(
    name: String,
    state: State<'_, AppState>,
) -> Result<playback::QueueSnapshot, PlaybackErrorDto> {
    state
        .playback
        .save_queue_snapshot(name)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn list_queue_snapshots(
    state: State<'_, AppState>,
) -> Result<Vec<playback::QueueSnapshotSummary>, PlaybackErrorDto> {
    state
        .playback
        .list_queue_snapshots()
        .map_err(|error| error.dto())
}

#[tauri::command]
fn restore_queue_snapshot(
    snapshot_id: crate::domain::QueueSnapshotId,
    state: State<'_, AppState>,
) -> Result<PlaybackSnapshot, PlaybackErrorDto> {
    state
        .playback
        .restore_queue_snapshot(snapshot_id)
        .map_err(|error| error.dto())
}

#[tauri::command]
fn delete_queue_snapshot(
    snapshot_id: crate::domain::QueueSnapshotId,
    state: State<'_, AppState>,
) -> Result<Vec<playback::QueueSnapshotSummary>, PlaybackErrorDto> {
    state
        .playback
        .delete_queue_snapshot(snapshot_id)
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
            let playlists = PlaylistService::new(library.database().clone());
            let inspector = TrackInspectorService::new(database.clone(), playlists.clone());
            let windows_slot: Arc<Mutex<Option<WindowsIntegrationService>>> =
                Arc::new(Mutex::new(None));
            let playback_sink = {
                let app_handle = app.handle().clone();
                let windows_slot = windows_slot.clone();
                Arc::new(move |snapshot: PlaybackSnapshot| {
                    let _ = app_handle.emit(PLAYBACK_STATE_EVENT, snapshot.clone());
                    let windows = windows_slot
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .clone();
                    if let Some(windows) = windows {
                        windows.on_playback_snapshot(&snapshot);
                    }
                })
            };
            let queue_sink = {
                let app_handle = app.handle().clone();
                Arc::new(move |workspace: playback::QueueWorkspace| {
                    let _ = app_handle.emit(QUEUE_STATE_EVENT, workspace);
                })
            };
            let playback = PlaybackService::new_with_queue_sink(
                library.clone(),
                media_tools.clone(),
                playback_sink,
                queue_sink,
            );
            let lyrics = LyricsService::new(database.clone(), library.clone())?;
            let bookmarks = BookmarkService::new(database.clone());
            library.register_watchers(sink.clone())?;
            let windows = WindowsIntegrationService::new(
                app.handle().clone(),
                database.clone(),
                playback.clone(),
            );
            app.manage(windows.clone());
            *windows_slot
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(windows.clone());
            windows.initialize();
            app.manage(AppState {
                database,
                library: library.clone(),
                media_tools,
                downloads,
                playback,
                lyrics,
                bookmarks,
                playlists,
                search,
                spotify_auth,
                fusion,
                source_resolver,
                inspector,
            });
            library.start_all_scans(sink)?;
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        return;
                    }
                    if let Some(windows) = app.try_state::<WindowsIntegrationService>() {
                        windows.handle_shortcut(shortcut.id());
                    }
                })
                .build(),
        )
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
            get_track_inspector,
            get_settings_snapshot,
            set_setting,
            get_windows_integration_snapshot,
            set_windows_integration_settings,
            set_global_shortcuts_enabled,
            update_global_shortcut,
            reset_global_shortcuts,
            open_overlay,
            close_overlay,
            toggle_overlay,
            set_gaming_click_through,
            list_output_profiles,
            create_output_profile,
            update_output_profile,
            delete_output_profile,
            apply_output_profile,
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
            list_playlists,
            get_playlist,
            create_playlist,
            rename_playlist,
            delete_playlist,
            duplicate_playlist,
            add_playlist_item,
            remove_playlist_item,
            reorder_playlist_item,
            create_playlist_branch,
            get_branch_changes,
            merge_branch_changes,
            discard_playlist_branch,
            play_playlist,
            queue_playlist,
            get_track_collection_states,
            set_track_liked,
            set_track_rating,
            list_tags,
            create_tag,
            rename_tag,
            delete_tag,
            add_track_tag,
            remove_track_tag,
            add_track_to_inbox,
            get_lyrics,
            save_manual_lyrics,
            delete_manual_lyrics,
            pick_and_import_lyrics_file,
            find_lrclib_best,
            search_lrclib,
            select_lrclib_candidate,
            clear_cached_lrclib,
            list_bookmarks,
            create_bookmark,
            update_bookmark,
            delete_bookmark,
            get_playback_snapshot,
            play_track,
            enqueue_track,
            play_track_next,
            toggle_play_pause,
            seek_playback,
            set_ab_loop_a,
            set_ab_loop_b,
            clear_ab_loop,
            save_ab_loop_preset,
            list_ab_loop_presets,
            apply_ab_loop_preset,
            delete_ab_loop_preset,
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
            get_queue_workspace,
            move_queue_entry,
            remove_queue_entry,
            set_queue_entry_pinned,
            clear_queue_section,
            save_queue_snapshot,
            list_queue_snapshots,
            restore_queue_snapshot,
            delete_queue_snapshot,
        ])
        .build(tauri::generate_context!())
        .expect("error while building SpotDIY")
        .run(|app_handle, event| {
            if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
                if let Some(windows) = app_handle.try_state::<WindowsIntegrationService>() {
                    windows.shutdown();
                }
                if let Some(state) = app_handle.try_state::<AppState>() {
                    let _ = state.search.cancel_search();
                    let _ = state.downloads.shutdown();
                    let _ = state.playback.shutdown();
                }
            }
        });
}
