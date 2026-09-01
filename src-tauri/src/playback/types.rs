use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{ProviderKind, SourceId, TrackId};

pub use super::queue::{
    QueueEntry, QueueEntryId, QueueRepository, QueueRepositoryError, QueueSection, QueueSnapshot,
    QueueSnapshotEntry, QueueSnapshotSummary, QueueWorkspace, QueueWorkspaceEntry, RepeatMode,
    TransientQueue,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackPlaybackRequest {
    pub track_id: TrackId,
    pub source_id: Option<SourceId>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackPhase {
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
    Seeking,
    Ended,
    Recovering,
    Failed,
    ShuttingDown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackErrorCode {
    ToolMissing,
    ToolBroken,
    SpawnFailed,
    IpcConnectTimeout,
    IpcDisconnected,
    ProtocolError,
    RequestTimeout,
    TrackNotFound,
    SourceNotFound,
    SourceMismatch,
    SourceNotPlayable,
    SourceUnavailable,
    LocalFileMissing,
    LoadFailed,
    SeekFailed,
    DeviceUnavailable,
    QueueEmpty,
    RecoveryRetrying,
    RecoveryExhausted,
    PersistenceFailed,
    QueueEntryNotFound,
    QueueEntryImmutable,
    InvalidQueuePosition,
    SnapshotNotFound,
    ShuttingDown,
}

#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[error("{detail}")]
pub struct PlaybackError {
    pub code: PlaybackErrorCode,
    pub detail: String,
    pub retryable: bool,
}

impl PlaybackError {
    pub(crate) fn new(code: PlaybackErrorCode, detail: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            detail: detail.into(),
            retryable,
        }
    }

    pub fn dto(&self) -> PlaybackErrorDto {
        PlaybackErrorDto {
            code: self.code,
            summary: self.detail.clone(),
            retryable: self.retryable,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackErrorDto {
    pub code: PlaybackErrorCode,
    pub summary: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackBackendHealth {
    pub ready: bool,
    pub connected: bool,
    pub detail: Option<String>,
    pub recovery_action: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
    pub selected: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSourceOption {
    pub source_id: SourceId,
    pub provider: ProviderKind,
    pub label: String,
    pub available: bool,
    pub availability_detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    pub revision: u64,
    pub phase: PlaybackPhase,
    pub current_queue_entry_id: Option<QueueEntryId>,
    pub current_track_id: Option<TrackId>,
    pub current_source_id: Option<SourceId>,
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub artwork_path: Option<PathBuf>,
    pub sources: Vec<PlaybackSourceOption>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume_percent: u8,
    pub muted: bool,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
    pub queue_length: usize,
    pub queue_index: Option<usize>,
    pub selected_audio_device: String,
    pub backend_health: PlaybackBackendHealth,
    pub recovering: bool,
    pub error: Option<PlaybackErrorDto>,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            phase: PlaybackPhase::Idle,
            current_queue_entry_id: None,
            current_track_id: None,
            current_source_id: None,
            title: None,
            artists: Vec::new(),
            album: None,
            artwork_path: None,
            sources: Vec::new(),
            position_ms: 0,
            duration_ms: None,
            volume_percent: 100,
            muted: false,
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            queue_length: 0,
            queue_index: None,
            selected_audio_device: "auto".to_owned(),
            backend_health: PlaybackBackendHealth::default(),
            recovering: false,
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_serializes_only_the_public_error_contract() {
        let error = PlaybackError::new(PlaybackErrorCode::LoadFailed, "the load failed", true);

        let dto = error.dto();
        assert_eq!(dto.code, PlaybackErrorCode::LoadFailed);
        assert_eq!(serde_json::to_value(dto).unwrap()["code"], "loadFailed");
    }
}
