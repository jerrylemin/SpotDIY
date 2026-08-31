pub mod backend;
pub mod mpv;
pub mod protocol;
pub mod queue;
pub mod types;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tokio::sync::mpsc as tokio_mpsc;
use tokio::sync::watch;

use crate::db::repository::TrackRepository;
use crate::domain::{ProviderKind, SourceId, TrackId, TrackSource, UnifiedTrack};
use crate::library::{LibraryError, LibraryService};
use crate::media_tools::MediaToolManager;

pub use self::backend::{
    BackendCommand, BackendEvent, EndFileReason, GenerationStampedBackendEvent, PlaybackBackend,
    PlaybackBackendSession,
};
pub use self::types::{
    AudioDevice, PlaybackBackendHealth, PlaybackError, PlaybackErrorCode, PlaybackErrorDto,
    PlaybackPhase, PlaybackSnapshot, PlaybackSourceOption, QueueEntry, QueueEntryId, RepeatMode,
    TrackPlaybackRequest, TransientQueue,
};
pub type BackendHealth = PlaybackBackendHealth;
use self::backend::BackendError;
use self::mpv::MpvBackend;

pub const PREVIOUS_RESTART_THRESHOLD_MS: u64 = 3_000;
pub const PLAYBACK_STATE_EVENT: &str = "playback://state";

const COMMAND_CAPACITY: usize = 64;
const COMMAND_SEND_TIMEOUT: Duration = Duration::from_millis(250);
const COMMAND_RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);
const CONTROLLER_TICK: Duration = Duration::from_millis(5);
const PENDING_LOAD_TIMEOUT: Duration = Duration::from_secs(10);
const CONTROLLER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
const RECOVERY_BACKOFF: [Duration; 3] = [
    Duration::from_millis(250),
    Duration::from_millis(750),
    Duration::from_millis(1_500),
];
const MAX_RECOVERY_ATTEMPTS: usize = 3;

pub type SnapshotSink = Arc<dyn Fn(PlaybackSnapshot) + Send + Sync>;
type BackendFactory = Arc<dyn Fn(u64) -> PlaybackBackendSession + Send + Sync>;

pub struct PlaybackService {
    command_tx: tokio_mpsc::Sender<Command>,
    snapshot_rx: watch::Receiver<PlaybackSnapshot>,
    accepting_commands: Arc<AtomicBool>,
    done_rx: Arc<Mutex<Receiver<()>>>,
    controller_thread: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl Clone for PlaybackService {
    fn clone(&self) -> Self {
        Self {
            command_tx: self.command_tx.clone(),
            snapshot_rx: self.snapshot_rx.clone(),
            accepting_commands: self.accepting_commands.clone(),
            done_rx: self.done_rx.clone(),
            controller_thread: self.controller_thread.clone(),
        }
    }
}

impl PlaybackService {
    pub fn new(library: LibraryService, manager: MediaToolManager, sink: SnapshotSink) -> Self {
        let backend_manager = manager.clone();
        Self::new_with_backend_factory(
            library,
            move |generation| MpvBackend::start(backend_manager.clone(), generation),
            sink,
        )
    }

    pub fn new_with_backend_factory<F>(
        library: LibraryService,
        backend_factory: F,
        sink: SnapshotSink,
    ) -> Self
    where
        F: Fn(u64) -> PlaybackBackendSession + Send + Sync + 'static,
    {
        let (command_tx, command_rx) = tokio_mpsc::channel(COMMAND_CAPACITY);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let (snapshot_tx, snapshot_rx) = watch::channel(PlaybackSnapshot::default());
        let factory: BackendFactory = Arc::new(backend_factory);
        let controller_thread = thread::Builder::new()
            .name("spotdiy-playback-controller".to_owned())
            .spawn(move || {
                let mut controller = Controller::new(library, factory, sink, snapshot_tx);
                controller.initialize();
                let _ = ready_tx.send(());
                controller.run(command_rx);
                let _ = done_tx.send(());
            })
            .expect("the playback controller thread should start");
        let _ = ready_rx.recv_timeout(COMMAND_RESPONSE_TIMEOUT);

        Self {
            command_tx,
            snapshot_rx,
            accepting_commands: Arc::new(AtomicBool::new(true)),
            done_rx: Arc::new(Mutex::new(done_rx)),
            controller_thread: Arc::new(Mutex::new(Some(controller_thread))),
        }
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        self.snapshot_rx.borrow().clone()
    }

    pub fn subscribe_snapshot(&self) -> watch::Receiver<PlaybackSnapshot> {
        self.snapshot_rx.clone()
    }

    pub fn play_track(
        &self,
        request: TrackPlaybackRequest,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::PlayTrack { request, reply })
    }

    pub fn enqueue_track(
        &self,
        request: TrackPlaybackRequest,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::EnqueueTrack { request, reply })
    }

    pub fn play_track_next(
        &self,
        request: TrackPlaybackRequest,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::PlayTrackNext { request, reply })
    }

    pub fn toggle_play_pause(&self) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::TogglePlayPause { reply })
    }

    pub fn seek_playback(&self, position_ms: u64) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::Seek { position_ms, reply })
    }

    pub fn next_track(&self) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::Next { reply })
    }

    pub fn previous_track(&self) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::Previous { reply })
    }

    pub fn set_playback_volume(
        &self,
        volume_percent: u8,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::SetVolume {
            volume_percent,
            reply,
        })
    }

    pub fn set_playback_muted(&self, muted: bool) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::SetMuted { muted, reply })
    }

    pub fn set_repeat_mode(
        &self,
        repeat_mode: RepeatMode,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::SetRepeat { repeat_mode, reply })
    }

    pub fn set_shuffle_enabled(&self, enabled: bool) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::SetShuffle { enabled, reply })
    }

    pub fn get_audio_devices(&self) -> Result<Vec<AudioDevice>, PlaybackError> {
        self.command(|reply| Command::GetAudioDevices { reply }, false)
    }

    pub fn set_audio_device(
        &self,
        name: impl Into<String>,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        let name = name.into();
        self.snapshot_command(|reply| Command::SetAudioDevice { name, reply })
    }

    pub fn switch_playback_source(
        &self,
        request: TrackPlaybackRequest,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::SwitchSource { request, reply })
    }

    pub fn retry_playback_backend(&self) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::RetryBackend { reply })
    }

    pub fn clear_playback_queue(&self) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot_command(|reply| Command::ClearQueue { reply })
    }

    /// Async product-shaped aliases for callers that already run on Tokio.
    /// The controller itself remains the single serialized mutation owner.
    pub async fn play_now(
        &self,
        track_id: TrackId,
        source_id: Option<SourceId>,
    ) -> Result<(), PlaybackError> {
        self.play_track(TrackPlaybackRequest {
            track_id,
            source_id,
        })
        .map(|_| ())
    }

    pub async fn enqueue(
        &self,
        track_id: TrackId,
        source_id: Option<SourceId>,
    ) -> Result<(), PlaybackError> {
        self.enqueue_track(TrackPlaybackRequest {
            track_id,
            source_id,
        })
        .map(|_| ())
    }

    pub async fn play_next(
        &self,
        track_id: TrackId,
        source_id: Option<SourceId>,
    ) -> Result<(), PlaybackError> {
        self.play_track_next(TrackPlaybackRequest {
            track_id,
            source_id,
        })
        .map(|_| ())
    }

    pub async fn seek(&self, position_ms: u64) -> Result<(), PlaybackError> {
        self.seek_playback(position_ms).map(|_| ())
    }

    pub async fn next(&self) -> Result<(), PlaybackError> {
        self.next_track().map(|_| ())
    }

    pub async fn previous(&self) -> Result<(), PlaybackError> {
        self.previous_track().map(|_| ())
    }

    pub async fn set_volume(&self, value: u8) -> Result<(), PlaybackError> {
        self.set_playback_volume(value).map(|_| ())
    }

    pub async fn set_muted(&self, muted: bool) -> Result<(), PlaybackError> {
        self.set_playback_muted(muted).map(|_| ())
    }

    pub async fn set_shuffle(&self, enabled: bool) -> Result<(), PlaybackError> {
        self.set_shuffle_enabled(enabled).map(|_| ())
    }

    pub async fn switch_source(&self, source_id: SourceId) -> Result<(), PlaybackError> {
        let track_id = self
            .snapshot()
            .current_track_id
            .ok_or_else(queue_empty_error)?;
        self.switch_playback_source(TrackPlaybackRequest {
            track_id,
            source_id: Some(source_id),
        })
        .map(|_| ())
    }

    pub async fn clear_queue(&self) -> Result<(), PlaybackError> {
        self.clear_playback_queue().map(|_| ())
    }

    pub async fn retry_backend(&self) -> Result<(), PlaybackError> {
        self.retry_playback_backend().map(|_| ())
    }

    pub fn shutdown(&self) -> Result<PlaybackSnapshot, PlaybackError> {
        if self
            .accepting_commands
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(self.snapshot());
        }

        let deadline = Instant::now() + CONTROLLER_SHUTDOWN_TIMEOUT;
        let result = self.command_with_timeout(
            |reply| Command::Shutdown { reply },
            true,
            deadline.saturating_duration_since(Instant::now()),
        );
        let remaining = deadline.saturating_duration_since(Instant::now());
        let controller_stopped = lock(&self.done_rx).recv_timeout(remaining).is_ok();
        if controller_stopped {
            let mut controller_thread = lock(&self.controller_thread);
            if controller_thread
                .as_ref()
                .is_some_and(JoinHandle::is_finished)
            {
                if let Some(handle) = controller_thread.take() {
                    let _ = handle.join();
                }
            }
        }
        if !controller_stopped {
            return Err(PlaybackError::new(
                PlaybackErrorCode::RequestTimeout,
                "the playback controller did not shut down within 3 seconds",
                true,
            ));
        }
        result
    }

    fn snapshot_command(
        &self,
        build: impl FnOnce(Sender<Result<PlaybackSnapshot, PlaybackError>>) -> Command,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        self.command(build, false)
    }

    fn command<T: Send + 'static>(
        &self,
        build: impl FnOnce(Sender<Result<T, PlaybackError>>) -> Command,
        allow_after_shutdown: bool,
    ) -> Result<T, PlaybackError> {
        self.command_with_timeout(build, allow_after_shutdown, COMMAND_RESPONSE_TIMEOUT)
    }

    fn command_with_timeout<T: Send + 'static>(
        &self,
        build: impl FnOnce(Sender<Result<T, PlaybackError>>) -> Command,
        allow_after_shutdown: bool,
        response_timeout: Duration,
    ) -> Result<T, PlaybackError> {
        if !allow_after_shutdown && !self.accepting_commands.load(Ordering::Acquire) {
            return Err(shutting_down_error());
        }
        let (reply_tx, reply_rx) = mpsc::channel();
        let mut command = build(reply_tx);
        let deadline = Instant::now() + COMMAND_SEND_TIMEOUT;
        loop {
            match self.command_tx.try_send(command) {
                Ok(()) => break,
                Err(tokio_mpsc::error::TrySendError::Full(returned))
                    if Instant::now() < deadline =>
                {
                    command = returned;
                    thread::yield_now();
                }
                Err(tokio_mpsc::error::TrySendError::Full(_)) => {
                    return Err(PlaybackError::new(
                        PlaybackErrorCode::RequestTimeout,
                        "the playback command queue is busy",
                        true,
                    ));
                }
                Err(tokio_mpsc::error::TrySendError::Closed(_)) => {
                    return Err(controller_unavailable_error());
                }
            }
        }
        reply_rx
            .recv_timeout(response_timeout)
            .map_err(|_| controller_unavailable_error())?
    }
}

impl Drop for PlaybackService {
    fn drop(&mut self) {
        if Arc::strong_count(&self.controller_thread) == 1 {
            let _ = self.shutdown();
        }
    }
}

