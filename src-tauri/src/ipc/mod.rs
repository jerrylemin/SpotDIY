use serde::Serialize;
use thiserror::Error;

use crate::db::Database;
use crate::downloads::{
    download_directory_status, media_tools_snapshot, DownloadDirectoryStatus, MediaToolsSnapshot,
};
use crate::media_tools::{MediaToolManager, YtDlpToolStatus};
use crate::search::types::ProviderRuntimeStatus;
use crate::settings::{SettingsError, SettingsRepository};
use crate::{
    db::DatabaseError,
    domain::{ProviderKind, SourceCapabilities},
    sources::spotify::{
        spotdl_runtime_status, SPOTIFY_RESULT_CAPABILITIES, SPOTIFY_SOURCE_CAPABILITIES,
    },
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub kind: ProviderKind,
    pub capabilities: SourceCapabilities,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub kind: ProviderKind,
    pub label: &'static str,
    pub configured: bool,
    pub available: bool,
    pub runtime_status: ProviderRuntimeStatus,
    pub capabilities: SourceCapabilities,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub version: &'static str,
    pub runtime: &'static str,
    pub storage_mode: &'static str,
    pub first_run: bool,
    pub tracks_indexed: u64,
    pub music_folders: Vec<String>,
    pub downloads_directory: Option<String>,
    pub download_directory_status: DownloadDirectoryStatus,
    pub providers: Vec<ProviderStatus>,
    pub media_tools: MediaToolsSnapshot,
}

#[derive(Debug, Error)]
pub enum StatusError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error("track count is outside the supported range: {0}")]
    TrackCount(i64),
}

const LOCAL_CAPABILITIES: SourceCapabilities = SourceCapabilities {
    search: true,
    playback: true,
    metadata: true,
    artwork: true,
    lyrics: true,
    downloads: false,
    popularity: false,
    release_date: false,
    lyrics_metadata: true,
};
const VIDEO_CAPABILITIES: SourceCapabilities = SourceCapabilities {
    search: true,
    playback: true,
    metadata: true,
    artwork: true,
    lyrics: false,
    downloads: true,
    popularity: true,
    release_date: false,
    lyrics_metadata: false,
};
pub fn app_status(version: &'static str, database: &Database) -> Result<AppStatus, StatusError> {
    let tracks_indexed: i64 = database.with_connection(|connection| {
        connection.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))
    })?;
    let tracks_indexed =
        u64::try_from(tracks_indexed).map_err(|_| StatusError::TrackCount(tracks_indexed))?;
    let music_folders: Vec<String> = database.with_connection(|connection| {
        let mut statement = connection.prepare(
            "SELECT path FROM library_folders WHERE enabled = 1 ORDER BY normalized_path_key COLLATE NOCASE",
        )?;
        let values = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>();
        values
    })?;
    let settings = SettingsRepository::new(database).get_snapshot()?;

    Ok(AppStatus {
        version,
        runtime: "tauri",
        storage_mode: match settings.storage_mode {
            crate::settings::StorageMode::Standard => "standard",
            crate::settings::StorageMode::Portable => "portable",
        },
        first_run: settings.first_run,
        tracks_indexed,
        music_folders: music_folders.clone(),
        downloads_directory: settings
            .downloads_directory
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        download_directory_status: download_directory_status(
            settings.downloads_directory.as_deref(),
        ),
        providers: provider_statuses(&music_folders, None, Some(spotdl_runtime_status())),
        media_tools: MediaToolsSnapshot::default(),
    })
}

pub fn app_status_with_runtime(
    version: &'static str,
    database: &Database,
    media_tools: &MediaToolManager,
) -> Result<AppStatus, StatusError> {
    let tracks_indexed: i64 = database.with_connection(|connection| {
        connection.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))
    })?;
    let tracks_indexed =
        u64::try_from(tracks_indexed).map_err(|_| StatusError::TrackCount(tracks_indexed))?;
    let music_folders: Vec<String> = database.with_connection(|connection| {
        let mut statement = connection.prepare(
            "SELECT path FROM library_folders WHERE enabled = 1 ORDER BY normalized_path_key COLLATE NOCASE",
        )?;
        let values = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>();
        values
    })?;
    let settings = SettingsRepository::new(database).get_snapshot()?;

    Ok(AppStatus {
        version,
        runtime: "tauri",
        storage_mode: match settings.storage_mode {
            crate::settings::StorageMode::Standard => "standard",
            crate::settings::StorageMode::Portable => "portable",
        },
        first_run: settings.first_run,
        tracks_indexed,
        music_folders: music_folders.clone(),
        downloads_directory: settings
            .downloads_directory
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        download_directory_status: download_directory_status(
            settings.downloads_directory.as_deref(),
        ),
        providers: provider_statuses(
            &music_folders,
            Some(media_tools.yt_dlp_status()),
            Some(spotdl_runtime_status()),
        ),
        media_tools: media_tools_snapshot(media_tools),
    })
}

