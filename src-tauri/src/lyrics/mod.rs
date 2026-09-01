pub mod parser;
pub mod providers;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::db::repository::{RepositoryError, TrackRepository};
use crate::db::{Database, DatabaseError};
use crate::domain::{ProviderKind, SourceId, TrackId, TrackSource, UnifiedTrack};
use crate::library::{folders::is_reparse_point, metadata, LibraryService};

use self::parser::{parse_lrc, parse_lrc_bytes, LyricsParseError, ParsedLyrics, MAX_LYRICS_BYTES};
use self::providers::{LrclibProvider, LyricsProviderError};

pub const MAX_MANUAL_LYRICS_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsSourceKind {
    Manual,
    Sidecar,
    Embedded,
    Lrclib,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsSyncKind {
    Plain,
    Timed,
    Instrumental,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsCueDto {
    pub start_ms: u64,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsAttribution {
    pub label: String,
    pub provider: String,
    pub url: Option<Url>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsDocument {
    pub track_id: TrackId,
    pub source: LyricsSourceKind,
    pub sync_kind: LyricsSyncKind,
    pub plain_text: Option<String>,
    pub cues: Vec<LyricsCueDto>,
    pub instrumental: bool,
    pub editable: bool,
    pub attribution: Option<LyricsAttribution>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsCandidate {
    pub provider_record_id: i64,
    pub track_name: String,
    pub artist_name: String,
    pub album_name: Option<String>,
    pub duration_ms: Option<u64>,
    pub instrumental: bool,
    pub has_plain: bool,
    pub has_synced: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ManualLyricsMode {
    Plain,
    Lrc,
}

#[derive(Debug, Error)]
pub enum LyricsError {
    #[error("track {0} was not found")]
    TrackNotFound(TrackId),
    #[error("source {source_id} was not found for track {track_id}")]
    SourceNotFound {
        track_id: TrackId,
        source_id: SourceId,
    },
    #[error("source {source_id} belongs to another track")]
    SourceMismatch {
        track_id: TrackId,
        source_id: SourceId,
    },
    #[error("lyrics are invalid")]
    InvalidLyrics,
    #[error("lyrics input exceeds the supported size limit")]
    InputTooLarge,
    #[error("lyrics input is not valid UTF-8")]
    InvalidUtf8,
    #[error("lyrics import accepts only .lrc and .txt files")]
    UnsupportedImport,
    #[error("selected lyrics file is unavailable")]
    ImportRead,
    #[error("lyrics file selection was cancelled")]
    ImportCancelled,
    #[error("LRCLIB returned no matching lyrics")]
    NotFound,
    #[error("LRCLIB rate limit exceeded")]
    RateLimited { retry_after_seconds: Option<u64> },
    #[error("LRCLIB provider request failed")]
    Provider,
    #[error("LRCLIB candidate is invalid for this track")]
    InvalidCandidate,
    #[error("lyrics cache was not found")]
    CacheNotFound,
    #[error("database operation failed: {0}")]
    Database(#[from] DatabaseError),
    #[error("repository operation failed: {0}")]
    Repository(#[from] RepositoryError),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("local lyrics source failed: {0}")]
    Local(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LyricsErrorCode {
    TrackNotFound,
    SourceNotFound,
    SourceMismatch,
    InvalidLyrics,
    InputTooLarge,
    InvalidUtf8,
    UnsupportedImport,
    ImportRead,
    ImportCancelled,
    NotFound,
    RateLimited,
    Provider,
    InvalidCandidate,
    CacheNotFound,
    Database,
    Local,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsErrorDto {
    pub code: LyricsErrorCode,
    pub detail: String,
    pub retry_after_seconds: Option<u64>,
}

impl LyricsError {
    pub fn code(&self) -> LyricsErrorCode {
        match self {
            Self::TrackNotFound(_) => LyricsErrorCode::TrackNotFound,
            Self::SourceNotFound { .. } => LyricsErrorCode::SourceNotFound,
            Self::SourceMismatch { .. } => LyricsErrorCode::SourceMismatch,
            Self::InvalidLyrics => LyricsErrorCode::InvalidLyrics,
            Self::InputTooLarge => LyricsErrorCode::InputTooLarge,
            Self::InvalidUtf8 => LyricsErrorCode::InvalidUtf8,
            Self::UnsupportedImport => LyricsErrorCode::UnsupportedImport,
            Self::ImportRead => LyricsErrorCode::ImportRead,
            Self::ImportCancelled => LyricsErrorCode::ImportCancelled,
            Self::NotFound => LyricsErrorCode::NotFound,
            Self::RateLimited { .. } => LyricsErrorCode::RateLimited,
            Self::Provider => LyricsErrorCode::Provider,
            Self::InvalidCandidate => LyricsErrorCode::InvalidCandidate,
            Self::CacheNotFound => LyricsErrorCode::CacheNotFound,
            Self::Database(_) | Self::Repository(_) | Self::Sqlite(_) => LyricsErrorCode::Database,
            Self::Local(_) => LyricsErrorCode::Local,
        }
    }

    pub fn dto(&self) -> LyricsErrorDto {
        LyricsErrorDto {
            code: self.code(),
            detail: self.to_string(),
            retry_after_seconds: match self {
                Self::RateLimited {
                    retry_after_seconds,
                } => *retry_after_seconds,
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LyricsLookup {
    pub track_name: String,
    pub artist_name: String,
    pub album_name: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug)]
struct StoredLyrics {
    source_kind: LyricsSourceKind,
    plain_text: Option<String>,
    synced_lrc: Option<String>,
    instrumental: bool,
}

#[derive(Clone)]
pub struct LyricsRepository {
    database: Database,
}

impl LyricsRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    fn get(
        &self,
        track_id: TrackId,
        source_kind: LyricsSourceKind,
    ) -> Result<Option<StoredLyrics>, LyricsError> {
        let source_kind_text = source_kind.as_str();
        self.database
            .with_connection(|connection| {
                connection
                .query_row(
                    "SELECT source_kind, plain_text, synced_lrc, instrumental, provider_record_id
                     FROM lyrics WHERE track_id = ?1 AND source_kind = ?2",
                    params![track_id.to_string(), source_kind_text],
                    |row| {
                        Ok(StoredLyrics {
                            source_kind: parse_source_kind(row.get::<_, String>(0)?)
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            plain_text: row.get(1)?,
                            synced_lrc: row.get(2)?,
                            instrumental: row.get::<_, i64>(3)? != 0,
                        })
                    },
                )
                .optional()
            })
            .map_err(LyricsError::from)
    }

    fn save_manual(
        &self,
        track_id: TrackId,
        plain_text: Option<&str>,
        synced_lrc: Option<&str>,
    ) -> Result<(), LyricsError> {
        let now = Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        self.database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO lyrics (
                    id, track_id, source_kind, plain_text, synced_lrc, instrumental,
                    provider_record_id, created_at, updated_at
                ) VALUES (?1, ?2, 'manual', ?3, ?4, 0, NULL, ?5, ?5)
                ON CONFLICT(track_id, source_kind) DO UPDATE SET
                    plain_text = excluded.plain_text,
                    synced_lrc = excluded.synced_lrc,
                    instrumental = 0,
                    provider_record_id = NULL,
                    updated_at = excluded.updated_at",
                    params![id, track_id.to_string(), plain_text, synced_lrc, now],
                )?;
                Ok(())
            })
            .map_err(LyricsError::from)
    }

    fn delete_manual(&self, track_id: TrackId) -> Result<(), LyricsError> {
        self.database
            .with_connection(|connection| {
                connection.execute(
                    "DELETE FROM lyrics WHERE track_id = ?1 AND source_kind = 'manual'",
                    params![track_id.to_string()],
                )?;
                Ok(())
            })
            .map_err(LyricsError::from)
    }

    fn save_lrclib(
        &self,
        track_id: TrackId,
        provider_record_id: i64,
        plain_text: Option<&str>,
        synced_lrc: Option<&str>,
        instrumental: bool,
    ) -> Result<(), LyricsError> {
        let now = Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        self.database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO lyrics (
                    id, track_id, source_kind, plain_text, synced_lrc, instrumental,
                    provider_record_id, created_at, updated_at
                ) VALUES (?1, ?2, 'lrclib', ?3, ?4, ?5, ?6, ?7, ?7)
                ON CONFLICT(track_id, source_kind) DO UPDATE SET
                    plain_text = excluded.plain_text,
                    synced_lrc = excluded.synced_lrc,
                    instrumental = excluded.instrumental,
                    provider_record_id = excluded.provider_record_id,
                    updated_at = excluded.updated_at",
                    params![
                        id,
                        track_id.to_string(),
                        plain_text,
                        synced_lrc,
                        i64::from(instrumental),
                        provider_record_id,
                        now,
                    ],
                )?;
                Ok(())
            })
            .map_err(LyricsError::from)
    }

    fn clear_lrclib(&self, track_id: TrackId) -> Result<(), LyricsError> {
        self.database
            .with_connection(|connection| {
                connection.execute(
                    "DELETE FROM lyrics WHERE track_id = ?1 AND source_kind = 'lrclib'",
                    params![track_id.to_string()],
                )?;
                Ok(())
            })
            .map_err(LyricsError::from)
    }
}

#[derive(Clone)]
pub struct LyricsService {
    database: Database,
    library: LibraryService,
    repository: LyricsRepository,
    provider: Arc<LrclibProvider>,
}

impl LyricsService {
    pub fn new(database: Database, library: LibraryService) -> Result<Self, LyricsError> {
        let provider = Arc::new(LrclibProvider::new().map_err(|_| LyricsError::Provider)?);
        Ok(Self::with_provider(database, library, provider))
    }

    pub fn with_provider(
        database: Database,
        library: LibraryService,
        provider: Arc<LrclibProvider>,
    ) -> Self {
        Self {
            repository: LyricsRepository::new(database.clone()),
            database,
            library,
            provider,
        }
    }

    pub fn get_lyrics(
        &self,
        track_id: TrackId,
        current_source_id: Option<SourceId>,
    ) -> Result<Option<LyricsDocument>, LyricsError> {
        let track = self.track(track_id)?;
        if let Some(stored) = self.repository.get(track_id, LyricsSourceKind::Manual)? {
            return Ok(Some(self.document_from_stored(track_id, stored)?));
        }

        let local_sources = self.local_sources(&track, current_source_id)?;
        for source in &local_sources {
            let Ok(path) = self.library.resolve_playback_path(track_id, source.id) else {
                continue;
            };
            if let Ok(Some(parsed)) = read_sidecar(&path) {
                return Ok(Some(document_from_parsed(
                    track_id,
                    LyricsSourceKind::Sidecar,
                    parsed,
                    false,
                    false,
                    None,
                )));
            }
        }

        for source in &local_sources {
            let Ok(path) = self.library.resolve_playback_path(track_id, source.id) else {
                continue;
            };
            let Ok(embedded) = metadata::read_embedded_lyrics(&path) else {
                continue;
            };
            if let Some(candidate) = embedded.iter().find(|value| !value.cues.is_empty()) {
                return Ok(Some(document_from_embedded(track_id, candidate)));
            }
        }

        for source in &local_sources {
            let Ok(path) = self.library.resolve_playback_path(track_id, source.id) else {
                continue;
            };
            let Ok(embedded) = metadata::read_embedded_lyrics(&path) else {
                continue;
            };
            if let Some(candidate) = embedded
                .iter()
                .find(|value| value.cues.is_empty() && value.plain_text.is_some())
            {
                return Ok(Some(document_from_embedded(track_id, candidate)));
            }
        }

        self.repository
            .get(track_id, LyricsSourceKind::Lrclib)?
            .map(|stored| self.document_from_stored(track_id, stored))
            .transpose()
    }

    pub fn save_manual_lyrics(
        &self,
        track_id: TrackId,
        mode: ManualLyricsMode,
        text: String,
    ) -> Result<LyricsDocument, LyricsError> {
        self.track(track_id)?;
        if text.len() > MAX_MANUAL_LYRICS_BYTES {
            return Err(LyricsError::InputTooLarge);
        }
        if text.trim().is_empty() {
            return Err(LyricsError::InvalidLyrics);
        }
        match mode {
            ManualLyricsMode::Plain => {
                self.repository.save_manual(track_id, Some(&text), None)?;
            }
            ManualLyricsMode::Lrc => {
                let parsed = parse_lrc(&text).map_err(map_parse_error)?;
                if parsed.cues.is_empty() {
                    return Err(LyricsError::InvalidLyrics);
                }
                self.repository
                    .save_manual(track_id, Some(&parsed.plain_text), Some(&text))?;
            }
        }
        self.get_lyrics(track_id, None)
            .and_then(|document| document.ok_or(LyricsError::InvalidLyrics))
    }

    pub fn delete_manual_lyrics(&self, track_id: TrackId) -> Result<(), LyricsError> {
        self.track(track_id)?;
        self.repository.delete_manual(track_id)
    }

    pub fn import_lyrics_file(
        &self,
        track_id: TrackId,
        path: impl AsRef<Path>,
    ) -> Result<LyricsDocument, LyricsError> {
        self.track(track_id)?;
        let path = path.as_ref();
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !extension.eq_ignore_ascii_case("lrc") && !extension.eq_ignore_ascii_case("txt") {
            return Err(LyricsError::UnsupportedImport);
        }
        let metadata = fs::symlink_metadata(path).map_err(|_| LyricsError::ImportRead)?;
        if metadata.file_type().is_symlink()
            || is_reparse_point(&metadata)
            || !metadata.is_file()
            || metadata.len() > MAX_LYRICS_BYTES as u64
        {
            return Err(if metadata.len() > MAX_LYRICS_BYTES as u64 {
                LyricsError::InputTooLarge
            } else {
                LyricsError::ImportRead
            });
        }
        let bytes = fs::read(path).map_err(|_| LyricsError::ImportRead)?;
        if bytes.len() > MAX_LYRICS_BYTES {
            return Err(LyricsError::InputTooLarge);
        }
        let text = String::from_utf8(bytes).map_err(|_| LyricsError::InvalidUtf8)?;
        if extension.eq_ignore_ascii_case("lrc") {
            if let Ok(parsed) = parse_lrc(&text) {
                if !parsed.cues.is_empty() {
                    return self.save_manual_lyrics(track_id, ManualLyricsMode::Lrc, text);
                }
            }
        }
        self.save_manual_lyrics(track_id, ManualLyricsMode::Plain, text)
    }

    pub async fn find_lrclib_best(&self, track_id: TrackId) -> Result<LyricsDocument, LyricsError> {
        let lookup = self.lookup(track_id)?;
        let record = self
            .provider
            .find_best(&lookup)
            .await
            .map_err(map_provider_error)?;
        self.cache_record(track_id, &lookup, record)
    }

    pub async fn search_lrclib(
        &self,
        track_id: TrackId,
    ) -> Result<Vec<LyricsCandidate>, LyricsError> {
        let lookup = self.lookup(track_id)?;
        self.provider
            .search(&lookup)
            .await
            .map_err(map_provider_error)
    }

    pub async fn select_lrclib_candidate(
        &self,
        track_id: TrackId,
        provider_record_id: i64,
    ) -> Result<LyricsDocument, LyricsError> {
        let lookup = self.lookup(track_id)?;
        let record = self
            .provider
            .get(provider_record_id)
            .await
            .map_err(map_provider_error)?;
        self.cache_record(track_id, &lookup, record)
    }

    pub fn clear_cached_lrclib(&self, track_id: TrackId) -> Result<(), LyricsError> {
        self.track(track_id)?;
        self.repository.clear_lrclib(track_id)
    }

    fn cache_record(
        &self,
        track_id: TrackId,
        lookup: &LyricsLookup,
        record: providers::LrclibRecord,
    ) -> Result<LyricsDocument, LyricsError> {
        validate_provider_record(&record, lookup)?;
        let provider_record_id = record.id.ok_or(LyricsError::InvalidCandidate)?;
        let synced_lrc = record
            .synced_lyrics
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned);
        let plain_text = record
            .plain_lyrics
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned);
        let instrumental = record.instrumental.unwrap_or(false);
        if !instrumental && synced_lrc.is_none() && plain_text.is_none() {
            return Err(LyricsError::InvalidLyrics);
        }
        self.repository.save_lrclib(
            track_id,
            provider_record_id,
            plain_text.as_deref(),
            synced_lrc.as_deref(),
            instrumental,
        )?;
        self.get_lyrics(track_id, None)
            .and_then(|document| document.ok_or(LyricsError::InvalidLyrics))
            .map(|document| {
                if document.source == LyricsSourceKind::Lrclib {
                    document
                } else {
                    document_from_provider_record(track_id, plain_text, synced_lrc, instrumental)
                }
            })
    }

    fn track(&self, track_id: TrackId) -> Result<UnifiedTrack, LyricsError> {
        TrackRepository::new(&self.database)
            .get(track_id)?
            .ok_or(LyricsError::TrackNotFound(track_id))
    }

    fn lookup(&self, track_id: TrackId) -> Result<LyricsLookup, LyricsError> {
        let track = self.track(track_id)?;
        Ok(LyricsLookup {
            track_name: track.title,
            artist_name: track
                .artists
                .first()
                .map(|artist| artist.name.clone())
                .unwrap_or_else(|| "Unknown Artist".to_owned()),
            album_name: track.album.map(|album| album.title),
            duration_ms: track.duration_ms,
        })
    }

    fn local_sources(
        &self,
        track: &UnifiedTrack,
        current_source_id: Option<SourceId>,
    ) -> Result<Vec<TrackSource>, LyricsError> {
        let mut sources = Vec::new();
        if let Some(source_id) = current_source_id {
            let source = track
                .sources
                .iter()
                .find(|source| source.id == source_id)
                .ok_or(LyricsError::SourceNotFound {
                    track_id: track.id,
                    source_id,
                })?;
            if source.provider_kind == ProviderKind::Local {
                sources.push(source.clone());
            }
        }
        if let Some(source_id) = track.preferred_source_id {
            if let Some(source) = track.sources.iter().find(|source| {
                source.id == source_id && source.provider_kind == ProviderKind::Local
            }) {
                if !sources
                    .iter()
                    .any(|item: &TrackSource| item.id == source.id)
                {
                    sources.push(source.clone());
                }
            }
        }
        let mut remaining = track
            .sources
            .iter()
            .filter(|source| {
                source.provider_kind == ProviderKind::Local
                    && !sources
                        .iter()
                        .any(|item: &TrackSource| item.id == source.id)
            })
            .cloned()
            .collect::<Vec<_>>();
        remaining.sort_by_key(|source| source.id.to_string());
        sources.extend(remaining);
        Ok(sources)
    }

    fn document_from_stored(
        &self,
        track_id: TrackId,
        stored: StoredLyrics,
    ) -> Result<LyricsDocument, LyricsError> {
        if stored.instrumental {
            return Ok(LyricsDocument {
                track_id,
                source: stored.source_kind,
                sync_kind: LyricsSyncKind::Instrumental,
                plain_text: None,
                cues: Vec::new(),
                instrumental: true,
                editable: stored.source_kind == LyricsSourceKind::Manual,
                attribution: attribution_for(stored.source_kind),
            });
        }
        if let Some(synced_lrc) = stored.synced_lrc.as_deref() {
            if let Ok(parsed) = parse_lrc(synced_lrc) {
                if !parsed.cues.is_empty() {
                    return Ok(document_from_parsed(
                        track_id,
                        stored.source_kind,
                        parsed,
                        false,
                        stored.source_kind == LyricsSourceKind::Manual,
                        attribution_for(stored.source_kind),
                    ));
                }
            }
        }
        let Some(plain_text) = stored.plain_text else {
            return Err(LyricsError::InvalidLyrics);
        };
        Ok(LyricsDocument {
            track_id,
            source: stored.source_kind,
            sync_kind: LyricsSyncKind::Plain,
            plain_text: Some(plain_text),
            cues: Vec::new(),
            instrumental: false,
            editable: stored.source_kind == LyricsSourceKind::Manual,
            attribution: attribution_for(stored.source_kind),
        })
    }
}

impl LyricsSourceKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Sidecar => "sidecar",
            Self::Embedded => "embedded",
            Self::Lrclib => "lrclib",
        }
    }
}

fn parse_source_kind(value: String) -> Result<LyricsSourceKind, ()> {
    match value.as_str() {
        "manual" => Ok(LyricsSourceKind::Manual),
        "lrclib" => Ok(LyricsSourceKind::Lrclib),
        _ => Err(()),
    }
}

fn read_sidecar(media_path: &Path) -> Result<Option<ParsedLyrics>, LyricsError> {
    let sidecar = media_path.with_extension("lrc");
    let metadata = match fs::symlink_metadata(&sidecar) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(LyricsError::Local("sidecar is unavailable".to_owned())),
    };
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) || !metadata.is_file() {
        return Err(LyricsError::Local(
            "sidecar is not a regular file".to_owned(),
        ));
    }
    if metadata.len() > MAX_LYRICS_BYTES as u64 {
        return Err(LyricsError::InputTooLarge);
    }
    let bytes = fs::read(&sidecar)
        .map_err(|_| LyricsError::Local("sidecar could not be read".to_owned()))?;
    parse_lrc_bytes(&bytes).map(Some).map_err(map_parse_error)
}