enum Command {
    PlayTrack {
        request: TrackPlaybackRequest,
        reply: SnapshotReply,
    },
    EnqueueTrack {
        request: TrackPlaybackRequest,
        reply: SnapshotReply,
    },
    PlayTrackNext {
        request: TrackPlaybackRequest,
        reply: SnapshotReply,
    },
    TogglePlayPause {
        reply: SnapshotReply,
    },
    Seek {
        position_ms: u64,
        reply: SnapshotReply,
    },
    Next {
        reply: SnapshotReply,
    },
    Previous {
        reply: SnapshotReply,
    },
    SetVolume {
        volume_percent: u8,
        reply: SnapshotReply,
    },
    SetMuted {
        muted: bool,
        reply: SnapshotReply,
    },
    SetRepeat {
        repeat_mode: RepeatMode,
        reply: SnapshotReply,
    },
    SetShuffle {
        enabled: bool,
        reply: SnapshotReply,
    },
    GetAudioDevices {
        reply: Sender<Result<Vec<AudioDevice>, PlaybackError>>,
    },
    SetAudioDevice {
        name: String,
        reply: SnapshotReply,
    },
    SwitchSource {
        request: TrackPlaybackRequest,
        reply: SnapshotReply,
    },
    RetryBackend {
        reply: SnapshotReply,
    },
    ClearQueue {
        reply: SnapshotReply,
    },
    Shutdown {
        reply: SnapshotReply,
    },
}

type SnapshotReply = Sender<Result<PlaybackSnapshot, PlaybackError>>;

struct ResolvedPlayback {
    track_id: TrackId,
    source_id: SourceId,
    path: PathBuf,
    title: String,
    artists: Vec<String>,
    album: Option<String>,
    artwork_path: Option<PathBuf>,
    sources: Vec<PlaybackSourceOption>,
    duration_ms: Option<u64>,
}

#[derive(Clone)]
struct PlaybackRestore {
    track_id: TrackId,
    source_id: SourceId,
    position_ms: u64,
    paused: bool,
}

enum LoadPurpose {
    Normal,
    SwitchTarget {
        prior: PlaybackRestore,
    },
    SwitchRollback {
        prior: PlaybackRestore,
        original_error: PlaybackError,
    },
    Recovery {
        token: u64,
    },
}

struct PendingLoad {
    resolved: ResolvedPlayback,
    desired_position_ms: u64,
    desired_paused: bool,
    purpose: LoadPurpose,
    deadline: Instant,
}

struct RecoveryPlan {
    token: u64,
    attempts: usize,
    due_at: Instant,
    restore: Option<PlaybackRestore>,
    waiting_for_backend: bool,
}

struct Controller {
    library: LibraryService,
    backend_factory: BackendFactory,
    backend: Arc<dyn PlaybackBackend>,
    backend_events: tokio_mpsc::Receiver<GenerationStampedBackendEvent>,
    backend_generation: u64,
    next_backend_generation: u64,
    terminal_failure_generation: Option<u64>,
    sink: SnapshotSink,
    snapshot_tx: watch::Sender<PlaybackSnapshot>,
    snapshot: PlaybackSnapshot,
    queue: TransientQueue,
    pending_load: Option<PendingLoad>,
    recovery: Option<RecoveryPlan>,
    next_recovery_token: u64,
    phase_before_seeking: Option<PlaybackPhase>,
    desired_paused: bool,
    shutting_down: bool,
    pending_audio_devices: Option<Sender<Result<Vec<AudioDevice>, PlaybackError>>>,
}

impl Controller {
    fn new(
        library: LibraryService,
        backend_factory: BackendFactory,
        sink: SnapshotSink,
        snapshot_tx: watch::Sender<PlaybackSnapshot>,
    ) -> Self {
        let backend_generation = 1;
        let PlaybackBackendSession { backend, events } = backend_factory(backend_generation);
        Self {
            library,
            backend_factory,
            backend,
            backend_events: events,
            backend_generation,
            next_backend_generation: backend_generation,
            terminal_failure_generation: None,
            sink,
            snapshot_tx,
            snapshot: PlaybackSnapshot::default(),
            queue: TransientQueue::new(),
            pending_load: None,
            recovery: None,
            next_recovery_token: 0,
            phase_before_seeking: None,
            desired_paused: false,
            shutting_down: false,
            pending_audio_devices: None,
        }
    }

    fn initialize(&mut self) {
        self.snapshot.phase = PlaybackPhase::Idle;
        self.snapshot.error = None;
        self.publish();
    }

    fn run(&mut self, mut command_rx: tokio_mpsc::Receiver<Command>) {
        while !self.shutting_down {
            self.process_backend_events();
            self.expire_pending_load();
            self.run_recovery_attempt();
            if self.shutting_down {
                break;
            }

            match command_rx.try_recv() {
                Ok(command) => {
                    self.handle_command(command);
                    for _ in 0..COMMAND_CAPACITY {
                        let Ok(command) = command_rx.try_recv() else {
                            break;
                        };
                        self.handle_command(command);
                        if self.shutting_down {
                            break;
                        }
                    }
                }
                Err(tokio_mpsc::error::TryRecvError::Empty) => {
                    thread::sleep(CONTROLLER_TICK);
                }
                Err(tokio_mpsc::error::TryRecvError::Disconnected) => {
                    let _ = self.backend.shutdown();
                    break;
                }
            }
        }
    }

    fn handle_command(&mut self, command: Command) {
        match command {
            Command::PlayTrack { request, reply } => {
                let result = self.play_track(request);
                let _ = reply.send(result);
            }
            Command::EnqueueTrack { request, reply } => {
                let result = self.enqueue(request, false);
                let _ = reply.send(result);
            }
            Command::PlayTrackNext { request, reply } => {
                let result = self.enqueue(request, true);
                let _ = reply.send(result);
            }
            Command::TogglePlayPause { reply } => {
                let result = self.toggle_play_pause();
                let _ = reply.send(result);
            }
            Command::Seek { position_ms, reply } => {
                let result = self.seek(position_ms);
                let _ = reply.send(result);
            }
            Command::Next { reply } => {
                let result = self.next_track();
                let _ = reply.send(result);
            }
            Command::Previous { reply } => {
                let result = self.previous_track();
                let _ = reply.send(result);
            }
            Command::SetVolume {
                volume_percent,
                reply,
            } => {
                let result = self.set_volume(volume_percent);
                let _ = reply.send(result);
            }
            Command::SetMuted { muted, reply } => {
                let result = self.set_muted(muted);
                let _ = reply.send(result);
            }
            Command::SetRepeat { repeat_mode, reply } => {
                self.snapshot.repeat_mode = repeat_mode;
                self.publish();
                let _ = reply.send(Ok(self.snapshot.clone()));
            }
            Command::SetShuffle { enabled, reply } => {
                self.queue.set_shuffle(enabled);
                self.snapshot.shuffle_enabled = enabled;
                self.publish();
                let _ = reply.send(Ok(self.snapshot.clone()));
            }
            Command::GetAudioDevices { reply } => {
                if self.pending_audio_devices.is_some() {
                    let _ = reply.send(Err(PlaybackError::new(
                        PlaybackErrorCode::RequestTimeout,
                        "an audio-device request is already pending",
                        true,
                    )));
                } else if let Err(error) = self.backend.send(BackendCommand::QueryAudioDevices) {
                    let _ = reply.send(Err(error));
                } else {
                    self.pending_audio_devices = Some(reply);
                }
            }
            Command::SetAudioDevice { name, reply } => {
                let result = self.set_audio_device(name);
                let _ = reply.send(result);
            }
            Command::SwitchSource { request, reply } => {
                let result = self.switch_source(request);
                let _ = reply.send(result);
            }
            Command::RetryBackend { reply } => {
                let result = self.retry_backend();
                let _ = reply.send(result);
            }
            Command::ClearQueue { reply } => {
                let result = self.clear_queue();
                let _ = reply.send(result);
            }
            Command::Shutdown { reply } => {
                let result = self.shutdown();
                let _ = reply.send(result);
            }
        }
    }

    fn play_track(
        &mut self,
        request: TrackPlaybackRequest,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        self.reject_during_recovery()?;
        let resolved = self.resolve_for_play(&request)?;
        self.recovery = None;
        self.pending_load = None;
        let entry = self.queue_entry(resolved.track_id, Some(resolved.source_id));
        self.queue.play_now(entry);
        self.start_load(resolved, 0, false, LoadPurpose::Normal)
    }

    fn enqueue(
        &mut self,
        request: TrackPlaybackRequest,
        play_next: bool,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        let resolved = self.resolve_for_play(&request)?;
        let entry = self.queue_entry(resolved.track_id, Some(resolved.source_id));
        if play_next {
            self.queue.insert_next(entry);
        } else {
            self.queue.append(entry);
        }
        self.publish();
        Ok(self.snapshot.clone())
    }

    fn toggle_play_pause(&mut self) -> Result<PlaybackSnapshot, PlaybackError> {
        if let Some(pending) = self.pending_load.as_mut() {
            pending.desired_paused = !pending.desired_paused;
            self.desired_paused = pending.desired_paused;
            self.publish();
            return Ok(self.snapshot.clone());
        }
        if self.snapshot.current_track_id.is_none() && !self.queue.entries().is_empty() {
            let index = self
                .queue
                .next_index(RepeatMode::Off)
                .ok_or_else(queue_empty_error)?;
            let entry = self.queue.entries()[index].clone();
            let resolved = self.resolve_for_play(&TrackPlaybackRequest {
                track_id: entry.track_id,
                source_id: entry.requested_source_id,
            })?;
            return self.start_load(resolved, 0, false, LoadPurpose::Normal);
        }
        match self.snapshot.phase {
            PlaybackPhase::Playing | PlaybackPhase::Seeking => {
                self.backend.send(BackendCommand::SetPaused(true))?;
                self.snapshot.phase = PlaybackPhase::Paused;
                self.desired_paused = true;
            }
            PlaybackPhase::Paused => {
                self.backend.send(BackendCommand::SetPaused(false))?;
                self.snapshot.phase = PlaybackPhase::Playing;
                self.desired_paused = false;
            }
            PlaybackPhase::Ended if self.snapshot.current_track_id.is_some() => {
                self.backend.send(BackendCommand::SeekAbsoluteMs(0))?;
                self.backend.send(BackendCommand::SetPaused(false))?;
                self.snapshot.position_ms = 0;
                self.snapshot.phase = PlaybackPhase::Playing;
                self.desired_paused = false;
            }
            PlaybackPhase::ShuttingDown => return Err(shutting_down_error()),
            _ if self.snapshot.current_track_id.is_none() => return Err(queue_empty_error()),
            _ => {
                return Err(invalid_state_error(
                    "playback cannot be toggled in this state",
                ))
            }
        }
        self.snapshot.error = None;
        self.publish();
        Ok(self.snapshot.clone())
    }

    fn seek(&mut self, position_ms: u64) -> Result<PlaybackSnapshot, PlaybackError> {
        let position_ms = clamp_position(position_ms, self.snapshot.duration_ms);
        if let Some(pending) = self.pending_load.as_mut() {
            pending.desired_position_ms = position_ms;
            self.snapshot.position_ms = position_ms;
            self.publish();
            return Ok(self.snapshot.clone());
        }
        if self.snapshot.current_track_id.is_none() {
            return Err(queue_empty_error());
        }
        let return_phase = if self.snapshot.phase == PlaybackPhase::Paused {
            PlaybackPhase::Paused
        } else {
            PlaybackPhase::Playing
        };
        self.snapshot.phase = PlaybackPhase::Seeking;
        self.publish();
        if let Err(error) = self
            .backend
            .send(BackendCommand::SeekAbsoluteMs(position_ms))
        {
            self.begin_recovery(error.clone());
            return Err(error);
        }
        self.snapshot.position_ms = position_ms;
        self.snapshot.phase = return_phase;
        self.snapshot.error = None;
        self.publish();
        Ok(self.snapshot.clone())
    }

    fn next_track(&mut self) -> Result<PlaybackSnapshot, PlaybackError> {
        self.reject_during_recovery()?;
        let repeat = if self.snapshot.repeat_mode == RepeatMode::All {
            RepeatMode::All
        } else {
            RepeatMode::Off
        };
        self.advance_queue(repeat, false)
    }

