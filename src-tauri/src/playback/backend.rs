use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use super::types::{
    AudioDevice, PlaybackBackendHealth, PlaybackError, PlaybackErrorCode, PlaybackSourceOption,
};
pub type BackendHealth = PlaybackBackendHealth;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum BackendCommand {
    Load { path: PathBuf, start_paused: bool },
    SetPaused(bool),
    SeekAbsoluteMs(u64),
    SetVolume(u8),
    SetMuted(bool),
    QueryAudioDevices,
    SelectAudioDevice(String),
    Stop,
    Shutdown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum BackendEvent {
    Ready,
    FileLoaded,
    PauseChanged(bool),
    PositionChanged(u64),
    DurationChanged(Option<u64>),
    SeekingChanged(bool),
    VolumeChanged(u8),
    MuteChanged(bool),
    AudioDevices(Vec<AudioDevice>),
    AudioDeviceChanged(String),
    EndFile(EndFileReason),
    Disconnected,
    ProcessExited { expected: bool, code: Option<i32> },
    ProtocolError(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndFileReason {
    Eof,
    Stop,
    Quit,
    Error,
    Redirect,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Error, Eq, PartialEq, Serialize)]
#[serde(tag = "code", content = "detail", rename_all = "camelCase")]
pub enum BackendError {
    #[error("backend is unavailable: {detail}")]
    Unavailable { detail: String },
    #[error("backend is not started")]
    NotStarted,
    #[error("backend disconnected")]
    Disconnected,
    #[error("backend operation timed out: {operation}")]
    Timeout { operation: String },
    #[error("backend protocol error: {detail}")]
    Protocol { detail: String },
    #[error("backend operation failed: {detail}")]
    Operation { detail: String },
}

pub struct PlaybackBackendSession {
    pub backend: Arc<dyn PlaybackBackend>,
    pub events: tokio::sync::mpsc::Receiver<BackendEvent>,
}

/// Product-level backend contract. The synchronous legacy methods remain on
/// the trait as the controller's compatibility seam; implementations enqueue
/// their actual work onto asynchronous workers.
pub trait PlaybackBackend: Send + Sync {
    fn send(&self, _command: BackendCommand) -> Result<(), PlaybackError> {
        Err(PlaybackError::new(
            PlaybackErrorCode::BackendOperation,
            "this backend does not expose the command enqueue seam",
            false,
        ))
    }

    fn start(&mut self) -> Result<(), BackendError>;
    fn load(&mut self, path: &Path) -> Result<(), BackendError>;
    fn pause(&mut self) -> Result<(), BackendError>;
    fn resume(&mut self) -> Result<(), BackendError>;
    fn seek(&mut self, position_ms: u64) -> Result<(), BackendError>;
    fn set_volume(&mut self, volume_percent: u8) -> Result<(), BackendError>;
    fn set_muted(&mut self, muted: bool) -> Result<(), BackendError>;
    fn list_audio_devices(&mut self) -> Result<Vec<AudioDevice>, BackendError>;
    fn set_audio_device(&mut self, name: &str) -> Result<(), BackendError>;
    fn stop(&mut self) -> Result<(), BackendError>;
    fn shutdown(&mut self) -> Result<(), PlaybackError>;
    fn health(&self) -> PlaybackBackendHealth;
    fn poll_events(&mut self) -> Vec<BackendEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SourceId;

    #[test]
    fn dto_serialization_is_camel_case_and_path_free() {
        let device = AudioDevice {
            name: "auto".to_owned(),
            description: "Default output".to_owned(),
            selected: true,
        };
        let source = PlaybackSourceOption {
            source_id: SourceId::new(),
            provider: crate::domain::ProviderKind::Local,
            label: "LOCAL".to_owned(),
            available: true,
        };
        let event = BackendEvent::AudioDevices(vec![device]);
        let json = serde_json::to_value((&event, &source)).unwrap();

        assert_eq!(json[0]["type"], "audioDevices");
        assert_eq!(json[1]["sourceId"], source.source_id.to_string());
        assert!(json.to_string().find("path").is_none());
    }

    #[test]
    fn end_file_reasons_have_wire_names() {
        assert_eq!(
            serde_json::to_string(&EndFileReason::Eof).unwrap(),
            "\"eof\""
        );
        assert_eq!(
            serde_json::to_string(&EndFileReason::Stop).unwrap(),
            "\"stop\""
        );
    }

    #[test]
    fn backend_error_is_structured() {
        let json = serde_json::to_value(BackendError::Timeout {
            operation: "load".to_owned(),
        })
        .unwrap();

        assert_eq!(json["code"], "timeout");
        assert_eq!(json["detail"]["operation"], "load");
    }
}
