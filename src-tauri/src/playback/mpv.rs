use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use crate::media_tools::{MediaToolManager, MediaToolStatus};

use super::backend::{
    AudioDevice, BackendError, BackendEvent, BackendHealth, EndFileReason, PlaybackBackend,
};

const MAX_FRAME_BYTES: usize = 64 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const LOAD_TIMEOUT: Duration = Duration::from_secs(15);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const POSITION_EVENT_INTERVAL: Duration = Duration::from_millis(250);

const OBSERVED_PROPERTIES: &[(i64, &str)] = &[
    (1, "pause"),
    (2, "time-pos"),
    (3, "duration"),
    (4, "volume"),
    (5, "mute"),
    (6, "seeking"),
    (7, "eof-reached"),
    (8, "audio-device"),
    (9, "audio-device-list"),
];

#[derive(Clone)]
struct SessionConfig {
    connect_timeout: Duration,
    request_timeout: Duration,
    load_timeout: Duration,
    shutdown_timeout: Duration,
    position_event_interval: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            connect_timeout: CONNECT_TIMEOUT,
            request_timeout: REQUEST_TIMEOUT,
            load_timeout: LOAD_TIMEOUT,
            shutdown_timeout: SHUTDOWN_TIMEOUT,
            position_event_interval: POSITION_EVENT_INTERVAL,
        }
    }
}

/// Managed external-mpv implementation of the frozen playback backend contract.
pub struct MpvBackend {
    manager: MediaToolManager,
    health: BackendHealth,
    buffered_events: VecDeque<BackendEvent>,
    next_generation: u64,
    config: SessionConfig,
    #[cfg(windows)]
    runtime: Option<tokio::runtime::Runtime>,
    #[cfg(windows)]
    session: Option<Session>,
}

impl MpvBackend {
    pub fn new(manager: MediaToolManager) -> Self {
        let health = initial_health(&manager);
        #[cfg(windows)]
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("spotdiy-mpv")
            .enable_all()
            .build()
            .ok();

        Self {
            manager,
            health,
            buffered_events: VecDeque::new(),
            next_generation: 0,
            config: SessionConfig::default(),
            #[cfg(windows)]
            runtime,
            #[cfg(windows)]
            session: None,
        }
    }

    #[cfg(windows)]
    fn request(
        &mut self,
        operation: &'static str,
        command: Vec<Value>,
    ) -> Result<Value, BackendError> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| BackendError::Unavailable {
                detail: "the mpv async runtime could not be initialized".to_owned(),
            })?;
        let session = self.session.as_ref().ok_or(BackendError::NotStarted)?;
        let result = runtime.block_on(send_request(
            &session.request_tx,
            operation,
            command,
            self.config.request_timeout,
        ));
        if matches!(
            result,
            Err(BackendError::Disconnected | BackendError::Protocol { .. })
        ) {
            self.mark_disconnected("the mpv session is no longer available");
        }
        result
    }

    fn mark_disconnected(&mut self, detail: &str) {
        self.health = BackendHealth {
            ready: false,
            connected: false,
            detail: Some(detail.to_owned()),
            recovery_action: Some("Retry the playback backend".to_owned()),
        };
    }

    #[cfg(windows)]
    fn drain_session_events(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        while let Ok(stamped) = session.event_rx.try_recv() {
            if stamped.generation != session.generation {
                continue;
            }
            match stamped.event {
                SessionEvent::Backend(event) => self.buffered_events.push_back(event),
                SessionEvent::FileLoaded => {
                    self.buffered_events
                        .push_back(BackendEvent::FileLoaded { duration_ms: None });
                }
                SessionEvent::Disconnected(error) => {
                    self.health = BackendHealth {
                        ready: false,
                        connected: false,
                        detail: Some("the mpv session disconnected".to_owned()),
                        recovery_action: Some("Retry the playback backend".to_owned()),
                    };
                    self.buffered_events
                        .push_back(BackendEvent::Error { error });
                }
            }
        }
    }
}

impl PlaybackBackend for MpvBackend {
    fn start(&mut self) -> Result<(), BackendError> {
        #[cfg(not(windows))]
        {
            self.mark_disconnected("mpv playback currently requires Windows named pipes");
            return Err(BackendError::Unavailable {
                detail: "mpv playback currently requires Windows named pipes".to_owned(),
            });
        }

        #[cfg(windows)]
        {
            if let Some(session) = self.session.as_ref() {
                if session.process_exit_rx.borrow().is_none() && !session.pipe_task.is_finished() {
                    return Ok(());
                }
            }

            if let Some(session) = self.session.take() {
                if let Some(runtime) = self.runtime.as_ref() {
                    let _ = runtime.block_on(shutdown_session(
                        session,
                        self.config.request_timeout,
                        self.config.shutdown_timeout,
                    ));
                }
            }
            self.buffered_events.clear();

            let tool_health = self.manager.refresh();
            if tool_health.status != MediaToolStatus::Ready {
                self.health = initial_health(&self.manager);
                return Err(BackendError::Unavailable {
                    detail: tool_health
                        .detail
                        .unwrap_or_else(|| "mpv is not ready".to_owned()),
                });
            }
            let executable = self
                .manager
                .mpv_path()
                .map(Path::to_path_buf)
                .ok_or_else(|| BackendError::Unavailable {
                    detail: "mpv is not available".to_owned(),
                })?;
            let runtime = self
                .runtime
                .as_ref()
                .ok_or_else(|| BackendError::Unavailable {
                    detail: "the mpv async runtime could not be initialized".to_owned(),
                })?;
            self.next_generation = self.next_generation.wrapping_add(1).max(1);
            let generation = self.next_generation;
            match runtime.block_on(spawn_session(&executable, generation, self.config.clone())) {
                Ok(session) => {
                    self.session = Some(session);
                    self.health = BackendHealth {
                        ready: true,
                        connected: true,
                        detail: None,
                        recovery_action: None,
                    };
                    Ok(())
                }
                Err(error) => {
                    self.mark_disconnected("mpv could not establish a playback session");
                    Err(error)
                }
            }
        }
    }

