use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::repository::TrackRepository;
use crate::db::{Database, DatabaseError};
use crate::domain::{AbLoopPresetId, BookmarkId, TrackId};

pub const MAX_BOOKMARK_NOTE_CHARS: usize = 500;
pub const MAX_PRESET_NAME_CHARS: usize = 80;
pub const MIN_AB_LOOP_LENGTH_MS: u64 = 250;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: BookmarkId,
    pub track_id: TrackId,
    pub position_ms: u64,
    pub note: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbLoopPreset {
    pub id: AbLoopPresetId,
    pub track_id: TrackId,
    pub name: String,
    pub a_ms: u64,
    pub b_ms: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum BookmarkError {
    #[error("track {0} was not found")]
    TrackNotFound(TrackId),
    #[error("bookmark {0} was not found")]
    BookmarkNotFound(BookmarkId),
    #[error("A/B loop preset {0} was not found")]
    PresetNotFound(AbLoopPresetId),
    #[error("bookmark position must be non-negative and fit the database")]
    InvalidPosition,
    #[error("bookmark position is outside the known track duration")]
    PositionOutsideDuration,
    #[error("bookmark note must be at most {MAX_BOOKMARK_NOTE_CHARS} Unicode scalar values")]
    NoteTooLong,
    #[error("bookmark note cannot be empty")]
    EmptyNote,
    #[error("A/B loop preset name must contain 1..{MAX_PRESET_NAME_CHARS} Unicode scalar values")]
    InvalidPresetName,
    #[error("A/B loop preset name already exists for this track")]
    DuplicatePresetName,
    #[error("A/B loop preset times are invalid")]
    InvalidLoop,
    #[error("A/B loop preset belongs to another track")]
    PresetTrackMismatch,
    #[error("database operation failed: {0}")]
    Database(#[from] DatabaseError),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid stored bookmark value")]
    InvalidStoredValue,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BookmarkErrorCode {
    TrackNotFound,
    BookmarkNotFound,
    PresetNotFound,
    InvalidPosition,
    PositionOutsideDuration,
    NoteTooLong,
    EmptyNote,
    InvalidPresetName,
    DuplicatePresetName,
    InvalidLoop,
    PresetTrackMismatch,
    Database,
    InvalidStoredValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkErrorDto {
    pub code: BookmarkErrorCode,
    pub detail: String,
}

impl BookmarkError {
    pub fn code(&self) -> BookmarkErrorCode {
        match self {
            Self::TrackNotFound(_) => BookmarkErrorCode::TrackNotFound,
            Self::BookmarkNotFound(_) => BookmarkErrorCode::BookmarkNotFound,
            Self::PresetNotFound(_) => BookmarkErrorCode::PresetNotFound,
            Self::InvalidPosition => BookmarkErrorCode::InvalidPosition,
            Self::PositionOutsideDuration => BookmarkErrorCode::PositionOutsideDuration,
            Self::NoteTooLong => BookmarkErrorCode::NoteTooLong,
            Self::EmptyNote => BookmarkErrorCode::EmptyNote,
            Self::InvalidPresetName => BookmarkErrorCode::InvalidPresetName,
            Self::DuplicatePresetName => BookmarkErrorCode::DuplicatePresetName,
            Self::InvalidLoop => BookmarkErrorCode::InvalidLoop,
            Self::PresetTrackMismatch => BookmarkErrorCode::PresetTrackMismatch,
            Self::Database(_) | Self::Sqlite(_) => BookmarkErrorCode::Database,
            Self::InvalidStoredValue => BookmarkErrorCode::InvalidStoredValue,
        }
    }

    pub fn dto(&self) -> BookmarkErrorDto {
        BookmarkErrorDto {
            code: self.code(),
            detail: self.to_string(),
        }
    }
}

#[derive(Clone)]
pub struct BookmarkService {
    database: Database,
}

impl BookmarkService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn list_bookmarks(&self, track_id: TrackId) -> Result<Vec<Bookmark>, BookmarkError> {
        let track = self.track(track_id)?;
        let _ = track;
        self.database
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT id, track_id, position_ms, note, created_at, updated_at
                     FROM bookmarks WHERE track_id = ?1 ORDER BY position_ms, id",
                )?;
                let rows = statement.query_map(params![track_id.to_string()], map_bookmark_row)?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .map_err(BookmarkError::from)?
            .into_iter()
            .map(parse_bookmark)
            .collect()
    }

    pub fn create_bookmark(
        &self,
        track_id: TrackId,
        position_ms: u64,
        note: String,
    ) -> Result<Bookmark, BookmarkError> {
        self.validate_position(track_id, position_ms)?;
        validate_note(&note)?;
        let id = BookmarkId::new();
        let now = Utc::now().to_rfc3339();
        let position_ms = i64::try_from(position_ms).map_err(|_| BookmarkError::InvalidPosition)?;
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO bookmarks (id, track_id, position_ms, note, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![id.to_string(), track_id.to_string(), position_ms, note, now],
            )?;
            Ok(())
        })?;
        self.get_bookmark(id)
    }

    pub fn update_bookmark(
        &self,
        id: BookmarkId,
        position_ms: u64,
        note: String,
    ) -> Result<Bookmark, BookmarkError> {
        let track_id = self.bookmark_track(id)?;
        self.validate_position(track_id, position_ms)?;
        validate_note(&note)?;
        let now = Utc::now().to_rfc3339();
        let position_ms = i64::try_from(position_ms).map_err(|_| BookmarkError::InvalidPosition)?;
        self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE bookmarks SET position_ms = ?1, note = ?2, updated_at = ?3 WHERE id = ?4",
                params![position_ms, note, now, id.to_string()],
            )?;
            Ok(())
        })?;
        self.get_bookmark(id)
    }

    pub fn delete_bookmark(&self, id: BookmarkId) -> Result<(), BookmarkError> {
        let changed = self.database.with_connection(|connection| {
            connection.execute(
                "DELETE FROM bookmarks WHERE id = ?1",
                params![id.to_string()],
            )
        })?;
        if changed == 0 {
            return Err(BookmarkError::BookmarkNotFound(id));
        }
        Ok(())
    }

    pub fn save_ab_loop_preset(
        &self,
        track_id: TrackId,
        name: String,
        a_ms: u64,
        b_ms: u64,
    ) -> Result<AbLoopPreset, BookmarkError> {
        let track = self.track(track_id)?;
        validate_loop(a_ms, b_ms, track.duration_ms)?;
        let name = normalize_preset_name(&name)?;
        let normalized_name = name.to_lowercase();
        let id = AbLoopPresetId::new();
        let now = Utc::now().to_rfc3339();
        let a_ms = i64::try_from(a_ms).map_err(|_| BookmarkError::InvalidLoop)?;
        let b_ms = i64::try_from(b_ms).map_err(|_| BookmarkError::InvalidLoop)?;
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO ab_loop_presets (
                    id, track_id, name, normalized_name, a_ms, b_ms, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                ON CONFLICT(track_id, normalized_name) DO UPDATE SET
                    name = excluded.name,
                    a_ms = excluded.a_ms,
                    b_ms = excluded.b_ms,
                    updated_at = excluded.updated_at",
                params![
                    id.to_string(),
                    track_id.to_string(),
                    name,
                    normalized_name,
                    a_ms,
                    b_ms,
                    now
                ],
            )
        })?;
        self.list_ab_loop_presets(track_id)?
            .into_iter()
            .find(|preset| preset.name.to_lowercase() == normalized_name)
            .ok_or(BookmarkError::InvalidStoredValue)
    }

    pub fn list_ab_loop_presets(
        &self,
        track_id: TrackId,
    ) -> Result<Vec<AbLoopPreset>, BookmarkError> {
        let _ = self.track(track_id)?;
        self.database
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT id, track_id, name, a_ms, b_ms, created_at, updated_at
                     FROM ab_loop_presets WHERE track_id = ?1 ORDER BY normalized_name, id",
                )?;
                let rows = statement.query_map(params![track_id.to_string()], map_preset_row)?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .map_err(BookmarkError::from)?
            .into_iter()
            .map(parse_preset)
            .collect()
    }

    pub fn get_ab_loop_preset(&self, id: AbLoopPresetId) -> Result<AbLoopPreset, BookmarkError> {
        let row = self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT id, track_id, name, a_ms, b_ms, created_at, updated_at
                     FROM ab_loop_presets WHERE id = ?1",
                    params![id.to_string()],
                    map_preset_row,
                )
                .optional()
        })?;
        row.map(parse_preset)
            .transpose()?
            .ok_or(BookmarkError::PresetNotFound(id))
    }

    pub fn delete_ab_loop_preset(&self, id: AbLoopPresetId) -> Result<(), BookmarkError> {
        let changed = self.database.with_connection(|connection| {
            connection.execute(
                "DELETE FROM ab_loop_presets WHERE id = ?1",
                params![id.to_string()],
            )
        })?;
        if changed == 0 {
            return Err(BookmarkError::PresetNotFound(id));
        }
        Ok(())
    }

    fn track(&self, track_id: TrackId) -> Result<crate::domain::UnifiedTrack, BookmarkError> {
        TrackRepository::new(&self.database)
            .get(track_id)
            .map_err(|_| BookmarkError::InvalidStoredValue)?
            .ok_or(BookmarkError::TrackNotFound(track_id))
    }

    fn validate_position(&self, track_id: TrackId, position_ms: u64) -> Result<(), BookmarkError> {
        let track = self.track(track_id)?;
        if let Some(duration_ms) = track.duration_ms {
            if position_ms > duration_ms {
                return Err(BookmarkError::PositionOutsideDuration);
            }
        }
        i64::try_from(position_ms).map_err(|_| BookmarkError::InvalidPosition)?;
        Ok(())
    }

    fn get_bookmark(&self, id: BookmarkId) -> Result<Bookmark, BookmarkError> {
        let row = self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT id, track_id, position_ms, note, created_at, updated_at
                     FROM bookmarks WHERE id = ?1",
                    params![id.to_string()],
                    map_bookmark_row,
                )
                .optional()
        })?;
        row.map(parse_bookmark)
            .transpose()?
            .ok_or(BookmarkError::BookmarkNotFound(id))
    }

    fn bookmark_track(&self, id: BookmarkId) -> Result<TrackId, BookmarkError> {
        let value: Option<String> = self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT track_id FROM bookmarks WHERE id = ?1",
                    params![id.to_string()],
                    |row| row.get(0),
                )
                .optional()
        })?;
        let Some(value) = value else {
            return Err(BookmarkError::BookmarkNotFound(id));
        };
        value.parse().map_err(|_| BookmarkError::InvalidStoredValue)
    }
}