fn document_from_parsed(
    track_id: TrackId,
    source: LyricsSourceKind,
    parsed: ParsedLyrics,
    instrumental: bool,
    editable: bool,
    attribution: Option<LyricsAttribution>,
) -> LyricsDocument {
    let sync_kind = if instrumental {
        LyricsSyncKind::Instrumental
    } else if parsed.cues.is_empty() {
        LyricsSyncKind::Plain
    } else {
        LyricsSyncKind::Timed
    };
    LyricsDocument {
        track_id,
        source,
        sync_kind,
        plain_text: (!parsed.plain_text.trim().is_empty()).then_some(parsed.plain_text),
        cues: parsed
            .cues
            .into_iter()
            .map(|cue| LyricsCueDto {
                start_ms: cue.start_ms,
                lines: cue.lines,
            })
            .collect(),
        instrumental,
        editable,
        attribution,
    }
}

fn document_from_embedded(
    track_id: TrackId,
    embedded: &metadata::EmbeddedLyrics,
) -> LyricsDocument {
    document_from_parsed(
        track_id,
        LyricsSourceKind::Embedded,
        ParsedLyrics {
            plain_text: embedded.plain_text.clone().unwrap_or_default(),
            cues: embedded.cues.clone(),
        },
        false,
        false,
        None,
    )
}