    fn load(&mut self, path: &Path) -> Result<(), BackendError> {
        #[cfg(not(windows))]
        {
            let _ = path;
            return Err(BackendError::Unavailable {
                detail: "mpv playback currently requires Windows named pipes".to_owned(),
            });
        }

        #[cfg(windows)]
        {
            if self.session.is_none() {
                return Err(BackendError::NotStarted);
            }
            if !path.is_absolute() || !path.is_file() {
                return Err(BackendError::Operation {
                    detail: "the local media path is not an existing regular file".to_owned(),
                });
            }
            let path = path.to_str().ok_or_else(|| BackendError::Operation {
                detail: "the local media path is not valid Unicode".to_owned(),
            })?;

            self.request(
                "load",
                vec![json!("loadfile"), json!(path), json!("replace")],
            )?;

            let runtime = self
                .runtime
                .as_ref()
                .ok_or_else(|| BackendError::Unavailable {
                    detail: "the mpv async runtime could not be initialized".to_owned(),
                })?;
            let session = self.session.as_mut().ok_or(BackendError::NotStarted)?;
            let preceding_events =
                runtime.block_on(wait_for_file_loaded(session, self.config.load_timeout))?;
            self.buffered_events.extend(preceding_events);

            let duration_ms = self
                .request(
                    "read duration",
                    vec![json!("get_property"), json!("duration")],
                )
                .ok()
                .and_then(|value| value.as_f64())
                .and_then(seconds_to_millis);
            self.buffered_events
                .push_back(BackendEvent::FileLoaded { duration_ms });
            Ok(())
        }
    }

    fn pause(&mut self) -> Result<(), BackendError> {
        #[cfg(windows)]
        {
            self.request(
                "pause",
                vec![json!("set_property"), json!("pause"), json!(true)],
            )?;
            Ok(())
        }
        #[cfg(not(windows))]
        Err(BackendError::NotStarted)
    }

    fn resume(&mut self) -> Result<(), BackendError> {
        #[cfg(windows)]
        {
            self.request(
                "resume",
                vec![json!("set_property"), json!("pause"), json!(false)],
            )?;
            Ok(())
        }
        #[cfg(not(windows))]
        Err(BackendError::NotStarted)
    }

    fn seek(&mut self, position_ms: u64) -> Result<(), BackendError> {
        #[cfg(windows)]
        {
            self.request(
                "seek",
                vec![
                    json!("seek"),
                    json!(position_ms as f64 / 1_000.0),
                    json!("absolute"),
                    json!("exact"),
                ],
            )?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = position_ms;
            Err(BackendError::NotStarted)
        }
    }

    fn set_volume(&mut self, volume_percent: u8) -> Result<(), BackendError> {
        #[cfg(windows)]
        {
            self.request(
                "set volume",
                vec![
                    json!("set_property"),
                    json!("volume"),
                    json!(volume_percent.min(100)),
                ],
            )?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = volume_percent;
            Err(BackendError::NotStarted)
        }
    }

    fn set_muted(&mut self, muted: bool) -> Result<(), BackendError> {
        #[cfg(windows)]
        {
            self.request(
                "set mute",
                vec![json!("set_property"), json!("mute"), json!(muted)],
            )?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = muted;
            Err(BackendError::NotStarted)
        }
    }

    fn list_audio_devices(&mut self) -> Result<Vec<AudioDevice>, BackendError> {
        #[cfg(windows)]
        {
            let value = self.request(
                "list audio devices",
                vec![json!("get_property"), json!("audio-device-list")],
            )?;
            parse_audio_devices(&value)
        }
        #[cfg(not(windows))]
        Err(BackendError::NotStarted)
    }

    fn set_audio_device(&mut self, name: &str) -> Result<(), BackendError> {
        if name.is_empty() {
            return Err(BackendError::Operation {
                detail: "the audio device name is empty".to_owned(),
            });
        }
        #[cfg(windows)]
        {
            self.request(
                "set audio device",
                vec![json!("set_property"), json!("audio-device"), json!(name)],
            )?;
            Ok(())
        }
        #[cfg(not(windows))]
        Err(BackendError::NotStarted)
    }

    fn stop(&mut self) -> Result<(), BackendError> {
        #[cfg(windows)]
        {
            self.request("stop", vec![json!("stop")])?;
            Ok(())
        }
        #[cfg(not(windows))]
        Err(BackendError::NotStarted)
    }

    fn shutdown(&mut self) -> Result<(), BackendError> {
        #[cfg(windows)]
        {
            let result = if let Some(session) = self.session.take() {
                let runtime = self
                    .runtime
                    .as_ref()
                    .ok_or_else(|| BackendError::Unavailable {
                        detail: "the mpv async runtime could not be initialized".to_owned(),
                    })?;
                runtime.block_on(shutdown_session(
                    session,
                    self.config.request_timeout,
                    self.config.shutdown_timeout,
                ))
            } else {
                Ok(())
            };
            self.buffered_events.clear();
            self.health = stopped_health(&self.manager);
            result
        }
        #[cfg(not(windows))]
        {
            self.buffered_events.clear();
            self.health = stopped_health(&self.manager);
            Ok(())
        }
    }

