use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};

use crate::media_tools::{MediaToolManager, MediaToolStatus};

use super::backend::{
    AudioDevice, BackendCommand, BackendError, BackendEvent, BackendHealth, EndFileReason,
    PlaybackBackend,
};
use super::protocol::{parse_frame as parse_protocol_frame, ProtocolFrame, RequestIdGenerator};
use super::{PlaybackError, PlaybackErrorCode};

pub(crate) use super::protocol::MAX_FRAME_BYTES;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const LOAD_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
const FORCED_REAP_TIMEOUT: Duration = Duration::from_millis(500);
const TASK_JOIN_TIMEOUT: Duration = Duration::from_millis(250);
const POSITION_EVENT_INTERVAL: Duration = Duration::from_millis(250);
const BACKEND_COMMAND_CAPACITY: usize = 64;

const OBSERVED_PROPERTIES: &[(i64, &str)] = &[
    (1, "pause"),
    (2, "time-pos"),
    (3, "duration"),
    (4, "volume"),
    (5, "mute"),
    (6, "seeking"),
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
    pending_commands: Mutex<VecDeque<BackendCommand>>,
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
            pending_commands: Mutex::new(VecDeque::new()),
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
                    self.buffered_events.push_back(BackendEvent::FileLoaded);
                }
                SessionEvent::Disconnected(_error) => {
                    self.health = BackendHealth {
                        ready: false,
                        connected: false,
                        detail: Some("the mpv session disconnected".to_owned()),
                        recovery_action: Some("Retry the playback backend".to_owned()),
                    };
                    self.buffered_events.push_back(BackendEvent::Disconnected);
                }
            }
        }
    }
}

impl PlaybackBackend for MpvBackend {
    fn send(&self, command: BackendCommand) -> Result<(), PlaybackError> {
        let mut pending = self.pending_commands.lock().map_err(|_| {
            PlaybackError::new(
                PlaybackErrorCode::BackendOperation,
                "the mpv command queue lock is poisoned",
                true,
            )
        })?;
        if pending.len() >= BACKEND_COMMAND_CAPACITY {
            return Err(PlaybackError::new(
                PlaybackErrorCode::ControllerBusy,
                "the mpv command queue is full",
                true,
            ));
        }
        pending.push_back(command);
        Ok(())
    }

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
                    self.buffered_events.push_back(BackendEvent::Ready);
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
                .push_back(BackendEvent::DurationChanged(duration_ms));
            self.buffered_events.push_back(BackendEvent::FileLoaded);
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
            let selected = self
                .request(
                    "read selected audio device",
                    vec![json!("get_property"), json!("audio-device")],
                )?
                .as_str()
                .map(str::to_owned);
            parse_audio_devices_with_selected(&value, selected.as_deref())
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