fn document_from_provider_record(
    track_id: TrackId,
    plain_text: Option<String>,
    synced_lrc: Option<String>,
    instrumental: bool,
) -> LyricsDocument {
    if instrumental {
        return LyricsDocument {
            track_id,
            source: LyricsSourceKind::Lrclib,
            sync_kind: LyricsSyncKind::Instrumental,
            plain_text: None,
            cues: Vec::new(),
            instrumental: true,
            editable: false,
            attribution: attribution_for(LyricsSourceKind::Lrclib),
        };
    }
    let parsed = synced_lrc
        .as_deref()
        .and_then(|value| parse_lrc(value).ok())
        .filter(|value| !value.cues.is_empty());
    if let Some(parsed) = parsed {
        return document_from_parsed(
            track_id,
            LyricsSourceKind::Lrclib,
            ParsedLyrics {
                plain_text: plain_text.unwrap_or(parsed.plain_text),
                cues: parsed.cues,
            },
            false,
            false,
            attribution_for(LyricsSourceKind::Lrclib),
        );
    }
    LyricsDocument {
        track_id,
        source: LyricsSourceKind::Lrclib,
        sync_kind: LyricsSyncKind::Plain,
        plain_text,
        cues: Vec::new(),
        instrumental: false,
        editable: false,
        attribution: attribution_for(LyricsSourceKind::Lrclib),
    }
}