    fn health(&self) -> BackendHealth {
        self.health.clone()
    }

    fn poll_events(&mut self) -> Vec<BackendEvent> {
        #[cfg(windows)]
        self.drain_session_events();
        self.buffered_events.drain(..).collect()
    }
}

impl Drop for MpvBackend {
    fn drop(&mut self) {
        let _ = PlaybackBackend::shutdown(self);
    }
}

fn initial_health(manager: &MediaToolManager) -> BackendHealth {
    let tool_health = manager.health();
    BackendHealth {
        ready: false,
        connected: false,
        detail: tool_health.detail.clone(),
        recovery_action: tool_health.recovery_action.clone(),
    }
}

fn stopped_health(manager: &MediaToolManager) -> BackendHealth {
    if manager.health().status == MediaToolStatus::Ready {
        BackendHealth {
            ready: false,
            connected: false,
            detail: Some("the mpv backend is stopped".to_owned()),
            recovery_action: None,
        }
    } else {
        initial_health(manager)
    }
}

#[derive(Debug, PartialEq)]
enum IncomingFrame {
    Reply {
        request_id: i64,
        error: String,
        data: Value,
    },
    Event(BackendEvent),
    FileLoaded,
    Ignored,
}

fn serialize_request(command: Vec<Value>, request_id: i64) -> Result<Vec<u8>, BackendError> {
    if command.is_empty() {
        return Err(protocol_error("an empty command cannot be serialized"));
    }
    let mut frame = serde_json::to_vec(&json!({
        "command": command,
        "request_id": request_id,
    }))
    .map_err(|_| protocol_error("a request could not be serialized"))?;
    frame.push(b'\n');
    Ok(frame)
}

fn parse_frame(frame: &[u8]) -> Result<IncomingFrame, BackendError> {
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
            .ok_or_else(|| protocol_error("mpv sent an invalid request id"))?;
        let error = object
            .get("error")
            .and_then(Value::as_str)
            .ok_or_else(|| protocol_error("mpv sent a reply without an error status"))?
            .to_owned();
        return Ok(IncomingFrame::Reply {
            request_id,
            error,
            data: object.get("data").cloned().unwrap_or(Value::Null),
        });
    }

    let event = object
        .get("event")
        .and_then(Value::as_str)
        .ok_or_else(|| protocol_error("mpv sent an unclassified frame"))?;
    match event {
        "property-change" => Ok(parse_property_change(&value)?
            .map(IncomingFrame::Event)
            .unwrap_or(IncomingFrame::Ignored)),
        "file-loaded" => Ok(IncomingFrame::FileLoaded),
        "end-file" => Ok(IncomingFrame::Event(parse_end_file(&value)?)),
        _ => Ok(IncomingFrame::Ignored),
    }
}

fn parse_property_change(value: &Value) -> Result<Option<BackendEvent>, BackendError> {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| protocol_error("a property event did not include a name"))?;
    let data = value.get("data").unwrap_or(&Value::Null);
    if data.is_null() && name != "audio-device" && name != "audio-device-list" {
        return Ok(None);
    }

    match name {
        "pause" => data
            .as_bool()
            .map(|paused| Some(BackendEvent::Pause { paused }))
            .ok_or_else(|| protocol_error("pause was not a boolean")),
        "time-pos" => data
            .as_f64()
            .and_then(seconds_to_millis)
            .map(|position_ms| Some(BackendEvent::Position { position_ms }))
            .ok_or_else(|| protocol_error("time-pos was not a valid number")),
        "seeking" => data
            .as_bool()
            .map(|seeking| Some(BackendEvent::Seeking { seeking }))
            .ok_or_else(|| protocol_error("seeking was not a boolean")),
        "audio-device" => {
            let name = if data.is_null() {
                None
            } else {
                Some(
                    data.as_str()
                        .ok_or_else(|| protocol_error("audio-device was not a string"))?
                        .to_owned(),
                )
            };
            Ok(Some(BackendEvent::AudioDevice { name }))
        }
        "audio-device-list" => Ok(Some(BackendEvent::AudioDeviceList {
            devices: if data.is_null() {
                Vec::new()
            } else {
                parse_audio_devices(data)?
            },
        })),
        "duration" | "volume" | "mute" | "eof-reached" => Ok(None),
        _ => Ok(None),
    }
}

fn parse_end_file(value: &Value) -> Result<BackendEvent, BackendError> {
    let reason = match value.get("reason").and_then(Value::as_str) {
        Some("eof") => EndFileReason::Eof,
        Some("stop") => EndFileReason::Stop,
        Some("quit") => EndFileReason::Quit,
        Some("error") => EndFileReason::Error,
        Some("redirect") => EndFileReason::Redirect,
        Some(_) | None => EndFileReason::Unknown,
    };
    Ok(BackendEvent::EndFile { reason })
}

fn parse_audio_devices(value: &Value) -> Result<Vec<AudioDevice>, BackendError> {
    let entries = value
        .as_array()
        .ok_or_else(|| protocol_error("audio-device-list was not an array"))?;
    entries
        .iter()
        .map(|entry| {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| protocol_error("an audio device did not include a name"))?
                .to_owned();
            let description = match entry.get("description") {
                Some(Value::String(description)) => Some(description.clone()),
                Some(Value::Null) | None => None,
                Some(_) => {
                    return Err(protocol_error(
                        "an audio device description was not a string",
                    ));
                }
            };
            Ok(AudioDevice {
                is_default: name == "auto",
                name,
                description,
            })
        })
        .collect()
}

