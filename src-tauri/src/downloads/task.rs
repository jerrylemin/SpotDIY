use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::db::{Database, DatabaseError};
use crate::domain::{ProviderKind, SourceId, TrackId};
use crate::search::types::ProviderRuntimeStatus;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct DownloadTaskId(Uuid);

impl DownloadTaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for DownloadTaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DownloadTaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl From<Uuid> for DownloadTaskId {
    fn from(value: Uuid) -> Self {
        Self::from_uuid(value)
    }
}

impl FromStr for DownloadTaskId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self::from_uuid)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadMode {
    Audio,
    Video,
}

impl DownloadMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

impl FromStr for DownloadMode {
    type Err = DownloadValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "audio" => Ok(Self::Audio),
            "video" => Ok(Self::Video),
            _ => Err(DownloadValueError::InvalidMode(value.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    Queued,
    Resolving,
    Downloading,
    Postprocessing,
    Completed,
    Failed,
    Cancelled,
}

impl DownloadState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Resolving => "resolving",
            Self::Downloading => "downloading",
            Self::Postprocessing => "postprocessing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::Resolving | Self::Downloading | Self::Postprocessing
        )
    }
}

impl FromStr for DownloadState {
    type Err = DownloadValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "resolving" => Ok(Self::Resolving),
            "downloading" => Ok(Self::Downloading),
            "postprocessing" => Ok(Self::Postprocessing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(DownloadValueError::InvalidState(value.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceQualityProvenance {
    ProviderEncoded,
    Unknown,
}

impl SourceQualityProvenance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProviderEncoded => "provider_encoded",
            Self::Unknown => "unknown",
        }
    }
}

impl FromStr for SourceQualityProvenance {
    type Err = DownloadValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "provider_encoded" => Ok(Self::ProviderEncoded),
            "unknown" => Ok(Self::Unknown),
            _ => Err(DownloadValueError::InvalidProvenance(value.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadErrorCode {
    InvalidRequest,
    UnsupportedProvider,
    InvalidProviderUrl,
    DownloadDirectoryNotConfigured,
    DownloadDirectoryInvalid,
    SourceNotFound,
    SourceTrackMismatch,
    ToolMissing,
    ToolBroken,
    ProcessFailed,
    OutputInvalid,
    FinalizationFailed,
    Cancelled,
    PersistenceFailed,
    ShuttingDown,
    Unknown,
}

impl DownloadErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalidRequest",
            Self::UnsupportedProvider => "unsupportedProvider",
            Self::InvalidProviderUrl => "invalidProviderUrl",
            Self::DownloadDirectoryNotConfigured => "downloadDirectoryNotConfigured",
            Self::DownloadDirectoryInvalid => "downloadDirectoryInvalid",
            Self::SourceNotFound => "sourceNotFound",
            Self::SourceTrackMismatch => "sourceTrackMismatch",
            Self::ToolMissing => "toolMissing",
            Self::ToolBroken => "toolBroken",
            Self::ProcessFailed => "processFailed",
            Self::OutputInvalid => "outputInvalid",
            Self::FinalizationFailed => "finalizationFailed",
            Self::Cancelled => "cancelled",
            Self::PersistenceFailed => "persistenceFailed",
            Self::ShuttingDown => "shuttingDown",
            Self::Unknown => "unknown",
        }
    }
}

impl FromStr for DownloadErrorCode {
    type Err = DownloadValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "invalidRequest" => Ok(Self::InvalidRequest),
            "unsupportedProvider" => Ok(Self::UnsupportedProvider),
            "invalidProviderUrl" => Ok(Self::InvalidProviderUrl),
            "downloadDirectoryNotConfigured" => Ok(Self::DownloadDirectoryNotConfigured),
            "downloadDirectoryInvalid" => Ok(Self::DownloadDirectoryInvalid),
            "sourceNotFound" => Ok(Self::SourceNotFound),
            "sourceTrackMismatch" => Ok(Self::SourceTrackMismatch),
            "toolMissing" => Ok(Self::ToolMissing),
            "toolBroken" => Ok(Self::ToolBroken),
            "processFailed" => Ok(Self::ProcessFailed),
            "outputInvalid" => Ok(Self::OutputInvalid),
            "finalizationFailed" => Ok(Self::FinalizationFailed),
            "cancelled" => Ok(Self::Cancelled),
            "persistenceFailed" => Ok(Self::PersistenceFailed),
            "shuttingDown" => Ok(Self::ShuttingDown),
            "unknown" => Ok(Self::Unknown),
            _ => Err(DownloadValueError::InvalidErrorCode(value.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: DownloadTaskId,
    pub provider_kind: ProviderKind,
    pub provider_item_id: String,
    pub canonical_url: String,
    pub target_track_id: Option<TrackId>,
    pub target_source_id: Option<SourceId>,
    pub title: String,
    pub artists: Vec<String>,
    pub artwork_url: Option<String>,
    pub mode: DownloadMode,
    pub state: DownloadState,
    pub destination_directory: PathBuf,
    pub output_path: Option<PathBuf>,
    pub output_extension: Option<String>,
    pub output_codec: Option<String>,
    pub source_quality_provenance: SourceQualityProvenance,
    pub transcoded: bool,
    pub expected_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub progress_permille: u16,
    pub speed_bytes_per_second: Option<u64>,
    pub eta_seconds: Option<u64>,
    pub retry_count: u32,
    pub error_code: Option<DownloadErrorCode>,
    pub error_detail: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub output_missing: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadRequest {
    pub provider_kind: ProviderKind,
    pub provider_item_id: String,
    pub canonical_url: String,
    pub target_track_id: Option<TrackId>,
    pub target_source_id: Option<SourceId>,
    pub title: String,
    pub artists: Vec<String>,
    pub artwork_url: Option<String>,
    pub mode: DownloadMode,
}

impl DownloadTask {
    pub fn from_request(request: DownloadRequest, destination_directory: PathBuf) -> Self {
        let now = Utc::now();
        Self {
            id: DownloadTaskId::new(),
            provider_kind: request.provider_kind,
            provider_item_id: request.provider_item_id,
            canonical_url: request.canonical_url,
            target_track_id: request.target_track_id,
            target_source_id: request.target_source_id,
            title: request.title,
            artists: request.artists,
            artwork_url: request.artwork_url,
            mode: request.mode,
            state: DownloadState::Queued,
            destination_directory,
            output_path: None,
            output_extension: None,
            output_codec: None,
            source_quality_provenance: SourceQualityProvenance::ProviderEncoded,
            transcoded: false,
            expected_bytes: None,
            downloaded_bytes: 0,
            progress_permille: 0,
            speed_bytes_per_second: None,
            eta_seconds: None,
            retry_count: 0,
            error_code: None,
            error_detail: None,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            output_missing: false,
        }
    }

    pub fn transition(&mut self, next: DownloadState) -> Result<(), DownloadTransitionError> {
        if !is_valid_transition(self.state, next) {
            return Err(DownloadTransitionError {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        self.updated_at = Utc::now();
        match next {
            DownloadState::Resolving | DownloadState::Downloading => {
                if self.started_at.is_none() {
                    self.started_at = Some(self.updated_at);
                }
            }
            DownloadState::Completed => {
                self.completed_at = Some(self.updated_at);
                self.progress_permille = 1000;
                self.error_code = None;
                self.error_detail = None;
            }
            DownloadState::Failed | DownloadState::Cancelled => {
                self.completed_at = None;
            }
            DownloadState::Queued | DownloadState::Postprocessing => {}
        }
        Ok(())
    }

    pub fn prepare_retry(&mut self) -> Result<(), DownloadTransitionError> {
        if !matches!(self.state, DownloadState::Failed | DownloadState::Cancelled) {
            return Err(DownloadTransitionError {
                from: self.state,
                to: DownloadState::Queued,
            });
        }
        self.transition(DownloadState::Queued)?;
        self.retry_count = self.retry_count.saturating_add(1);
        self.output_path = None;
        self.output_extension = None;
        self.output_codec = None;
        self.expected_bytes = None;
        self.downloaded_bytes = 0;
        self.progress_permille = 0;
        self.speed_bytes_per_second = None;
        self.eta_seconds = None;
        self.error_code = None;
        self.error_detail = None;
        self.started_at = None;
        self.completed_at = None;
        self.output_missing = false;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub(crate) fn requeue_after_interruption(&mut self) {
        if self.state.is_active() {
            self.state = DownloadState::Queued;
            self.updated_at = Utc::now();
            self.completed_at = None;
        }
    }
}

pub const fn is_valid_transition(from: DownloadState, to: DownloadState) -> bool {
    matches!(
        (from, to),
        (DownloadState::Queued, DownloadState::Resolving)
            | (DownloadState::Queued, DownloadState::Cancelled)
            | (DownloadState::Resolving, DownloadState::Downloading)
            | (DownloadState::Resolving, DownloadState::Failed)
            | (DownloadState::Resolving, DownloadState::Cancelled)
            | (DownloadState::Downloading, DownloadState::Postprocessing)
            | (DownloadState::Downloading, DownloadState::Failed)
            | (DownloadState::Downloading, DownloadState::Cancelled)
            | (DownloadState::Postprocessing, DownloadState::Completed)
            | (DownloadState::Postprocessing, DownloadState::Failed)
            | (DownloadState::Postprocessing, DownloadState::Cancelled)
            | (DownloadState::Failed, DownloadState::Queued)
            | (DownloadState::Cancelled, DownloadState::Queued)
    )
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadToolStatus {
    pub status: ProviderRuntimeStatus,
    pub version: Option<String>,
    pub detail: Option<String>,
}

impl Default for DownloadToolStatus {
    fn default() -> Self {
        Self {
            status: ProviderRuntimeStatus::Unknown,
            version: None,
            detail: None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaToolsSnapshot {
    pub yt_dlp: DownloadToolStatus,
    pub ffmpeg: DownloadToolStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSnapshot {
    pub revision: u64,
    pub tasks: Vec<DownloadTask>,
    pub max_concurrent: u8,
    pub downloads_directory: Option<PathBuf>,
    pub tools: MediaToolsSnapshot,
}

impl Default for DownloadSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            tasks: Vec::new(),
            max_concurrent: 2,
            downloads_directory: None,
            tools: MediaToolsSnapshot::default(),
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DownloadValueError {
    #[error("invalid download mode {0}")]
    InvalidMode(String),
    #[error("invalid download state {0}")]
    InvalidState(String),
    #[error("invalid source-quality provenance {0}")]
    InvalidProvenance(String),
    #[error("invalid download error code {0}")]
    InvalidErrorCode(String),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("invalid download transition from {from:?} to {to:?}")]
pub struct DownloadTransitionError {
    pub from: DownloadState,
    pub to: DownloadState,
}

#[derive(Debug, Error)]
pub enum DownloadRepositoryError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("database operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not serialize download artists: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("invalid persisted download field {field}: {value}")]
    InvalidValue { field: &'static str, value: String },
}

pub struct DownloadRepository<'database> {
    database: &'database Database,
}

impl<'database> DownloadRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn insert(&self, task: &DownloadTask) -> Result<(), DownloadRepositoryError> {
        let connection = self.database.connection()?;
        connection.execute(
            "INSERT INTO downloads (
                id, provider_kind, provider_item_id, canonical_url, target_track_id,
                target_source_id, title, artists_json, artwork_url, mode, state,
                destination_directory, output_path, output_extension, output_codec,
                source_quality_provenance, transcoded, expected_bytes, downloaded_bytes,
                progress_permille, speed_bytes_per_second, eta_seconds, retry_count,
                error_code, error_detail, created_at, updated_at, started_at, completed_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29
             )",
            rusqlite::params_from_iter(task_values(task)?),
        )?;
        Ok(())
    }

    pub fn get(&self, id: DownloadTaskId) -> Result<Option<DownloadTask>, DownloadRepositoryError> {
        let connection = self.database.connection()?;
        connection
            .query_row(
                &format!("{DOWNLOAD_SELECT} WHERE id = ?1"),
                params![id.to_string()],
                map_download_row,
            )
            .optional()?
            .map(parse_download_row)
            .transpose()
    }

    pub fn list(&self) -> Result<Vec<DownloadTask>, DownloadRepositoryError> {
        let connection = self.database.connection()?;
        let mut statement = connection.prepare(&format!(
            "{DOWNLOAD_SELECT} ORDER BY created_at ASC, id ASC"
        ))?;
        let rows = statement.query_map([], map_download_row)?;
        rows.map(|row| {
            row.map_err(DownloadRepositoryError::from)
                .and_then(parse_download_row)
        })
        .collect()
    }

    pub fn queued(&self) -> Result<Vec<DownloadTask>, DownloadRepositoryError> {
        let connection = self.database.connection()?;
        let mut statement = connection.prepare(&format!(
            "{DOWNLOAD_SELECT} WHERE state = 'queued' ORDER BY created_at ASC, id ASC"
        ))?;
        let rows = statement.query_map([], map_download_row)?;
        rows.map(|row| {
            row.map_err(DownloadRepositoryError::from)
                .and_then(parse_download_row)
        })
        .collect()
    }

    pub fn update(&self, task: &DownloadTask) -> Result<(), DownloadRepositoryError> {
        let connection = self.database.connection()?;
        connection.execute(
            "UPDATE downloads SET
                provider_kind = ?2, provider_item_id = ?3, canonical_url = ?4,
                target_track_id = ?5, target_source_id = ?6, title = ?7, artists_json = ?8,
                artwork_url = ?9, mode = ?10, state = ?11, destination_directory = ?12,
                output_path = ?13, output_extension = ?14, output_codec = ?15,
                source_quality_provenance = ?16, transcoded = ?17, expected_bytes = ?18,
                downloaded_bytes = ?19, progress_permille = ?20,
                speed_bytes_per_second = ?21, eta_seconds = ?22, retry_count = ?23,
                error_code = ?24, error_detail = ?25, created_at = ?26, updated_at = ?27,
                started_at = ?28, completed_at = ?29
             WHERE id = ?1",
            rusqlite::params_from_iter(task_values(task)?),
        )?;
        Ok(())
    }

    pub fn max_concurrent(&self) -> Result<u8, DownloadRepositoryError> {
        let connection = self.database.connection()?;
        let value: i64 = connection.query_row(
            "SELECT max_concurrent FROM download_settings WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        u8::try_from(value).map_err(|_| DownloadRepositoryError::InvalidValue {
            field: "download_settings.max_concurrent",
            value: value.to_string(),
        })
    }

    pub fn set_max_concurrent(&self, value: u8) -> Result<(), DownloadRepositoryError> {
        if !(1..=4).contains(&value) {
            return Err(DownloadRepositoryError::InvalidValue {
                field: "download_settings.max_concurrent",
                value: value.to_string(),
            });
        }
        let connection = self.database.connection()?;
        connection.execute(
            "UPDATE download_settings SET max_concurrent = ?1 WHERE id = 1",
            params![value],
        )?;
        Ok(())
    }

    pub fn recover_interrupted(&self) -> Result<Vec<DownloadTaskId>, DownloadRepositoryError> {
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        let mut statement = transaction.prepare(
            "SELECT id FROM downloads
             WHERE state IN ('resolving', 'downloading', 'postprocessing')
             ORDER BY created_at ASC, id ASC",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let now = Utc::now().to_rfc3339();
        transaction.execute(
            "UPDATE downloads SET
                state = 'queued', started_at = NULL, completed_at = NULL,
                expected_bytes = NULL, downloaded_bytes = 0, progress_permille = 0,
                speed_bytes_per_second = NULL, eta_seconds = NULL,
                error_code = NULL, error_detail = NULL, updated_at = ?1
             WHERE state IN ('resolving', 'downloading', 'postprocessing')",
            params![now],
        )?;
        transaction.commit()?;
        ids.into_iter()
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| DownloadRepositoryError::InvalidValue {
                        field: "downloads.id",
                        value,
                    })
            })
            .collect()
    }
}

const DOWNLOAD_SELECT: &str = "SELECT
    id, provider_kind, provider_item_id, canonical_url, target_track_id, target_source_id,
    title, artists_json, artwork_url, mode, state, destination_directory, output_path,
    output_extension, output_codec, source_quality_provenance, transcoded, expected_bytes,
    downloaded_bytes, progress_permille, speed_bytes_per_second, eta_seconds, retry_count,
    error_code, error_detail, created_at, updated_at, started_at, completed_at
    FROM downloads";

fn task_values(
    task: &DownloadTask,
) -> Result<Vec<rusqlite::types::Value>, DownloadRepositoryError> {
    let artists_json = serde_json::to_string(&task.artists)?;
    Ok(vec![
        rusqlite::types::Value::Text(task.id.to_string()),
        rusqlite::types::Value::Text(task.provider_kind.as_str().to_owned()),
        rusqlite::types::Value::Text(task.provider_item_id.clone()),
        rusqlite::types::Value::Text(task.canonical_url.clone()),
        optional_text(task.target_track_id.map(|value| value.to_string())),
        optional_text(task.target_source_id.map(|value| value.to_string())),
        rusqlite::types::Value::Text(task.title.clone()),
        rusqlite::types::Value::Text(artists_json),
        optional_text(task.artwork_url.clone()),
        rusqlite::types::Value::Text(task.mode.as_str().to_owned()),
        rusqlite::types::Value::Text(task.state.as_str().to_owned()),
        rusqlite::types::Value::Text(task.destination_directory.to_string_lossy().into_owned()),
        optional_text(
            task.output_path
                .as_ref()
                .map(|value| value.to_string_lossy().into_owned()),
        ),
        optional_text(task.output_extension.clone()),
        optional_text(task.output_codec.clone()),
        rusqlite::types::Value::Text(task.source_quality_provenance.as_str().to_owned()),
        rusqlite::types::Value::Integer(i64::from(task.transcoded)),
        optional_i64(task.expected_bytes, "downloads.expected_bytes")?,
        rusqlite::types::Value::Integer(numeric_i64(
            task.downloaded_bytes,
            "downloads.downloaded_bytes",
        )?),
        rusqlite::types::Value::Integer(i64::from(task.progress_permille)),
        optional_i64(
            task.speed_bytes_per_second,
            "downloads.speed_bytes_per_second",
        )?,
        optional_i64(task.eta_seconds, "downloads.eta_seconds")?,
        rusqlite::types::Value::Integer(numeric_i64(
            task.retry_count.into(),
            "downloads.retry_count",
        )?),
        optional_text(task.error_code.map(|value| value.as_str().to_owned())),
        optional_text(task.error_detail.clone()),
        rusqlite::types::Value::Text(task.created_at.to_rfc3339()),
        rusqlite::types::Value::Text(task.updated_at.to_rfc3339()),
        optional_text(task.started_at.map(|value| value.to_rfc3339())),
        optional_text(task.completed_at.map(|value| value.to_rfc3339())),
    ])
}

fn optional_text(value: Option<String>) -> rusqlite::types::Value {
    value.map_or(rusqlite::types::Value::Null, rusqlite::types::Value::Text)
}

fn optional_i64(
    value: Option<u64>,
    field: &'static str,
) -> Result<rusqlite::types::Value, DownloadRepositoryError> {
    value
        .map(|value| numeric_i64(value, field).map(rusqlite::types::Value::Integer))
        .transpose()
        .map(|value| value.unwrap_or(rusqlite::types::Value::Null))
}

fn numeric_i64(value: u64, field: &'static str) -> Result<i64, DownloadRepositoryError> {
    i64::try_from(value).map_err(|_| DownloadRepositoryError::InvalidValue {
        field,
        value: value.to_string(),
    })
}

struct RawDownloadRow {
    id: String,
    provider_kind: String,
    provider_item_id: String,
    canonical_url: String,
    target_track_id: Option<String>,
    target_source_id: Option<String>,
    title: String,
    artists_json: String,
    artwork_url: Option<String>,
    mode: String,
    state: String,
    destination_directory: String,
    output_path: Option<String>,
    output_extension: Option<String>,
    output_codec: Option<String>,
    source_quality_provenance: String,
    transcoded: i64,
    expected_bytes: Option<i64>,
    downloaded_bytes: i64,
    progress_permille: i64,
    speed_bytes_per_second: Option<i64>,
    eta_seconds: Option<i64>,
    retry_count: i64,
    error_code: Option<String>,
    error_detail: Option<String>,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
}

fn map_download_row(row: &Row<'_>) -> rusqlite::Result<RawDownloadRow> {
    Ok(RawDownloadRow {
        id: row.get(0)?,
        provider_kind: row.get(1)?,
        provider_item_id: row.get(2)?,
        canonical_url: row.get(3)?,
        target_track_id: row.get(4)?,
        target_source_id: row.get(5)?,
        title: row.get(6)?,
        artists_json: row.get(7)?,
        artwork_url: row.get(8)?,
        mode: row.get(9)?,
        state: row.get(10)?,
        destination_directory: row.get(11)?,
        output_path: row.get(12)?,
        output_extension: row.get(13)?,
        output_codec: row.get(14)?,
        source_quality_provenance: row.get(15)?,
        transcoded: row.get(16)?,
        expected_bytes: row.get(17)?,
        downloaded_bytes: row.get(18)?,
        progress_permille: row.get(19)?,
        speed_bytes_per_second: row.get(20)?,
        eta_seconds: row.get(21)?,
        retry_count: row.get(22)?,
        error_code: row.get(23)?,
        error_detail: row.get(24)?,
        created_at: row.get(25)?,
        updated_at: row.get(26)?,
        started_at: row.get(27)?,
        completed_at: row.get(28)?,
    })
}

fn parse_download_row(row: RawDownloadRow) -> Result<DownloadTask, DownloadRepositoryError> {
    let provider_kind =
        row.provider_kind
            .parse()
            .map_err(|_| DownloadRepositoryError::InvalidValue {
                field: "downloads.provider_kind",
                value: row.provider_kind.clone(),
            })?;
    let artists = serde_json::from_str(&row.artists_json)?;
    let state = row
        .state
        .parse()
        .map_err(|_| DownloadRepositoryError::InvalidValue {
            field: "downloads.state",
            value: row.state.clone(),
        })?;
    let output_path = row.output_path.map(PathBuf::from);
    let output_missing = state == DownloadState::Completed
        && output_path.as_ref().map_or(true, |path| {
            fs::symlink_metadata(path)
                .map(|metadata| metadata.file_type().is_symlink() || !metadata.is_file())
                .unwrap_or(true)
        });
    let task = DownloadTask {
        id: row
            .id
            .parse()
            .map_err(|_| DownloadRepositoryError::InvalidValue {
                field: "downloads.id",
                value: row.id.clone(),
            })?,
        provider_kind,
        provider_item_id: row.provider_item_id,
        canonical_url: row.canonical_url,
        target_track_id: parse_optional_id(row.target_track_id, "downloads.target_track_id")?,
        target_source_id: parse_optional_id(row.target_source_id, "downloads.target_source_id")?,
        title: row.title,
        artists,
        artwork_url: row.artwork_url,
        mode: row
            .mode
            .parse()
            .map_err(|_| DownloadRepositoryError::InvalidValue {
                field: "downloads.mode",
                value: row.mode.clone(),
            })?,
        state,
        destination_directory: PathBuf::from(row.destination_directory),
        output_path,
        output_extension: row.output_extension,
        output_codec: row.output_codec,
        source_quality_provenance: row.source_quality_provenance.parse().map_err(|_| {
            DownloadRepositoryError::InvalidValue {
                field: "downloads.source_quality_provenance",
                value: row.source_quality_provenance.clone(),
            }
        })?,
        transcoded: parse_bool(row.transcoded, "downloads.transcoded")?,
        expected_bytes: parse_optional_u64(row.expected_bytes, "downloads.expected_bytes")?,
        downloaded_bytes: parse_u64(row.downloaded_bytes, "downloads.downloaded_bytes")?,
        progress_permille: u16::try_from(row.progress_permille).map_err(|_| {
            DownloadRepositoryError::InvalidValue {
                field: "downloads.progress_permille",
                value: row.progress_permille.to_string(),
            }
        })?,
        speed_bytes_per_second: parse_optional_u64(
            row.speed_bytes_per_second,
            "downloads.speed_bytes_per_second",
        )?,
        eta_seconds: parse_optional_u64(row.eta_seconds, "downloads.eta_seconds")?,
        retry_count: u32::try_from(row.retry_count).map_err(|_| {
            DownloadRepositoryError::InvalidValue {
                field: "downloads.retry_count",
                value: row.retry_count.to_string(),
            }
        })?,
        error_code: row
            .error_code
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| DownloadRepositoryError::InvalidValue {
                        field: "downloads.error_code",
                        value,
                    })
            })
            .transpose()?,
        error_detail: row.error_detail,
        created_at: parse_timestamp(row.created_at, "downloads.created_at")?,
        updated_at: parse_timestamp(row.updated_at, "downloads.updated_at")?,
        started_at: row
            .started_at
            .map(|value| parse_timestamp(value, "downloads.started_at"))
            .transpose()?,
        completed_at: row
            .completed_at
            .map(|value| parse_timestamp(value, "downloads.completed_at"))
            .transpose()?,
        output_missing,
    };
    Ok(task)
}

fn parse_optional_id<T>(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<T>, DownloadRepositoryError>
where
    T: FromStr<Err = uuid::Error>,
{
    value
        .map(|value| {
            value
                .parse()
                .map_err(|_| DownloadRepositoryError::InvalidValue { field, value })
        })
        .transpose()
}

fn parse_bool(value: i64, field: &'static str) -> Result<bool, DownloadRepositoryError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(DownloadRepositoryError::InvalidValue {
            field,
            value: value.to_string(),
        }),
    }
}

fn parse_u64(value: i64, field: &'static str) -> Result<u64, DownloadRepositoryError> {
    u64::try_from(value).map_err(|_| DownloadRepositoryError::InvalidValue {
        field,
        value: value.to_string(),
    })
}

fn parse_optional_u64(
    value: Option<i64>,
    field: &'static str,
) -> Result<Option<u64>, DownloadRepositoryError> {
    value.map(|value| parse_u64(value, field)).transpose()
}

fn parse_timestamp(
    value: String,
    field: &'static str,
) -> Result<DateTime<Utc>, DownloadRepositoryError> {
    DateTime::parse_from_rfc3339(&value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| DownloadRepositoryError::InvalidValue { field, value })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, TempDatabasePath};