pub fn source_capabilities() -> Vec<ProviderCapabilities> {
    vec![
        ProviderCapabilities {
            kind: ProviderKind::Local,
            capabilities: LOCAL_CAPABILITIES,
        },
        ProviderCapabilities {
            kind: ProviderKind::Youtube,
            capabilities: VIDEO_CAPABILITIES,
        },
        ProviderCapabilities {
            kind: ProviderKind::Soundcloud,
            capabilities: VIDEO_CAPABILITIES,
        },
        ProviderCapabilities {
            kind: ProviderKind::Spotify,
            capabilities: SPOTIFY_SOURCE_CAPABILITIES,
        },
    ]
}

pub(crate) fn provider_capabilities(provider: ProviderKind) -> SourceCapabilities {
    match provider {
        ProviderKind::Local => LOCAL_CAPABILITIES,
        ProviderKind::Youtube | ProviderKind::Soundcloud => VIDEO_CAPABILITIES,
        ProviderKind::Spotify => SPOTIFY_SOURCE_CAPABILITIES,
    }
}

pub fn provider_statuses(
    music_folders: &[String],
    yt_dlp: Option<YtDlpToolStatus>,
    spotify_runtime: Option<ProviderRuntimeStatus>,
) -> Vec<ProviderStatus> {
    let local_configured = !music_folders.is_empty();
    let yt_dlp = yt_dlp.unwrap_or(YtDlpToolStatus {
        status: ProviderRuntimeStatus::Missing,
        executable: None,
        version: None,
        detail: Some("yt-dlp status is not available".into()),
    });
    let spotify_runtime = spotify_runtime.unwrap_or(ProviderRuntimeStatus::Missing);
    let spotify_available = spotify_runtime == ProviderRuntimeStatus::Ready;
    vec![
        ProviderStatus {
            kind: ProviderKind::Local,
            label: "Local library",
            configured: local_configured,
            available: true,
            runtime_status: ProviderRuntimeStatus::Ready,
            capabilities: LOCAL_CAPABILITIES,
            detail: if local_configured {
                "Local library is ready.".into()
            } else {
                "Add a music folder to begin indexing.".into()
            },
        },
        ProviderStatus {
            kind: ProviderKind::Youtube,
            label: "YouTube",
            configured: yt_dlp.status == ProviderRuntimeStatus::Ready,
            available: yt_dlp.status == ProviderRuntimeStatus::Ready,
            runtime_status: yt_dlp.status,
            capabilities: VIDEO_CAPABILITIES,
            detail: yt_dlp
                .detail
                .clone()
                .unwrap_or_else(|| "yt-dlp is ready for metadata search.".into()),
        },
        ProviderStatus {
            kind: ProviderKind::Soundcloud,
            label: "SoundCloud",
            configured: yt_dlp.status == ProviderRuntimeStatus::Ready,
            available: yt_dlp.status == ProviderRuntimeStatus::Ready,
            runtime_status: yt_dlp.status,
            capabilities: VIDEO_CAPABILITIES,
            detail: yt_dlp
                .detail
                .unwrap_or_else(|| "yt-dlp is ready for metadata search.".into()),
        },
        ProviderStatus {
            kind: ProviderKind::Spotify,
            label: "Spotify",
            configured: spotify_available,
            available: spotify_available,
            runtime_status: spotify_runtime,
            capabilities: SPOTIFY_RESULT_CAPABILITIES,
            detail: if spotify_available {
                "spotdl is ready for Spotify search and source-matched audio downloads.".into()
            } else {
                "Install spotdl to search Spotify and download matched audio.".into()
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, TempDatabasePath};
    use crate::domain::{ProviderKind, TrackId};
    use crate::media_tools::YtDlpToolStatus;
    use crate::search::types::{
        ProviderSearchEvent, ProviderSearchSection, ProviderSearchState, SearchId,
    };
    use crate::settings::SettingsRepository;
    use rusqlite::params;

    #[test]
    fn app_status_reads_track_count_and_persisted_first_run_state() {
        let path = TempDatabasePath::new("ipc-status");
        let database = Database::open(path.path()).unwrap();
        let track_id = TrackId::new();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO tracks (
                        id, title, normalized_title, duration_ms, version_qualifiers_json,
                        preferred_source_id, created_at, updated_at
                     ) VALUES (?1, 'Signal Test', 'signal test', NULL, '[]', NULL, ?2, ?2)",
                    params![track_id.to_string(), "2026-01-01T00:00:00Z"],
                )?;
                Ok(())
            })
            .unwrap();

        let initial = app_status("0.1.0", &database).unwrap();
        assert_eq!(initial.tracks_indexed, 1);
        assert!(initial.first_run);
        assert_eq!(initial.storage_mode, "standard");
        assert_eq!(initial.providers.len(), 4);

        SettingsRepository::new(&database)
            .mark_initialized()
            .unwrap();
        assert!(!app_status("0.1.0", &database).unwrap().first_run);
    }

    fn ready_yt_dlp_status() -> YtDlpToolStatus {
        YtDlpToolStatus {
            status: ProviderRuntimeStatus::Ready,
            executable: Some("C:\\tools\\yt-dlp.exe".into()),
            version: Some("2026.08.19".into()),
            detail: None,
        }
    }

    #[test]
    fn provider_status_reports_local_folder_configuration() {
        let statuses = provider_statuses(
            &["C:\\Music".into()],
            None,
            Some(ProviderRuntimeStatus::Missing),
        );
        let local = statuses
            .iter()
            .find(|status| status.kind == ProviderKind::Local)
            .unwrap();
        assert!(local.configured);
        assert!(local.available);
        assert_eq!(local.runtime_status, ProviderRuntimeStatus::Ready);
    }

    #[test]
    fn provider_status_shares_one_ytdlp_state_for_youtube_and_soundcloud() {
        let statuses = provider_statuses(
            &[],
            Some(ready_yt_dlp_status()),
            Some(ProviderRuntimeStatus::Missing),
        );
        let youtube = statuses
            .iter()
            .find(|status| status.kind == ProviderKind::Youtube)
            .unwrap();
        let soundcloud = statuses
            .iter()
            .find(|status| status.kind == ProviderKind::Soundcloud)
            .unwrap();
        assert!(youtube.available && soundcloud.available);
        assert_eq!(youtube.runtime_status, soundcloud.runtime_status);
        assert_eq!(youtube.detail, soundcloud.detail);
    }

    #[test]
    fn provider_status_reports_spotify_as_ready_when_spotdl_is_available() {
        let spotify = provider_statuses(&[], None, Some(ProviderRuntimeStatus::Ready))
            .into_iter()
            .find(|status| status.kind == ProviderKind::Spotify)
            .unwrap();
        assert!(spotify.available);
        assert_eq!(spotify.runtime_status, ProviderRuntimeStatus::Ready);
        assert!(spotify.capabilities.downloads);
    }

    #[test]
    fn spotify_popularity_capability_is_false() {
        let spotify = source_capabilities()
            .into_iter()
            .find(|provider| provider.kind == ProviderKind::Spotify)
            .unwrap();
        assert!(!spotify.capabilities.popularity);
    }

    #[test]
    fn startup_succeeds_without_mpv_ytdlp_spotify_or_network() {
        let statuses = provider_statuses(
            &[],
            Some(YtDlpToolStatus {
                status: ProviderRuntimeStatus::Missing,
                executable: None,
                version: None,
                detail: Some("yt-dlp is missing".into()),
            }),
            Some(ProviderRuntimeStatus::Missing),
        );
        assert_eq!(statuses.len(), 4);
        assert!(statuses
            .iter()
            .any(|status| status.kind == ProviderKind::Youtube && !status.available));
    }

    #[test]
    fn search_ipc_serializes_only_normalized_fields() {
        let event = ProviderSearchEvent {
            search_id: SearchId::new(),
            section: ProviderSearchSection {
                provider: ProviderKind::Local,
                state: ProviderSearchState::Ready,
                results: Vec::new(),
                error: None,
            },
        };
        let value = serde_json::to_value(event).unwrap();
        assert!(value.get("searchId").is_some());
        assert!(value.get("section").is_some());
        assert!(value.to_string().contains("results"));
        assert!(!value.to_string().contains("stderr"));
        assert!(!value.to_string().contains("access_token"));
    }

    #[test]
    fn open_provider_result_rejects_wrong_host() {
        assert!(crate::sources::validate_provider_url(
            ProviderKind::Youtube,
            "https://evil.example/video"
        )
        .is_err());
    }

    #[test]
    fn open_provider_result_rejects_non_https() {
        assert!(crate::sources::validate_provider_url(
            ProviderKind::Youtube,
            "http://www.youtube.com/watch?v=signal"
        )
        .is_err());
    }

    #[test]
    fn stale_search_events_are_tagged_with_search_id() {
        let search_id = SearchId::new();
        let event = ProviderSearchEvent {
            search_id,
            section: ProviderSearchSection {
                provider: ProviderKind::Local,
                state: ProviderSearchState::Ready,
                results: Vec::new(),
                error: None,
            },
        };
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["searchId"], serde_json::to_value(search_id).unwrap());
    }
}