fn seconds_to_millis(seconds: f64) -> Option<u64> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    let millis = (seconds * 1_000.0).round();
    if millis >= u64::MAX as f64 {
        Some(u64::MAX)
    } else {
        Some(millis as u64)
    }
}

fn protocol_error(detail: &str) -> BackendError {
    BackendError::Protocol {
        detail: detail.to_owned(),
    }
}

#[cfg(windows)]
mod windows_session {
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::io::{self, ErrorKind};
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::Stdio;

    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
    use tokio::process::{Child, Command};
    use tokio::sync::{mpsc, oneshot, watch};
    use tokio::task::JoinHandle;
    use tokio::time::{self, Instant, MissedTickBehavior};
    use uuid::Uuid;
    use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    use super::*;

    pub(super) struct Session {
        pub(super) generation: u64,
        pub(super) request_tx: mpsc::Sender<OutboundRequest>,
        pub(super) event_rx: mpsc::UnboundedReceiver<StampedSessionEvent>,
        pub(super) process_control_tx: mpsc::Sender<ProcessControl>,
        pub(super) process_exit_rx: watch::Receiver<Option<ProcessExit>>,
        pub(super) pipe_task: JoinHandle<()>,
        process_task: JoinHandle<()>,
    }

    pub(super) struct OutboundRequest {
        operation: &'static str,
        command: Vec<Value>,
        timeout: Duration,
        response: oneshot::Sender<Result<Value, BackendError>>,
    }

    struct PendingRequest {
        operation: &'static str,
        deadline: Instant,
        response: oneshot::Sender<Result<Value, BackendError>>,
    }

    #[derive(Clone, Debug)]
    pub(super) struct ProcessExit;

    #[derive(Clone, Copy, Debug)]
    pub(super) enum ProcessControl {
        Kill,
    }

    #[derive(Debug)]
    pub(super) enum SessionEvent {
        Backend(BackendEvent),
        FileLoaded,
        Disconnected(BackendError),
    }

    #[derive(Debug)]
    pub(super) struct StampedSessionEvent {
        pub(super) generation: u64,
        pub(super) event: SessionEvent,
    }

    struct RequestIds {
        next: i64,
    }

    impl RequestIds {
        fn new() -> Self {
            Self { next: 1 }
        }

        fn take(&mut self) -> i64 {
            let current = self.next;
            self.next = if self.next == i64::MAX {
                1
            } else {
                self.next + 1
            };
            current
        }
    }

    struct BoundedFrameReader<R> {
        reader: R,
        buffer: Vec<u8>,
    }

    impl<R: AsyncRead + Unpin> BoundedFrameReader<R> {
        fn new(reader: R) -> Self {
            Self {
                reader,
                buffer: Vec::with_capacity(4 * 1024),
            }
        }

        async fn next_frame(&mut self) -> Result<Option<Vec<u8>>, BackendError> {
            loop {
                if let Some(newline) = self.buffer.iter().position(|byte| *byte == b'\n') {
                    if newline > MAX_FRAME_BYTES {
                        return Err(protocol_error("an mpv frame exceeded the size limit"));
                    }
                    let mut frame: Vec<u8> = self.buffer.drain(..=newline).collect();
                    frame.pop();
                    if frame.last() == Some(&b'\r') {
                        frame.pop();
                    }
                    return Ok(Some(frame));
                }
                if self.buffer.len() > MAX_FRAME_BYTES {
                    return Err(protocol_error("an mpv frame exceeded the size limit"));
                }

                let mut chunk = [0_u8; 4 * 1024];
                let read = self
                    .reader
                    .read(&mut chunk)
                    .await
                    .map_err(|_| BackendError::Disconnected)?;
                if read == 0 {
                    if self.buffer.is_empty() {
                        return Ok(None);
                    }
                    return Err(protocol_error("mpv disconnected during a frame"));
                }
                self.buffer.extend_from_slice(&chunk[..read]);
            }
        }
    }

    pub(super) async fn spawn_session(
        executable: &Path,
        generation: u64,
        config: SessionConfig,
    ) -> Result<Session, BackendError> {
        let pipe_name = fresh_pipe_name();
        let mut command = Command::new(executable);
        command
            .args(mpv_args(&pipe_name))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
        let child = command.spawn().map_err(|_| BackendError::Unavailable {
            detail: "mpv could not be started".to_owned(),
        })?;

        let (process_control_tx, process_control_rx) = mpsc::channel(1);
        let (process_exit_tx, mut process_exit_rx) = watch::channel(None);
        let process_task = tokio::spawn(monitor_child(child, process_control_rx, process_exit_tx));

        let client =
            match connect_pipe(&pipe_name, config.connect_timeout, &mut process_exit_rx).await {
                Ok(client) => client,
                Err(error) => {
                    let _ = process_control_tx.send(ProcessControl::Kill).await;
                    let _ = wait_for_process_exit(&mut process_exit_rx).await;
                    let _ = process_task.await;
                    return Err(error);
                }
            };

        let (request_tx, request_rx) = mpsc::channel(32);
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let pipe_task = tokio::spawn(run_pipe_session(
            client,
            generation,
            request_rx,
            event_tx,
            process_exit_rx.clone(),
            config.position_event_interval,
        ));
        let mut session = Session {
            generation,
            request_tx,
            event_rx,
            process_control_tx,
            process_exit_rx,
            pipe_task,
            process_task,
        };

        let handshake = async {
            let idle_active = send_request(
                &session.request_tx,
                "health check",
                vec![json!("get_property"), json!("idle-active")],
                config.request_timeout,
            )
            .await?;
            if idle_active.as_bool().is_none() {
                return Err(protocol_error("mpv returned an invalid health response"));
            }
            for (observation_id, property) in OBSERVED_PROPERTIES {
                send_request(
                    &session.request_tx,
                    "register observation",
                    vec![
                        json!("observe_property"),
                        json!(observation_id),
                        json!(property),
                    ],
                    config.request_timeout,
                )
                .await?;
            }
            Ok(())
        }
        .await;

        if let Err(error) = handshake {
            force_and_reap(&mut session).await;
            return Err(error);
        }
        Ok(session)
    }