    fn request() -> DownloadRequest {
        DownloadRequest {
            provider_kind: ProviderKind::Youtube,
            provider_item_id: "video-1".to_owned(),
            canonical_url: "https://www.youtube.com/watch?v=video-1".to_owned(),
            target_track_id: None,
            target_source_id: None,
            title: "Signal Test".to_owned(),
            artists: vec!["Test Artist".to_owned()],
            artwork_url: Some("https://i.ytimg.com/vi/video-1/hqdefault.jpg".to_owned()),
            mode: DownloadMode::Audio,
        }
    }

    #[test]
    fn state_machine_accepts_only_documented_transitions() {
        assert!(is_valid_transition(
            DownloadState::Queued,
            DownloadState::Resolving
        ));
        assert!(is_valid_transition(
            DownloadState::Postprocessing,
            DownloadState::Completed
        ));
        assert!(is_valid_transition(
            DownloadState::Cancelled,
            DownloadState::Queued
        ));
        assert!(!is_valid_transition(
            DownloadState::Completed,
            DownloadState::Queued
        ));
        assert!(!is_valid_transition(
            DownloadState::Queued,
            DownloadState::Completed
        ));
    }

    #[test]
    fn retry_clears_transient_fields_and_increments_count() {
        let mut task = DownloadTask::from_request(request(), PathBuf::from("C:\\Downloads"));
        task.state = DownloadState::Failed;
        task.retry_count = 2;
        task.output_path = Some(PathBuf::from("C:\\Downloads\\old.mp4"));
        task.downloaded_bytes = 123;
        task.progress_permille = 400;
        task.error_code = Some(DownloadErrorCode::ProcessFailed);
        task.prepare_retry().unwrap();

        assert_eq!(task.state, DownloadState::Queued);
        assert_eq!(task.retry_count, 3);
        assert_eq!(task.downloaded_bytes, 0);
        assert_eq!(task.progress_permille, 0);
        assert!(task.output_path.is_none());
        assert!(task.error_code.is_none());
    }