fn attribution_for(source: LyricsSourceKind) -> Option<LyricsAttribution> {
    (source == LyricsSourceKind::Lrclib).then(|| LyricsAttribution {
        label: "Lyrics from LRCLIB".to_owned(),
        provider: "LRCLIB".to_owned(),
        url: Url::parse("https://lrclib.net").ok(),
    })
}

fn map_parse_error(error: LyricsParseError) -> LyricsError {
    match error {
        LyricsParseError::InputTooLarge => LyricsError::InputTooLarge,
        LyricsParseError::InvalidUtf8 => LyricsError::InvalidUtf8,
        LyricsParseError::TooManyCues | LyricsParseError::TimestampOverflow => {
            LyricsError::InvalidLyrics
        }
    }
}

fn map_provider_error(error: LyricsProviderError) -> LyricsError {
    match error {
        LyricsProviderError::NotFound => LyricsError::NotFound,
        LyricsProviderError::RateLimited {
            retry_after_seconds,
        } => LyricsError::RateLimited {
            retry_after_seconds,
        },
        LyricsProviderError::OversizedResponse => LyricsError::InputTooLarge,
        LyricsProviderError::InvalidResponse => LyricsError::Provider,
        LyricsProviderError::Timeout
        | LyricsProviderError::Network
        | LyricsProviderError::UnsafeUrl => LyricsError::Provider,
    }
}