    pub(super) async fn send_request(
        request_tx: &mpsc::Sender<OutboundRequest>,
        operation: &'static str,
        command: Vec<Value>,
        timeout: Duration,
    ) -> Result<Value, BackendError> {
        let (response_tx, response_rx) = oneshot::channel();
        time::timeout(
            timeout,
            request_tx.send(OutboundRequest {
                operation,
                command,
                timeout,
                response: response_tx,
            }),
        )
        .await
        .map_err(|_| BackendError::Timeout {
            operation: operation.to_owned(),
        })?
        .map_err(|_| BackendError::Disconnected)?;
        response_rx.await.map_err(|_| BackendError::Disconnected)?
    }

    pub(super) async fn wait_for_file_loaded(
        session: &mut Session,
        timeout: Duration,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        wait_for_file_loaded_events(session.generation, &mut session.event_rx, timeout).await
    }

    pub(super) async fn wait_for_file_loaded_events(
        generation: u64,
        event_rx: &mut mpsc::UnboundedReceiver<StampedSessionEvent>,
        timeout: Duration,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        let deadline = Instant::now() + timeout;
        let mut preceding = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(BackendError::Timeout {
                    operation: "load".to_owned(),
                });
            }
            let stamped = time::timeout(remaining, event_rx.recv())
                .await
                .map_err(|_| BackendError::Timeout {
                    operation: "load".to_owned(),
                })?
                .ok_or(BackendError::Disconnected)?;
            if stamped.generation != generation {
                continue;
            }
            match stamped.event {
                SessionEvent::Backend(event) => preceding.push(event),
                SessionEvent::FileLoaded => return Ok(preceding),
                SessionEvent::Disconnected(error) => return Err(error),
            }
        }
    }

    pub(super) async fn shutdown_session(
        mut session: Session,
        request_timeout: Duration,
        shutdown_timeout: Duration,
    ) -> Result<(), BackendError> {
        let deadline = Instant::now() + shutdown_timeout;
        let quit_timeout = request_timeout.min(shutdown_timeout);
        let _ = send_request(
            &session.request_tx,
            "quit",
            vec![json!("quit")],
            quit_timeout,
        )
        .await;

        let remaining = deadline.saturating_duration_since(Instant::now());
        let exited = if session.process_exit_rx.borrow().is_some() {
            true
        } else if remaining.is_zero() {
            false
        } else {
            time::timeout(
                remaining,
                wait_for_process_exit(&mut session.process_exit_rx),
            )
            .await
            .is_ok()
        };
        if !exited {
            session
                .process_control_tx
                .send(ProcessControl::Kill)
                .await
                .map_err(|_| BackendError::Disconnected)?;
            wait_for_process_exit(&mut session.process_exit_rx).await?;
        }

        drop(session.request_tx);
        if time::timeout(Duration::from_millis(500), &mut session.pipe_task)
            .await
            .is_err()
        {
            session.pipe_task.abort();
            let _ = session.pipe_task.await;
        }
        let _ = session.process_task.await;
        Ok(())
    }

    async fn force_and_reap(session: &mut Session) {
        let _ = session.process_control_tx.send(ProcessControl::Kill).await;
        let _ = wait_for_process_exit(&mut session.process_exit_rx).await;
        session.pipe_task.abort();
        let _ = (&mut session.pipe_task).await;
        let _ = (&mut session.process_task).await;
    }

    async fn monitor_child(
        mut child: Child,
        mut control_rx: mpsc::Receiver<ProcessControl>,
        exit_tx: watch::Sender<Option<ProcessExit>>,
    ) {
        tokio::select! {
            _ = child.wait() => {}
            control = control_rx.recv() => {
                if matches!(control, Some(ProcessControl::Kill)) {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                }
            }
        }
        let _ = exit_tx.send(Some(ProcessExit));
    }

    async fn wait_for_process_exit(
        process_exit_rx: &mut watch::Receiver<Option<ProcessExit>>,
    ) -> Result<(), BackendError> {
        while process_exit_rx.borrow().is_none() {
            process_exit_rx
                .changed()
                .await
                .map_err(|_| BackendError::Disconnected)?;
        }
        Ok(())
    }

    async fn connect_pipe(
        pipe_name: &str,
        timeout: Duration,
        process_exit_rx: &mut watch::Receiver<Option<ProcessExit>>,
    ) -> Result<NamedPipeClient, BackendError> {
        let deadline = Instant::now() + timeout;
        loop {
            if process_exit_rx.borrow().is_some() {
                return Err(BackendError::Operation {
                    detail: "mpv exited before its IPC session was ready".to_owned(),
                });
            }
            match ClientOptions::new().open(pipe_name) {
                Ok(client) => return Ok(client),
                Err(error) if is_retryable_pipe_error(&error) => {}
                Err(_) => {
                    return Err(BackendError::Operation {
                        detail: "mpv IPC could not be opened".to_owned(),
                    });
                }
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(BackendError::Timeout {
                    operation: "connect".to_owned(),
                });
            }
            let delay = remaining.min(Duration::from_millis(25));
            tokio::select! {
                _ = time::sleep(delay) => {}
                changed = process_exit_rx.changed() => {
                    if changed.is_err() || process_exit_rx.borrow().is_some() {
                        return Err(BackendError::Operation {
                            detail: "mpv exited before its IPC session was ready".to_owned(),
                        });
                    }
                }
            }
        }
    }

    fn is_retryable_pipe_error(error: &io::Error) -> bool {
        error.kind() == ErrorKind::NotFound || error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
    }

    async fn run_pipe_session(
        client: NamedPipeClient,
        generation: u64,
        mut request_rx: mpsc::Receiver<OutboundRequest>,
        event_tx: mpsc::UnboundedSender<StampedSessionEvent>,
        mut process_exit_rx: watch::Receiver<Option<ProcessExit>>,
        position_event_interval: Duration,
    ) {
        let (reader, mut writer) = tokio::io::split(client);
        let mut frame_reader = BoundedFrameReader::new(reader);
        let mut pending = HashMap::<i64, PendingRequest>::new();
        let mut request_ids = RequestIds::new();
        let mut ticker = time::interval(Duration::from_millis(10));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut last_position_event: Option<Instant> = None;
        let mut disconnect_error = BackendError::Disconnected;

        loop {
            tokio::select! {
                changed = process_exit_rx.changed() => {
                    if changed.is_err() || process_exit_rx.borrow().is_some() {
                        disconnect_error = BackendError::Disconnected;
                        break;
                    }
                }
                request = request_rx.recv() => {
                    let Some(request) = request else {
                        break;
                    };
                    let request_id = request_ids.take();
                    let frame = match serialize_request(request.command, request_id) {
                        Ok(frame) => frame,
                        Err(error) => {
                            let _ = request.response.send(Err(error));
                            continue;
                        }
                    };
                    pending.insert(request_id, PendingRequest {
                        operation: request.operation,
                        deadline: Instant::now() + request.timeout,
                        response: request.response,
                    });
                    if writer.write_all(&frame).await.is_err() {
                        disconnect_error = BackendError::Disconnected;
                        break;
                    }
                }
                frame = frame_reader.next_frame() => {
                    let frame = match frame {
                        Ok(Some(frame)) => frame,
                        Ok(None) => {
                            disconnect_error = BackendError::Disconnected;
                            break;
                        }
                        Err(error) => {
                            disconnect_error = error;
                            break;
                        }
                    };
                    match parse_frame(&frame) {
                        Ok(IncomingFrame::Reply { request_id, error, data }) => {
                            if let Some(pending_request) = pending.remove(&request_id) {
                                let result = if error == "success" {
                                    Ok(data)
                                } else {
                                    Err(BackendError::Operation {
                                        detail: format!("mpv rejected {}: {error}", pending_request.operation),
                                    })
                                };
                                let _ = pending_request.response.send(result);
                            }
                        }
                        Ok(IncomingFrame::Event(event)) => {
                            let emit = if matches!(event, BackendEvent::Position { .. }) {
                                let now = Instant::now();
                                let allowed = last_position_event
                                    .map(|last| now.duration_since(last) >= position_event_interval)
                                    .unwrap_or(true);
                                if allowed {
                                    last_position_event = Some(now);
                                }
                                allowed
                            } else {
                                true
                            };
                            if emit {
                                let _ = event_tx.send(StampedSessionEvent {
                                    generation,
                                    event: SessionEvent::Backend(event),
                                });
                            }
                        }
                        Ok(IncomingFrame::FileLoaded) => {
                            let _ = event_tx.send(StampedSessionEvent {
                                generation,
                                event: SessionEvent::FileLoaded,
                            });
                        }
                        Ok(IncomingFrame::Ignored) => {}
                        Err(error) => {
                            disconnect_error = error;
                            break;
                        }
                    }
                }
                _ = ticker.tick() => {
                    let now = Instant::now();
                    let expired: Vec<i64> = pending
                        .iter()
                        .filter_map(|(request_id, request)|
                            (request.deadline <= now).then_some(*request_id))
                        .collect();
                    for request_id in expired {
                        if let Some(request) = pending.remove(&request_id) {
                            let _ = request.response.send(Err(BackendError::Timeout {
                                operation: request.operation.to_owned(),
                            }));
                        }
                    }
                }
            }
        }

        for (_, request) in pending.drain() {
            let _ = request.response.send(Err(disconnect_error.clone()));
        }
        let _ = event_tx.send(StampedSessionEvent {
            generation,
            event: SessionEvent::Disconnected(disconnect_error),
        });
    }

    fn fresh_pipe_name() -> String {
        format!(r"\\.\pipe\spotdiy-mpv-{}", Uuid::new_v4().simple())
    }

    fn mpv_args(pipe_name: &str) -> Vec<OsString> {
        vec![
            "--no-config".into(),
            "--idle=yes".into(),
            "--keep-open=no".into(),
            format!("--input-ipc-server={pipe_name}").into(),
            "--terminal=no".into(),
            "--audio-device=auto".into(),
        ]
    }

    #[cfg(test)]
    pub(super) mod test_support {
        use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

        use super::*;

        pub(crate) async fn connected_pair() -> (NamedPipeServer, NamedPipeClient) {
            let pipe_name = fresh_pipe_name();
            let server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(&pipe_name)
                .unwrap();
            let client = ClientOptions::new().open(&pipe_name).unwrap();
            server.connect().await.unwrap();
            (server, client)
        }

        pub(crate) fn start_protocol_session(
            client: NamedPipeClient,
            request_timeout: Duration,
        ) -> (
            mpsc::Sender<OutboundRequest>,
            mpsc::UnboundedReceiver<StampedSessionEvent>,
            JoinHandle<()>,
            watch::Sender<Option<ProcessExit>>,
        ) {
            let (request_tx, request_rx) = mpsc::channel(8);
            let (event_tx, event_rx) = mpsc::unbounded_channel();
            let (process_exit_tx, process_exit_rx) = watch::channel(None);
            let task = tokio::spawn(run_pipe_session(
                client,
                7,
                request_rx,
                event_tx,
                process_exit_rx,
                Duration::from_millis(250),
            ));
            let _ = request_timeout;
            (request_tx, event_rx, task, process_exit_tx)
        }

        pub(crate) fn exact_args(pipe_name: &str) -> Vec<OsString> {
            mpv_args(pipe_name)
        }
    }
}