    fn shutdown(&mut self) -> Result<(), super::PlaybackError> {
        #[cfg(windows)]
        {
            let result = if let Some(session) = self.session.take() {
                let runtime = self
                    .runtime
                    .as_ref()
                    .ok_or_else(|| BackendError::Unavailable {
                        detail: "the mpv async runtime could not be initialized".to_owned(),
                    })
                    .map_err(super::playback_error_from_backend)?;
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
            result.map_err(super::playback_error_from_backend)
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
        self.process_pending_commands();
        #[cfg(windows)]
        self.drain_session_events();
        self.buffered_events.drain(..).collect()
    }
}

impl MpvBackend {
    fn process_pending_commands(&mut self) {
        let commands = self
            .pending_commands
            .lock()
            .map(|mut pending| pending.drain(..).collect::<Vec<_>>())
            .unwrap_or_default();
        for command in commands {
            let result = match command {
                BackendCommand::Load { path, start_paused } => {
                    self.load(&path)
                        .and_then(|()| if start_paused { self.pause() } else { Ok(()) })
                }
                BackendCommand::SetPaused(paused) => {
                    if paused {
                        self.pause()
                    } else {
                        self.resume()
                    }
                }
                BackendCommand::SeekAbsoluteMs(position_ms) => self.seek(position_ms),
                BackendCommand::SetVolume(volume_percent) => self.set_volume(volume_percent),
                BackendCommand::SetMuted(muted) => self.set_muted(muted),
                BackendCommand::QueryAudioDevices => self.list_audio_devices().map(|devices| {
                    self.buffered_events
                        .push_back(BackendEvent::AudioDevices(devices));
                }),
                BackendCommand::SelectAudioDevice(name) => self.set_audio_device(&name),
                BackendCommand::Stop => self.stop(),
                BackendCommand::Shutdown => {
                    return {
                        let _ = PlaybackBackend::shutdown(self);
                    }
                }
            };
            if let Err(error) = result {
                self.buffered_events
                    .push_back(BackendEvent::ProtocolError(error.to_string()));
            }
        }
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
    super::protocol::serialize_request(command, request_id)
}

fn parse_frame(frame: &[u8]) -> Result<IncomingFrame, BackendError> {
    match parse_protocol_frame(frame)? {
        ProtocolFrame::Reply(reply) => Ok(IncomingFrame::Reply {
            request_id: reply.request_id,
            error: reply.error,
            data: reply.data.unwrap_or(Value::Null),
        }),
        ProtocolFrame::Event(event) => match event.event.as_str() {
            "property-change" => {
                let value = json!({"name": event.name, "data": event.data});
                Ok(parse_property_change(&value)?
                    .map(IncomingFrame::Event)
                    .unwrap_or(IncomingFrame::Ignored))
            }
            "file-loaded" => Ok(IncomingFrame::FileLoaded),
            "end-file" => {
                let value = json!({"reason": event.reason});
                Ok(IncomingFrame::Event(parse_end_file(&value)?))
            }
            _ => Ok(IncomingFrame::Ignored),
        },
    }
}

fn parse_property_change(value: &Value) -> Result<Option<BackendEvent>, BackendError> {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| protocol_error("a property event did not include a name"))?;
    let data = value.get("data").unwrap_or(&Value::Null);
    if data.is_null() && name != "audio-device" && name != "audio-device-list" && name != "duration"
    {
        return Ok(None);
    }

    match name {
        "pause" => data
            .as_bool()
            .map(|paused| Some(BackendEvent::PauseChanged(paused)))
            .ok_or_else(|| protocol_error("pause was not a boolean")),
        "time-pos" => data
            .as_f64()
            .and_then(seconds_to_millis)
            .map(|position_ms| Some(BackendEvent::PositionChanged(position_ms)))
            .ok_or_else(|| protocol_error("time-pos was not a valid number")),
        "seeking" => data
            .as_bool()
            .map(|seeking| Some(BackendEvent::SeekingChanged(seeking)))
            .ok_or_else(|| protocol_error("seeking was not a boolean")),
        "duration" => {
            if data.is_null() {
                Ok(Some(BackendEvent::DurationChanged(None)))
            } else {
                data.as_f64()
                    .and_then(seconds_to_millis)
                    .map(|duration_ms| Some(BackendEvent::DurationChanged(Some(duration_ms))))
                    .ok_or_else(|| protocol_error("duration was not a valid number"))
            }
        }
        "volume" => data
            .as_f64()
            .filter(|volume| volume.is_finite())
            .map(|volume| {
                Some(BackendEvent::VolumeChanged(
                    volume.clamp(0.0, 100.0).round() as u8,
                ))
            })
            .ok_or_else(|| protocol_error("volume was not a valid number")),
        "mute" => data
            .as_bool()
            .map(|muted| Some(BackendEvent::MuteChanged(muted)))
            .ok_or_else(|| protocol_error("mute was not a boolean")),
        "audio-device" => {
            let name = if data.is_null() {
                "auto".to_owned()
            } else {
                data.as_str()
                    .ok_or_else(|| protocol_error("audio-device was not a string"))?
                    .to_owned()
            };
            Ok(Some(BackendEvent::AudioDeviceChanged(name)))
        }
        "audio-device-list" => Ok(Some(BackendEvent::AudioDevices(if data.is_null() {
            Vec::new()
        } else {
            parse_audio_devices(data)?
        }))),
        "eof-reached" => Ok(None),
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
    Ok(BackendEvent::EndFile(reason))
}

fn parse_audio_devices(value: &Value) -> Result<Vec<AudioDevice>, BackendError> {
    parse_audio_devices_with_selected(value, None)
}

fn parse_audio_devices_with_selected(
    value: &Value,
    selected_name: Option<&str>,
) -> Result<Vec<AudioDevice>, BackendError> {
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
            let selected = entry
                .get("selected")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| selected_name == Some(name.as_str()));
            Ok(AudioDevice {
                name,
                description: description.unwrap_or_default(),
                selected,
            })
        })
        .collect()
}