    fn previous_track(&mut self) -> Result<PlaybackSnapshot, PlaybackError> {
        self.reject_during_recovery()?;
        if self.snapshot.current_track_id.is_none() {
            return Err(queue_empty_error());
        }
        if self.snapshot.position_ms > PREVIOUS_RESTART_THRESHOLD_MS {
            return self.seek(0);
        }
        let Some(index) = self
            .queue
            .previous_index_with_repeat(self.snapshot.repeat_mode)
        else {
            return self.seek(0);
        };
        let entry = self.queue.entries()[index].clone();
        let resolved = self.resolve_for_play(&TrackPlaybackRequest {
            track_id: entry.track_id,
            source_id: entry.requested_source_id,
        })?;
        self.start_load(resolved, 0, false, LoadPurpose::Normal)
    }

    fn set_volume(&mut self, volume_percent: u8) -> Result<PlaybackSnapshot, PlaybackError> {
        let volume_percent = volume_percent.min(100);
        self.snapshot.volume_percent = volume_percent;
        if self.backend.health().connected {
            if let Err(error) = self.backend.send(BackendCommand::SetVolume(volume_percent)) {
                self.publish();
                return Err(error);
            }
        }
        self.publish();
        Ok(self.snapshot.clone())
    }

    fn set_muted(&mut self, muted: bool) -> Result<PlaybackSnapshot, PlaybackError> {
        self.snapshot.muted = muted;
        if self.backend.health().connected {
            if let Err(error) = self.backend.send(BackendCommand::SetMuted(muted)) {
                self.publish();
                return Err(error);
            }
        }
        self.publish();
        Ok(self.snapshot.clone())
    }

    fn set_audio_device(&mut self, name: String) -> Result<PlaybackSnapshot, PlaybackError> {
        if name.trim().is_empty() {
            return Err(PlaybackError::new(
                PlaybackErrorCode::DeviceUnavailable,
                "the audio device name is empty",
                false,
            ));
        }
        self.backend
            .send(BackendCommand::SelectAudioDevice(name.clone()))?;
        self.snapshot.selected_audio_device = name;
        self.publish();
        Ok(self.snapshot.clone())
    }

    fn switch_source(
        &mut self,
        request: TrackPlaybackRequest,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        self.reject_during_recovery()?;
        let current_track_id = self
            .snapshot
            .current_track_id
            .ok_or_else(queue_empty_error)?;
        if request.track_id != current_track_id {
            return Err(PlaybackError::new(
                PlaybackErrorCode::SourceMismatch,
                "a source switch must target the current track",
                false,
            ));
        }
        let source_id = request.source_id.ok_or_else(|| {
            PlaybackError::new(
                PlaybackErrorCode::SourceNotFound,
                "a source switch requires a source id",
                false,
            )
        })?;
        if self.snapshot.current_source_id == Some(source_id) {
            return Ok(self.snapshot.clone());
        }
        let resolved = self.resolve_exact_source(current_track_id, source_id)?;
        let prior = self.current_restore()?;
        self.start_load(
            resolved,
            prior.position_ms,
            prior.paused,
            LoadPurpose::SwitchTarget { prior },
        )
    }

    fn retry_backend(&mut self) -> Result<PlaybackSnapshot, PlaybackError> {
        if self.shutting_down {
            return Err(shutting_down_error());
        }
        self.recovery = None;
        self.pending_load = None;
        self.begin_recovery(PlaybackError::new(
            PlaybackErrorCode::IpcDisconnected,
            "retrying the playback backend",
            true,
        ));
        Ok(self.snapshot.clone())
    }

    fn clear_queue(&mut self) -> Result<PlaybackSnapshot, PlaybackError> {
        self.recovery = None;
        self.pending_load = None;
        if self.snapshot.current_track_id.is_some() && self.backend.health().connected {
            self.backend.send(BackendCommand::Stop)?;
        }
        self.queue.clear();
        self.clear_current_metadata();
        self.desired_paused = false;
        self.snapshot.phase = PlaybackPhase::Idle;
        self.snapshot.recovering = false;
        self.snapshot.error = None;
        self.publish();
        Ok(self.snapshot.clone())
    }

    fn shutdown(&mut self) -> Result<PlaybackSnapshot, PlaybackError> {
        self.shutting_down = true;
        self.recovery = None;
        self.pending_load = None;
        if let Some(reply) = self.pending_audio_devices.take() {
            let _ = reply.send(Err(shutting_down_error()));
        }
        self.snapshot.phase = PlaybackPhase::ShuttingDown;
        self.snapshot.recovering = false;
        self.snapshot.error = None;
        self.publish();
        let result = self.backend.shutdown();
        self.publish();
        result.map(|()| self.snapshot.clone())
    }

    fn process_backend_events(&mut self) {
        for _ in 0..COMMAND_CAPACITY.saturating_mul(2) {
            let stamped = match self.backend_events.try_recv() {
                Ok(stamped) => stamped,
                Err(tokio_mpsc::error::TryRecvError::Empty) => break,
                Err(tokio_mpsc::error::TryRecvError::Disconnected) => {
                    self.handle_backend_failure_for_generation(
                        self.backend_generation,
                        PlaybackError::new(
                            PlaybackErrorCode::IpcDisconnected,
                            "the playback backend event worker stopped",
                            true,
                        ),
                        true,
                    );
                    break;
                }
            };
            if self.shutting_down {
                break;
            }
            // A replacement backend owns a new event receiver, but an already
            // buffered event may still be observed while recovery swaps it.
            // Keep the generation check at this controller boundary so stale
            // FileLoaded, position, EOF, and failure events cannot mutate state.
            if stamped.generation != self.backend_generation {
                continue;
            }
            self.process_backend_event(stamped.event);
        }

        let health = self.backend.health();
        if health != self.snapshot.backend_health {
            let was_connected = self.snapshot.backend_health.connected;
            self.snapshot.backend_health = health.clone();
            self.publish();
            if was_connected
                && !health.connected
                && !matches!(
                    self.snapshot.phase,
                    PlaybackPhase::Recovering | PlaybackPhase::Failed | PlaybackPhase::ShuttingDown
                )
            {
                self.handle_backend_failure_for_generation(
                    self.backend_generation,
                    PlaybackError::new(
                        PlaybackErrorCode::IpcDisconnected,
                        "the playback backend disconnected",
                        true,
                    ),
                    true,
                );
            }
        }
    }

    fn process_backend_event(&mut self, event: BackendEvent) {
        match event {
            BackendEvent::FileLoaded => self.finish_pending_load(None),
            BackendEvent::PositionChanged(position_ms) => {
                if self.snapshot.current_track_id.is_some() && self.pending_load.is_none() {
                    self.snapshot.position_ms =
                        clamp_position(position_ms, self.snapshot.duration_ms);
                    self.publish();
                }
            }
            BackendEvent::PauseChanged(paused) => {
                if self.pending_load.is_none()
                    && matches!(
                        self.snapshot.phase,
                        PlaybackPhase::Playing | PlaybackPhase::Paused | PlaybackPhase::Seeking
                    )
                {
                    self.snapshot.phase = if paused {
                        PlaybackPhase::Paused
                    } else {
                        PlaybackPhase::Playing
                    };
                    self.desired_paused = paused;
                    self.publish();
                }
            }
            BackendEvent::SeekingChanged(seeking) => {
                if self.pending_load.is_none() && self.snapshot.current_track_id.is_some() {
                    if seeking {
                        self.phase_before_seeking = Some(self.snapshot.phase);
                        self.snapshot.phase = PlaybackPhase::Seeking;
                    } else {
                        self.snapshot.phase = match self.phase_before_seeking.take() {
                            Some(PlaybackPhase::Paused) => PlaybackPhase::Paused,
                            _ => PlaybackPhase::Playing,
                        };
                    }
                    self.publish();
                }
            }
            BackendEvent::EndFile(reason) => {
                if reason == EndFileReason::Eof && self.pending_load.is_none() {
                    self.handle_eof();
                }
            }
            BackendEvent::AudioDeviceChanged(name) => {
                self.snapshot.selected_audio_device = name;
                self.publish();
            }
            BackendEvent::AudioDevices(devices) => {
                if let Some(reply) = self.pending_audio_devices.take() {
                    let _ = reply.send(Ok(devices));
                }
            }
            BackendEvent::DurationChanged(duration_ms) => {
                if let Some(pending) = self.pending_load.as_mut() {
                    pending.resolved.duration_ms = duration_ms.or(pending.resolved.duration_ms);
                    self.snapshot.duration_ms = pending.resolved.duration_ms;
                } else {
                    self.snapshot.duration_ms = duration_ms;
                    self.snapshot.position_ms =
                        clamp_position(self.snapshot.position_ms, duration_ms);
                }
                self.publish();
            }
            BackendEvent::VolumeChanged(volume_percent) => {
                self.snapshot.volume_percent = volume_percent.min(100);
                self.publish();
            }
            BackendEvent::MuteChanged(muted) => {
                self.snapshot.muted = muted;
                self.publish();
            }
            BackendEvent::Disconnected => self.handle_backend_failure_for_generation(
                self.backend_generation,
                PlaybackError::new(
                    PlaybackErrorCode::IpcDisconnected,
                    "the playback backend disconnected",
                    true,
                ),
                true,
            ),
            BackendEvent::ProcessExited { expected, code } if !expected => {
                self.handle_backend_failure_for_generation(
                    self.backend_generation,
                    PlaybackError::new(
                        PlaybackErrorCode::IpcDisconnected,
                        format!("the mpv process exited unexpectedly (code {code:?})"),
                        true,
                    ),
                    true,
                );
            }
            BackendEvent::ProcessExited { .. } => {}
            BackendEvent::ProtocolError(detail) => self.handle_backend_failure_for_generation(
                self.backend_generation,
                PlaybackError::new(PlaybackErrorCode::ProtocolError, detail, true),
                true,
            ),
            BackendEvent::Failure(error) => {
                let terminal = is_terminal_backend_failure(&error);
                self.handle_backend_failure_for_generation(
                    self.backend_generation,
                    error,
                    terminal,
                );
            }
            BackendEvent::Ready => {
                if self
                    .recovery
                    .as_ref()
                    .is_some_and(|plan| plan.waiting_for_backend && plan.restore.is_none())
                {
                    self.recovery = None;
                    self.snapshot.phase = PlaybackPhase::Idle;
                    self.snapshot.recovering = false;
                    self.snapshot.error = None;
                }
                self.publish();
            }
        }
    }

    fn handle_eof(&mut self) {
        let result = self.advance_queue(self.snapshot.repeat_mode, true);
        if let Err(error) = result {
            self.snapshot.phase = PlaybackPhase::Failed;
            self.set_error(error);
            self.publish();
        }
    }

    fn advance_queue(
        &mut self,
        repeat_mode: RepeatMode,
        from_eof: bool,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        // A queue advance already owns the current transition until its
        // FileLoaded event completes. This also makes a concurrently queued
        // Next command harmless when the backend reports EOF for the same
        // track.
        if self.pending_load.is_some() {
            return Ok(self.snapshot.clone());
        }
        let Some(index) = self.queue.next_index(repeat_mode) else {
            if self.queue.entries().is_empty() {
                return Err(queue_empty_error());
            }
            self.snapshot.phase = PlaybackPhase::Ended;
            self.desired_paused = true;
            if from_eof {
                if let Some(duration_ms) = self.snapshot.duration_ms {
                    self.snapshot.position_ms = duration_ms;
                }
            }
            self.snapshot.error = None;
            self.publish();
            return Ok(self.snapshot.clone());
        };
        let entry = self.queue.entries()[index].clone();
        let resolved = self.resolve_for_play(&TrackPlaybackRequest {
            track_id: entry.track_id,
            source_id: entry.requested_source_id,
        })?;
        self.start_load(resolved, 0, false, LoadPurpose::Normal)
    }