#[cfg(windows)]
use windows_session::{
    send_request, shutdown_session, spawn_session, wait_for_file_loaded, Session, SessionEvent,
};

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn requests_are_compact_newline_frames_with_signed_ids() {
        let first = serialize_request(vec![json!("get_property"), json!("pause")], 1).unwrap();
        let second =
            serialize_request(vec![json!("get_property"), json!("pause")], i64::MAX).unwrap();

        assert!(first.ends_with(b"\n"));
        assert_eq!(first.iter().filter(|byte| **byte == b'\n').count(), 1);
        assert_eq!(
            serde_json::from_slice::<Value>(&first[..first.len() - 1]).unwrap()["request_id"],
            1
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&second[..second.len() - 1]).unwrap()["request_id"],
            i64::MAX
        );
        assert_ne!(first, second);
    }

    #[test]
    fn malformed_and_oversized_frames_are_rejected_before_use() {
        assert!(matches!(
            parse_frame(b"{not-json}"),
            Err(BackendError::Protocol { .. })
        ));
        assert!(matches!(
            parse_frame(&vec![b'x'; MAX_FRAME_BYTES + 1]),
            Err(BackendError::Protocol { .. })
        ));
    }

    #[test]
    fn replies_require_signed_request_ids() {
        assert!(matches!(
            parse_frame(br#"{"request_id":9223372036854775808,"error":"success"}"#),
            Err(BackendError::Protocol { .. })
        ));
        assert_eq!(
            parse_frame(br#"{"request_id":-7,"error":"success","data":true}"#).unwrap(),
            IncomingFrame::Reply {
                request_id: -7,
                error: "success".to_owned(),
                data: json!(true),
            }
        );
    }

    #[test]
    fn property_events_convert_to_typed_product_events() {
        assert_eq!(
            parse_property_change(&json!({"name":"pause","data":true})).unwrap(),
            Some(BackendEvent::Pause { paused: true })
        );
        assert_eq!(
            parse_property_change(&json!({"name":"time-pos","data":1.234})).unwrap(),
            Some(BackendEvent::Position { position_ms: 1_234 })
        );
        assert_eq!(
            parse_property_change(&json!({"name":"seeking","data":false})).unwrap(),
            Some(BackendEvent::Seeking { seeking: false })
        );
        assert_eq!(
            parse_property_change(&json!({"name":"audio-device","data":"auto"})).unwrap(),
            Some(BackendEvent::AudioDevice {
                name: Some("auto".to_owned())
            })
        );
    }

    #[test]
    fn lifecycle_and_unknown_events_are_classified() {
        assert_eq!(
            parse_frame(br#"{"event":"file-loaded"}"#).unwrap(),
            IncomingFrame::FileLoaded
        );
        assert_eq!(
            parse_frame(br#"{"event":"client-message","args":[]}"#).unwrap(),
            IncomingFrame::Ignored
        );
        assert_eq!(
            parse_frame(br#"{"event":"end-file","reason":"eof"}"#).unwrap(),
            IncomingFrame::Event(BackendEvent::EndFile {
                reason: EndFileReason::Eof
            })
        );
        assert_eq!(
            parse_end_file(&json!({"reason":"stop"})).unwrap(),
            BackendEvent::EndFile {
                reason: EndFileReason::Stop
            }
        );
    }

    #[test]
    fn audio_devices_and_seconds_are_normalized() {
        let devices = parse_audio_devices(&json!([
            {"name":"auto","description":"Default output"},
            {"name":"wasapi/device","description":"Speakers"}
        ]))
        .unwrap();

        assert_eq!(devices.len(), 2);
        assert!(devices[0].is_default);
        assert!(!devices[1].is_default);
        assert_eq!(seconds_to_millis(2.345), Some(2_345));
        assert_eq!(seconds_to_millis(-1.0), None);
        assert_eq!(seconds_to_millis(f64::NAN), None);
    }

    #[test]
    fn observation_registration_is_limited_to_product_state() {
        assert_eq!(
            OBSERVED_PROPERTIES,
            &[
                (1, "pause"),
                (2, "time-pos"),
                (3, "duration"),
                (4, "volume"),
                (5, "mute"),
                (6, "seeking"),
                (7, "eof-reached"),
                (8, "audio-device"),
                (9, "audio-device-list"),
            ]
        );
    }

    #[cfg(windows)]
    mod windows_protocol {
        use std::ffi::OsString;

        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
        use tokio::time;

        use super::*;
        use crate::playback::mpv::windows_session::test_support::{
            connected_pair, exact_args, start_protocol_session,
        };
        use crate::playback::mpv::windows_session::{
            send_request, wait_for_file_loaded_events, SessionEvent, StampedSessionEvent,
        };

        #[test]
        fn launch_arguments_are_exact_and_structured() {
            let pipe_name = r"\\.\pipe\opaque";
            assert_eq!(
                exact_args(pipe_name),
                vec![
                    OsString::from("--no-config"),
                    OsString::from("--idle=yes"),
                    OsString::from("--keep-open=no"),
                    OsString::from(format!("--input-ipc-server={pipe_name}")),
                    OsString::from("--terminal=no"),
                    OsString::from("--audio-device=auto"),
                ]
            );
        }

        #[tokio::test]
        async fn correlates_replies_across_interleaved_events_with_unique_ids() {
            let (server, client) = connected_pair().await;
            let (request_tx, mut event_rx, task, _process_exit_tx) =
                start_protocol_session(client, Duration::from_secs(1));
            let (read_half, mut write_half) = tokio::io::split(server);
            let mut reader = BufReader::new(read_half);

            let first_request = tokio::spawn({
                let request_tx = request_tx.clone();
                async move {
                    send_request(
                        &request_tx,
                        "first",
                        vec![json!("get_property"), json!("pause")],
                        Duration::from_secs(1),
                    )
                    .await
                }
            });
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let first_id = serde_json::from_str::<Value>(&line).unwrap()["request_id"]
                .as_i64()
                .unwrap();
            write_half
                .write_all(
                    format!(
                        "{{\"event\":\"property-change\",\"name\":\"pause\",\"data\":true}}\n{{\"request_id\":{first_id},\"error\":\"success\",\"data\":true}}\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            assert_eq!(first_request.await.unwrap().unwrap(), json!(true));
            let event = time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(matches!(
                event.event,
                SessionEvent::Backend(BackendEvent::Pause { paused: true })
            ));

            let second_request = tokio::spawn({
                let request_tx = request_tx.clone();
                async move {
                    send_request(
                        &request_tx,
                        "second",
                        vec![json!("get_property"), json!("mute")],
                        Duration::from_secs(1),
                    )
                    .await
                }
            });
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            let second_id = serde_json::from_str::<Value>(&line).unwrap()["request_id"]
                .as_i64()
                .unwrap();
            assert_ne!(first_id, second_id);
            write_half
                .write_all(
                    format!(
                        "{{\"request_id\":{second_id},\"error\":\"success\",\"data\":false}}\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            assert_eq!(second_request.await.unwrap().unwrap(), json!(false));

            drop(write_half);
            drop(reader);
            drop(request_tx);
            time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap();
        }

        #[tokio::test]
        async fn eof_disconnects_and_fails_pending_requests() {
            let (mut server, client) = connected_pair().await;
            let (request_tx, mut event_rx, task, _process_exit_tx) =
                start_protocol_session(client, Duration::from_secs(1));
            let request = tokio::spawn({
                let request_tx = request_tx.clone();
                async move {
                    send_request(
                        &request_tx,
                        "pending",
                        vec![json!("get_property"), json!("pause")],
                        Duration::from_secs(1),
                    )
                    .await
                }
            });
            let mut bytes = [0_u8; 256];
            let _ = server.read(&mut bytes).await.unwrap();
            drop(server);

            assert_eq!(request.await.unwrap(), Err(BackendError::Disconnected));
            let event = time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(matches!(
                event.event,
                SessionEvent::Disconnected(BackendError::Disconnected)
            ));
            time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap();
        }

        #[tokio::test]
        async fn requests_time_out_when_the_server_does_not_reply() {
            let (mut server, client) = connected_pair().await;
            let (request_tx, _event_rx, task, _process_exit_tx) =
                start_protocol_session(client, Duration::from_millis(50));
            let request = tokio::spawn({
                let request_tx = request_tx.clone();
                async move {
                    send_request(
                        &request_tx,
                        "silent request",
                        vec![json!("get_property"), json!("pause")],
                        Duration::from_millis(50),
                    )
                    .await
                }
            });
            let mut bytes = [0_u8; 256];
            let _ = server.read(&mut bytes).await.unwrap();
            assert_eq!(
                request.await.unwrap(),
                Err(BackendError::Timeout {
                    operation: "silent request".to_owned()
                })
            );
            drop(server);
            drop(request_tx);
            time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap();
        }

        #[tokio::test]
        async fn load_wait_ignores_interleaved_events_until_file_loaded() {
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
            event_tx
                .send(StampedSessionEvent {
                    generation: 7,
                    event: SessionEvent::Backend(BackendEvent::Pause { paused: false }),
                })
                .unwrap();
            event_tx
                .send(StampedSessionEvent {
                    generation: 6,
                    event: SessionEvent::FileLoaded,
                })
                .unwrap();
            event_tx
                .send(StampedSessionEvent {
                    generation: 7,
                    event: SessionEvent::FileLoaded,
                })
                .unwrap();

            assert_eq!(
                wait_for_file_loaded_events(7, &mut event_rx, Duration::from_secs(1))
                    .await
                    .unwrap(),
                vec![BackendEvent::Pause { paused: false }]
            );
        }
    }
}