fn seconds_to_millis(seconds: f64) -> Option<u64> {
    if !seconds.is_finite() {
        return None;
    }
    let millis = (seconds.max(0.0) * 1_000.0).round();
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
        deadline: Instant,
        response: oneshot::Sender<Result<Value, BackendError>>,
    }

    pub(super) struct PendingRequest {
        operation: &'static str,
        deadline: Instant,
        response: oneshot::Sender<Result<Value, BackendError>>,
    }

    #[derive(Clone, Debug)]
    pub(super) struct ProcessExit {
        pub(super) result: Result<(), BackendError>,
        pub(super) code: Option<i32>,
    }

    #[derive(Debug)]
    pub(super) enum ProcessControl {
        Kill {
            response: oneshot::Sender<Result<(), BackendError>>,
        },
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
                    if newline >= MAX_FRAME_BYTES {
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
        let mut process_task =
            tokio::spawn(monitor_child(child, process_control_rx, process_exit_tx));

        let client =
            match connect_pipe(&pipe_name, config.connect_timeout, &mut process_exit_rx).await {
                Ok(client) => client,
                Err(error) => {
                    let cleanup = force_process_and_reap(
                        &process_control_tx,
                        &mut process_exit_rx,
                        &mut process_task,
                        FORCED_REAP_TIMEOUT,
                    )
                    .await;
                    return cleanup.and(Err(error));
                }
            };

        let (request_tx, request_rx) = mpsc::channel(32);
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let pipe_task = tokio::spawn(run_pipe_session(
            client,
            generation,
            request_rx,
            event_tx,
            process_control_tx.clone(),
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
            return force_and_reap(&mut session, FORCED_REAP_TIMEOUT)
                .await
                .and(Err(error));
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
        let deadline = Instant::now() + timeout;
        time::timeout(
            timeout,
            request_tx.send(OutboundRequest {
                operation,
                command,
                deadline,
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
        let _ = time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            send_request(
                &session.request_tx,
                "quit",
                vec![json!("quit")],
                quit_timeout,
            ),
        )
        .await;

        let remaining = deadline.saturating_duration_since(Instant::now());
        let graceful_exit = if session.process_exit_rx.borrow().is_some() {
            wait_for_process_exit(&mut session.process_exit_rx).await
        } else if remaining.is_zero() {
            Err(BackendError::Timeout {
                operation: "shutdown".to_owned(),
            })
        } else {
            match time::timeout(
                remaining,
                wait_for_process_exit(&mut session.process_exit_rx),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err(BackendError::Timeout {
                    operation: "shutdown".to_owned(),
                }),
            }
        };

        let process_result = match graceful_exit {
            Ok(()) => {
                join_task_bounded(
                    &mut session.process_task,
                    TASK_JOIN_TIMEOUT,
                    "process monitor join",
                )
                .await
            }
            Err(BackendError::Timeout { .. }) => {
                force_process_and_reap(
                    &session.process_control_tx,
                    &mut session.process_exit_rx,
                    &mut session.process_task,
                    FORCED_REAP_TIMEOUT,
                )
                .await
            }
            Err(error) => {
                let cleanup = abort_and_join_task(
                    &mut session.process_task,
                    TASK_JOIN_TIMEOUT,
                    "process monitor abort",
                )
                .await;
                cleanup.and(Err(error))
            }
        };

        drop(session.request_tx);
        let pipe_result = join_task_bounded(
            &mut session.pipe_task,
            TASK_JOIN_TIMEOUT,
            "pipe session join",
        )
        .await;
        process_result.and(pipe_result)
    }

    pub(super) async fn force_and_reap(
        session: &mut Session,
        reap_timeout: Duration,
    ) -> Result<(), BackendError> {
        let process_result = force_process_and_reap(
            &session.process_control_tx,
            &mut session.process_exit_rx,
            &mut session.process_task,
            reap_timeout,
        )
        .await;
        let pipe_result = abort_and_join_task(
            &mut session.pipe_task,
            TASK_JOIN_TIMEOUT,
            "pipe session abort",
        )
        .await;
        process_result.and(pipe_result)
    }

    async fn monitor_child(
        mut child: Child,
        mut control_rx: mpsc::Receiver<ProcessControl>,
        exit_tx: watch::Sender<Option<ProcessExit>>,
    ) {
        let (result, code) = tokio::select! {
            status = child.wait() => match status {
                Ok(status) => (Ok(()), status.code()),
                Err(_) => (Err(BackendError::Operation {
                    detail: "mpv process wait failed".to_owned(),
                }), None),
            },
            control = control_rx.recv() => {
                let response = control.map(|ProcessControl::Kill { response }| response);
                let kill_result = child.start_kill().map_err(|_| BackendError::Operation {
                    detail: "mpv process termination failed".to_owned(),
                });
                if let Some(response) = response {
                    let _ = response.send(kill_result.clone());
                }
                match kill_result {
                    Ok(()) => match time::timeout(FORCED_REAP_TIMEOUT, child.wait()).await {
                        Ok(Ok(status)) => (Ok(()), status.code()),
                        Ok(Err(_)) => (Err(BackendError::Operation {
                            detail: "mpv process reap failed".to_owned(),
                        }), None),
                        Err(_) => (Err(BackendError::Timeout {
                            operation: "force reap".to_owned(),
                        }), None),
                    },
                    Err(error) => (Err(error), None),
                }
            }
        };
        let _ = exit_tx.send(Some(ProcessExit { result, code }));
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
        process_exit_rx
            .borrow()
            .as_ref()
            .map(|exit| exit.result.clone())
            .unwrap_or(Err(BackendError::Disconnected))
    }

    pub(super) async fn force_process_and_reap(
        process_control_tx: &mpsc::Sender<ProcessControl>,
        process_exit_rx: &mut watch::Receiver<Option<ProcessExit>>,
        process_task: &mut JoinHandle<()>,
        reap_timeout: Duration,
    ) -> Result<(), BackendError> {
        if process_exit_rx.borrow().is_some() {
            let exit_result = wait_for_process_exit(process_exit_rx).await;
            let join_result =
                join_task_bounded(process_task, TASK_JOIN_TIMEOUT, "process monitor join").await;
            return exit_result.and(join_result);
        }

        let deadline = Instant::now() + reap_timeout;
        let (response_tx, response_rx) = oneshot::channel();
        let send_result = time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            process_control_tx.send(ProcessControl::Kill {
                response: response_tx,
            }),
        )
        .await;
        let kill_result = match send_result {
            Ok(Ok(())) => {
                match time::timeout(
                    deadline.saturating_duration_since(Instant::now()),
                    response_rx,
                )
                .await
                {
                    Ok(Ok(result)) => result,
                    Ok(Err(_)) => Err(BackendError::Disconnected),
                    Err(_) => Err(BackendError::Timeout {
                        operation: "force kill".to_owned(),
                    }),
                }
            }
            Ok(Err(_)) => Err(BackendError::Disconnected),
            Err(_) => Err(BackendError::Timeout {
                operation: "force kill".to_owned(),
            }),
        };

        if let Err(error) = kill_result {
            if process_exit_rx.borrow().is_some() {
                let exit_result = wait_for_process_exit(process_exit_rx).await;
                let join_result =
                    join_task_bounded(process_task, TASK_JOIN_TIMEOUT, "process monitor join")
                        .await;
                return exit_result.and(join_result);
            }
            let cleanup =
                abort_and_join_task(process_task, TASK_JOIN_TIMEOUT, "process monitor abort").await;
            return cleanup.and(Err(error));
        }

        let exit_result = match time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            wait_for_process_exit(process_exit_rx),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(BackendError::Timeout {
                operation: "force reap".to_owned(),
            }),
        };
        if let Err(error) = exit_result {
            let cleanup =
                abort_and_join_task(process_task, TASK_JOIN_TIMEOUT, "process monitor abort").await;
            return cleanup.and(Err(error));
        }

        join_task_bounded(process_task, TASK_JOIN_TIMEOUT, "process monitor join").await
    }

    async fn join_task_bounded(
        task: &mut JoinHandle<()>,
        timeout: Duration,
        operation: &'static str,
    ) -> Result<(), BackendError> {
        match time::timeout(timeout, &mut *task).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) if error.is_cancelled() => Ok(()),
            Ok(Err(_)) => Err(BackendError::Operation {
                detail: format!("{operation} failed"),
            }),
            Err(_) => {
                let cleanup = abort_and_join_task(task, timeout, operation).await;
                cleanup.and(Err(BackendError::Timeout {
                    operation: operation.to_owned(),
                }))
            }
        }
    }

    async fn abort_and_join_task(
        task: &mut JoinHandle<()>,
        timeout: Duration,
        operation: &'static str,
    ) -> Result<(), BackendError> {
        task.abort();
        match time::timeout(timeout, &mut *task).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) if error.is_cancelled() => Ok(()),
            Ok(Err(_)) => Err(BackendError::Operation {
                detail: format!("{operation} failed"),
            }),
            Err(_) => Err(BackendError::Timeout {
                operation: operation.to_owned(),
            }),
        }
    }

    async fn connect_pipe(
        pipe_name: &str,
        timeout: Duration,
        process_exit_rx: &mut watch::Receiver<Option<ProcessExit>>,
    ) -> Result<NamedPipeClient, BackendError> {
        let deadline = Instant::now() + timeout;
        let mut retry_delay = Duration::from_millis(50);
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
            let delay = remaining.min(retry_delay);
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
            retry_delay = (retry_delay + Duration::from_millis(50)).min(Duration::from_millis(250));
        }
    }

    fn is_retryable_pipe_error(error: &io::Error) -> bool {
        error.kind() == ErrorKind::NotFound || error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum FrameWriteError {
        Disconnected,
        Timeout,
    }

    pub(super) async fn write_frame_bounded<W: tokio::io::AsyncWrite + Unpin>(
        writer: &mut W,
        frame: &[u8],
        deadline: Instant,
    ) -> Result<(), FrameWriteError> {
        match time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            writer.write_all(frame),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(FrameWriteError::Disconnected),
            Err(_) => Err(FrameWriteError::Timeout),
        }
    }

    pub(super) async fn write_pending_request<W: tokio::io::AsyncWrite + Unpin>(
        writer: &mut W,
        pending: &mut HashMap<i64, PendingRequest>,
        request_id: i64,
        frame: &[u8],
        operation: &'static str,
        deadline: Instant,
        response: oneshot::Sender<Result<Value, BackendError>>,
    ) -> Result<(), BackendError> {
        pending.insert(
            request_id,
            PendingRequest {
                operation,
                deadline,
                response,
            },
        );
        match write_frame_bounded(writer, frame, deadline).await {
            Ok(()) => Ok(()),
            Err(FrameWriteError::Disconnected) => {
                if let Some(request) = pending.remove(&request_id) {
                    let _ = request.response.send(Err(BackendError::Disconnected));
                }
                Err(BackendError::Disconnected)
            }
            Err(FrameWriteError::Timeout) => {
                if let Some(request) = pending.remove(&request_id) {
                    let operation = request.operation.to_owned();
                    let _ = request
                        .response
                        .send(Err(BackendError::Timeout { operation }));
                }
                Err(BackendError::Timeout {
                    operation: "write request".to_owned(),
                })
            }
        }
    }

    pub(super) fn signal_unhealthy_generation_kill(
        process_control_tx: &mpsc::Sender<ProcessControl>,
        event_tx: &mpsc::UnboundedSender<StampedSessionEvent>,
        generation: u64,
    ) -> Result<(), BackendError> {
        let (response_tx, response_rx) = oneshot::channel();
        match process_control_tx.try_send(ProcessControl::Kill {
            response: response_tx,
        }) {
            Ok(()) => {
                let event_tx = event_tx.clone();
                tokio::spawn(async move {
                    let error = match time::timeout(FORCED_REAP_TIMEOUT, response_rx).await {
                        Ok(Ok(Ok(()))) => None,
                        Ok(Ok(Err(error))) => Some(error),
                        Ok(Err(_)) => Some(BackendError::Disconnected),
                        Err(_) => Some(BackendError::Timeout {
                            operation: "force kill".to_owned(),
                        }),
                    };
                    if let Some(error) = error {
                        let _ = event_tx.send(StampedSessionEvent {
                            generation,
                            event: SessionEvent::Disconnected(error),
                        });
                    }
                });
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                // The capacity-one channel carries only Kill, so a full channel
                // means termination is already pending for this generation.
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(BackendError::Operation {
                detail: "mpv process termination channel is closed".to_owned(),
            }),
        }
    }

    pub(super) fn terminate_after_write_failure(
        write_error: BackendError,
        process_control_tx: &mpsc::Sender<ProcessControl>,
        event_tx: &mpsc::UnboundedSender<StampedSessionEvent>,
        generation: u64,
    ) -> BackendError {
        signal_unhealthy_generation_kill(process_control_tx, event_tx, generation)
            .err()
            .unwrap_or(write_error)
    }

    async fn run_pipe_session(
        client: NamedPipeClient,
        generation: u64,
        mut request_rx: mpsc::Receiver<OutboundRequest>,
        event_tx: mpsc::UnboundedSender<StampedSessionEvent>,
        process_control_tx: mpsc::Sender<ProcessControl>,
        mut process_exit_rx: watch::Receiver<Option<ProcessExit>>,
        position_event_interval: Duration,
    ) {
        let (reader, mut writer) = tokio::io::split(client);
        let mut frame_reader = BoundedFrameReader::new(reader);
        let mut pending = HashMap::<i64, PendingRequest>::new();
        let request_ids = RequestIdGenerator::new();
        let mut ticker = time::interval(Duration::from_millis(10));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut last_position_event: Option<Instant> = None;
        let mut disconnect_error = BackendError::Disconnected;

        loop {
            tokio::select! {
                changed = process_exit_rx.changed() => {
                    if changed.is_err() {
                        disconnect_error = BackendError::Disconnected;
                        break;
                    }
                    if let Some(exit) = process_exit_rx.borrow().clone() {
                        let _ = event_tx.send(StampedSessionEvent {
                            generation,
                            event: SessionEvent::Backend(BackendEvent::ProcessExited {
                                expected: false,
                                code: exit.code,
                            }),
                        });
                        disconnect_error = exit.result.err().unwrap_or(BackendError::Disconnected);
                        break;
                    }
                }
                request = request_rx.recv() => {
                    let Some(request) = request else {
                        break;
                    };
                    let OutboundRequest {
                        operation,
                        command,
                        deadline,
                        response,
                    } = request;
                    let request_id = request_ids.next_id();
                    let frame = match serialize_request(command, request_id) {
                        Ok(frame) => frame,
                        Err(error) => {
                            let _ = response.send(Err(error));
                            continue;
                        }
                    };
                    if let Err(error) = write_pending_request(
                        &mut writer,
                        &mut pending,
                        request_id,
                        &frame,
                        operation,
                        deadline,
                        response,
                    )
                    .await
                    {
                        disconnect_error = terminate_after_write_failure(
                            error,
                            &process_control_tx,
                            &event_tx,
                            generation,
                        );
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
                            let emit = if matches!(event, BackendEvent::PositionChanged(_)) {
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
            "--terminal=no".into(),
            "--input-terminal=no".into(),
            "--audio-display=no".into(),
            format!("--input-ipc-server={pipe_name}").into(),
        ]
    }

    #[cfg(test)]
    pub(super) mod test_support {
        use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

        use super::*;

        pub(crate) type ProtocolSessionParts = (
            mpsc::Sender<OutboundRequest>,
            mpsc::UnboundedReceiver<StampedSessionEvent>,
            JoinHandle<()>,
            watch::Sender<Option<ProcessExit>>,
            mpsc::Receiver<ProcessControl>,
        );

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
        ) -> ProtocolSessionParts {
            let (request_tx, request_rx) = mpsc::channel(8);
            let (event_tx, event_rx) = mpsc::unbounded_channel();
            let (process_control_tx, process_control_rx) = mpsc::channel(1);
            let (process_exit_tx, process_exit_rx) = watch::channel(None);
            let task = tokio::spawn(run_pipe_session(
                client,
                7,
                request_rx,
                event_tx,
                process_control_tx,
                process_exit_rx,
                Duration::from_millis(250),
            ));
            let _ = request_timeout;
            (
                request_tx,
                event_rx,
                task,
                process_exit_tx,
                process_control_rx,
            )
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
    fn replies_require_positive_request_ids() {
        assert!(matches!(
            parse_frame(br#"{"request_id":9223372036854775808,"error":"success"}"#),
            Err(BackendError::Protocol { .. })
        ));
        assert!(matches!(
            parse_frame(br#"{"request_id":-7,"error":"success","data":true}"#),
            Err(BackendError::Protocol { .. })
        ));
        assert_eq!(
            parse_frame(br#"{"request_id":7,"error":"success","data":true}"#).unwrap(),
            IncomingFrame::Reply {
                request_id: 7,
                error: "success".to_owned(),
                data: json!(true),
            }
        );
    }

    #[test]
    fn property_events_convert_to_typed_product_events() {
        assert_eq!(
            parse_property_change(&json!({"name":"pause","data":true})).unwrap(),
            Some(BackendEvent::PauseChanged(true))
        );
        assert_eq!(
            parse_property_change(&json!({"name":"time-pos","data":1.234})).unwrap(),
            Some(BackendEvent::PositionChanged(1_234))
        );
        assert_eq!(
            parse_property_change(&json!({"name":"seeking","data":false})).unwrap(),
            Some(BackendEvent::SeekingChanged(false))
        );
        assert_eq!(
            parse_property_change(&json!({"name":"audio-device","data":"auto"})).unwrap(),
            Some(BackendEvent::AudioDeviceChanged("auto".to_owned()))
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
            IncomingFrame::Event(BackendEvent::EndFile(EndFileReason::Eof))
        );
        assert_eq!(
            parse_end_file(&json!({"reason":"stop"})).unwrap(),
            BackendEvent::EndFile(EndFileReason::Stop)
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
        assert_eq!(devices[0].name, "auto");
        assert!(!devices[0].selected);
        assert!(!devices[1].selected);
        let selected_devices = parse_audio_devices_with_selected(
            &json!([
                {"name":"auto","description":"Default output"},
                {"name":"wasapi/device","description":"Speakers"}
            ]),
            Some("wasapi/device"),
        )
        .unwrap();
        assert!(!selected_devices[0].selected);
        assert!(selected_devices[1].selected);
        assert_eq!(seconds_to_millis(2.345), Some(2_345));
        assert_eq!(seconds_to_millis(-1.0), Some(0));
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
            ]
        );
    }

    #[cfg(windows)]
    mod windows_protocol {
        use std::collections::HashMap;
        use std::ffi::OsString;
        use std::future::pending;
        use std::io;
        use std::pin::Pin;
        use std::task::{Context, Poll};

        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
        use tokio::time;

        use super::*;
        use crate::playback::mpv::windows_session::test_support::{
            connected_pair, exact_args, start_protocol_session,
        };
        use crate::playback::mpv::windows_session::{
            force_process_and_reap, send_request, terminate_after_write_failure,
            wait_for_file_loaded_events, write_frame_bounded, write_pending_request,
            FrameWriteError, ProcessControl, ProcessExit, SessionEvent, StampedSessionEvent,
        };

        struct PendingWriter;

        impl AsyncWrite for PendingWriter {
            fn poll_write(
                self: Pin<&mut Self>,
                _context: &mut Context<'_>,
                _buffer: &[u8],
            ) -> Poll<io::Result<usize>> {
                Poll::Pending
            }

            fn poll_flush(
                self: Pin<&mut Self>,
                _context: &mut Context<'_>,
            ) -> Poll<io::Result<()>> {
                Poll::Ready(Ok(()))
            }

            fn poll_shutdown(
                self: Pin<&mut Self>,
                _context: &mut Context<'_>,
            ) -> Poll<io::Result<()>> {
                Poll::Ready(Ok(()))
            }
        }

        #[test]
        fn launch_arguments_are_exact_and_structured() {
            let pipe_name = r"\\.\pipe\opaque";
            assert_eq!(
                exact_args(pipe_name),
                vec![
                    OsString::from("--no-config"),
                    OsString::from("--idle=yes"),
                    OsString::from("--terminal=no"),
                    OsString::from("--input-terminal=no"),
                    OsString::from("--audio-display=no"),
                    OsString::from(format!("--input-ipc-server={pipe_name}")),
                ]
            );
        }

        #[tokio::test]
        async fn correlates_replies_across_interleaved_events_with_unique_ids() {
            let (server, client) = connected_pair().await;
            let (request_tx, mut event_rx, task, _process_exit_tx, _process_control_rx) =
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
                SessionEvent::Backend(BackendEvent::PauseChanged(true))
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
            let (request_tx, mut event_rx, task, _process_exit_tx, _process_control_rx) =
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
        async fn process_exit_is_emitted_with_generation_and_exit_code() {
            let (_server, client) = connected_pair().await;
            let (request_tx, mut event_rx, task, process_exit_tx, _process_control_rx) =
                start_protocol_session(client, Duration::from_secs(1));
            process_exit_tx
                .send(Some(ProcessExit {
                    result: Ok(()),
                    code: Some(17),
                }))
                .unwrap();

            let exited = time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(exited.generation, 7);
            assert!(matches!(
                exited.event,
                SessionEvent::Backend(BackendEvent::ProcessExited {
                    expected: false,
                    code: Some(17),
                })
            ));

            let disconnected = time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(matches!(
                disconnected.event,
                SessionEvent::Disconnected(BackendError::Disconnected)
            ));
            drop(request_tx);
            time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap();
        }

        #[tokio::test]
        async fn requests_time_out_when_the_server_does_not_reply() {
            let (mut server, client) = connected_pair().await;
            let (request_tx, _event_rx, task, _process_exit_tx, _process_control_rx) =
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
        async fn blocked_pipe_writes_stop_at_the_request_deadline() {
            let mut writer = PendingWriter;
            let started = time::Instant::now();
            let frame_result = time::timeout(
                Duration::from_secs(1),
                write_frame_bounded(
                    &mut writer,
                    b"frame",
                    time::Instant::now() + Duration::from_millis(30),
                ),
            )
            .await
            .unwrap();

            assert_eq!(frame_result, Err(FrameWriteError::Timeout));
            assert!(started.elapsed() < Duration::from_millis(500));
            let (response_tx, response_rx) = tokio::sync::oneshot::channel();
            let mut pending = HashMap::new();
            let actor_result = write_pending_request(
                &mut writer,
                &mut pending,
                41,
                b"frame",
                "stalled write",
                time::Instant::now() + Duration::from_millis(30),
                response_tx,
            )
            .await;
            assert_eq!(
                response_rx.await.unwrap(),
                Err(BackendError::Timeout {
                    operation: "stalled write".to_owned()
                })
            );
            assert!(pending.is_empty());
            let write_error = actor_result.unwrap_err();
            assert_eq!(
                write_error,
                BackendError::Timeout {
                    operation: "write request".to_owned()
                }
            );

            let (process_control_tx, mut process_control_rx) = tokio::sync::mpsc::channel(1);
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
            assert_eq!(
                terminate_after_write_failure(write_error, &process_control_tx, &event_tx, 7),
                BackendError::Timeout {
                    operation: "write request".to_owned()
                }
            );
            let control = process_control_rx.try_recv().unwrap();
            let ProcessControl::Kill { response } = control;
            response
                .send(Err(BackendError::Operation {
                    detail: "injected write-path kill failure".to_owned(),
                }))
                .unwrap();
            let event = time::timeout(Duration::from_secs(1), event_rx.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(matches!(
                event,
                StampedSessionEvent {
                    generation: 7,
                    event: SessionEvent::Disconnected(BackendError::Operation { detail })
                } if detail == "injected write-path kill failure"
            ));

            let (closed_control_tx, closed_control_rx) = tokio::sync::mpsc::channel(1);
            drop(closed_control_rx);
            assert_eq!(
                terminate_after_write_failure(
                    BackendError::Disconnected,
                    &closed_control_tx,
                    &event_tx,
                    7,
                ),
                BackendError::Operation {
                    detail: "mpv process termination channel is closed".to_owned()
                }
            );
        }

        #[tokio::test]
        async fn forced_reap_timeout_aborts_and_joins_the_process_monitor() {
            let (process_control_tx, mut process_control_rx) = tokio::sync::mpsc::channel(1);
            let (process_exit_tx, mut process_exit_rx) =
                tokio::sync::watch::channel::<Option<ProcessExit>>(None);
            let mut process_task = tokio::spawn(async move {
                if let Some(ProcessControl::Kill { response }) = process_control_rx.recv().await {
                    response.send(Ok(())).unwrap();
                    let _keep_exit_channel_open = process_exit_tx;
                    pending::<()>().await;
                }
            });
            let started = time::Instant::now();

            let result = time::timeout(
                Duration::from_secs(1),
                force_process_and_reap(
                    &process_control_tx,
                    &mut process_exit_rx,
                    &mut process_task,
                    Duration::from_millis(30),
                ),
            )
            .await
            .unwrap();

            assert_eq!(
                result,
                Err(BackendError::Timeout {
                    operation: "force reap".to_owned()
                })
            );
            assert!(started.elapsed() < Duration::from_millis(500));
            assert!(process_task.is_finished());
        }

        #[tokio::test]
        async fn force_kill_failure_is_returned_after_bounded_monitor_cleanup() {
            let (process_control_tx, mut process_control_rx) = tokio::sync::mpsc::channel(1);
            let (process_exit_tx, mut process_exit_rx) =
                tokio::sync::watch::channel::<Option<ProcessExit>>(None);
            let mut process_task = tokio::spawn(async move {
                if let Some(ProcessControl::Kill { response }) = process_control_rx.recv().await {
                    response
                        .send(Err(BackendError::Operation {
                            detail: "injected start_kill failure".to_owned(),
                        }))
                        .unwrap();
                    let _keep_exit_channel_open = process_exit_tx;
                    pending::<()>().await;
                }
            });

            let result = time::timeout(
                Duration::from_secs(1),
                force_process_and_reap(
                    &process_control_tx,
                    &mut process_exit_rx,
                    &mut process_task,
                    Duration::from_millis(30),
                ),
            )
            .await
            .unwrap();

            assert_eq!(
                result,
                Err(BackendError::Operation {
                    detail: "injected start_kill failure".to_owned()
                })
            );
            assert!(process_task.is_finished());
        }

        #[tokio::test]
        async fn load_wait_ignores_interleaved_events_until_file_loaded() {
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
            event_tx
                .send(StampedSessionEvent {
                    generation: 7,
                    event: SessionEvent::Backend(BackendEvent::PauseChanged(false)),
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
                vec![BackendEvent::PauseChanged(false)]
            );
        }
    }
}