fn validate_provider_record(
    record: &providers::LrclibRecord,
    lookup: &LyricsLookup,
) -> Result<(), LyricsError> {
    let Some(track_name) = record.track_name.as_deref() else {
        return Err(LyricsError::InvalidCandidate);
    };
    let Some(artist_name) = record.artist_name.as_deref() else {
        return Err(LyricsError::InvalidCandidate);
    };
    if normalize_lookup_text(track_name) != normalize_lookup_text(&lookup.track_name)
        || normalize_lookup_text(artist_name) != normalize_lookup_text(&lookup.artist_name)
    {
        return Err(LyricsError::InvalidCandidate);
    }
    if let (Some(expected), Some(actual)) = (
        lookup.duration_ms,
        providers::duration_ms(record.duration).map_err(|_| LyricsError::InvalidCandidate)?,
    ) {
        let delta = expected.abs_diff(actual);
        if delta > 10_000 {
            return Err(LyricsError::InvalidCandidate);
        }
    }
    Ok(())
}

fn normalize_lookup_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_and_sync_contracts_serialize_as_stable_strings() {
        assert_eq!(
            serde_json::to_value(LyricsSourceKind::Lrclib).unwrap(),
            "lrclib"
        );
        assert_eq!(
            serde_json::to_value(LyricsSyncKind::Timed).unwrap(),
            "timed"
        );
    }

    #[test]
    fn attribution_is_only_added_for_lrclib() {
        assert!(attribution_for(LyricsSourceKind::Sidecar).is_none());
        assert_eq!(
            attribution_for(LyricsSourceKind::Lrclib).unwrap().label,
            "Lyrics from LRCLIB"
        );
    }

    #[test]
    fn manual_lrc_requires_a_valid_timed_cue() {
        let parsed = parse_lrc("[metadata]plain").unwrap();
        assert!(parsed.cues.is_empty());
        assert!(matches!(
            map_parse_error(LyricsParseError::TooManyCues),
            LyricsError::InvalidLyrics
        ));
    }
}