    #[test]
    fn repository_round_trip_preserves_download_identity_and_metadata() {
        let path = TempDatabasePath::new("download-round-trip");
        let database = Database::open(path.path()).unwrap();
        let repository = DownloadRepository::new(&database);
        let task = DownloadTask::from_request(request(), PathBuf::from("C:\\Downloads"));

        repository.insert(&task).unwrap();
        let loaded = repository.get(task.id).unwrap().unwrap();

        assert_eq!(loaded.id, task.id);
        assert_eq!(loaded.provider_kind, ProviderKind::Youtube);
        assert_eq!(loaded.provider_item_id, "video-1");
        assert_eq!(loaded.artists, vec!["Test Artist"]);
        assert_eq!(
            loaded.source_quality_provenance,
            SourceQualityProvenance::ProviderEncoded
        );
        assert!(!loaded.transcoded);
        assert_eq!(repository.max_concurrent().unwrap(), 2);
    }

    #[test]
    fn repository_recovery_requeues_active_tasks_without_touching_destination() {
        let path = TempDatabasePath::new("download-recovery");
        let database = Database::open(path.path()).unwrap();
        let repository = DownloadRepository::new(&database);
        let mut task = DownloadTask::from_request(request(), PathBuf::from("C:\\Downloads"));
        task.state = DownloadState::Downloading;
        task.downloaded_bytes = 10;
        repository.insert(&task).unwrap();

        let recovered = repository.recover_interrupted().unwrap();
        let loaded = repository.get(task.id).unwrap().unwrap();

        assert_eq!(recovered, vec![task.id]);
        assert_eq!(loaded.state, DownloadState::Queued);
        assert_eq!(loaded.downloaded_bytes, 0);
        assert_eq!(loaded.destination_directory, PathBuf::from("C:\\Downloads"));
    }
}