    fn start_load(
        &mut self,
        resolved: ResolvedPlayback,
        desired_position_ms: u64,
        desired_paused: bool,
        purpose: LoadPurpose,
    ) -> Result<PlaybackSnapshot, PlaybackError> {
        self.desired_paused = desired_paused;
        self.apply_resolved(&resolved);
        self.snapshot.phase = PlaybackPhase::Loading;
        self.snapshot.position_ms = clamp_position(desired_position_ms, resolved.duration_ms);
        self.snapshot.duration_ms = resolved.duration_ms;
        self.snapshot.recovering = matches!(purpose, LoadPurpose::Recovery { .. });
        self.snapshot.error = None;
        self.pending_load = Some(PendingLoad {
            resolved,
            desired_position_ms,
            desired_paused,
            purpose,
            deadline: Instant::now() + PENDING_LOAD_TIMEOUT,
        });
        self.publish();

        let load_path = self
            .pending_load
            .as_ref()
            .expect("pending load was just assigned")
            .resolved
            .path
            .clone();
        if let Err(error) = self.backend.send(BackendCommand::Load {
            path: load_path,
            start_paused: desired_paused,
        }) {
            let pending = self
                .pending_load
                .take()
                .expect("failed load retains its pending state");
            let returned_error = if matches!(&pending.purpose, LoadPurpose::SwitchTarget { .. }) {
                PlaybackError::new(
                    PlaybackErrorCode::LoadFailed,
                    format!("source switch failed: {}", error.detail),
                    true,
                )
            } else {
                error.clone()
            };
            self.handle_load_failure(pending, error.clone());
            return Err(returned_error);
        }
        Ok(self.snapshot.clone())
    }

    fn finish_pending_load(&mut self, observed_duration_ms: Option<u64>) {
        let Some(pending) = self.pending_load.take() else {
            return;
        };
        let duration_ms = observed_duration_ms.or(pending.resolved.duration_ms);
        let position_ms = clamp_position(pending.desired_position_ms, duration_ms);
        let mut restoration = vec![
            BackendCommand::SetVolume(self.snapshot.volume_percent),
            BackendCommand::SetMuted(self.snapshot.muted),
            BackendCommand::SelectAudioDevice(self.snapshot.selected_audio_device.clone()),
        ];
        if position_ms > 0 {
            restoration.push(BackendCommand::SeekAbsoluteMs(position_ms));
        }
        restoration.push(BackendCommand::SetPaused(pending.desired_paused));
        for command in restoration {
            if let Err(error) = self.backend.send(command) {
                self.handle_load_failure(pending, error);
                return;
            }
        }

        self.apply_resolved(&pending.resolved);
        self.snapshot.duration_ms = duration_ms;
        self.snapshot.position_ms = position_ms;
        self.snapshot.phase = if pending.desired_paused {
            PlaybackPhase::Paused
        } else {
            PlaybackPhase::Playing
        };
        self.desired_paused = pending.desired_paused;
        self.snapshot.recovering = false;
        match pending.purpose {
            LoadPurpose::SwitchTarget { .. } => {
                let _ = self
                    .queue
                    .set_current_requested_source_id(Some(pending.resolved.source_id));
                self.snapshot.error = None;
            }
            LoadPurpose::SwitchRollback {
                prior,
                original_error,
            } => {
                let _ = self
                    .queue
                    .set_current_requested_source_id(Some(prior.source_id));
                self.set_error(original_error);
            }
            LoadPurpose::Recovery { token } => {
                if self
                    .recovery
                    .as_ref()
                    .is_some_and(|plan| plan.token == token)
                {
                    self.recovery = None;
                }
                let _ = self
                    .queue
                    .set_current_requested_source_id(Some(pending.resolved.source_id));
                self.snapshot.error = None;
            }
            LoadPurpose::Normal => {
                let _ = self
                    .queue
                    .set_current_requested_source_id(Some(pending.resolved.source_id));
                self.snapshot.error = None;
            }
        }
        self.publish();
    }

    fn handle_load_failure(&mut self, pending: PendingLoad, error: PlaybackError) {
        match pending.purpose {
            LoadPurpose::SwitchTarget { prior } => {
                self.begin_switch_rollback(
                    prior,
                    PlaybackError::new(
                        PlaybackErrorCode::LoadFailed,
                        format!("source switch failed: {}", error.detail),
                        true,
                    ),
                );
            }
            LoadPurpose::SwitchRollback {
                prior,
                original_error,
            } => {
                self.restore_snapshot_identity(&prior);
                let rollback_error = PlaybackError::new(
                    error.code,
                    format!(
                        "{}; prior source restoration also failed: {}",
                        original_error.detail, error.detail
                    ),
                    error.retryable,
                );
                // The controller identity now points at the restored source,
                // so normal recovery reloads that source rather than the failed
                // replacement and never advances the queue. A failed rollback
                // is a backend failure even when its public operation code is
                // LoadFailed or SeekFailed.
                self.begin_recovery(rollback_error);
            }
            LoadPurpose::Recovery { token } => {
                self.schedule_recovery_failure(token, error);
            }
            LoadPurpose::Normal => {
                if is_recoverable_backend_failure(&error) {
                    self.begin_recovery(error);
                } else {
                    self.snapshot.phase = PlaybackPhase::Failed;
                    self.snapshot.recovering = false;
                    self.set_error(error);
                    self.publish();
                }
            }
        }
    }

    fn begin_switch_rollback(&mut self, prior: PlaybackRestore, original_error: PlaybackError) {
        match self.resolve_exact_source(prior.track_id, prior.source_id) {
            Ok(resolved) => {
                let result = self.start_load(
                    resolved,
                    prior.position_ms,
                    prior.paused,
                    LoadPurpose::SwitchRollback {
                        prior: prior.clone(),
                        original_error: original_error.clone(),
                    },
                );
                if result.is_err()
                    && self.snapshot.phase != PlaybackPhase::Failed
                    && self.recovery.is_none()
                {
                    self.restore_snapshot_identity(&prior);
                    self.snapshot.phase = PlaybackPhase::Failed;
                    self.set_error(original_error);
                    self.publish();
                }
            }
            Err(rollback_error) => {
                self.restore_snapshot_identity(&prior);
                self.snapshot.phase = PlaybackPhase::Failed;
                self.snapshot.error = Some(
                    PlaybackError::new(
                        PlaybackErrorCode::LoadFailed,
                        format!(
                            "{}; prior source could not be resolved: {}",
                            original_error.detail, rollback_error.detail
                        ),
                        true,
                    )
                    .dto(),
                );
                self.publish();
            }
        }
    }

    fn expire_pending_load(&mut self) {
        if self
            .pending_load
            .as_ref()
            .is_some_and(|pending| Instant::now() >= pending.deadline)
        {
            let pending = self
                .pending_load
                .take()
                .expect("expired pending load exists");
            self.handle_load_failure(
                pending,
                PlaybackError::new(
                    PlaybackErrorCode::LoadFailed,
                    "timed out waiting for file-loaded confirmation",
                    true,
                ),
            );
        }
    }

    fn handle_backend_failure_for_generation(
        &mut self,
        generation: u64,
        error: PlaybackError,
        terminal: bool,
    ) {
        if generation != self.backend_generation
            || (terminal && self.terminal_failure_generation == Some(generation))
        {
            return;
        }
        if terminal {
            self.terminal_failure_generation = Some(generation);
        }
        if let Some(reply) = self.pending_audio_devices.take() {
            let _ = reply.send(Err(error.clone()));
        }
        if let Some(pending) = self.pending_load.take() {
            self.handle_load_failure(pending, error);
        } else if let Some(token) = self
            .recovery
            .as_ref()
            .filter(|plan| plan.waiting_for_backend)
            .map(|plan| plan.token)
        {
            self.schedule_recovery_failure(token, error);
        } else {
            self.begin_recovery(error);
        }
    }

    fn begin_recovery(&mut self, error: PlaybackError) {
        if self.shutting_down || self.recovery.is_some() {
            return;
        }
        self.next_recovery_token = self.next_recovery_token.wrapping_add(1).max(1);
        let token = self.next_recovery_token;
        let restore = self.current_restore().ok();
        self.recovery = Some(RecoveryPlan {
            token,
            attempts: 0,
            due_at: Instant::now() + RECOVERY_BACKOFF[0],
            restore,
            waiting_for_backend: false,
        });
        self.snapshot.phase = PlaybackPhase::Recovering;
        self.snapshot.recovering = true;
        self.snapshot.error = Some(
            PlaybackError::new(
                PlaybackErrorCode::RecoveryRetrying,
                format!("{}; recovery attempt pending", error.detail),
                true,
            )
            .dto(),
        );
        self.publish();
    }

    fn run_recovery_attempt(&mut self) {
        if self.pending_load.is_some() {
            return;
        }
        let Some(mut plan) = self.recovery.take() else {
            return;
        };
        if plan.waiting_for_backend {
            self.recovery = Some(plan);
            return;
        }
        if Instant::now() < plan.due_at {
            self.recovery = Some(plan);
            return;
        }
        plan.attempts += 1;
        plan.waiting_for_backend = true;
        let token = plan.token;
        let restore = plan.restore.clone();
        self.recovery = Some(plan);

        let _ = self.backend.shutdown();
        self.replace_backend();
        if let Some(restore) = restore {
            match self.resolve_for_play(&TrackPlaybackRequest {
                track_id: restore.track_id,
                source_id: Some(restore.source_id),
            }) {
                Ok(resolved) => {
                    let _ = self.start_load(
                        resolved,
                        restore.position_ms,
                        restore.paused,
                        LoadPurpose::Recovery { token },
                    );
                }
                Err(error) => self.schedule_recovery_failure(token, error),
            }
        } else {
            // Wait for the new worker's Ready or Failure event. Clearing the
            // recovery plan here would reset its bounded attempt counter if
            // startup fails before a track is loaded.
            self.publish();
        }
    }

    fn schedule_recovery_failure(&mut self, token: u64, error: PlaybackError) {
        let Some(plan) = self.recovery.as_mut() else {
            return;
        };
        if plan.token != token {
            return;
        }
        plan.waiting_for_backend = false;
        if plan.attempts >= MAX_RECOVERY_ATTEMPTS {
            self.recovery = None;
            self.snapshot.phase = PlaybackPhase::Failed;
            self.snapshot.recovering = false;
            self.snapshot.error = Some(PlaybackError::new(
                PlaybackErrorCode::RecoveryExhausted,
                format!(
                    "playback backend recovery exhausted after {MAX_RECOVERY_ATTEMPTS} attempts: {}",
                    error.detail
                ),
                true,
            )
            .dto());
            self.publish();
            return;
        }
        plan.due_at = Instant::now() + RECOVERY_BACKOFF[plan.attempts];
        self.snapshot.phase = PlaybackPhase::Recovering;
        self.snapshot.recovering = true;
        self.snapshot.error = Some(
            PlaybackError::new(
                PlaybackErrorCode::RecoveryRetrying,
                format!(
                    "playback backend recovery attempt {} failed: {}",
                    plan.attempts, error.detail
                ),
                true,
            )
            .dto(),
        );
        self.publish();
    }

    fn replace_backend(&mut self) {
        self.next_backend_generation = self.next_backend_generation.wrapping_add(1).max(1);
        let generation = self.next_backend_generation;
        let PlaybackBackendSession { backend, events } = (self.backend_factory)(generation);
        self.backend = backend;
        self.backend_events = events;
        self.backend_generation = generation;
        self.terminal_failure_generation = None;
    }

