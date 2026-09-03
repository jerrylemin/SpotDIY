//! Bounded, read-only data for the visual exploration workspaces.

use std::path::PathBuf;

use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::{Database, DatabaseError};
use crate::domain::TrackId;
use crate::library::LibraryService;

pub const DEFAULT_DATASET_LIMIT: u32 = 2_000;
pub const MAX_DATASET_LIMIT: u32 = 5_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VisualAudioQuality {
    Lossless,
    Lossy,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualDatasetRequest {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub liked_only: bool,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

impl Default for VisualDatasetRequest {
    fn default() -> Self {
        Self {
            query: None,
            genre: None,
            artist: None,
            liked_only: false,
            limit: DEFAULT_DATASET_LIMIT,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualTrackPoint {
    pub track_id: TrackId,
    pub title: String,
    pub primary_artist: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub genres: Vec<String>,
    pub year: Option<u16>,
    pub date_added: String,
    pub last_played: Option<String>,
    pub liked: bool,
    pub rating: Option<u8>,
    pub qualified_plays: u64,
    pub listened_ms: u64,
    pub audio_quality: VisualAudioQuality,
    pub provider_count: u64,
    pub artwork_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualLibraryDataset {
    pub total_tracks: u64,
    pub returned_tracks: u64,
    pub truncated: bool,
    pub tracks: Vec<VisualTrackPoint>,
}

#[derive(Debug, Error)]
pub enum VisualExplorerError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON operation failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid visual dataset input: {0}")]
    InvalidInput(String),
    #[error("invalid visual dataset data: {0}")]
    InvalidData(String),
}

#[derive(Clone)]
pub struct VisualExplorerService {
    database: Database,
    library: LibraryService,
}

impl VisualExplorerService {
    pub fn new(database: Database, library: LibraryService) -> Self {
        Self { database, library }
    }

    pub fn dataset(
        &self,
        request: VisualDatasetRequest,
    ) -> Result<VisualLibraryDataset, VisualExplorerError> {
        let request = normalize_request(request)?;
        let (query, genre, artist) = (
            request.query.as_deref(),
            request.genre.as_deref(),
            request.artist.as_deref(),
        );
        let liked_only = i64::from(request.liked_only);
        let total = self.database.with_connection(|connection| {
            connection.query_row(
                &total_sql(),
                params![query, genre, artist, liked_only],
                |row| row.get::<_, i64>(0),
            )
        })?;
        let total = non_negative(total, "visual_dataset.total_tracks")?;
        let rows = self.database.with_connection(|connection| {
            let mut statement = connection.prepare(&dataset_sql())?;
            let rows = statement
                .query_map(
                    params![query, genre, artist, liked_only, i64::from(request.limit)],
                    map_raw_track,
                )?
                .collect::<Result<Vec<_>, _>>();
            rows
        })?;
        let tracks = rows
            .into_iter()
            .map(|row| self.parse_track(row))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(VisualLibraryDataset {
            total_tracks: total,
            returned_tracks: tracks.len() as u64,
            truncated: total > u64::from(request.limit),
            tracks,
        })
    }

    fn parse_track(&self, row: RawVisualTrack) -> Result<VisualTrackPoint, VisualExplorerError> {
        let track_id = TrackId::parse_str(&row.track_id)
            .map_err(|error| VisualExplorerError::InvalidData(format!("track ID: {error}")))?;
        let artists = parse_json_array(&row.artists, "artists")?;
        let genres = parse_json_array(&row.genres, "genres")?;
        let artwork_path = row
            .artwork_cache_key
            .as_deref()
            .and_then(|key| self.library.artwork_path(key))
            .filter(|path| path.is_file());
        Ok(VisualTrackPoint {
            track_id,
            title: row.title,
            primary_artist: row.primary_artist,
            artists,
            album: row.album,
            genres,
            year: parse_year(row.release_date),
            date_added: row.date_added,
            last_played: row.last_played,
            liked: row.liked != 0,
            rating: row
                .rating
                .map(|value| u8::try_from(value).map_err(|_| invalid_number("rating", value)))
                .transpose()?,
            qualified_plays: non_negative(row.qualified_plays, "qualified_plays")?,
            listened_ms: non_negative(row.listened_ms, "listened_ms")?,
            audio_quality: match row.audio_quality {
                2 => VisualAudioQuality::Lossless,
                1 => VisualAudioQuality::Lossy,
                _ => VisualAudioQuality::Unknown,
            },
            provider_count: non_negative(row.provider_count, "provider_count")?,
            artwork_path,
        })
    }
}

fn default_limit() -> u32 {
    DEFAULT_DATASET_LIMIT
}

fn normalize_request(
    mut request: VisualDatasetRequest,
) -> Result<VisualDatasetRequest, VisualExplorerError> {
    if !(1..=MAX_DATASET_LIMIT).contains(&request.limit) {
        return Err(VisualExplorerError::InvalidInput(format!(
            "limit must be between 1 and {MAX_DATASET_LIMIT}"
        )));
    }
    request.query = normalize_filter(request.query, true);
    request.genre = normalize_filter(request.genre, false);
    request.artist = normalize_filter(request.artist, false);
    Ok(request)
}

fn normalize_filter(value: Option<String>, fuzzy: bool) -> Option<String> {
    let value = value?.trim().to_owned();
    if value.is_empty() {
        return None;
    }
    if fuzzy {
        Some(format!("%{}%", escape_like(&value)))
    } else {
        Some(value)
    }
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn parse_json_array(value: &str, field: &'static str) -> Result<Vec<String>, VisualExplorerError> {
    serde_json::from_str(value)
        .map_err(|error| VisualExplorerError::InvalidData(format!("{field}: {error}")))
}

fn parse_year(value: Option<String>) -> Option<u16> {
    value.and_then(|value| value.get(..4)?.parse().ok())
}

fn non_negative(value: i64, field: &'static str) -> Result<u64, VisualExplorerError> {
    u64::try_from(value).map_err(|_| invalid_number(field, value))
}

fn invalid_number(field: &'static str, value: i64) -> VisualExplorerError {
    VisualExplorerError::InvalidData(format!("{field} is negative or invalid: {value}"))
}

struct RawVisualTrack {
    track_id: String,
    title: String,
    primary_artist: String,
    artists: String,
    album: Option<String>,
    genres: String,
    release_date: Option<String>,
    date_added: String,
    last_played: Option<String>,
    liked: i64,
    rating: Option<i64>,
    qualified_plays: i64,
    listened_ms: i64,
    audio_quality: i64,
    provider_count: i64,
    artwork_cache_key: Option<String>,
}

fn map_raw_track(row: &Row<'_>) -> Result<RawVisualTrack, rusqlite::Error> {
    Ok(RawVisualTrack {
        track_id: row.get(0)?,
        title: row.get(1)?,
        primary_artist: row.get(2)?,
        artists: row.get(3)?,
        album: row.get(4)?,
        genres: row.get(5)?,
        release_date: row.get(6)?,
        date_added: row.get(7)?,
        last_played: row.get(8)?,
        liked: row.get(9)?,
        rating: row.get(10)?,
        qualified_plays: row.get(11)?,
        listened_ms: row.get(12)?,
        audio_quality: row.get(13)?,
        provider_count: row.get(14)?,
        artwork_cache_key: row.get(15)?,
    })
}

const FILTER_SQL: &str = r#"
WHERE (
    ?1 IS NULL
    OR lower(t.title) LIKE lower(?1) ESCAPE '\'
    OR lower(coalesce(al.title, '')) LIKE lower(?1) ESCAPE '\'
    OR EXISTS (
        SELECT 1 FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
        WHERE ta.track_id = t.id AND lower(a.name) LIKE lower(?1) ESCAPE '\'
    )
    OR EXISTS (
        SELECT 1 FROM track_genres tg
        WHERE tg.track_id = t.id AND lower(tg.genre) LIKE lower(?1) ESCAPE '\'
    )
)
AND (
    ?2 IS NULL OR EXISTS (
        SELECT 1 FROM track_genres tg
        WHERE tg.track_id = t.id AND lower(tg.normalized_genre) = lower(?2)
    )
)
AND (
    ?3 IS NULL OR EXISTS (
        SELECT 1 FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
        WHERE ta.track_id = t.id AND lower(a.name) = lower(?3)
    )
)
AND (
    ?4 = 0 OR EXISTS (SELECT 1 FROM likes l WHERE l.track_id = t.id)
)
"#;

fn total_sql() -> String {
    format!(
        "SELECT COUNT(*) FROM tracks t LEFT JOIN albums al ON al.id = t.album_id {}",
        FILTER_SQL
    )
}

fn dataset_sql() -> String {
    format!(
        concat!(
    "SELECT t.id, t.title,\n",
    "       coalesce((SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id ",
    "                  WHERE ta.track_id = t.id ORDER BY ta.artist_order, a.id LIMIT 1), ''),\n",
    "       coalesce((SELECT json_group_array(a.name) FROM track_artists ta JOIN artists a ON a.id = ta.artist_id ",
    "                  WHERE ta.track_id = t.id ORDER BY ta.artist_order, a.id), '[]'),\n",
    "       al.title,\n",
    "       coalesce((SELECT json_group_array(tg.genre) FROM track_genres tg WHERE tg.track_id = t.id ",
    "                  ORDER BY tg.normalized_genre, tg.genre), '[]'),\n",
    "       al.release_date, t.created_at,\n",
    "       (SELECT max(h.started_at) FROM play_history h WHERE h.track_id = t.id),\n",
    "       CASE WHEN EXISTS (SELECT 1 FROM likes l WHERE l.track_id = t.id) THEN 1 ELSE 0 END,\n",
    "       (SELECT r.rating FROM ratings r WHERE r.track_id = t.id),\n",
    "       coalesce((SELECT sum(h.qualified_play) FROM play_history h WHERE h.track_id = t.id), 0),\n",
    "       coalesce((SELECT sum(h.listened_ms) FROM play_history h WHERE h.track_id = t.id), 0),\n",
    "       CASE\n",
    "         WHEN EXISTS (SELECT 1 FROM local_files lf JOIN track_sources s ON s.id = lf.source_id\n",
    "                      WHERE s.track_id = t.id AND (lower(coalesce(lf.codec, '')) IN ('flac', 'alac', 'wavpack', 'ape')\n",
    "                        OR lower(coalesce(lf.codec, '')) LIKE 'pcm_%')) THEN 2\n",
    "         WHEN EXISTS (SELECT 1 FROM local_files lf JOIN track_sources s ON s.id = lf.source_id\n",
    "                      WHERE s.track_id = t.id AND lower(coalesce(lf.codec, '')) <> '') THEN 1\n",
    "         ELSE 0 END,\n",
    "       (SELECT count(DISTINCT s.provider_kind) FROM track_sources s WHERE s.track_id = t.id),\n",
    "       (SELECT lf.artwork_cache_key FROM local_files lf JOIN track_sources s ON s.id = lf.source_id\n",
    "        WHERE s.track_id = t.id AND lf.artwork_cache_key IS NOT NULL ORDER BY lf.source_id LIMIT 1)\n",
    "FROM tracks t LEFT JOIN albums al ON al.id = t.album_id\n",
    "{}",
    "ORDER BY\n",
    "  CASE WHEN EXISTS (SELECT 1 FROM likes l WHERE l.track_id = t.id) THEN 1 ELSE 0 END DESC,\n",
    "  CASE WHEN (SELECT r.rating FROM ratings r WHERE r.track_id = t.id) IS NULL THEN 1 ELSE 0 END ASC,\n",
    "  (SELECT r.rating FROM ratings r WHERE r.track_id = t.id) DESC,\n",
    "  coalesce((SELECT sum(h.listened_ms) FROM play_history h WHERE h.track_id = t.id), 0) DESC,\n",
    "  CASE WHEN (SELECT max(h.started_at) FROM play_history h WHERE h.track_id = t.id) IS NULL THEN 1 ELSE 0 END ASC,\n",
    "  (SELECT max(h.started_at) FROM play_history h WHERE h.track_id = t.id) DESC,\n",
    "  t.created_at DESC, t.id ASC\n",
    "LIMIT ?5"
        ),
        FILTER_SQL
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::LibraryService;
    use uuid::Uuid;

    fn service(label: &str) -> (Database, VisualExplorerService) {
        let database = Database::open(
            std::env::temp_dir().join(format!("spotdiy-visual-{label}-{}.sqlite3", Uuid::new_v4())),
        )
        .unwrap();
        let artwork_root =
            std::env::temp_dir().join(format!("spotdiy-visual-artwork-{}", Uuid::new_v4()));
        let library = LibraryService::new(database.clone(), artwork_root).unwrap();
        let explorer = VisualExplorerService::new(database.clone(), library);
        (database, explorer)
    }

    fn seed(database: &Database) {
        database
            .with_connection(|connection| {
                connection.execute_batch(
                    "
                    INSERT INTO artists (id, name, created_at, updated_at) VALUES
                      ('00000000-0000-0000-0000-000000000101', 'Artist One', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                      ('00000000-0000-0000-0000-000000000102', 'Artist Two', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                    INSERT INTO albums (id, title, release_date, created_at, updated_at) VALUES
                      ('00000000-0000-0000-0000-000000000201', 'Album One', '2024-07-01', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                      ('00000000-0000-0000-0000-000000000202', 'Album Two', NULL, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                    INSERT INTO tracks (id, title, normalized_title, album_id, duration_ms, created_at, updated_at) VALUES
                      ('00000000-0000-0000-0000-000000000001', 'Track One', 'track one', '00000000-0000-0000-0000-000000000201', 100000, '2026-01-03T00:00:00Z', '2026-01-03T00:00:00Z'),
                      ('00000000-0000-0000-0000-000000000002', 'Track Two', 'track two', '00000000-0000-0000-0000-000000000202', 200000, '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z'),
                      ('00000000-0000-0000-0000-000000000003', 'Track Three', 'track three', NULL, 300000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                    INSERT INTO track_sources (id, track_id, provider_kind, provider_item_id, duration_ms, available, can_playback, created_at, updated_at) VALUES
                      ('00000000-0000-0000-0000-000000000301', '00000000-0000-0000-0000-000000000001', 'local', 'local-one', 100000, 1, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                      ('00000000-0000-0000-0000-000000000302', '00000000-0000-0000-0000-000000000001', 'youtube', 'youtube-one', 100000, 1, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                      ('00000000-0000-0000-0000-000000000303', '00000000-0000-0000-0000-000000000002', 'local', 'local-two', 200000, 1, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                      ('00000000-0000-0000-0000-000000000304', '00000000-0000-0000-0000-000000000003', 'local', 'local-three', 300000, 1, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                    INSERT INTO local_files (source_id, path, codec, index_status, created_at, updated_at) VALUES
                      ('00000000-0000-0000-0000-000000000301', 'C:/private/music/track-one.flac', 'FLAC', 'indexed', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                      ('00000000-0000-0000-0000-000000000303', 'C:/private/music/track-two.mp3', 'MP3', 'indexed', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                      ('00000000-0000-0000-0000-000000000304', 'C:/private/music/track-three.wav', 'PCM_S16LE', 'indexed', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                    INSERT INTO track_artists (track_id, artist_id, artist_order) VALUES
                      ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000101', 0),
                      ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000102', 0),
                      ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000101', 0);
                    INSERT INTO track_genres (track_id, genre, normalized_genre) VALUES
                      ('00000000-0000-0000-0000-000000000001', 'Electronic', 'electronic'),
                      ('00000000-0000-0000-0000-000000000002', 'Ambient', 'ambient'),
                      ('00000000-0000-0000-0000-000000000003', 'Electronic', 'electronic');
                    INSERT INTO likes (track_id, liked_at) VALUES ('00000000-0000-0000-0000-000000000001', '2026-01-04T00:00:00Z');
                    INSERT INTO ratings (track_id, rating, updated_at) VALUES
                      ('00000000-0000-0000-0000-000000000001', 5, '2026-01-04T00:00:00Z'),
                      ('00000000-0000-0000-0000-000000000002', 4, '2026-01-04T00:00:00Z');
                    INSERT INTO play_history (id, track_id, source_id, title_snapshot, artists_json, album_snapshot, provider_kind, started_at, ended_at, local_date, local_hour, local_weekday, listened_ms, duration_ms, outcome, qualified_play, created_at) VALUES
                      ('00000000-0000-0000-0000-000000000401', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000301', 'Track One', '[\"Artist One\"]', 'Album One', 'local', '2026-01-04T00:00:00Z', '2026-01-04T00:01:00Z', '2026-01-04', 0, 0, 70000, 100000, 'stopped', 1, '2026-01-04T00:01:00Z'),
                      ('00000000-0000-0000-0000-000000000402', '00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000303', 'Track Two', '[\"Artist Two\"]', 'Album Two', 'local', '2026-01-03T00:00:00Z', '2026-01-03T00:01:00Z', '2026-01-03', 0, 0, 50000, 200000, 'stopped', 1, '2026-01-03T00:01:00Z');
                    ",
                )
            })
            .unwrap();
    }

    #[test]
    fn request_defaults_and_bounds_are_explicit() {
        assert_eq!(default_limit(), 2_000);
        assert_eq!(VisualDatasetRequest::default().limit, 2_000);
        assert!(normalize_request(VisualDatasetRequest {
            limit: 0,
            ..Default::default()
        })
        .is_err());
        assert!(normalize_request(VisualDatasetRequest {
            limit: 5_001,
            ..Default::default()
        })
        .is_err());
        let request = normalize_request(VisualDatasetRequest {
            query: Some("100%".to_owned()),
            limit: 1,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(request.query.as_deref(), Some("%100\\%%"));
    }

    #[test]
    fn dataset_empty_state_is_bounded_and_read_only() {
        let (_database, explorer) = service("empty");
        let result = explorer.dataset(Default::default()).unwrap();
        assert_eq!(result, VisualLibraryDataset::default());
    }

    #[test]
    fn dataset_filters_orders_aggregates_and_does_not_leak_media_paths() {
        let (database, explorer) = service("contract");
        seed(&database);
        let result = explorer.dataset(Default::default()).unwrap();
        assert_eq!(result.total_tracks, 3);
        assert_eq!(result.returned_tracks, 3);
        assert_eq!(
            result.tracks[0].track_id.to_string(),
            "00000000-0000-0000-0000-000000000001"
        );
        assert_eq!(result.tracks[0].artists, vec!["Artist One"]);
        assert_eq!(result.tracks[0].genres, vec!["Electronic"]);
        assert_eq!(result.tracks[0].year, Some(2024));
        assert_eq!(result.tracks[0].qualified_plays, 1);
        assert_eq!(result.tracks[0].listened_ms, 70_000);
        assert_eq!(result.tracks[0].audio_quality, VisualAudioQuality::Lossless);
        assert_eq!(result.tracks[0].provider_count, 2);
        assert!(result.tracks[0].artwork_path.is_none());
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("C:/private/music"));

        let filtered = explorer
            .dataset(VisualDatasetRequest {
                query: Some("track one".to_owned()),
                genre: Some("ELECTRONIC".to_owned()),
                artist: Some("artist one".to_owned()),
                liked_only: true,
                limit: 2_000,
            })
            .unwrap();
        assert_eq!(filtered.returned_tracks, 1);
        assert_eq!(filtered.tracks[0].title, "Track One");
    }

    #[test]
    fn dataset_reports_truncation_and_respects_hard_limit() {
        let (database, explorer) = service("truncated");
        seed(&database);
        let result = explorer
            .dataset(VisualDatasetRequest {
                limit: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.total_tracks, 3);
        assert_eq!(result.returned_tracks, 1);
        assert!(result.truncated);
        assert_eq!(result.tracks.len(), 1);
        assert!(normalize_request(VisualDatasetRequest {
            limit: 5_001,
            ..Default::default()
        })
        .is_err());
    }
}
