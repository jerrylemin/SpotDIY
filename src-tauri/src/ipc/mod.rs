use serde::Serialize;
use thiserror::Error;

use crate::db::Database;
use crate::settings::{SettingsError, SettingsRepository};
use crate::{
    db::DatabaseError,
    domain::{ProviderKind, SourceCapabilities},
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
    pub capabilities: SourceCapabilities,
    pub detail: &'static str,
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
    pub providers: Vec<ProviderStatus>,
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
    release_date: true,
    lyrics_metadata: false,
};
const SPOTIFY_CAPABILITIES: SourceCapabilities = SourceCapabilities {
    search: true,
    playback: false,
    metadata: true,
    artwork: true,
    lyrics: false,
    downloads: false,
    popularity: true,
    release_date: true,
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
        music_folders,
        providers: provider_statuses(),
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
            capabilities: SPOTIFY_CAPABILITIES,
        },
    ]
}

fn provider_statuses() -> Vec<ProviderStatus> {
    vec![
        ProviderStatus {
            kind: ProviderKind::Local,
            label: "Local library",
            configured: false,
            available: true,
            capabilities: LOCAL_CAPABILITIES,
            detail: "Add a music folder to begin indexing.",
        },
        ProviderStatus {
            kind: ProviderKind::Youtube,
            label: "YouTube",
            configured: false,
            available: false,
            capabilities: VIDEO_CAPABILITIES,
            detail: "Provider adapter awaits media-tool verification.",
        },
        ProviderStatus {
            kind: ProviderKind::Soundcloud,
            label: "SoundCloud",
            configured: false,
            available: false,
            capabilities: VIDEO_CAPABILITIES,
            detail: "Provider adapter awaits media-tool verification.",
        },
        ProviderStatus {
            kind: ProviderKind::Spotify,
            label: "Spotify catalog",
            configured: false,
            available: false,
            capabilities: SPOTIFY_CAPABILITIES,
            detail: "Connect Client Credentials locally to search the catalog.",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, TempDatabasePath};
    use crate::domain::TrackId;
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
}