    fn resolve_for_play(
        &self,
        request: &TrackPlaybackRequest,
    ) -> Result<ResolvedPlayback, PlaybackError> {
        let track = self.track(request.track_id)?;
        if let Some(source_id) = request.source_id {
            let source = track
                .sources
                .iter()
                .find(|source| source.id == source_id)
                .ok_or_else(|| {
                    PlaybackError::new(
                        PlaybackErrorCode::SourceNotFound,
                        format!("playback source {source_id} was not found"),
                        false,
                    )
                })?;
            let path = self
                .library
                .resolve_playback_path(track.id, source_id)
                .map_err(playback_error_from_library)?;
            return Ok(self.resolved_playback(&track, source, path));
        }

        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        if let Some(source_id) = track.preferred_source_id {
            if seen.insert(source_id) {
                candidates.push(source_id);
            }
        }
        for source in &track.sources {
            if seen.insert(source.id) {
                candidates.push(source.id);
            }
        }

        let mut last_error = None;
        for source_id in candidates {
            let Some(source) = track.sources.iter().find(|source| source.id == source_id) else {
                last_error = Some(playback_error_from_library(
                    self.library
                        .resolve_playback_path(track.id, source_id)
                        .expect_err("a source absent from the track cannot resolve"),
                ));
                continue;
            };
            match self.library.resolve_playback_path(track.id, source_id) {
                Ok(path) => return Ok(self.resolved_playback(&track, source, path)),
                Err(error) => last_error = Some(playback_error_from_library(error)),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            PlaybackError::new(
                PlaybackErrorCode::SourceNotFound,
                format!("track {} has no playback source", track.id),
                false,
            )
        }))
    }

    fn resolve_exact_source(
        &self,
        track_id: TrackId,
        source_id: SourceId,
    ) -> Result<ResolvedPlayback, PlaybackError> {
        let track = self.track(track_id)?;
        let source = track
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .ok_or_else(|| {
                playback_error_from_library(
                    self.library
                        .resolve_playback_path(track_id, source_id)
                        .expect_err("a source absent from the track cannot resolve"),
                )
            })?;
        let path = self
            .library
            .resolve_playback_path(track_id, source_id)
            .map_err(playback_error_from_library)?;
        Ok(self.resolved_playback(&track, source, path))
    }

    fn track(&self, track_id: TrackId) -> Result<UnifiedTrack, PlaybackError> {
        TrackRepository::new(self.library.database())
            .get(track_id)
            .map_err(|error| {
                PlaybackError::new(
                    PlaybackErrorCode::SourceUnavailable,
                    format!("could not read track: {error}"),
                    true,
                )
            })?
            .ok_or_else(|| {
                PlaybackError::new(
                    PlaybackErrorCode::TrackNotFound,
                    format!("track {track_id} was not found"),
                    false,
                )
            })
    }

    fn resolved_playback(
        &self,
        track: &UnifiedTrack,
        source: &TrackSource,
        path: PathBuf,
    ) -> ResolvedPlayback {
        let sources = track
            .sources
            .iter()
            .map(|candidate| PlaybackSourceOption {
                source_id: candidate.id,
                provider: candidate.provider_kind,
                label: source_label(candidate.provider_kind),
                available: self
                    .library
                    .resolve_playback_path(track.id, candidate.id)
                    .is_ok(),
            })
            .collect();
        let artwork_path = source
            .local_file
            .as_ref()
            .and_then(|local| local.artwork_cache_key.as_deref())
            .and_then(|key| self.library.artwork_path(key));
        ResolvedPlayback {
            track_id: track.id,
            source_id: source.id,
            path,
            title: track.title.clone(),
            artists: track
                .artists
                .iter()
                .map(|artist| artist.name.clone())
                .collect(),
            album: track.album.as_ref().map(|album| album.title.clone()),
            artwork_path,
            sources,
            duration_ms: source.duration_ms.or(track.duration_ms),
        }
    }

    fn apply_resolved(&mut self, resolved: &ResolvedPlayback) {
        self.snapshot.current_track_id = Some(resolved.track_id);
        self.snapshot.current_source_id = Some(resolved.source_id);
        self.snapshot.title = Some(resolved.title.clone());
        self.snapshot.artists = resolved.artists.clone();
        self.snapshot.album = resolved.album.clone();
        self.snapshot.artwork_path = resolved.artwork_path.clone();
        self.snapshot.sources = resolved.sources.clone();
        self.snapshot.duration_ms = resolved.duration_ms;
    }

    fn current_restore(&self) -> Result<PlaybackRestore, PlaybackError> {
        Ok(PlaybackRestore {
            track_id: self
                .snapshot
                .current_track_id
                .ok_or_else(queue_empty_error)?,
            source_id: self
                .snapshot
                .current_source_id
                .ok_or_else(queue_empty_error)?,
            position_ms: self.snapshot.position_ms,
            paused: self.desired_paused,
        })
    }

    fn restore_snapshot_identity(&mut self, restore: &PlaybackRestore) {
        if let Ok(resolved) = self.resolve_exact_source(restore.track_id, restore.source_id) {
            self.apply_resolved(&resolved);
            self.snapshot.position_ms = clamp_position(restore.position_ms, resolved.duration_ms);
        } else {
            self.snapshot.current_track_id = Some(restore.track_id);
            self.snapshot.current_source_id = Some(restore.source_id);
            self.snapshot.position_ms = restore.position_ms;
        }
    }

    fn reject_during_recovery(&self) -> Result<(), PlaybackError> {
        if self.shutting_down {
            return Err(shutting_down_error());
        }
        if self.recovery.is_some() {
            return Err(invalid_state_error(
                "playback transport is unavailable during backend recovery",
            ));
        }
        Ok(())
    }

    fn queue_entry(&mut self, track_id: TrackId, source_id: Option<SourceId>) -> QueueEntry {
        QueueEntry::new(track_id, source_id)
    }

    fn clear_current_metadata(&mut self) {
        self.snapshot.current_track_id = None;
        self.snapshot.current_source_id = None;
        self.snapshot.title = None;
        self.snapshot.artists.clear();
        self.snapshot.album = None;
        self.snapshot.artwork_path = None;
        self.snapshot.sources.clear();
        self.snapshot.position_ms = 0;
        self.snapshot.duration_ms = None;
    }

    fn set_error(&mut self, error: PlaybackError) {
        self.snapshot.error = Some(error.dto());
    }

    fn publish(&mut self) {
        self.snapshot.queue_length = self.queue.entries().len();
        self.snapshot.queue_index = self.queue.current_index();
        self.snapshot.current_queue_entry_id = self.queue.current_entry().map(|entry| entry.id);
        self.snapshot.shuffle_enabled = self.queue.is_shuffle_enabled();
        self.snapshot.backend_health = self.backend.health();
        self.snapshot.revision = self
            .snapshot
            .revision
            .checked_add(1)
            .expect("playback snapshot revision exhausted");
        let _ = self.snapshot_tx.send(self.snapshot.clone());
        let snapshot = self.snapshot.clone();
        let sink = self.sink.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sink(snapshot)));
    }
}

fn playback_error_from_library(error: LibraryError) -> PlaybackError {
    match error {
        LibraryError::SourceNotFound { source_id } => PlaybackError::new(
            PlaybackErrorCode::SourceNotFound,
            format!("playback source {source_id} was not found"),
            false,
        ),
        LibraryError::SourceMismatch {
            source_id,
            expected_track_id,
            actual_track_id,
        } => PlaybackError::new(
            PlaybackErrorCode::SourceMismatch,
            format!(
                "source {source_id} belongs to track {actual_track_id}, not {expected_track_id}"
            ),
            false,
        ),
        LibraryError::SourceUnavailable { source_id, detail } => {
            let code = if detail.contains("not local") || detail.contains("playback capability") {
                PlaybackErrorCode::SourceNotPlayable
            } else if detail.contains("local file is missing")
                || detail.contains("local file is unavailable")
            {
                PlaybackErrorCode::LocalFileMissing
            } else {
                PlaybackErrorCode::SourceUnavailable
            };
            PlaybackError::new(
                code,
                format!("playback source {source_id} is unavailable: {detail}"),
                true,
            )
        }
        other => PlaybackError::new(
            PlaybackErrorCode::SourceUnavailable,
            format!("library playback resolution failed: {other}"),
            true,
        ),
    }
}

fn playback_error_from_backend(error: BackendError) -> PlaybackError {
    match error {
        BackendError::Unavailable { detail } => {
            let code = if detail.contains("not found") || detail.contains("not available") {
                PlaybackErrorCode::ToolMissing
            } else if detail.contains("mpv") {
                PlaybackErrorCode::ToolBroken
            } else {
                PlaybackErrorCode::SpawnFailed
            };
            PlaybackError::new(code, detail, true)
        }
        BackendError::NotStarted => PlaybackError::new(
            PlaybackErrorCode::SpawnFailed,
            "the playback backend is not started",
            true,
        ),
        BackendError::Disconnected => PlaybackError::new(
            PlaybackErrorCode::IpcDisconnected,
            "the playback backend disconnected",
            true,
        ),
        BackendError::Timeout { operation } => {
            let code = match operation.as_str() {
                "connect" => PlaybackErrorCode::IpcConnectTimeout,
                "load" => PlaybackErrorCode::LoadFailed,
                operation if operation.contains("seek") => PlaybackErrorCode::SeekFailed,
                _ => PlaybackErrorCode::RequestTimeout,
            };
            PlaybackError::new(
                code,
                format!("playback backend operation timed out: {operation}"),
                true,
            )
        }
        BackendError::Protocol { detail } => {
            PlaybackError::new(PlaybackErrorCode::ProtocolError, detail, true)
        }
        BackendError::Operation { detail } => {
            let code = if detail.contains("seek") {
                PlaybackErrorCode::SeekFailed
            } else if detail.contains("load") || detail.contains("file") {
                PlaybackErrorCode::LoadFailed
            } else {
                PlaybackErrorCode::SpawnFailed
            };
            PlaybackError::new(code, detail, true)
        }
    }
}

fn is_recoverable_backend_failure(error: &PlaybackError) -> bool {
    matches!(
        error.code,
        PlaybackErrorCode::ToolMissing
            | PlaybackErrorCode::ToolBroken
            | PlaybackErrorCode::SpawnFailed
            | PlaybackErrorCode::IpcConnectTimeout
            | PlaybackErrorCode::IpcDisconnected
            | PlaybackErrorCode::ProtocolError
            | PlaybackErrorCode::RequestTimeout
    )
}

fn is_terminal_backend_failure(error: &PlaybackError) -> bool {
    matches!(
        error.code,
        PlaybackErrorCode::ToolMissing
            | PlaybackErrorCode::ToolBroken
            | PlaybackErrorCode::SpawnFailed
            | PlaybackErrorCode::IpcConnectTimeout
            | PlaybackErrorCode::IpcDisconnected
            | PlaybackErrorCode::ProtocolError
    )
}

fn source_label(provider: ProviderKind) -> String {
    provider.as_str().to_ascii_uppercase()
}

fn clamp_position(position_ms: u64, duration_ms: Option<u64>) -> u64 {
    duration_ms.map_or(position_ms, |duration_ms| position_ms.min(duration_ms))
}

fn queue_empty_error() -> PlaybackError {
    PlaybackError::new(
        PlaybackErrorCode::QueueEmpty,
        "the playback queue is empty",
        false,
    )
}

fn invalid_state_error(detail: impl Into<String>) -> PlaybackError {
    PlaybackError::new(PlaybackErrorCode::RequestTimeout, detail, true)
}

fn shutting_down_error() -> PlaybackError {
    PlaybackError::new(
        PlaybackErrorCode::ShuttingDown,
        "the playback service is shutting down",
        false,
    )
}