fn validate_note(note: &str) -> Result<(), BookmarkError> {
    if note.chars().count() > MAX_BOOKMARK_NOTE_CHARS {
        return Err(BookmarkError::NoteTooLong);
    }
    Ok(())
}

fn normalize_preset_name(value: &str) -> Result<String, BookmarkError> {
    let name = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let length = name.chars().count();
    if !(1..=MAX_PRESET_NAME_CHARS).contains(&length) {
        return Err(BookmarkError::InvalidPresetName);
    }
    Ok(name)
}

pub fn validate_loop(a_ms: u64, b_ms: u64, duration_ms: Option<u64>) -> Result<(), BookmarkError> {
    if b_ms <= a_ms || b_ms - a_ms < MIN_AB_LOOP_LENGTH_MS {
        return Err(BookmarkError::InvalidLoop);
    }
    if let Some(duration_ms) = duration_ms {
        if b_ms > duration_ms {
            return Err(BookmarkError::PositionOutsideDuration);
        }
    }
    i64::try_from(a_ms).map_err(|_| BookmarkError::InvalidLoop)?;
    i64::try_from(b_ms).map_err(|_| BookmarkError::InvalidLoop)?;
    Ok(())
}

#[derive(Debug)]
struct RawBookmark {
    id: String,
    track_id: String,
    position_ms: i64,
    note: String,
    created_at: String,
    updated_at: String,
}

