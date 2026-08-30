use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{ProviderKind, SourceId};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendHealth {
    pub ready: bool,
    pub connected: bool,
    pub detail: Option<String>,
    pub recovery_action: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BackendEvent {
    FileLoaded { duration_ms: Option<u64> },
    Position { position_ms: u64 },
    Pause { paused: bool },
    Seeking { seeking: bool },
    EndFile { reason: EndFileReason },
    AudioDevice { name: Option<String> },
    AudioDeviceList { devices: Vec<AudioDevice> },
    Error { error: BackendError },
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSourceOption {
    pub source_id: SourceId,
    pub provider: ProviderKind,
    pub label: String,
    pub available: bool,
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

pub trait PlaybackBackend: Send {
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
    fn shutdown(&mut self) -> Result<(), BackendError>;
    fn health(&self) -> BackendHealth;
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
            description: Some("Default output".to_owned()),
            is_default: true,
        };
        let source = PlaybackSourceOption {
            source_id: SourceId::new(),
            provider: ProviderKind::Local,
            label: "LOCAL".to_owned(),
            available: true,
        };
        let event = BackendEvent::AudioDeviceList {
            devices: vec![device],
        };
        let json = serde_json::to_value((&event, &source)).unwrap();

        assert_eq!(json[0]["type"], "audioDeviceList");
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