fn controller_unavailable_error() -> PlaybackError {
    PlaybackError::new(
        PlaybackErrorCode::IpcDisconnected,
        "the playback controller is unavailable",
        true,
    )
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use rusqlite::params;

    use super::*;
    use crate::db::{Database, TempDatabasePath};
    use crate::domain::{LibraryFolderId, LibraryPageRequest, SourceId, TrackId};
    use crate::library::folders::normalize_file_path;
    use crate::library::LibraryService;
    use crate::playback::backend::{
        AudioDevice, BackendCommand, BackendEvent, BackendHealth, EndFileReason,
        GenerationStampedBackendEvent, PlaybackBackend, PlaybackBackendSession,
    };
    use crate::playback::{PlaybackError, RepeatMode};

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum FakeOperation {
        Start,
        Load(PathBuf),
        Pause,
        Resume,
        Seek(u64),
        Volume(u8),
        Muted(bool),
        ListDevices,
        Device(String),
        Stop,
        Shutdown,
    }

    struct FakeState {
        operations: Vec<FakeOperation>,
        health: BackendHealth,
        start_failures: usize,
        load_failures: usize,
        load_disconnect_failures: usize,
        auto_file_loaded: bool,
        duration_ms: Option<u64>,
        devices: Vec<AudioDevice>,
    }

    impl Default for FakeState {
        fn default() -> Self {
            Self {
                operations: Vec::new(),
                health: BackendHealth::default(),
                start_failures: 0,
                load_failures: 0,
                load_disconnect_failures: 0,
                auto_file_loaded: false,
                duration_ms: Some(10_000),
                devices: vec![AudioDevice {
                    name: "auto".to_owned(),
                    description: "Default output".to_owned(),
                    selected: true,
                }],
            }
        }
    }

    #[derive(Default)]
    struct FakeControl {
        state: Mutex<FakeState>,
        event_tx: Mutex<Option<(u64, tokio_mpsc::Sender<GenerationStampedBackendEvent>)>>,
    }

    impl FakeControl {
        fn session(self: &Arc<Self>, generation: u64) -> PlaybackBackendSession {
            let (event_tx, event_rx) = tokio_mpsc::channel(COMMAND_CAPACITY);
            let startup_event = {
                let mut state = self.state.lock().unwrap();
                state.operations.push(FakeOperation::Start);
                if state.start_failures > 0 {
                    state.start_failures -= 1;
                    state.health = BackendHealth {
                        ready: false,
                        connected: false,
                        detail: Some("injected start failure".to_owned()),
                        recovery_action: Some("Retry".to_owned()),
                    };
                    BackendEvent::Failure(PlaybackError::new(
                        PlaybackErrorCode::SpawnFailed,
                        "injected start failure",
                        true,
                    ))
                } else {
                    state.health = BackendHealth {
                        ready: true,
                        connected: true,
                        detail: None,
                        recovery_action: None,
                    };
                    BackendEvent::Ready
                }
            };
            *self.event_tx.lock().unwrap() = Some((generation, event_tx.clone()));
            let _ = event_tx.try_send(GenerationStampedBackendEvent::new(
                generation,
                startup_event,
            ));
            PlaybackBackendSession {
                backend: Arc::new(FakeBackend::new(self.clone())),
                events: event_rx,
            }
        }

        fn push_event(&self, event: BackendEvent) {
            if matches!(
                &event,
                BackendEvent::Disconnected
                    | BackendEvent::ProtocolError(_)
                    | BackendEvent::Failure(_)
            ) {
                let mut state = self.state.lock().unwrap();
                state.health.ready = false;
                state.health.connected = false;
            }
            let generation = self
                .event_tx
                .lock()
                .unwrap()
                .as_ref()
                .map(|(generation, _)| *generation);
            if let Some(generation) = generation {
                self.push_event_for_generation(generation, event);
            }
        }

        fn push_event_for_generation(&self, generation: u64, event: BackendEvent) {
            if let Some((_, event_tx)) = self.event_tx.lock().unwrap().clone() {
                let _ = event_tx.try_send(GenerationStampedBackendEvent::new(generation, event));
            }
        }

        fn auto_file_loaded(&self, enabled: bool, duration_ms: Option<u64>) {
            let mut state = self.state.lock().unwrap();
            state.auto_file_loaded = enabled;
            state.duration_ms = duration_ms;
        }

        fn fail_next_starts(&self, count: usize) {
            self.state.lock().unwrap().start_failures = count;
        }

        fn fail_next_loads(&self, count: usize) {
            self.state.lock().unwrap().load_failures = count;
        }

        fn disconnect_next_loads(&self, count: usize) {
            self.state.lock().unwrap().load_disconnect_failures = count;
        }

        fn operations(&self) -> Vec<FakeOperation> {
            self.state.lock().unwrap().operations.clone()
        }
    }

    struct FakeBackend {
        control: Arc<FakeControl>,
    }

    impl FakeBackend {
        fn new(control: Arc<FakeControl>) -> Self {
            Self { control }
        }
    }

    impl PlaybackBackend for FakeBackend {
        fn send(&self, command: BackendCommand) -> Result<(), PlaybackError> {
            match command {
                BackendCommand::Load { path, start_paused } => {
                    let (failure, auto_file_loaded, duration_ms) = {
                        let mut state = self.control.state.lock().unwrap();
                        state.operations.push(FakeOperation::Load(path));
                        if state.load_disconnect_failures > 0 {
                            state.load_disconnect_failures -= 1;
                            state.health.ready = false;
                            state.health.connected = false;
                            (
                                Some(PlaybackError::new(
                                    PlaybackErrorCode::IpcDisconnected,
                                    "injected load disconnect",
                                    true,
                                )),
                                false,
                                None,
                            )
                        } else if state.load_failures > 0 {
                            state.load_failures -= 1;
                            (
                                Some(PlaybackError::new(
                                    PlaybackErrorCode::LoadFailed,
                                    "injected load failure",
                                    true,
                                )),
                                false,
                                None,
                            )
                        } else {
                            (None, state.auto_file_loaded, state.duration_ms)
                        }
                    };
                    if let Some(error) = failure {
                        return Err(error);
                    }
                    if auto_file_loaded {
                        self.control
                            .push_event(BackendEvent::DurationChanged(duration_ms));
                        self.control.push_event(BackendEvent::FileLoaded);
                    }
                    if start_paused {
                        self.send(BackendCommand::SetPaused(true))?;
                    }
                    Ok(())
                }
                BackendCommand::SetPaused(paused) => {
                    self.control
                        .state
                        .lock()
                        .unwrap()
                        .operations
                        .push(if paused {
                            FakeOperation::Pause
                        } else {
                            FakeOperation::Resume
                        });
                    Ok(())
                }
                BackendCommand::SeekAbsoluteMs(position_ms) => {
                    self.control
                        .state
                        .lock()
                        .unwrap()
                        .operations
                        .push(FakeOperation::Seek(position_ms));
                    Ok(())
                }
                BackendCommand::SetVolume(volume_percent) => {
                    self.control
                        .state
                        .lock()
                        .unwrap()
                        .operations
                        .push(FakeOperation::Volume(volume_percent));
                    Ok(())
                }
                BackendCommand::SetMuted(muted) => {
                    self.control
                        .state
                        .lock()
                        .unwrap()
                        .operations
                        .push(FakeOperation::Muted(muted));
                    Ok(())
                }
                BackendCommand::QueryAudioDevices => {
                    let devices = {
                        let mut state = self.control.state.lock().unwrap();
                        state.operations.push(FakeOperation::ListDevices);
                        state.devices.clone()
                    };
                    self.control.push_event(BackendEvent::AudioDevices(devices));
                    Ok(())
                }
                BackendCommand::SelectAudioDevice(name) => {
                    self.control
                        .state
                        .lock()
                        .unwrap()
                        .operations
                        .push(FakeOperation::Device(name));
                    Ok(())
                }
                BackendCommand::Stop => {
                    self.control
                        .state
                        .lock()
                        .unwrap()
                        .operations
                        .push(FakeOperation::Stop);
                    Ok(())
                }
                BackendCommand::Shutdown => self.shutdown(),
            }
        }

        fn shutdown(&self) -> Result<(), PlaybackError> {
            let mut state = self.control.state.lock().unwrap();
            state.operations.push(FakeOperation::Shutdown);
            state.health.ready = false;
            state.health.connected = false;
            Ok(())
        }

        fn health(&self) -> BackendHealth {
            self.control.state.lock().unwrap().health.clone()
        }
    }

    struct TestLibrary {
        service: LibraryService,
        database: Database,
        tracks: Vec<TrackPlaybackRequest>,
        folder_id: LibraryFolderId,
        root: tempfile::TempDir,
        _artwork: tempfile::TempDir,
        _database_path: TempDatabasePath,
    }

    fn test_library(track_count: usize) -> TestLibrary {
        let root = tempfile::tempdir().unwrap();
        for index in 0..track_count {
            std::fs::write(
                root.path().join(format!("track-{index}.wav")),
                minimal_wav_with_sample(index as i16),
            )
            .unwrap();
        }
        let database_path = TempDatabasePath::new("playback-service");
        let database = Database::open(database_path.path()).unwrap();
        let artwork = tempfile::tempdir().unwrap();
        let service = LibraryService::new(database.clone(), artwork.path()).unwrap();
        let folder = service
            .add_folders(vec![root.path().to_path_buf()])
            .unwrap()
            .remove(0);
        service.scan_folder_now(folder.id, false, None).unwrap();
        let page = service
            .page(LibraryPageRequest {
                page_size: 100,
                ..LibraryPageRequest::default()
            })
            .unwrap();
        let tracks = page
            .items
            .iter()
            .map(|track| TrackPlaybackRequest {
                track_id: track.track_id,
                source_id: Some(track.source_id),
            })
            .collect();
        TestLibrary {
            service,
            database,
            tracks,
            folder_id: folder.id,
            root,
            _artwork: artwork,
            _database_path: database_path,
        }
    }

    fn add_local_source(
        library: &TestLibrary,
        track_id: TrackId,
        file_name: &str,
        duration_ms: u64,
    ) -> SourceId {
        let path = library.root.path().join(file_name);
        std::fs::write(&path, minimal_wav_with_sample(99)).unwrap();
        let (display_path, normalized_path_key) = normalize_file_path(&path).unwrap();
        let source_id = SourceId::new();
        library
            .database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO track_sources (
                        id, track_id, provider_kind, provider_item_id, duration_ms,
                        available, can_playback, can_metadata, created_at, updated_at
                     ) VALUES (?1, ?2, 'local', ?3, ?4, 1, 1, 1, ?5, ?5)",
                    params![
                        source_id.to_string(),
                        track_id.to_string(),
                        format!("alternate-{source_id}"),
                        i64::try_from(duration_ms).unwrap(),
                        "2026-01-01T00:00:00Z"
                    ],
                )?;
                connection.execute(
                    "INSERT INTO local_files (
                        source_id, path, created_at, updated_at, library_folder_id,
                        normalized_path_key, index_status
                     ) VALUES (?1, ?2, ?3, ?3, ?4, ?5, 'indexed')",
                    params![
                        source_id.to_string(),
                        display_path.to_string_lossy().into_owned(),
                        "2026-01-01T00:00:00Z",
                        library.folder_id.to_string(),
                        normalized_path_key
                    ],
                )?;
                Ok(())
            })
            .unwrap();
        source_id
    }

    fn service_with(
        library: &TestLibrary,
    ) -> (
        PlaybackService,
        Arc<FakeControl>,
        Arc<Mutex<Vec<PlaybackSnapshot>>>,
    ) {
        let control = Arc::new(FakeControl::default());
        let factory_control = control.clone();
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let sink_snapshots = snapshots.clone();
        let sink: SnapshotSink = Arc::new(move |snapshot| {
            sink_snapshots.lock().unwrap().push(snapshot);
        });
        let service = PlaybackService::new_with_backend_factory(
            library.service.clone(),
            move |generation| factory_control.session(generation),
            sink,
        );
        (service, control, snapshots)
    }

    fn wait_for_snapshot(
        service: &PlaybackService,
        predicate: impl Fn(&PlaybackSnapshot) -> bool,
    ) -> PlaybackSnapshot {
        wait_for_snapshot_with_timeout(service, Duration::from_secs(2), predicate)
    }

    fn wait_for_snapshot_with_timeout(
        service: &PlaybackService,
        timeout: Duration,
        predicate: impl Fn(&PlaybackSnapshot) -> bool,
    ) -> PlaybackSnapshot {
        let deadline = Instant::now() + timeout;
        loop {
            let snapshot = service.snapshot();
            if predicate(&snapshot) {
                return snapshot;
            }
            assert!(Instant::now() < deadline, "last snapshot: {snapshot:?}");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn wait_until_playing(service: &PlaybackService) -> PlaybackSnapshot {
        wait_for_snapshot(service, |snapshot| {
            matches!(
                snapshot.phase,
                PlaybackPhase::Playing | PlaybackPhase::Paused
            )
        })
    }

    #[test]
    fn idle_startup_and_wire_contracts_are_stable() {
        let library = test_library(1);
        let (service, control, snapshots) = service_with(&library);
        let snapshot = service.snapshot();

        assert_eq!(snapshot.phase, PlaybackPhase::Idle);
        assert!(snapshot.backend_health.ready);
        assert!(snapshot.backend_health.connected);
        assert_eq!(snapshot.queue_length, 0);
        assert!(snapshot.revision > 0);
        assert_eq!(control.operations(), vec![FakeOperation::Start]);
        assert_eq!(
            serde_json::to_string(&PlaybackPhase::ShuttingDown).unwrap(),
            "\"shuttingDown\""
        );
        let request_json = serde_json::to_value(library.tracks[0].clone()).unwrap();
        assert!(request_json.get("trackId").is_some());
        assert!(request_json.get("sourceId").is_some());
        assert!(request_json.get("path").is_none());
        assert_eq!(snapshots.lock().unwrap()[0].phase, PlaybackPhase::Idle);
        service.shutdown().unwrap();
    }

    #[test]
    fn local_play_waits_for_file_loaded_and_restores_a_queued_clamped_seek() {
        let library = test_library(1);
        let (service, control, _) = service_with(&library);

        let loading = service.play_track(library.tracks[0].clone()).unwrap();
        assert_eq!(loading.phase, PlaybackPhase::Loading);
        let still_loading = service.seek_playback(9_000).unwrap();
        assert_eq!(still_loading.phase, PlaybackPhase::Loading);

        control.push_event(BackendEvent::DurationChanged(Some(5_000)));
        control.push_event(BackendEvent::FileLoaded);
        let playing = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing && snapshot.position_ms == 5_000
        });
        assert_eq!(playing.duration_ms, Some(5_000));
        assert!(control.operations().contains(&FakeOperation::Seek(5_000)));

        assert_eq!(
            service.toggle_play_pause().unwrap().phase,
            PlaybackPhase::Paused
        );
        let paused_seek = service.seek_playback(1_250).unwrap();
        assert_eq!(paused_seek.phase, PlaybackPhase::Paused);
        assert_eq!(paused_seek.position_ms, 1_250);
        assert_eq!(
            service.toggle_play_pause().unwrap().phase,
            PlaybackPhase::Playing
        );
        service.shutdown().unwrap();
    }

    #[test]
    fn enqueue_on_an_empty_queue_does_not_autoplay_but_transport_can_start_it() {
        let library = test_library(1);
        let (service, control, _) = service_with(&library);

        let queued = service.enqueue_track(library.tracks[0].clone()).unwrap();
        assert_eq!(queued.phase, PlaybackPhase::Idle);
        assert_eq!(queued.queue_length, 1);
        assert!(queued.current_track_id.is_none());
        assert!(!control
            .operations()
            .iter()
            .any(|operation| matches!(operation, FakeOperation::Load(_))));

        let loading = service.toggle_play_pause().unwrap();
        assert_eq!(loading.phase, PlaybackPhase::Loading);
        control.push_event(BackendEvent::FileLoaded);
        let playing = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && snapshot.current_track_id == Some(library.tracks[0].track_id)
        });
        assert_eq!(playing.queue_length, 1);
        service.shutdown().unwrap();
    }

    #[test]
    fn volume_mute_and_audio_device_commands_update_authoritative_state() {
        let library = test_library(1);
        let (service, control, _) = service_with(&library);

        assert_eq!(
            service.set_playback_volume(250).unwrap().volume_percent,
            100
        );
        assert!(service.set_playback_muted(true).unwrap().muted);
        let devices = service.get_audio_devices().unwrap();
        assert_eq!(devices[0].name, "auto");
        let selected = service.set_audio_device("auto").unwrap();
        assert_eq!(selected.selected_audio_device, "auto");
        assert!(control.operations().contains(&FakeOperation::Volume(100)));
        assert!(control.operations().contains(&FakeOperation::Muted(true)));
        service.shutdown().unwrap();
    }

    #[test]
    fn next_play_next_and_the_exact_previous_threshold_follow_queue_policy() {
        let library = test_library(3);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(true, Some(10_000));
        service.play_track(library.tracks[0].clone()).unwrap();
        wait_until_playing(&service);
        service.enqueue_track(library.tracks[1].clone()).unwrap();
        service.play_track_next(library.tracks[2].clone()).unwrap();

        service.next_track().unwrap();
        let next = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && snapshot.current_track_id == Some(library.tracks[2].track_id)
        });
        assert_eq!(next.queue_length, 3);

        control.push_event(BackendEvent::PositionChanged(3_001));
        wait_for_snapshot(&service, |snapshot| snapshot.position_ms == 3_001);
        let restarted = service.previous_track().unwrap();
        assert_eq!(restarted.current_track_id, Some(library.tracks[2].track_id));
        assert_eq!(restarted.position_ms, 0);

        control.push_event(BackendEvent::PositionChanged(3_000));
        wait_for_snapshot(&service, |snapshot| snapshot.position_ms == 3_000);
        service.previous_track().unwrap();
        let previous = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && snapshot.current_track_id == Some(library.tracks[0].track_id)
        });
        assert_eq!(previous.queue_index, Some(0));
        service.shutdown().unwrap();
    }

    #[test]
    fn only_eof_advances_and_repeat_off_one_all_have_distinct_boundaries() {
        let library = test_library(2);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(true, Some(1_000));
        service.play_track(library.tracks[0].clone()).unwrap();
        wait_until_playing(&service);
        service.enqueue_track(library.tracks[1].clone()).unwrap();

        for reason in [
            EndFileReason::Stop,
            EndFileReason::Quit,
            EndFileReason::Error,
            EndFileReason::Redirect,
            EndFileReason::Unknown,
        ] {
            control.push_event(BackendEvent::EndFile(reason));
        }
        thread::sleep(Duration::from_millis(25));
        assert_eq!(
            service.snapshot().current_track_id,
            Some(library.tracks[0].track_id)
        );

        control.push_event(BackendEvent::EndFile(EndFileReason::Eof));
        wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && snapshot.current_track_id == Some(library.tracks[1].track_id)
        });
        control.push_event(BackendEvent::EndFile(EndFileReason::Eof));
        wait_for_snapshot(&service, |snapshot| snapshot.phase == PlaybackPhase::Ended);

        service.set_repeat_mode(RepeatMode::One).unwrap();
        service.play_track(library.tracks[0].clone()).unwrap();
        wait_until_playing(&service);
        let loads_before = control
            .operations()
            .iter()
            .filter(|operation| matches!(operation, FakeOperation::Load(_)))
            .count();
        control.push_event(BackendEvent::EndFile(EndFileReason::Eof));
        let deadline = Instant::now() + Duration::from_secs(2);
        while control
            .operations()
            .iter()
            .filter(|operation| matches!(operation, FakeOperation::Load(_)))
            .count()
            <= loads_before
        {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
        });
        let loads_after = control
            .operations()
            .iter()
            .filter(|operation| matches!(operation, FakeOperation::Load(_)))
            .count();
        assert!(loads_after > loads_before);

        service.set_repeat_mode(RepeatMode::All).unwrap();
        service.enqueue_track(library.tracks[1].clone()).unwrap();
        service.next_track().unwrap();
        wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && snapshot.current_track_id == Some(library.tracks[1].track_id)
        });
        control.push_event(BackendEvent::EndFile(EndFileReason::Eof));
        wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && snapshot.current_track_id == Some(library.tracks[0].track_id)
        });
        service.shutdown().unwrap();
    }

    #[test]
    fn simultaneous_next_and_eof_advances_once() {
        let library = test_library(3);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(false, Some(10_000));
        service.play_track(library.tracks[0].clone()).unwrap();
        control.push_event(BackendEvent::FileLoaded);
        wait_until_playing(&service);
        service.enqueue_track(library.tracks[1].clone()).unwrap();
        service.enqueue_track(library.tracks[2].clone()).unwrap();

        let service = Arc::new(service);
        let next_service = service.clone();
        let next = thread::spawn(move || next_service.next_track());
        control.push_event(BackendEvent::EndFile(EndFileReason::Eof));
        assert!(next.join().unwrap().is_ok());

        // Both possible command/event arrival orders leave one load pending;
        // completing that load must land on the same next queue entry.
        control.push_event(BackendEvent::FileLoaded);
        let advanced = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && snapshot.current_track_id == Some(library.tracks[1].track_id)
        });
        assert_eq!(advanced.queue_index, Some(1));
        assert_eq!(advanced.queue_length, 3);
        thread::sleep(Duration::from_millis(25));
        assert_eq!(
            service.snapshot().current_track_id,
            Some(library.tracks[1].track_id)
        );
        assert_eq!(service.snapshot().queue_index, Some(1));
        service.shutdown().unwrap();
    }

    #[test]
    fn stale_generation_events_cannot_mutate_the_active_backend_state() {
        let library = test_library(2);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(false, Some(10_000));
        let loading = service.play_track(library.tracks[0].clone()).unwrap();
        assert_eq!(loading.phase, PlaybackPhase::Loading);

        control.push_event_for_generation(0, BackendEvent::FileLoaded);
        control.push_event_for_generation(0, BackendEvent::PositionChanged(4_000));
        thread::sleep(Duration::from_millis(25));
        assert_eq!(service.snapshot().phase, PlaybackPhase::Loading);
        assert_eq!(service.snapshot().position_ms, 0);

        control.push_event(BackendEvent::FileLoaded);
        wait_until_playing(&service);
        service.enqueue_track(library.tracks[1].clone()).unwrap();
        let before_stale_events = service.snapshot();
        control.push_event_for_generation(0, BackendEvent::PositionChanged(7_000));
        control.push_event_for_generation(0, BackendEvent::EndFile(EndFileReason::Eof));
        control.push_event_for_generation(
            0,
            BackendEvent::Failure(PlaybackError::new(
                PlaybackErrorCode::ProtocolError,
                "stale generation failure",
                true,
            )),
        );
        thread::sleep(Duration::from_millis(35));

        let after_stale_events = service.snapshot();
        assert_eq!(after_stale_events.phase, PlaybackPhase::Playing);
        assert_eq!(
            after_stale_events.current_track_id,
            before_stale_events.current_track_id
        );
        assert_eq!(
            after_stale_events.queue_index,
            before_stale_events.queue_index
        );
        assert_eq!(
            after_stale_events.position_ms,
            before_stale_events.position_ms
        );
        assert!(after_stale_events.error.is_none());
        service.shutdown().unwrap();
    }

    #[test]
    fn shuffle_preserves_the_current_entry_and_canonical_queue_length() {
        let library = test_library(3);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(true, Some(10_000));
        service.play_track(library.tracks[0].clone()).unwrap();
        wait_until_playing(&service);
        service.enqueue_track(library.tracks[1].clone()).unwrap();
        service.enqueue_track(library.tracks[2].clone()).unwrap();
        let before = service.snapshot();

        let shuffled = service.set_shuffle_enabled(true).unwrap();
        assert!(shuffled.shuffle_enabled);
        assert_eq!(shuffled.current_track_id, before.current_track_id);
        assert_eq!(shuffled.queue_index, before.queue_index);
        assert_eq!(shuffled.queue_length, 3);
        let canonical = service.set_shuffle_enabled(false).unwrap();
        assert_eq!(canonical.current_track_id, before.current_track_id);
        service.shutdown().unwrap();
    }

    #[test]
    fn missing_unavailable_and_non_local_sources_never_reach_the_backend() {
        let missing_track_library = test_library(1);
        let (missing_service, missing_control, _) = service_with(&missing_track_library);
        let error = missing_service
            .play_track(TrackPlaybackRequest {
                track_id: TrackId::new(),
                source_id: None,
            })
            .unwrap_err();
        assert_eq!(error.code, PlaybackErrorCode::TrackNotFound);
        assert!(!missing_control
            .operations()
            .iter()
            .any(|operation| matches!(operation, FakeOperation::Load(_))));
        missing_service.shutdown().unwrap();

        for (label, statement) in [
            (
                "unavailable",
                "UPDATE track_sources SET available = 0 WHERE id = ?1",
            ),
            (
                "non-local",
                "UPDATE track_sources SET provider_kind = 'youtube' WHERE id = ?1",
            ),
            (
                "not-playable",
                "UPDATE track_sources SET can_playback = 0 WHERE id = ?1",
            ),
        ] {
            let library = test_library(1);
            library
                .database
                .with_connection(|connection| {
                    connection.execute(
                        statement,
                        params![library.tracks[0].source_id.unwrap().to_string()],
                    )?;
                    Ok(())
                })
                .unwrap();
            let (service, control, _) = service_with(&library);
            let error = service.play_track(library.tracks[0].clone()).unwrap_err();
            let expected = match label {
                "non-local" | "not-playable" => PlaybackErrorCode::SourceNotPlayable,
                _ => PlaybackErrorCode::SourceUnavailable,
            };
            assert_eq!(error.code, expected, "{label}");
            assert!(!control
                .operations()
                .iter()
                .any(|operation| matches!(operation, FakeOperation::Load(_))));
            service.shutdown().unwrap();
        }
    }

    #[test]
    fn an_invalid_requested_source_is_rejected_without_fallback() {
        let library = test_library(1);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(true, Some(10_000));
        let error = service
            .play_track(TrackPlaybackRequest {
                track_id: library.tracks[0].track_id,
                source_id: Some(SourceId::new()),
            })
            .unwrap_err();
        assert_eq!(error.code, PlaybackErrorCode::SourceNotFound);
        assert!(!control
            .operations()
            .iter()
            .any(|operation| matches!(operation, FakeOperation::Load(_))));
        service.shutdown().unwrap();
    }

    #[test]
    fn source_switch_preserves_track_queue_timestamp_pause_mix_and_device() {
        let library = test_library(1);
        let alternate =
            add_local_source(&library, library.tracks[0].track_id, "alternate.wav", 2_000);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(true, Some(10_000));
        service.play_track(library.tracks[0].clone()).unwrap();
        wait_until_playing(&service);
        control.push_event(BackendEvent::PositionChanged(4_000));
        wait_for_snapshot(&service, |snapshot| snapshot.position_ms == 4_000);
        service.toggle_play_pause().unwrap();
        service.set_playback_volume(37).unwrap();
        service.set_playback_muted(true).unwrap();
        service.set_audio_device("auto").unwrap();
        control.auto_file_loaded(true, Some(2_000));

        service
            .switch_playback_source(TrackPlaybackRequest {
                track_id: library.tracks[0].track_id,
                source_id: Some(alternate),
            })
            .unwrap();
        let switched = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Paused && snapshot.current_source_id == Some(alternate)
        });
        assert_eq!(switched.current_track_id, Some(library.tracks[0].track_id));
        assert_eq!(switched.queue_index, Some(0));
        assert_eq!(switched.queue_length, 1);
        assert_eq!(switched.position_ms, 2_000);
        assert_eq!(switched.volume_percent, 37);
        assert!(switched.muted);
        assert_eq!(switched.selected_audio_device, "auto");
        service.shutdown().unwrap();
    }

    #[test]
    fn source_switch_failure_restores_the_prior_source_without_queue_advance() {
        let library = test_library(1);
        let alternate =
            add_local_source(&library, library.tracks[0].track_id, "alternate.wav", 2_000);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(true, Some(10_000));
        service.play_track(library.tracks[0].clone()).unwrap();
        let before = wait_until_playing(&service);
        control.fail_next_loads(1);

        let error = service
            .switch_playback_source(TrackPlaybackRequest {
                track_id: library.tracks[0].track_id,
                source_id: Some(alternate),
            })
            .unwrap_err();
        assert_eq!(error.code, PlaybackErrorCode::LoadFailed);
        let restored = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && snapshot.current_source_id == before.current_source_id
                && snapshot.error.is_some()
        });
        assert_eq!(restored.queue_index, before.queue_index);
        assert_eq!(restored.queue_length, before.queue_length);
        service.shutdown().unwrap();
    }

    #[test]
    fn failed_source_switch_rollback_enters_normal_recovery_without_queue_advance() {
        let library = test_library(1);
        let alternate =
            add_local_source(&library, library.tracks[0].track_id, "alternate.wav", 2_000);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(true, Some(10_000));
        service.play_track(library.tracks[0].clone()).unwrap();
        let before = wait_until_playing(&service);
        control.fail_next_loads(2);

        service
            .switch_playback_source(TrackPlaybackRequest {
                track_id: library.tracks[0].track_id,
                source_id: Some(alternate),
            })
            .unwrap_err();
        let recovering = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Recovering
                && snapshot.current_track_id == before.current_track_id
                && snapshot.current_source_id == before.current_source_id
        });
        assert_eq!(recovering.queue_length, before.queue_length);
        assert_eq!(recovering.queue_index, before.queue_index);
        let restored = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && snapshot.current_track_id == before.current_track_id
                && snapshot.current_source_id == before.current_source_id
                && snapshot.error.is_none()
        });
        assert_eq!(restored.title, before.title);
        assert_eq!(restored.queue_length, before.queue_length);
        assert_eq!(restored.queue_index, before.queue_index);
        service.shutdown().unwrap();
    }

    #[test]
    fn crash_recovery_is_generation_scoped_bounded_and_explicitly_retryable() {
        let library = test_library(1);
        let (service, control, snapshots) = service_with(&library);
        control.auto_file_loaded(true, Some(10_000));
        service.play_track(library.tracks[0].clone()).unwrap();
        wait_until_playing(&service);
        control.push_event(BackendEvent::PositionChanged(2_200));
        wait_for_snapshot(&service, |snapshot| snapshot.position_ms == 2_200);
        service.toggle_play_pause().unwrap();
        assert_eq!(service.snapshot().phase, PlaybackPhase::Paused);
        let revision_before_recovery = service.snapshot().revision;

        control.push_event(BackendEvent::Disconnected);
        let recovered = wait_for_snapshot(&service, |snapshot| {
            !snapshot.recovering
                && snapshot.position_ms == 2_200
                && snapshot.revision > revision_before_recovery
        });
        assert_eq!(
            recovered.phase,
            PlaybackPhase::Paused,
            "operations: {:?}; phases: {:?}",
            control.operations(),
            snapshots
                .lock()
                .unwrap()
                .iter()
                .map(|snapshot| snapshot.phase)
                .collect::<Vec<_>>()
        );
        assert!(recovered.error.is_none());
        assert!(snapshots
            .lock()
            .unwrap()
            .iter()
            .any(|snapshot| snapshot.phase == PlaybackPhase::Recovering));

        control.fail_next_starts(3);
        control.push_event(BackendEvent::Disconnected);
        let exhausted =
            wait_for_snapshot_with_timeout(&service, Duration::from_secs(5), |snapshot| {
                snapshot.phase == PlaybackPhase::Failed
                    && snapshot
                        .error
                        .as_ref()
                        .is_some_and(|error| error.code == PlaybackErrorCode::RecoveryExhausted)
            });
        assert!(exhausted.error.unwrap().retryable);

        service.retry_playback_backend().unwrap();
        let retried = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Paused && snapshot.error.is_none()
        });
        assert_eq!(retried.current_track_id, Some(library.tracks[0].track_id));
        let start_count = control
            .operations()
            .iter()
            .filter(|operation| operation == &&FakeOperation::Start)
            .count();
        assert_eq!(start_count, 6);
        service.shutdown().unwrap();
    }

    #[test]
    fn disconnect_during_load_retries_the_current_track() {
        let library = test_library(1);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(true, Some(10_000));
        control.disconnect_next_loads(1);

        assert!(service.play_track(library.tracks[0].clone()).is_err());
        let recovered = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && !snapshot.recovering
                && snapshot.error.is_none()
        });

        assert_eq!(recovered.current_track_id, Some(library.tracks[0].track_id));
        assert_eq!(recovered.position_ms, 0);
        service.shutdown().unwrap();
    }

    #[test]
    fn disconnect_event_during_load_enters_recovery_and_reloads_the_current_track() {
        let library = test_library(1);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(false, Some(10_000));
        let loading = service.play_track(library.tracks[0].clone()).unwrap();
        assert_eq!(loading.phase, PlaybackPhase::Loading);

        control.push_event(BackendEvent::Disconnected);
        control.auto_file_loaded(true, Some(10_000));
        let recovered = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && !snapshot.recovering
                && snapshot.error.is_none()
        });
        assert_eq!(recovered.current_track_id, Some(library.tracks[0].track_id));
        assert_eq!(recovered.queue_index, Some(0));
        service.shutdown().unwrap();
    }

    #[test]
    fn duplicate_terminal_events_start_one_recovery_for_a_backend_generation() {
        let library = test_library(1);
        let (service, control, snapshots) = service_with(&library);
        control.auto_file_loaded(true, Some(10_000));
        service.play_track(library.tracks[0].clone()).unwrap();
        wait_until_playing(&service);
        let starts_before = control
            .operations()
            .iter()
            .filter(|operation| operation == &&FakeOperation::Start)
            .count();
        let revision_before_failure = service.snapshot().revision;

        control.push_event(BackendEvent::Disconnected);
        control.push_event(BackendEvent::ProcessExited {
            expected: false,
            code: Some(9),
        });
        let recovering = wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Recovering
                && snapshot.revision > revision_before_failure
        });
        wait_for_snapshot(&service, |snapshot| {
            snapshot.phase == PlaybackPhase::Playing
                && !snapshot.recovering
                && snapshot.error.is_none()
                && snapshot.revision > recovering.revision
        });

        let starts_after = control
            .operations()
            .iter()
            .filter(|operation| operation == &&FakeOperation::Start)
            .count();
        assert_eq!(starts_after, starts_before + 1);
        assert_eq!(
            snapshots
                .lock()
                .unwrap()
                .iter()
                .filter(|snapshot| snapshot.phase == PlaybackPhase::Recovering)
                .count(),
            1
        );
        service.shutdown().unwrap();
    }

    #[test]
    fn shutdown_is_bounded_publishes_state_and_rejects_new_work() {
        let library = test_library(1);
        let (service, control, _) = service_with(&library);
        let snapshot = service.shutdown().unwrap();
        assert_eq!(snapshot.phase, PlaybackPhase::ShuttingDown);
        let error = service.set_playback_muted(true).unwrap_err();
        assert_eq!(error.code, PlaybackErrorCode::ShuttingDown);
        assert!(control.operations().contains(&FakeOperation::Shutdown));
    }

    #[test]
    fn shutdown_during_load_is_bounded_and_clears_the_pending_transition() {
        let library = test_library(1);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(false, Some(10_000));
        let loading = service.play_track(library.tracks[0].clone()).unwrap();
        assert_eq!(loading.phase, PlaybackPhase::Loading);

        let started = Instant::now();
        let shutdown = service.shutdown().unwrap();
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(shutdown.phase, PlaybackPhase::ShuttingDown);
        assert!(control.operations().contains(&FakeOperation::Shutdown));
    }

    #[test]
    fn commands_racing_with_eof_and_crash_remain_serialized_and_monotonic() {
        let library = test_library(2);
        let (service, control, snapshots) = service_with(&library);
        control.auto_file_loaded(true, Some(10_000));
        service.play_track(library.tracks[0].clone()).unwrap();
        wait_until_playing(&service);
        service.enqueue_track(library.tracks[1].clone()).unwrap();
        let service = Arc::new(service);

        let next_service = service.clone();
        let next = thread::spawn(move || next_service.next_track());
        control.push_event(BackendEvent::EndFile(EndFileReason::Eof));
        let _ = next.join().unwrap();

        let volume_service = service.clone();
        let volume = thread::spawn(move || volume_service.set_playback_volume(61));
        control.push_event(BackendEvent::Disconnected);
        let _ = volume.join().unwrap();
        wait_for_snapshot(&service, |snapshot| {
            !matches!(
                snapshot.phase,
                PlaybackPhase::Loading | PlaybackPhase::Recovering
            )
        });
        let revisions = snapshots
            .lock()
            .unwrap()
            .iter()
            .map(|snapshot| snapshot.revision)
            .collect::<Vec<_>>();
        assert!(revisions.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(service.snapshot().queue_length, 2);
        service.shutdown().unwrap();
    }

    #[test]
    fn clear_queue_stops_without_treating_stop_as_eof() {
        let library = test_library(1);
        let (service, control, _) = service_with(&library);
        control.auto_file_loaded(true, Some(10_000));
        service.play_track(library.tracks[0].clone()).unwrap();
        wait_until_playing(&service);
        let cleared = service.clear_playback_queue().unwrap();
        assert_eq!(cleared.phase, PlaybackPhase::Idle);
        assert_eq!(cleared.queue_length, 0);
        control.push_event(BackendEvent::EndFile(EndFileReason::Stop));
        thread::sleep(Duration::from_millis(25));
        assert_eq!(service.snapshot().phase, PlaybackPhase::Idle);
        assert!(control.operations().contains(&FakeOperation::Stop));
        service.shutdown().unwrap();
    }

    fn minimal_wav_with_sample(sample: i16) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(46);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&38_u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&8_000_u32.to_le_bytes());
        bytes.extend_from_slice(&16_000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&sample.to_le_bytes());
        bytes
    }
}