fn map_bookmark_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawBookmark> {
    Ok(RawBookmark {
        id: row.get(0)?,
        track_id: row.get(1)?,
        position_ms: row.get(2)?,
        note: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn parse_bookmark(row: RawBookmark) -> Result<Bookmark, BookmarkError> {
    Ok(Bookmark {
        id: row
            .id
            .parse()
            .map_err(|_| BookmarkError::InvalidStoredValue)?,
        track_id: row
            .track_id
            .parse()
            .map_err(|_| BookmarkError::InvalidStoredValue)?,
        position_ms: u64::try_from(row.position_ms)
            .map_err(|_| BookmarkError::InvalidStoredValue)?,
        note: row.note,
        created_at: parse_time(&row.created_at)?,
        updated_at: parse_time(&row.updated_at)?,
    })
}

#[derive(Debug)]
struct RawPreset {
    id: String,
    track_id: String,
    name: String,
    a_ms: i64,
    b_ms: i64,
    created_at: String,
    updated_at: String,
}

fn map_preset_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawPreset> {
    Ok(RawPreset {
        id: row.get(0)?,
        track_id: row.get(1)?,
        name: row.get(2)?,
        a_ms: row.get(3)?,
        b_ms: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn parse_preset(row: RawPreset) -> Result<AbLoopPreset, BookmarkError> {
    Ok(AbLoopPreset {
        id: row
            .id
            .parse()
            .map_err(|_| BookmarkError::InvalidStoredValue)?,
        track_id: row
            .track_id
            .parse()
            .map_err(|_| BookmarkError::InvalidStoredValue)?,
        name: row.name,
        a_ms: u64::try_from(row.a_ms).map_err(|_| BookmarkError::InvalidStoredValue)?,
        b_ms: u64::try_from(row.b_ms).map_err(|_| BookmarkError::InvalidStoredValue)?,
        created_at: parse_time(&row.created_at)?,
        updated_at: parse_time(&row.updated_at)?,
    })
}

fn parse_time(value: &str) -> Result<DateTime<Utc>, BookmarkError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| BookmarkError::InvalidStoredValue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::TempDatabasePath;
    use crate::domain::{SourceCapabilities, TrackSource, UnifiedTrack, VersionInfo};
    use std::path::PathBuf;

    fn fixture() -> (Database, TrackId) {
        let path = TempDatabasePath::new("bookmarks");
        let database = Database::open(path.path()).unwrap();
        let track_id = TrackId::new();
        let source = TrackSource::new_local(
            crate::domain::SourceId::new(),
            track_id,
            PathBuf::from("C:\\Music\\synthetic.flac"),
            SourceCapabilities::default(),
        )
        .unwrap();
        let track = UnifiedTrack::new(
            track_id,
            "Synthetic Track",
            Vec::new(),
            None,
            Some(10_000),
            VersionInfo::standard(),
            vec![source],
        )
        .unwrap();
        TrackRepository::new(&database).create(&track).unwrap();
        (database, track_id)
    }

    #[test]
    fn bookmarks_are_ordered_and_validate_note_and_duration() {
        let (database, track_id) = fixture();
        let service = BookmarkService::new(database);
        let later = service
            .create_bookmark(track_id, 9_000, "later".to_owned())
            .unwrap();
        let earlier = service
            .create_bookmark(track_id, 1_000, "earlier".to_owned())
            .unwrap();
        assert_eq!(
            service.list_bookmarks(track_id).unwrap(),
            vec![earlier.clone(), later.clone()]
        );
        assert_eq!(
            service
                .create_bookmark(track_id, 10_001, String::new())
                .unwrap_err()
                .to_string(),
            "bookmark position is outside the known track duration"
        );
        assert!(matches!(
            service.create_bookmark(track_id, 100, "x".repeat(MAX_BOOKMARK_NOTE_CHARS + 1)),
            Err(BookmarkError::NoteTooLong)
        ));
        service.delete_bookmark(earlier.id).unwrap();
        service.delete_bookmark(later.id).unwrap();
        assert!(service.list_bookmarks(track_id).unwrap().is_empty());
    }

    #[test]
    fn preset_names_collapse_whitespace_and_are_case_insensitive() {
        let (database, track_id) = fixture();
        let service = BookmarkService::new(database);
        let first = service
            .save_ab_loop_preset(track_id, "  Verse   A ".to_owned(), 1_000, 2_000)
            .unwrap();
        let second = service
            .save_ab_loop_preset(track_id, "VERSE A".to_owned(), 2_000, 3_000)
            .unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(second.name, "VERSE A");
        assert_eq!(service.list_ab_loop_presets(track_id).unwrap().len(), 1);
    }

    #[test]
    fn loop_length_and_duration_are_checked() {
        assert!(matches!(
            validate_loop(0, 249, None),
            Err(BookmarkError::InvalidLoop)
        ));
        assert!(matches!(
            validate_loop(0, 1_001, Some(1_000)),
            Err(BookmarkError::PositionOutsideDuration)
        ));
    }
}
