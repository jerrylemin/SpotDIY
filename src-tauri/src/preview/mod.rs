//! Disposable local-only audio previews.
//!
//! Preview deliberately does not share queue, recorder, or playback identity
//! with the normal playback controller.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{SourceId, TrackId};
use crate::library::LibraryService;
use crate::media_tools::MediaToolManager;
use crate::playback::{PlaybackPhase, PlaybackService};

const PREVIEW_LIMIT: Duration = Duration::from_secs(8);
const PREVIEW_MAX_VOLUME: u8 = 35;

trait PreviewProcess: Send {
    fn try_wait(&mut self) -> Result<bool, String>;
    fn stop(&mut self);
}

trait PreviewBackend: Send + Sync {
    fn spawn(
        &self,
        path: &Path,
        start_ms: u64,
        volume: u8,
    ) -> Result<Box<dyn PreviewProcess>, String>;
}

struct MpvPreviewBackend {
    tools: MediaToolManager,
}

struct MpvPreviewProcess(Child);

impl PreviewProcess for MpvPreviewProcess {
    fn try_wait(&mut self) -> Result<bool, String> {
        self.0
            .try_wait()
            .map(|status| status.is_some())
            .map_err(|error| error.to_string())
    }

    fn stop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl PreviewBackend for MpvPreviewBackend {
    fn spawn(
        &self,
        path: &Path,
        start_ms: u64,
        volume: u8,
    ) -> Result<Box<dyn PreviewProcess>, String> {
        let executable = self
            .tools
            .require_mpv()
            .map_err(|error| error.to_string())?;
        let child = Command::new(executable)
            .arg("--no-config")
            .arg("--no-video")
            .arg("--audio-display=no")
            .arg("--really-quiet")
            .arg(format!("--volume={volume}"))
            .arg(format!(
                "--start={}.{:03}",
                start_ms / 1_000,
                start_ms % 1_000
            ))
            .arg("--length=8")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?;
        Ok(Box::new(MpvPreviewProcess(child)))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewPhase {
    Idle,
    Loading,
    Playing,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewState {
    pub phase: PreviewPhase,
    pub track_id: Option<TrackId>,
    pub started_at_ms: Option<u64>,
    pub error: Option<String>,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            phase: PreviewPhase::Idle,
            track_id: None,
            started_at_ms: None,
            error: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum PreviewError {
    #[error("Pause playback to preview.")]
    MainPlaybackActive,
    #[error("No playable local audio is available for this track.")]
    NoLocalSource,
    #[error("SpotDIY could not start the local preview backend: {0}")]
    Spawn(String),
    #[error("SpotDIY preview is shutting down.")]
    ShuttingDown,
}

struct PreviewInner {
    audio_gate: Mutex<()>,
    state: Mutex<PreviewState>,
    child: Mutex<Option<Box<dyn PreviewProcess>>>,
    generation: AtomicU64,
    shutting_down: AtomicBool,
}

#[derive(Clone)]
pub struct PreviewService {
    library: LibraryService,
    playback: PlaybackService,
    backend: Arc<dyn PreviewBackend>,
    inner: Arc<PreviewInner>,
}

impl PreviewService {
    pub fn new(
        library: LibraryService,
        playback: PlaybackService,
        tools: MediaToolManager,
    ) -> Self {
        Self::with_backend(library, playback, Arc::new(MpvPreviewBackend { tools }))
    }

    fn with_backend(
        library: LibraryService,
        playback: PlaybackService,
        backend: Arc<dyn PreviewBackend>,
    ) -> Self {
        Self {
            library,
            playback,
            backend,
            inner: Arc::new(PreviewInner {
                audio_gate: Mutex::new(()),
                state: Mutex::new(PreviewState::default()),
                child: Mutex::new(None),
                generation: AtomicU64::new(0),
                shutting_down: AtomicBool::new(false),
            }),
        }
    }

    pub fn state(&self) -> PreviewState {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn start_preview(&self, track_id: TrackId) -> Result<PreviewState, PreviewError> {
        let _audio_gate = self
            .inner
            .audio_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.start_preview_locked(track_id)
    }

    /// Stop a preview and keep normal playback serialized until `action` completes.
    pub fn with_preview_stopped<T>(&self, action: impl FnOnce() -> T) -> T {
        let _audio_gate = self
            .inner
            .audio_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.stop_owned_process_locked();
        action()
    }

    fn start_preview_locked(&self, track_id: TrackId) -> Result<PreviewState, PreviewError> {
        if self.inner.shutting_down.load(Ordering::Acquire) {
            return Err(PreviewError::ShuttingDown);
        }
        let main_phase = self.playback.snapshot().phase;
        if !main_playback_allows_preview(main_phase) {
            return Err(PreviewError::MainPlaybackActive);
        }

        self.stop_owned_process_locked();
        self.set_state(PreviewState {
            phase: PreviewPhase::Loading,
            track_id: Some(track_id),
            started_at_ms: None,
            error: None,
        });

        let Some((source_id, duration_ms)) = self.local_source(track_id)? else {
            let error = PreviewError::NoLocalSource;
            self.set_failed(track_id, error.to_string());
            return Err(error);
        };
        let path = match self.library.resolve_playback_path(track_id, source_id) {
            Ok(path) => path,
            Err(error) => {
                let error = PreviewError::Spawn(error.to_string());
                self.set_failed(track_id, error.to_string());
                return Err(error);
            }
        };

        let snapshot = self.playback.snapshot();
        let start_ms = preview_start_ms(duration_ms);
        let volume = preview_volume(snapshot.volume_percent);
        let child = match self.backend.spawn(&path, start_ms, volume) {
            Ok(child) => child,
            Err(error) => {
                let error = PreviewError::Spawn(error.to_string());
                self.set_failed(track_id, error.to_string());
                return Err(error);
            }
        };
        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        *self
            .inner
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(child);
        let started_at_ms = now_ms();
        self.set_state(PreviewState {
            phase: PreviewPhase::Playing,
            track_id: Some(track_id),
            started_at_ms: Some(started_at_ms),
            error: None,
        });
        self.spawn_reaper(generation);
        Ok(self.state())
    }

    pub fn cancel_preview(&self) -> PreviewState {
        let _audio_gate = self
            .inner
            .audio_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.stop_owned_process_locked();
        self.set_state(PreviewState::default());
        self.state()
    }

    pub fn shutdown(&self) -> PreviewState {
        let _audio_gate = self
            .inner
            .audio_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.inner.shutting_down.store(true, Ordering::Release);
        self.stop_owned_process_locked();
        self.set_state(PreviewState::default());
        self.state()
    }

    fn local_source(
        &self,
        track_id: TrackId,
    ) -> Result<Option<(SourceId, Option<u64>)>, PreviewError> {
        self.library
            .database()
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT ts.id, coalesce(ts.duration_ms, t.duration_ms)
                         FROM track_sources ts
                         INNER JOIN tracks t ON t.id = ts.track_id
                         INNER JOIN local_files lf ON lf.source_id = ts.id
                         WHERE ts.track_id = ?1
                           AND ts.provider_kind = 'local'
                           AND ts.available = 1
                           AND ts.can_playback = 1
                           AND lf.index_status = 'indexed'
                         ORDER BY CASE WHEN t.preferred_source_id = ts.id THEN 0 ELSE 1 END,
                                  ts.id ASC
                         LIMIT 1",
                        [track_id.to_string()],
                        |row| {
                            let source_id: String = row.get(0)?;
                            let duration_ms: Option<i64> = row.get(1)?;
                            Ok((source_id, duration_ms))
                        },
                    )
                    .optional()
            })
            .map_err(|error| PreviewError::Spawn(error.to_string()))?
            .map(|(source_id, duration_ms)| {
                let source_id = SourceId::parse_str(&source_id)
                    .map_err(|error| PreviewError::Spawn(error.to_string()))?;
                let duration_ms = duration_ms
                    .map(|value| {
                        u64::try_from(value).map_err(|error| PreviewError::Spawn(error.to_string()))
                    })
                    .transpose()?;
                Ok((source_id, duration_ms))
            })
            .transpose()
    }

    fn set_state(&self, state: PreviewState) {
        *self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = state;
    }

    fn set_failed(&self, track_id: TrackId, error: String) {
        self.set_state(PreviewState {
            phase: PreviewPhase::Failed,
            track_id: Some(track_id),
            started_at_ms: None,
            error: Some(error),
        });
    }

    fn stop_owned_process_locked(&self) {
        self.inner.generation.fetch_add(1, Ordering::AcqRel);
        let child = self
            .inner
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(mut child) = child {
            child.stop();
        }
        self.set_state(PreviewState::default());
    }

    fn spawn_reaper(&self, generation: u64) {
        let inner = self.inner.clone();
        thread::Builder::new()
            .name("spotdiy-preview-reaper".to_owned())
            .spawn(move || {
                let deadline = Instant::now() + PREVIEW_LIMIT;
                loop {
                    if inner.generation.load(Ordering::Acquire) != generation {
                        return;
                    }
                    let finished = {
                        let mut child = inner
                            .child
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        if inner.generation.load(Ordering::Acquire) != generation {
                            return;
                        }
                        match child.as_mut() {
                            Some(child) => child.try_wait().unwrap_or(true),
                            None => true,
                        }
                    };
                    if finished || Instant::now() >= deadline {
                        let _audio_gate = inner
                            .audio_gate
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        let mut child = {
                            let mut child_guard = inner
                                .child
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            if inner.generation.load(Ordering::Acquire) != generation {
                                return;
                            }
                            child_guard.take()
                        };
                        if let Some(child) = child.as_mut() {
                            child.stop();
                        }
                        if inner.generation.load(Ordering::Acquire) == generation {
                            *inner
                                .state
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner()) = PreviewState {
                                phase: PreviewPhase::Idle,
                                track_id: None,
                                started_at_ms: None,
                                error: None,
                            };
                        }
                        return;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            })
            .ok();
    }
}

fn main_playback_allows_preview(phase: PlaybackPhase) -> bool {
    matches!(
        phase,
        PlaybackPhase::Idle | PlaybackPhase::Paused | PlaybackPhase::Ended
    )
}

fn preview_start_ms(duration_ms: Option<u64>) -> u64 {
    duration_ms
        .map(|duration| (duration.saturating_mul(3) / 10).min(30_000))
        .unwrap_or(0)
}

fn preview_volume(main_volume: u8) -> u8 {
    main_volume.min(PREVIEW_MAX_VOLUME)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::library::LibraryService;
    use crate::media_tools::MediaToolManager;
    use std::path::PathBuf;
    use std::sync::mpsc;

    #[derive(Default)]
    struct FakePreviewBackend {
        spawn_count: AtomicU64,
    }

    struct FakePreviewProcess {
        stopped: Arc<AtomicBool>,
    }

    struct ReaperProbeProcess {
        checked: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
        finished: mpsc::Sender<()>,
    }

    impl PreviewProcess for FakePreviewProcess {
        fn try_wait(&mut self) -> Result<bool, String> {
            Ok(self.stopped.load(Ordering::Acquire))
        }

        fn stop(&mut self) {
            self.stopped.store(true, Ordering::Release);
        }
    }

    impl PreviewProcess for ReaperProbeProcess {
        fn try_wait(&mut self) -> Result<bool, String> {
            self.checked.send(()).unwrap();
            self.release.recv().unwrap();
            self.finished.send(()).unwrap();
            Ok(true)
        }

        fn stop(&mut self) {}
    }

    impl PreviewBackend for FakePreviewBackend {
        fn spawn(
            &self,
            _path: &Path,
            _start_ms: u64,
            _volume: u8,
        ) -> Result<Box<dyn PreviewProcess>, String> {
            self.spawn_count.fetch_add(1, Ordering::AcqRel);
            Ok(Box::new(FakePreviewProcess {
                stopped: Arc::new(AtomicBool::new(false)),
            }))
        }
    }

    #[test]
    fn preview_policy_is_bounded_and_deterministic() {
        assert_eq!(preview_start_ms(None), 0);
        assert_eq!(preview_start_ms(Some(10_000)), 3_000);
        assert_eq!(preview_start_ms(Some(120_000)), 30_000);
        assert_eq!(preview_volume(100), 35);
        assert_eq!(preview_volume(20), 20);
        assert!(main_playback_allows_preview(PlaybackPhase::Idle));
        assert!(main_playback_allows_preview(PlaybackPhase::Paused));
        assert!(main_playback_allows_preview(PlaybackPhase::Ended));
        assert!(!main_playback_allows_preview(PlaybackPhase::Playing));
        assert!(!main_playback_allows_preview(PlaybackPhase::Seeking));
        assert!(!main_playback_allows_preview(PlaybackPhase::Loading));
        assert!(!main_playback_allows_preview(PlaybackPhase::Recovering));
    }

    #[test]
    fn preview_backend_is_injectable_without_an_eight_second_sleep() {
        let backend = FakePreviewBackend::default();
        let mut process = backend
            .spawn(Path::new("C:/synthetic/track.flac"), 30_000, 35)
            .unwrap();
        assert_eq!(backend.spawn_count.load(Ordering::Acquire), 1);
        assert!(!process.try_wait().unwrap());
        process.stop();
        assert!(process.try_wait().unwrap());
    }

    #[test]
    fn preview_gate_stops_owned_audio_and_serializes_the_normal_transition() {
        let root = tempfile::tempdir().unwrap();
        let database = Database::open(root.path().join("preview.sqlite3")).unwrap();
        let library = LibraryService::new(database, root.path().join("artwork")).unwrap();
        let playback = PlaybackService::new(
            library.clone(),
            MediaToolManager::with_override(PathBuf::from("missing-mpv")),
            Arc::new(|_| {}),
        );
        let preview = PreviewService::with_backend(
            library,
            playback.clone(),
            Arc::new(FakePreviewBackend::default()),
        );
        let stopped = Arc::new(AtomicBool::new(false));
        *preview.inner.child.lock().unwrap() = Some(Box::new(FakePreviewProcess {
            stopped: stopped.clone(),
        }));
        let track_id = TrackId::new();
        preview.set_state(PreviewState {
            phase: PreviewPhase::Playing,
            track_id: Some(track_id),
            started_at_ms: Some(1),
            error: None,
        });

        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let held_preview = preview.clone();
        let first = thread::spawn(move || {
            held_preview.with_preview_stopped(|| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            });
        });
        entered_rx.recv().unwrap();
        assert!(stopped.load(Ordering::Acquire));

        let (attempted_tx, attempted_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();
        let waiting_preview = preview.clone();
        let second = thread::spawn(move || {
            attempted_tx.send(()).unwrap();
            let result =
                waiting_preview.with_preview_stopped(|| Err::<(), _>("normal transition failed"));
            assert_eq!(result.unwrap_err(), "normal transition failed");
            completed_tx.send(()).unwrap();
        });
        attempted_rx.recv().unwrap();
        assert!(completed_rx.try_recv().is_err());

        release_tx.send(()).unwrap();
        first.join().unwrap();
        second.join().unwrap();
        assert!(completed_rx.recv().is_ok());
        assert_eq!(preview.state(), PreviewState::default());
        playback.shutdown().unwrap();
    }

    #[test]
    fn stale_preview_reaper_cannot_reset_a_new_generation() {
        let root = tempfile::tempdir().unwrap();
        let database = Database::open(root.path().join("preview.sqlite3")).unwrap();
        let library = LibraryService::new(database, root.path().join("artwork")).unwrap();
        let playback = PlaybackService::new(
            library.clone(),
            MediaToolManager::with_override(PathBuf::from("missing-mpv")),
            Arc::new(|_| {}),
        );
        let preview = PreviewService::with_backend(
            library,
            playback.clone(),
            Arc::new(FakePreviewBackend::default()),
        );
        let old_track_id = TrackId::new();
        let new_track_id = TrackId::new();
        preview.inner.generation.store(7, Ordering::Release);
        preview.set_state(PreviewState {
            phase: PreviewPhase::Playing,
            track_id: Some(old_track_id),
            started_at_ms: Some(1),
            error: None,
        });
        let (checked_tx, checked_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        *preview.inner.child.lock().unwrap() = Some(Box::new(ReaperProbeProcess {
            checked: checked_tx,
            release: release_rx,
            finished: finished_tx,
        }));

        preview.spawn_reaper(7);
        checked_rx.recv().unwrap();
        preview.inner.generation.store(8, Ordering::Release);
        preview.set_state(PreviewState {
            phase: PreviewPhase::Playing,
            track_id: Some(new_track_id),
            started_at_ms: Some(2),
            error: None,
        });
        release_tx.send(()).unwrap();
        finished_rx.recv().unwrap();

        assert_eq!(preview.state().track_id, Some(new_track_id));
        assert_eq!(preview.state().phase, PreviewPhase::Playing);
        playback.shutdown().unwrap();
    }
}
