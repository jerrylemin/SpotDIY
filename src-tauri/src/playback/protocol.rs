use std::sync::atomic::{AtomicI64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::backend::BackendError;

pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
pub struct MpvRequest {
    pub command: Vec<Value>,
    pub request_id: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct MpvReply {
    pub request_id: i64,
    pub error: String,
    pub data: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct MpvEvent {
    pub event: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub data: Option<Value>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolFrame {
    Reply(MpvReply),
    Event(MpvEvent),
}

#[derive(Debug)]
pub struct RequestIdGenerator {
    next: AtomicI64,
}

impl Default for RequestIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestIdGenerator {
    pub const fn new() -> Self {
        Self {
            next: AtomicI64::new(1),
        }
    }

    pub fn next_id(&self) -> i64 {
        self.next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(if current == i64::MAX { 1 } else { current + 1 })
            })
            .unwrap_or(1)
    }
}

pub fn serialize_request(command: Vec<Value>, request_id: i64) -> Result<Vec<u8>, BackendError> {
    if command.is_empty() || request_id <= 0 {
        return Err(protocol_error(
            "an mpv request must have a command and positive id",
        ));
    }
    let mut frame = serde_json::to_vec(&MpvRequest {
        command,
        request_id,
    })
    .map_err(|_| protocol_error("an mpv request could not be serialized"))?;
    frame.push(b'\n');
    if frame.len() > MAX_FRAME_BYTES {
        return Err(protocol_error("an mpv request exceeded the size limit"));
    }
    Ok(frame)
}

pub fn parse_frame(frame: &[u8]) -> Result<ProtocolFrame, BackendError> {
    if frame.len() > MAX_FRAME_BYTES {
        return Err(protocol_error("an mpv frame exceeded the size limit"));
    }
    let value: Value =
        serde_json::from_slice(frame).map_err(|_| protocol_error("mpv sent malformed JSON"))?;
    let object = value
        .as_object()
        .ok_or_else(|| protocol_error("mpv sent a non-object frame"))?;
    if let Some(request_id) = object.get("request_id") {
        let request_id = request_id
            .as_i64()
            .filter(|request_id| *request_id > 0)
            .ok_or_else(|| protocol_error("mpv sent an invalid request id"))?;
        let error = object
            .get("error")
            .and_then(Value::as_str)
            .ok_or_else(|| protocol_error("mpv sent a reply without an error status"))?;
        return Ok(ProtocolFrame::Reply(MpvReply {
            request_id,
            error: error.to_owned(),
            data: object.get("data").cloned(),
        }));
    }
    object
        .get("event")
        .and_then(Value::as_str)
        .ok_or_else(|| protocol_error("mpv sent an unclassified frame"))?;
    Ok(ProtocolFrame::Event(
        serde_json::from_value(value).map_err(|_| protocol_error("mpv sent an invalid event"))?,
    ))
}

fn protocol_error(detail: &str) -> BackendError {
    BackendError::Protocol {
        detail: detail.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_ids_are_positive_and_unique() {
        let generator = RequestIdGenerator::new();
        assert_eq!(generator.next_id(), 1);
        assert_eq!(generator.next_id(), 2);
    }

    #[test]
    fn request_serialization_is_newline_delimited() {
        let frame = serialize_request(vec![json!("get_property"), json!("pause")], 1).unwrap();
        assert_eq!(frame.last(), Some(&b'\n'));
        assert_eq!(
            serde_json::from_slice::<Value>(&frame[..frame.len() - 1]).unwrap()["request_id"],
            1
        );
    }

    #[test]
    fn replies_and_interleaved_events_are_typed() {
        assert!(matches!(
            parse_frame(br#"{"event":"property-change","name":"pause","data":true}"#),
            Ok(ProtocolFrame::Event(_))
        ));
        assert!(matches!(
            parse_frame(br#"{"request_id":1,"error":"success","data":true}"#),
            Ok(ProtocolFrame::Reply(MpvReply { request_id: 1, .. }))
        ));
    }

    #[test]
    fn malformed_and_oversized_frames_are_rejected() {
        assert!(parse_frame(b"not json").is_err());
        assert!(parse_frame(&vec![b'x'; MAX_FRAME_BYTES + 1]).is_err());
    }
}
