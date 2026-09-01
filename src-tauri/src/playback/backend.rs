use std::path::PathBuf;
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
    Failure(PlaybackError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationStampedBackendEvent {
    pub generation: u64,
    pub event: BackendEvent,
}

impl GenerationStampedBackendEvent {
    pub fn new(generation: u64, event: BackendEvent) -> Self {
        Self { generation, event }
    }
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
    pub events: tokio::sync::mpsc::Receiver<GenerationStampedBackendEvent>,
}

/// Product-level command-enqueue contract. Implementations keep request/reply
/// work behind their bounded worker and never expose mpv protocol details.
pub trait PlaybackBackend: Send + Sync {
    fn send(&self, command: BackendCommand) -> Result<(), PlaybackError>;
    fn health(&self) -> PlaybackBackendHealth;
    fn shutdown(&self) -> Result<(), PlaybackError>;
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
            availability_detail: None,
        };
        let event = BackendEvent::AudioDevices(vec![device]);
        let json = serde_json::to_value((&event, &source)).unwrap();

        assert_eq!(json[0]["type"], "audioDevices");
        assert_eq!(json[1]["sourceId"], source.source_id.to_string());
        assert!(json.to_string().find("path").is_none());
    }

    #[test]
    fn backend_event_envelopes_preserve_generation_identity() {
        let event = GenerationStampedBackendEvent::new(7, BackendEvent::FileLoaded);

        assert_eq!(event.generation, 7);
        assert_eq!(event.event, BackendEvent::FileLoaded);
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
