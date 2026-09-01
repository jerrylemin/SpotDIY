pub mod progress;
pub mod task;

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use thiserror::Error;
use tokio::sync::mpsc;

use crate::db::repository::{SourceRepository, TrackRepository};
use crate::db::APPLICATION_DATA_DIRECTORY;
use crate::domain::{ProviderKind, SourceId, TrackId};
use crate::media_tools::{FfmpegToolStatus, MediaToolHealth, MediaToolManager, YtDlpToolStatus};
use crate::search::types::{SearchCancellation, SearchResult};
use crate::settings::SettingsRepository;
use crate::sources::yt_dlp::{
    TokioYtDlpProcessRunner, YtDlpDownloadEvent, YtDlpDownloadProcessError,
    YtDlpDownloadProcessOutput, YtDlpDownloadRunner, YT_DLP_DOWNLOAD_EVENT_CHANNEL_CAPACITY,
};
use crate::sources::{sanitize_artwork_url, validate_provider_url};

pub use progress::{parse_file_line, parse_progress_line, DownloadProgressUpdate};
pub use task::{
    is_valid_transition, DownloadErrorCode, DownloadMode, DownloadRepository,
    DownloadRepositoryError, DownloadRequest, DownloadSnapshot, DownloadState, DownloadTask,
    DownloadTaskId, DownloadToolStatus, MediaToolsSnapshot, SourceQualityProvenance,
};

pub const DOWNLOAD_STATE_EVENT: &str = "downloads://state";
const DEFAULT_MAX_CONCURRENT: u8 = 2;
const MIN_MAX_CONCURRENT: u8 = 1;
const MAX_MAX_CONCURRENT: u8 = 4;
const PROGRESS_PERSIST_INTERVAL: Duration = Duration::from_millis(250);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_COLLISION_ATTEMPTS: u32 = 1000;
const MAX_ERROR_DETAIL_LENGTH: usize = 1024;
const MAX_FILENAME_CHARS: usize = 180;

pub type DownloadSnapshotSink = Arc<dyn Fn(DownloadSnapshot) + Send + Sync + 'static>;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("download persistence failed: {0}")]
    Persistence(String),
    #[error("download task {0} was not found")]
    TaskNotFound(DownloadTaskId),
    #[error("{code:?}: {detail}")]
    Invalid {
        code: DownloadErrorCode,
        detail: String,
    },
    #[error("download task transition failed: {0}")]
    Transition(#[from] task::DownloadTransitionError),
    #[error("download filesystem operation {operation} failed: {source}")]
    Filesystem {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
}

impl DownloadError {
    pub fn code(&self) -> DownloadErrorCode {
        match self {
            Self::Persistence(_) => DownloadErrorCode::PersistenceFailed,
            Self::TaskNotFound(_) | Self::Invalid { .. } | Self::Transition(_) => {
                DownloadErrorCode::InvalidRequest
            }
            Self::Filesystem { .. } => DownloadErrorCode::FinalizationFailed,
        }
    }
}

struct RunningTask {
    cancellation: SearchCancellation,
}

struct RuntimeState {
    revision: u64,
    running: HashMap<DownloadTaskId, RunningTask>,
    latest_progress: HashMap<DownloadTaskId, DownloadProgressUpdate>,
    progress_persisted_at: HashMap<DownloadTaskId, Instant>,
    shutting_down: bool,
}

struct DownloadServiceInner {
    database: crate::db::Database,
    media_tools: MediaToolManager,
    runner: Arc<dyn YtDlpDownloadRunner>,
    task_temp_root: PathBuf,
    sink: Option<DownloadSnapshotSink>,
    runtime: Mutex<RuntimeState>,
    done: Condvar,
}

#[derive(Clone)]
pub struct DownloadService {
    inner: Arc<DownloadServiceInner>,
}

impl DownloadService {
    pub fn new(
        database: crate::db::Database,
        media_tools: MediaToolManager,
        sink: Option<DownloadSnapshotSink>,
    ) -> Result<Self, DownloadError> {
        let task_temp_root = default_task_temp_root();
        Self::with_runner(
            database,
            media_tools,
            task_temp_root,
            Arc::new(TokioYtDlpProcessRunner::default()),
            sink,
        )
    }

    pub fn with_task_root(
        database: crate::db::Database,
        media_tools: MediaToolManager,
        task_temp_root: PathBuf,
        sink: Option<DownloadSnapshotSink>,
    ) -> Result<Self, DownloadError> {
        Self::with_runner(
            database,
            media_tools,
            task_temp_root,
            Arc::new(TokioYtDlpProcessRunner::default()),
            sink,
        )
    }

    pub fn with_runner(
        database: crate::db::Database,
        media_tools: MediaToolManager,
        task_temp_root: PathBuf,
        runner: Arc<dyn YtDlpDownloadRunner>,
        sink: Option<DownloadSnapshotSink>,
    ) -> Result<Self, DownloadError> {
        let service = Self {
            inner: Arc::new(DownloadServiceInner {
                database,
                media_tools,
                runner,
                task_temp_root,
                sink,
                runtime: Mutex::new(RuntimeState {
                    revision: 0,
                    running: HashMap::new(),
                    latest_progress: HashMap::new(),
                    progress_persisted_at: HashMap::new(),
                    shutting_down: false,
                }),
                done: Condvar::new(),
            }),
        };
        service.recover_on_startup()?;
        Ok(service)
    }

    pub fn start(&self) -> Result<(), DownloadError> {
        self.schedule();
        Ok(())
    }

    pub fn snapshot(&self) -> Result<DownloadSnapshot, DownloadError> {
        let revision = self
            .inner
            .runtime
            .lock()
            .map_err(|_| persistence_error("download runtime lock is poisoned"))?
            .revision;
        self.build_snapshot(revision)
    }

    pub fn queue_search_result_download(
        &self,
        result: SearchResult,
        mode: DownloadMode,
    ) -> Result<DownloadTask, DownloadError> {
        let provider = validate_download_provider(result.provider)?;
        if result.entity_kind != crate::search::types::SearchEntityKind::Track {
            return Err(invalid(
                DownloadErrorCode::InvalidRequest,
                "only track search results can be downloaded",
            ));
        }
        let provider_item_id = non_empty(result.provider_item_id, "provider item ID")?;
        let canonical_url = result.canonical_url.as_ref().ok_or_else(|| {
            invalid(
                DownloadErrorCode::InvalidProviderUrl,
                "a canonical provider URL is required",
            )
        })?;
        let canonical_url = validate_provider_url(provider, canonical_url.as_url().as_str())
            .map_err(|_| {
                invalid(
                    DownloadErrorCode::InvalidProviderUrl,
                    "the provider URL is not allowed",
                )
            })?;
        let destination = self.download_destination()?;
        let artwork_url = result
            .artwork_url
            .as_ref()
            .and_then(|url| sanitize_artwork_url(url.as_url().as_str()))
            .map(|url| url.as_url().as_str().to_owned());
        let request = DownloadRequest {
            provider_kind: provider,
            provider_item_id,
            canonical_url: canonical_url.as_url().as_str().to_owned(),
            target_track_id: result.local_track_id,
            target_source_id: None,
            title: result.title,
            artists: result.artists,
            artwork_url,
            mode,
        };
        self.insert_task(request, destination)
    }

    pub fn queue_source_download(
        &self,
        track_id: TrackId,
        source_id: SourceId,
        mode: DownloadMode,
    ) -> Result<DownloadTask, DownloadError> {
        let source = SourceRepository::new(&self.inner.database)
            .get(source_id)
            .map_err(|error| persistence_error(error.to_string()))?
            .ok_or_else(|| {
                invalid(
                    DownloadErrorCode::SourceNotFound,
                    "the requested source was not found",
                )
            })?;
        if source.track_id != track_id {
            return Err(invalid(
                DownloadErrorCode::SourceTrackMismatch,
                "the source does not belong to the requested track",
            ));
        }
        let provider = validate_download_provider(source.provider_kind)?;
        if !source.capabilities.downloads {
            return Err(invalid(
                DownloadErrorCode::UnsupportedProvider,
                "the source does not advertise download capability",
            ));
        }
        let source_uri = source.source_uri.as_ref().ok_or_else(|| {
            invalid(
                DownloadErrorCode::InvalidProviderUrl,
                "the source has no canonical provider URL",
            )
        })?;
        let canonical_url = validate_provider_url(provider, source_uri.as_str()).map_err(|_| {
            invalid(
                DownloadErrorCode::InvalidProviderUrl,
                "the source URL is not allowed",
            )
        })?;
        let track = TrackRepository::new(&self.inner.database)
            .get(track_id)
            .map_err(|error| persistence_error(error.to_string()))?
            .ok_or_else(|| {
                invalid(
                    DownloadErrorCode::SourceNotFound,
                    "the requested track was not found",
                )
            })?;
        let destination = self.download_destination()?;
        let request = DownloadRequest {
            provider_kind: provider,
            provider_item_id: non_empty(source.provider_item_id, "provider item ID")?,
            canonical_url: canonical_url.as_url().as_str().to_owned(),
            target_track_id: Some(track_id),
            target_source_id: Some(source_id),
            title: track.title,
            artists: track
                .artists
                .into_iter()
                .map(|artist| artist.name)
                .collect(),
            artwork_url: None,
            mode,
        };
        self.insert_task(request, destination)
    }

    pub fn cancel_download(&self, id: DownloadTaskId) -> Result<DownloadTask, DownloadError> {
        let mut task = self.task(id)?;
        match task.state {
            DownloadState::Queued => {
                task.transition(DownloadState::Cancelled)?;
                task.error_code = Some(DownloadErrorCode::Cancelled);
                task.error_detail = Some("download cancelled before it started".to_owned());
                self.save_task(&task)?;
                self.clear_runtime_progress(id);
                self.publish_snapshot();
                self.schedule();
                Ok(task)
            }
            DownloadState::Resolving
            | DownloadState::Downloading
            | DownloadState::Postprocessing => {
                if let Ok(runtime) = self.inner.runtime.lock() {
                    if let Some(running) = runtime.running.get(&id) {
                        running.cancellation.cancel();
                    }
                }
                Ok(task)
            }
            DownloadState::Completed => Err(invalid(
                DownloadErrorCode::InvalidRequest,
                "completed downloads are immutable",
            )),
            DownloadState::Failed | DownloadState::Cancelled => Ok(task),
        }
    }

    pub fn retry_download(&self, id: DownloadTaskId) -> Result<DownloadTask, DownloadError> {
        let mut task = self.task(id)?;
        task.prepare_retry()?;
        cleanup_owned_task_temp(&self.inner.task_temp_root, id)
            .map_err(|source| filesystem_error("remove task temporary directory", source))?;
        self.save_task(&task)?;
        self.clear_runtime_progress(id);
        self.publish_snapshot();
        self.schedule();
        Ok(task)
    }

    pub fn set_download_concurrency(
        &self,
        max_concurrent: u8,
    ) -> Result<DownloadSnapshot, DownloadError> {
        if !(MIN_MAX_CONCURRENT..=MAX_MAX_CONCURRENT).contains(&max_concurrent) {
            return Err(invalid(
                DownloadErrorCode::InvalidRequest,
                "download concurrency must be between 1 and 4",
            ));
        }
        DownloadRepository::new(&self.inner.database)
            .set_max_concurrent(max_concurrent)
            .map_err(|error| persistence_error(error.to_string()))?;
        self.publish_snapshot();
        self.schedule();
        self.snapshot()
    }

    pub fn trusted_output_path(&self, id: DownloadTaskId) -> Result<PathBuf, DownloadError> {
        let task = self.task(id)?;
        if task.state != DownloadState::Completed {
            return Err(invalid(
                DownloadErrorCode::InvalidRequest,
                "only completed downloads have an output location",
            ));
        }
        let output = task.output_path.ok_or_else(|| {
            invalid(
                DownloadErrorCode::OutputInvalid,
                "the completed download has no recorded output",
            )
        })?;
        validate_final_output(&task.destination_directory, &output)
            .map_err(|error| invalid(DownloadErrorCode::OutputInvalid, &error))?;
        Ok(output)
    }

    pub fn shutdown(&self) -> Result<(), DownloadError> {
        let cancellations = self.begin_shutdown();
        for cancellation in cancellations {
            cancellation.cancel();
        }
        if tokio::runtime::Handle::try_current().is_err() {
            let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .map_err(|_| persistence_error("download runtime lock is poisoned"))?;
            while !runtime.running.is_empty() {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                let (next, _) = self
                    .inner
                    .done
                    .wait_timeout(runtime, remaining)
                    .map_err(|_| persistence_error("download shutdown wait failed"))?;
                runtime = next;
            }
        }
        Ok(())
    }

    pub async fn shutdown_async(&self) -> Result<(), DownloadError> {
        let cancellations = self.begin_shutdown();
        for cancellation in cancellations {
            cancellation.cancel();
        }
        let deadline = tokio::time::sleep(SHUTDOWN_TIMEOUT);
        tokio::pin!(deadline);
        loop {
            let empty = self
                .inner
                .runtime
                .lock()
                .map(|runtime| runtime.running.is_empty())
                .unwrap_or(true);
            if empty {
                return Ok(());
            }
            tokio::select! {
                _ = &mut deadline => return Ok(()),
                _ = tokio::time::sleep(Duration::from_millis(10)) => {}
            }
        }
    }

    fn begin_shutdown(&self) -> Vec<SearchCancellation> {
        let Ok(mut runtime) = self.inner.runtime.lock() else {
            return Vec::new();
        };
        runtime.shutting_down = true;
        runtime
            .running
            .values()
            .map(|running| running.cancellation.clone())
            .collect()
    }

    fn insert_task(
        &self,
        request: DownloadRequest,
        destination: PathBuf,
    ) -> Result<DownloadTask, DownloadError> {
        let task = DownloadTask::from_request(request, destination);
        DownloadRepository::new(&self.inner.database)
            .insert(&task)
            .map_err(|error| persistence_error(error.to_string()))?;
        self.publish_snapshot();
        self.schedule();
        Ok(task)
    }

    fn task(&self, id: DownloadTaskId) -> Result<DownloadTask, DownloadError> {
        DownloadRepository::new(&self.inner.database)
            .get(id)
            .map_err(|error| persistence_error(error.to_string()))?
            .ok_or(DownloadError::TaskNotFound(id))
    }

    fn save_task(&self, task: &DownloadTask) -> Result<(), DownloadError> {
        DownloadRepository::new(&self.inner.database)
            .update(task)
            .map_err(|error| persistence_error(error.to_string()))
    }

    fn download_destination(&self) -> Result<PathBuf, DownloadError> {
        let configured = SettingsRepository::new(&self.inner.database)
            .get_downloads_directory()
            .map_err(|error| persistence_error(error.to_string()))?
            .ok_or_else(|| {
                invalid(
                    DownloadErrorCode::DownloadDirectoryNotConfigured,
                    "choose a download directory before starting a download",
                )
            })?;
        validate_download_directory(&configured)
            .map_err(|source| invalid(DownloadErrorCode::DownloadDirectoryInvalid, &source))
    }

    fn recover_on_startup(&self) -> Result<(), DownloadError> {
        let recovered = DownloadRepository::new(&self.inner.database)
            .recover_interrupted()
            .map_err(|error| persistence_error(error.to_string()))?;
        for id in recovered {
            cleanup_owned_task_temp(&self.inner.task_temp_root, id).map_err(|source| {
                filesystem_error("clean interrupted task temporary directory", source)
            })?;
        }
        Ok(())
    }

    fn schedule(&self) {
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        loop {
            let task = match self.next_schedulable_task() {
                Ok(Some(task)) => task,
                Ok(None) | Err(_) => return,
            };
            let cancellation = SearchCancellation::new();
            let should_start = match self.inner.runtime.lock() {
                Ok(mut runtime) => {
                    if runtime.shutting_down
                        || runtime.running.len()
                            >= usize::from(self.max_concurrent().unwrap_or(DEFAULT_MAX_CONCURRENT))
                    {
                        false
                    } else if runtime.running.contains_key(&task.id) {
                        false
                    } else {
                        runtime.running.insert(
                            task.id,
                            RunningTask {
                                cancellation: cancellation.clone(),
                            },
                        );
                        true
                    }
                }
                Err(_) => false,
            };
            if !should_start {
                return;
            }
            let service = self.clone();
            tokio::spawn(async move {
                service.run_task(task, cancellation).await;
            });
        }
    }

    fn next_schedulable_task(&self) -> Result<Option<DownloadTask>, DownloadError> {
        let queued = DownloadRepository::new(&self.inner.database)
            .queued()
            .map_err(|error| persistence_error(error.to_string()))?;
        let runtime = self
            .inner
            .runtime
            .lock()
            .map_err(|_| persistence_error("download runtime lock is poisoned"))?;
        Ok(queued
            .into_iter()
            .find(|task| !runtime.running.contains_key(&task.id)))
    }

    fn max_concurrent(&self) -> Result<u8, DownloadError> {
        DownloadRepository::new(&self.inner.database)
            .max_concurrent()
            .map_err(|error| persistence_error(error.to_string()))
    }

    async fn run_task(&self, mut task: DownloadTask, cancellation: SearchCancellation) {
        let result = self.run_task_inner(&mut task, cancellation.clone()).await;
        if let Err((code, detail)) = result {
            if cancellation.subscribe().borrow().to_owned() {
                if self.is_shutting_down() {
                    let _ = self.requeue_after_shutdown(&mut task);
                } else {
                    let _ = self.mark_cancelled(&mut task);
                }
            } else {
                let _ = self.mark_failed(&mut task, code, &detail);
            }
        }
        self.finish_running(task.id);
        self.schedule();
    }

    async fn run_task_inner(
        &self,
        task: &mut DownloadTask,
        cancellation: SearchCancellation,
    ) -> Result<(), (DownloadErrorCode, String)> {
        if self.is_cancelled(&cancellation) {
            return Err((
                DownloadErrorCode::Cancelled,
                "download was cancelled".to_owned(),
            ));
        }
        self.transition_and_publish(task, DownloadState::Resolving)
            .map_err(|error| (error.code(), error.to_string()))?;
        if let Err(detail) = validate_download_directory(&task.destination_directory) {
            return Err((DownloadErrorCode::DownloadDirectoryInvalid, detail));
        }
        let ytdlp = self.inner.media_tools.yt_dlp_status();
        let Some(ytdlp_path) = ready_yt_dlp(&ytdlp) else {
            return Err((
                tool_error_code(ytdlp.status),
                ytdlp
                    .detail
                    .unwrap_or_else(|| "yt-dlp is unavailable".to_owned()),
            ));
        };
        let ffmpeg = if task.mode == DownloadMode::Video {
            let status = self.inner.media_tools.ffmpeg_status();
            let Some(path) = ready_ffmpeg(&status) else {
                return Err((
                    tool_error_code(status.health_as_runtime_status()),
                    status
                        .detail
                        .unwrap_or_else(|| "FFmpeg is unavailable for video merging".to_owned()),
                ));
            };
            Some(path)
        } else {
            None
        };
        let task_root = create_owned_task_temp(&self.inner.task_temp_root, task.id)
            .map_err(|error| (DownloadErrorCode::FinalizationFailed, error.to_string()))?;
        let args = build_download_args(task, &task_root, ffmpeg.as_deref());
        self.transition_and_publish(task, DownloadState::Downloading)
            .map_err(|error| (error.code(), error.to_string()))?;

        let (event_tx, mut event_rx) = mpsc::channel(YT_DLP_DOWNLOAD_EVENT_CHANNEL_CAPACITY);
        let mut process = Box::pin(self.inner.runner.run_download(
            &ytdlp_path,
            &args,
            cancellation.clone(),
            event_tx,
        ));
        let mut reported_output = None;
        let process_result = loop {
            tokio::select! {
                result = &mut process => break result,
                event = event_rx.recv() => {
                    let Some(event) = event else { continue };
                    self.handle_process_event(task, event, &mut reported_output);
                }
            }
        };
        while let Ok(event) = event_rx.try_recv() {
            self.handle_process_event(task, event, &mut reported_output);
        }
        process_result.map_err(|error| process_error(&error))?;
        if self.is_cancelled(&cancellation) {
            return Err((
                DownloadErrorCode::Cancelled,
                "download was cancelled".to_owned(),
            ));
        }
        self.transition_and_publish(task, DownloadState::Postprocessing)
            .map_err(|error| (error.code(), error.to_string()))?;
        let source = select_task_output(&task_root, reported_output)
            .map_err(|detail| (DownloadErrorCode::OutputInvalid, detail))?;
        task.output_extension = output_extension(&source);
        task.output_codec = None;
        task.transcoded = false;
        task.output_path = Some(
            finalize_download_output(&source, &task.destination_directory, task)
                .map_err(|error| (DownloadErrorCode::FinalizationFailed, error.to_string()))?,
        );
        self.transition_and_publish(task, DownloadState::Completed)
            .map_err(|error| (error.code(), error.to_string()))?;
        cleanup_owned_task_temp(&self.inner.task_temp_root, task.id)
            .map_err(|error| (DownloadErrorCode::FinalizationFailed, error.to_string()))?;
        Ok(())
    }

    fn handle_process_event(
        &self,
        task: &mut DownloadTask,
        event: YtDlpDownloadEvent,
        reported_output: &mut Option<PathBuf>,
    ) {
        match event {
            YtDlpDownloadEvent::StdoutLine(line) => {
                if let Some(progress) = parse_progress_line(&line) {
                    task.expected_bytes = progress.total_bytes.or(progress.total_bytes_estimate);
                    task.downloaded_bytes = progress.downloaded_bytes;
                    task.progress_permille = if progress.status == "finished" {
                        1000
                    } else {
                        progress.progress_permille()
                    };
                    task.speed_bytes_per_second = progress.speed_bytes_per_second;
                    task.eta_seconds = progress.eta_seconds;
                    let should_persist = self.update_latest_progress(task.id, progress.clone());
                    if should_persist {
                        let _ = self.save_task(task);
                        self.mark_progress_persisted(task.id);
                        self.publish_snapshot();
                    }
                } else if let Some(path) = parse_file_line(&line) {
                    *reported_output = Some(path);
                }
            }
            YtDlpDownloadEvent::StderrLine(_) => {}
        }
    }

    fn transition_and_publish(
        &self,
        task: &mut DownloadTask,
        state: DownloadState,
    ) -> Result<(), DownloadError> {
        task.transition(state)?;
        self.save_task(task)?;
        self.clear_runtime_progress(task.id);
        self.publish_snapshot();
        Ok(())
    }

    fn mark_failed(
        &self,
        task: &mut DownloadTask,
        code: DownloadErrorCode,
        detail: &str,
    ) -> Result<(), DownloadError> {
        if task.state.is_active() {
            task.transition(DownloadState::Failed)?;
        }
        task.error_code = Some(code);
        task.error_detail = bounded_detail(detail.to_owned());
        task.speed_bytes_per_second = None;
        task.eta_seconds = None;
        self.save_task(task)?;
        self.clear_runtime_progress(task.id);
        self.publish_snapshot();
        let _ = cleanup_owned_task_temp(&self.inner.task_temp_root, task.id);
        Ok(())
    }

    fn mark_cancelled(&self, task: &mut DownloadTask) -> Result<(), DownloadError> {
        if task.state.is_active() {
            task.transition(DownloadState::Cancelled)?;
        }
        task.error_code = Some(DownloadErrorCode::Cancelled);
        task.error_detail = Some("download cancelled".to_owned());
        task.speed_bytes_per_second = None;
        task.eta_seconds = None;
        self.save_task(task)?;
        self.clear_runtime_progress(task.id);
        let _ = cleanup_owned_task_temp(&self.inner.task_temp_root, task.id);
        self.publish_snapshot();
        Ok(())
    }

    fn requeue_after_shutdown(&self, task: &mut DownloadTask) -> Result<(), DownloadError> {
        if task.state.is_active() {
            task.requeue_after_interruption();
        }
        task.started_at = None;
        task.error_code = None;
        task.error_detail = None;
        task.expected_bytes = None;
        task.downloaded_bytes = 0;
        task.progress_permille = 0;
        task.speed_bytes_per_second = None;
        task.eta_seconds = None;
        self.save_task(task)?;
        self.clear_runtime_progress(task.id);
        let _ = cleanup_owned_task_temp(&self.inner.task_temp_root, task.id);
        self.publish_snapshot();
        Ok(())
    }

    fn is_cancelled(&self, cancellation: &SearchCancellation) -> bool {
        *cancellation.subscribe().borrow()
    }

    fn is_shutting_down(&self) -> bool {
        self.inner
            .runtime
            .lock()
            .map(|runtime| runtime.shutting_down)
            .unwrap_or(true)
    }

    fn update_latest_progress(&self, id: DownloadTaskId, progress: DownloadProgressUpdate) -> bool {
        let Ok(mut runtime) = self.inner.runtime.lock() else {
            return false;
        };
        runtime.latest_progress.insert(id, progress);
        runtime
            .progress_persisted_at
            .get(&id)
            .is_none_or(|last| last.elapsed() >= PROGRESS_PERSIST_INTERVAL)
    }

    fn mark_progress_persisted(&self, id: DownloadTaskId) {
        if let Ok(mut runtime) = self.inner.runtime.lock() {
            runtime.progress_persisted_at.insert(id, Instant::now());
        }
    }

    fn clear_runtime_progress(&self, id: DownloadTaskId) {
        if let Ok(mut runtime) = self.inner.runtime.lock() {
            runtime.latest_progress.remove(&id);
            runtime.progress_persisted_at.remove(&id);
        }
    }

    fn finish_running(&self, id: DownloadTaskId) {
        if let Ok(mut runtime) = self.inner.runtime.lock() {
            runtime.running.remove(&id);
            runtime.latest_progress.remove(&id);
            runtime.progress_persisted_at.remove(&id);
            self.inner.done.notify_all();
        }
    }

    fn publish_snapshot(&self) {
        let Ok(mut runtime) = self.inner.runtime.lock() else {
            return;
        };
        runtime.revision = runtime.revision.saturating_add(1);
        let revision = runtime.revision;
        drop(runtime);
        let Some(sink) = &self.inner.sink else {
            return;
        };
        if let Ok(snapshot) = self.build_snapshot(revision) {
            sink(snapshot);
        }
    }

    fn build_snapshot(&self, revision: u64) -> Result<DownloadSnapshot, DownloadError> {
        let mut tasks = DownloadRepository::new(&self.inner.database)
            .list()
            .map_err(|error| persistence_error(error.to_string()))?;
        if let Ok(runtime) = self.inner.runtime.lock() {
            for task in &mut tasks {
                if let Some(progress) = runtime.latest_progress.get(&task.id) {
                    task.expected_bytes = progress.total_bytes.or(progress.total_bytes_estimate);
                    task.downloaded_bytes = progress.downloaded_bytes;
                    task.progress_permille = if progress.status == "finished" {
                        1000
                    } else {
                        progress.progress_permille()
                    };
                    task.speed_bytes_per_second = progress.speed_bytes_per_second;
                    task.eta_seconds = progress.eta_seconds;
                }
            }
        }
        let settings = SettingsRepository::new(&self.inner.database)
            .get_snapshot()
            .map_err(|error| persistence_error(error.to_string()))?;
        let max_concurrent = DownloadRepository::new(&self.inner.database)
            .max_concurrent()
            .map_err(|error| persistence_error(error.to_string()))?;
        Ok(DownloadSnapshot {
            revision,
            tasks,
            max_concurrent,
            downloads_directory: settings.downloads_directory,
            tools: media_tools_snapshot(&self.inner.media_tools),
        })
    }
}

fn default_task_temp_root() -> PathBuf {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    local_app_data
        .join(APPLICATION_DATA_DIRECTORY)
        .join("cache")
        .join("downloads")
}

fn validate_download_provider(provider: ProviderKind) -> Result<ProviderKind, DownloadError> {
    match provider {
        ProviderKind::Youtube | ProviderKind::Soundcloud => Ok(provider),
        ProviderKind::Spotify => Err(invalid(
            DownloadErrorCode::UnsupportedProvider,
            "Spotify downloads are not supported",
        )),
        ProviderKind::Local => Err(invalid(
            DownloadErrorCode::UnsupportedProvider,
            "local sources are not provider downloads",
        )),
    }
}

fn non_empty(value: String, field: &'static str) -> Result<String, DownloadError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(invalid(
            DownloadErrorCode::InvalidRequest,
            &format!("{field} cannot be empty"),
        ));
    }
    Ok(value)
}

fn invalid(code: DownloadErrorCode, detail: &str) -> DownloadError {
    DownloadError::Invalid {
        code,
        detail: detail.to_owned(),
    }
}

fn persistence_error(detail: impl Into<String>) -> DownloadError {
    DownloadError::Persistence(detail.into())
}

fn filesystem_error(operation: &'static str, source: io::Error) -> DownloadError {
    DownloadError::Filesystem { operation, source }
}

fn validate_download_directory(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("the download directory must be an absolute path".to_owned());
    }
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err("the download directory cannot be a symlink or reparse point".to_owned());
        }
        if !metadata.is_dir() {
            return Err("the download directory is not a directory".to_owned());
        }
    } else {
        fs::create_dir_all(path)
            .map_err(|error| format!("the download directory could not be created: {error}"))?;
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("the download directory could not be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("the download directory is not a regular directory".to_owned());
    }
    fs::canonicalize(path)
        .map_err(|error| format!("the download directory could not be resolved: {error}"))
}

fn create_owned_task_temp(root: &Path, id: DownloadTaskId) -> io::Result<PathBuf> {
    ensure_owned_root(root)?;
    let task_root = root.join(id.to_string());
    if task_root.parent() != Some(root) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "task temporary path escaped its root",
        ));
    }
    if let Ok(metadata) = fs::symlink_metadata(&task_root) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "task temporary path is not an owned directory",
            ));
        }
    } else {
        fs::create_dir_all(&task_root)?;
    }
    Ok(task_root)
}

fn ensure_owned_root(root: &Path) -> io::Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(root) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "download cache root is not a regular directory",
            ));
        }
    } else {
        fs::create_dir_all(root)?;
    }
    Ok(())
}

fn cleanup_owned_task_temp(root: &Path, id: DownloadTaskId) -> io::Result<()> {
    ensure_owned_root(root)?;
    let task_root = root.join(id.to_string());
    if task_root.parent() != Some(root) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "task cleanup path escaped its root",
        ));
    }
    let Ok(metadata) = fs::symlink_metadata(&task_root) else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing to clean a reparse-point task directory",
        ));
    }
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "task cleanup target is not a directory",
        ));
    }
    fs::remove_dir_all(task_root)
}

pub fn build_download_args(
    task: &DownloadTask,
    task_root: &Path,
    ffmpeg: Option<&Path>,
) -> Vec<String> {
    let template = "download:SPOTDIY_PROGRESS\t%(progress.status)s\t%(progress.downloaded_bytes)s\t%(progress.total_bytes)s\t%(progress.total_bytes_estimate)s\t%(progress.speed)s\t%(progress.eta)s";
    let mut args = vec![
        "--no-config".to_owned(),
        "--no-playlist".to_owned(),
        "--newline".to_owned(),
        "--no-warnings".to_owned(),
        "--progress-template".to_owned(),
        template.to_owned(),
        "--print".to_owned(),
        "after_move:SPOTDIY_FILE\t%(filepath)s".to_owned(),
        "--output".to_owned(),
        task_root
            .join("media.%(ext)s")
            .to_string_lossy()
            .into_owned(),
        "--format".to_owned(),
    ];
    match task.mode {
        DownloadMode::Audio => args.push("bestaudio/best".to_owned()),
        DownloadMode::Video => {
            args.push("bv*+ba/b".to_owned());
            args.extend([
                "--merge-output-format".to_owned(),
                "mkv".to_owned(),
                "--ffmpeg-location".to_owned(),
                ffmpeg
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ]);
        }
    }
    args.push(task.canonical_url.clone());
    args
}

fn select_task_output(task_root: &Path, reported: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(reported) = reported {
        return validate_task_output(task_root, &reported);
    }
    let entries = fs::read_dir(task_root)
        .map_err(|error| format!("task output directory could not be read: {error}"))?;
    let mut candidates = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("task output entry could not be read: {error}"))?;
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("media."))
        {
            candidates.push(validate_task_output(task_root, &path)?);
        }
    }
    match candidates.as_slice() {
        [candidate] => Ok(candidate.clone()),
        [] => Err("yt-dlp did not produce a media file".to_owned()),
        _ => Err("yt-dlp produced more than one candidate media file".to_owned()),
    }
}

pub fn validate_task_output(task_root: &Path, path: &Path) -> Result<PathBuf, String> {
    if path.parent() != Some(task_root) {
        return Err("yt-dlp output escaped its owned task directory".to_owned());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "yt-dlp output has an invalid filename".to_owned())?;
    if !name.starts_with("media.") || name.contains(['/', '\\']) || name == "media." {
        return Err("yt-dlp output filename is not an owned media filename".to_owned());
    }
    let root_metadata = fs::symlink_metadata(task_root)
        .map_err(|error| format!("task directory could not be inspected: {error}"))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err("task directory is not a regular owned directory".to_owned());
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("yt-dlp output could not be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("yt-dlp output is not a regular file".to_owned());
    }
    let canonical_root = fs::canonicalize(task_root)
        .map_err(|error| format!("task directory could not be resolved: {error}"))?;
    let canonical_output = fs::canonicalize(path)
        .map_err(|error| format!("yt-dlp output could not be resolved: {error}"))?;
    if canonical_output.parent() != Some(canonical_root.as_path()) {
        return Err("yt-dlp output resolved outside its owned task directory".to_owned());
    }
    Ok(path.to_path_buf())
}

fn validate_final_output(destination: &Path, path: &Path) -> Result<PathBuf, String> {
    let destination = validate_download_directory(destination)?;
    if path.parent() != Some(destination.as_path()) {
        return Err("completed output is outside its configured download directory".to_owned());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "completed output has an invalid filename".to_owned())?;
    if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\']) {
        return Err("completed output filename is not trusted".to_owned());
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("completed output could not be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("completed output is not a regular file".to_owned());
    }
    let canonical_output = fs::canonicalize(path)
        .map_err(|error| format!("completed output could not be resolved: {error}"))?;
    if canonical_output.parent() != Some(destination.as_path()) {
        return Err("completed output resolved outside its configured directory".to_owned());
    }
    Ok(path.to_path_buf())
}

pub fn sanitize_filename_component(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    sanitized = sanitized.trim().trim_end_matches([' ', '.']).to_owned();
    if is_reserved_windows_name(&sanitized) {
        sanitized.insert(0, '_');
    }
    sanitized
}

pub fn final_filename_base(task: &DownloadTask) -> String {
    let artist = sanitize_filename_component(&task.artists.join(", "));
    let title = sanitize_filename_component(&task.title);
    let provider_id = sanitize_filename_component(&task.provider_item_id);
    let base = if !artist.is_empty() && !title.is_empty() {
        format!(
            "{artist} - {title} [{}-{provider_id}]",
            task.provider_kind.as_str()
        )
    } else {
        format!("{}-{provider_id}", task.provider_kind.as_str())
    };
    let mut bounded = base.chars().take(MAX_FILENAME_CHARS).collect::<String>();
    bounded = bounded.trim_end_matches([' ', '.']).to_owned();
    if bounded.is_empty() {
        format!("{}-item", task.provider_kind.as_str())
    } else {
        bounded
    }
}

fn is_reserved_windows_name(value: &str) -> bool {
    let stem = value.split('.').next().unwrap_or_default();
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
    ) || (stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.as_bytes().last().is_some_and(u8::is_ascii_digit)
        && stem.as_bytes()[3] != b'0')
}

fn output_extension(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    (!extension.is_empty()
        && extension.len() <= 10
        && extension
            .chars()
            .all(|character| character.is_ascii_alphanumeric()))
    .then_some(extension)
}

fn finalize_download_output(
    source: &Path,
    destination: &Path,
    task: &DownloadTask,
) -> io::Result<PathBuf> {
    let extension = output_extension(source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "download output has no safe extension",
        )
    })?;
    let destination = validate_download_directory(destination)
        .map_err(|detail| io::Error::new(io::ErrorKind::InvalidInput, detail))?;
    let base = final_filename_base(task);
    for attempt in 1..=MAX_COLLISION_ATTEMPTS {
        let suffix = if attempt == 1 {
            String::new()
        } else {
            format!(" ({attempt})")
        };
        let final_path = destination.join(format!("{base}{suffix}.{extension}"));
        if final_path.parent() != Some(destination.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "final output escaped its destination",
            ));
        }
        if fs::symlink_metadata(&final_path).is_ok() {
            continue;
        }
        let temp_path = destination.join(format!(".spotdiy-download-{}-{attempt}.tmp", task.id));
        if fs::symlink_metadata(&temp_path).is_ok() {
            continue;
        }
        let result: io::Result<PathBuf> = (|| {
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            let mut input = File::open(source)?;
            io::copy(&mut input, &mut output)?;
            output.flush()?;
            output.sync_all()?;
            drop(output);
            fs::rename(&temp_path, &final_path)?;
            Ok(final_path.clone())
        })();
        match result {
            Ok(path) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temp_path);
            }
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                return Err(error);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "download filename collision limit reached",
    ))
}

fn ready_yt_dlp(status: &YtDlpToolStatus) -> Option<PathBuf> {
    (status.status == crate::search::types::ProviderRuntimeStatus::Ready)
        .then(|| status.executable.clone())
        .flatten()
}

fn ready_ffmpeg(status: &FfmpegToolStatus) -> Option<PathBuf> {
    (status.health == MediaToolHealth::Ready)
        .then(|| status.executable.clone())
        .flatten()
}

fn tool_error_code(status: crate::search::types::ProviderRuntimeStatus) -> DownloadErrorCode {
    match status {
        crate::search::types::ProviderRuntimeStatus::Missing
        | crate::search::types::ProviderRuntimeStatus::Unsupported => {
            DownloadErrorCode::ToolMissing
        }
        _ => DownloadErrorCode::ToolBroken,
    }
}

fn media_tools_snapshot(media_tools: &MediaToolManager) -> MediaToolsSnapshot {
    let yt_dlp = media_tools.yt_dlp_status();
    let ffmpeg = media_tools.ffmpeg_status();
    MediaToolsSnapshot {
        yt_dlp: DownloadToolStatus {
            status: yt_dlp.status,
            version: yt_dlp.version,
            detail: yt_dlp.detail,
        },
        ffmpeg: DownloadToolStatus {
            status: ffmpeg.health_as_runtime_status(),
            version: ffmpeg.version,
            detail: ffmpeg.detail,
        },
    }
}

trait FfmpegRuntimeStatus {
    fn health_as_runtime_status(&self) -> crate::search::types::ProviderRuntimeStatus;
}

impl FfmpegRuntimeStatus for FfmpegToolStatus {
    fn health_as_runtime_status(&self) -> crate::search::types::ProviderRuntimeStatus {
        match self.health {
            MediaToolHealth::Ready => crate::search::types::ProviderRuntimeStatus::Ready,
            MediaToolHealth::Missing => crate::search::types::ProviderRuntimeStatus::Missing,
            MediaToolHealth::Broken => crate::search::types::ProviderRuntimeStatus::Broken,
        }
    }
}

fn process_error(error: &YtDlpDownloadProcessError) -> (DownloadErrorCode, String) {
    match error {
        YtDlpDownloadProcessError::Cancelled => (
            DownloadErrorCode::Cancelled,
            "download was cancelled".to_owned(),
        ),
        YtDlpDownloadProcessError::Spawn => (
            DownloadErrorCode::ToolBroken,
            "yt-dlp could not be started".to_owned(),
        ),
        YtDlpDownloadProcessError::Read
        | YtDlpDownloadProcessError::StdoutLineTooLong
        | YtDlpDownloadProcessError::StderrLineTooLong => {
            (DownloadErrorCode::ProcessFailed, error.to_string())
        }
        YtDlpDownloadProcessError::NonZeroExit { .. } => (
            DownloadErrorCode::ProcessFailed,
            "yt-dlp exited unsuccessfully".to_owned(),
        ),
    }
}

fn bounded_detail(detail: String) -> Option<String> {
    if detail.trim().is_empty() {
        return None;
    }
    Some(detail.chars().take(MAX_ERROR_DETAIL_LENGTH).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;

    use crate::db::{Database, TempDatabasePath};
    use crate::media_tools::{FfmpegToolStatus, YtDlpToolStatus};
    use crate::search::types::{SafeUrl, SearchEntityKind};
    use crate::settings::{SettingValue, SettingsRepository};

    #[derive(Clone, Copy)]
    enum FakeBehavior {
        Complete,
        DelayedComplete,
        Fail,
        WaitForCancellation,
    }

    struct FakeDownloadRunner {
        behaviors: StdMutex<Vec<FakeBehavior>>,
        starts: Arc<AtomicUsize>,
        active: Arc<AtomicUsize>,
        max_active: Arc<AtomicUsize>,
    }

    impl FakeDownloadRunner {
        fn new(behaviors: Vec<FakeBehavior>) -> Arc<Self> {
            Arc::new(Self {
                behaviors: StdMutex::new(behaviors),
                starts: Arc::new(AtomicUsize::new(0)),
                active: Arc::new(AtomicUsize::new(0)),
                max_active: Arc::new(AtomicUsize::new(0)),
            })
        }

        fn next_behavior(&self) -> FakeBehavior {
            self.behaviors
                .lock()
                .unwrap()
                .pop()
                .unwrap_or(FakeBehavior::Complete)
        }

        fn output_path(args: &[String]) -> PathBuf {
            let output = args
                .windows(2)
                .find(|window| window[0] == "--output")
                .map(|window| window[1].replace("%(ext)s", "webm"))
                .expect("fake download has an output argument");
            PathBuf::from(output)
        }
    }

    impl YtDlpDownloadRunner for FakeDownloadRunner {
        fn run_download<'a>(
            &'a self,
            _executable: &'a Path,
            args: &'a [String],
            cancellation: SearchCancellation,
            events: mpsc::Sender<YtDlpDownloadEvent>,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<YtDlpDownloadProcessOutput, YtDlpDownloadProcessError>>
                    + Send
                    + 'a,
            >,
        > {
            let behavior = self.next_behavior();
            let output = Self::output_path(args);
            let starts = self.starts.clone();
            let active = self.active.clone();
            let max_active = self.max_active.clone();
            Box::pin(async move {
                starts.fetch_add(1, Ordering::Relaxed);
                let current = active.fetch_add(1, Ordering::Relaxed) + 1;
                max_active.fetch_max(current, Ordering::Relaxed);
                let result = match behavior {
                    FakeBehavior::Complete => {
                        fake_complete(&events, &output).await;
                        Ok(YtDlpDownloadProcessOutput {
                            exit_code: Some(0),
                            diagnostic: String::new(),
                        })
                    }
                    FakeBehavior::DelayedComplete => {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        fake_complete(&events, &output).await;
                        Ok(YtDlpDownloadProcessOutput {
                            exit_code: Some(0),
                            diagnostic: String::new(),
                        })
                    }
                    FakeBehavior::Fail => Err(YtDlpDownloadProcessError::NonZeroExit {
                        code: Some(7),
                        diagnostic: "fixture failure".to_owned(),
                    }),
                    FakeBehavior::WaitForCancellation => {
                        let mut receiver = cancellation.subscribe();
                        while !*receiver.borrow() {
                            if receiver.changed().await.is_err() {
                                break;
                            }
                        }
                        Err(YtDlpDownloadProcessError::Cancelled)
                    }
                };
                active.fetch_sub(1, Ordering::Relaxed);
                result
            })
        }
    }

    async fn fake_complete(events: &mpsc::Sender<YtDlpDownloadEvent>, output: &Path) {
        let line = "SPOTDIY_PROGRESS\tfinished\t4\t4\tNA\tNA\tNA".to_owned();
        let _ = events.send(YtDlpDownloadEvent::StdoutLine(line)).await;
        fs::write(output, b"fixture").unwrap();
        let _ = events
            .send(YtDlpDownloadEvent::StdoutLine(format!(
                "SPOTDIY_FILE\t{}",
                output.display()
            )))
            .await;
    }

    fn ready_media_tools() -> MediaToolManager {
        MediaToolManager::with_test_statuses(
            YtDlpToolStatus {
                status: crate::search::types::ProviderRuntimeStatus::Ready,
                executable: Some(PathBuf::from("yt-dlp")),
                version: Some("2026.08.19".to_owned()),
                detail: None,
            },
            FfmpegToolStatus {
                health: MediaToolHealth::Ready,
                executable: Some(PathBuf::from("ffmpeg")),
                version: Some("9.0.1".to_owned()),
                detail: None,
            },
        )
    }

    fn safe_url(value: &str) -> SafeUrl {
        serde_json::from_str(&format!("\"{value}\"")).unwrap()
    }

    fn search_result(provider: ProviderKind) -> SearchResult {
        SearchResult {
            provider,
            entity_kind: SearchEntityKind::Track,
            provider_item_id: "fixture-id".to_owned(),
            canonical_url: Some(safe_url(match provider {
                ProviderKind::Youtube => "https://www.youtube.com/watch?v=fixture-id",
                ProviderKind::Soundcloud => "https://soundcloud.com/artist/fixture-id",
                ProviderKind::Spotify => "https://open.spotify.com/track/fixture-id",
                ProviderKind::Local => "https://www.youtube.com/watch?v=fixture-id",
            })),
            title: "Fixture title".to_owned(),
            artists: vec!["Fixture artist".to_owned()],
            album: None,
            duration_ms: Some(4_000),
            artwork_url: None,
            published_at: None,
            engagement_count: None,
            engagement_kind: None,
            explicit: None,
            local_track_id: None,
            local_source_id: None,
            original_rank: 1,
        }
    }

    fn configure_downloads(database: &Database, path: &Path) {
        SettingsRepository::new(database)
            .set_setting(SettingValue::DownloadsDirectory(Some(path.to_path_buf())))
            .unwrap();
    }

    async fn wait_for_state(
        service: &DownloadService,
        id: DownloadTaskId,
        expected: DownloadState,
    ) -> DownloadTask {
        for _ in 0..200 {
            if let Some(task) = service
                .snapshot()
                .unwrap()
                .tasks
                .into_iter()
                .find(|task| task.id == id)
            {
                if task.state == expected {
                    return task;
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("download task did not reach {expected:?}");
    }

    fn task() -> DownloadTask {
        DownloadTask::from_request(
            DownloadRequest {
                provider_kind: ProviderKind::Youtube,
                provider_item_id: "id/with:bad".to_owned(),
                canonical_url: "https://www.youtube.com/watch?v=id".to_owned(),
                target_track_id: None,
                target_source_id: None,
                title: "CON: signal?".to_owned(),
                artists: vec!["Artist".to_owned()],
                artwork_url: None,
                mode: DownloadMode::Audio,
            },
            std::env::temp_dir(),
        )
    }

    #[test]
    fn argv_is_structured_and_keeps_audio_provider_encoding() {
        let task = task();
        let args = build_download_args(&task, Path::new(r"C:\owned\task"), None);
        assert!(args.contains(&"--no-config".to_owned()));
        assert!(args.contains(&"--no-playlist".to_owned()));
        assert!(args.contains(&"bestaudio/best".to_owned()));
        assert!(!args.iter().any(|argument| argument == "flac"));
        assert_eq!(args.last(), Some(&task.canonical_url));
    }

    #[test]
    fn video_argv_requires_ffmpeg_and_uses_mkv_without_reencoding() {
        let mut task = task();
        task.mode = DownloadMode::Video;
        let args = build_download_args(
            &task,
            Path::new(r"C:\owned\task"),
            Some(Path::new(r"C:\tools\ffmpeg.exe")),
        );
        assert!(args
            .windows(2)
            .any(|window| window == ["--merge-output-format", "mkv"]));
        assert!(args
            .windows(2)
            .any(|window| window == ["--ffmpeg-location", r"C:\tools\ffmpeg.exe"]));
        assert!(args.contains(&"bv*+ba/b".to_owned()));
    }

    #[test]
    fn filename_sanitization_handles_invalid_and_reserved_names() {
        let task = task();
        let base = final_filename_base(&task);
        assert!(!base.contains(':'));
        assert!(!base.contains('?'));
        assert!(base.contains("_"));
        assert!(sanitize_filename_component("CON") != "CON");
        assert!(sanitize_filename_component("name... ").ends_with("name"));
    }

    #[test]
    fn task_output_rejects_escape_absolute_and_symlink_paths() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("task");
        fs::create_dir_all(&root).unwrap();
        let output = root.join("media.webm");
        File::create(&output).unwrap();
        assert!(validate_task_output(&root, &output).is_ok());
        assert!(validate_task_output(&root, &directory.path().join("media.webm")).is_err());
        assert!(validate_task_output(&root, &root.join("..\u{5c}media.webm")).is_err());
    }

    #[test]
    fn finalization_avoids_existing_files_and_uses_destination_side_temp() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("media.webm");
        fs::write(&source, b"data").unwrap();
        let destination = directory.path().join("destination");
        fs::create_dir_all(&destination).unwrap();
        let task = DownloadTask::from_request(
            DownloadRequest {
                provider_kind: ProviderKind::Youtube,
                provider_item_id: "id".to_owned(),
                canonical_url: "https://www.youtube.com/watch?v=id".to_owned(),
                target_track_id: None,
                target_source_id: None,
                title: "Title".to_owned(),
                artists: vec!["Artist".to_owned()],
                artwork_url: None,
                mode: DownloadMode::Audio,
            },
            destination.clone(),
        );
        let first = finalize_download_output(&source, &destination, &task).unwrap();
        let second = finalize_download_output(&source, &destination, &task).unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(first).unwrap(), b"data");
        assert_eq!(fs::read(second).unwrap(), b"data");
        assert!(!fs::read_dir(&destination).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
    }

    #[test]
    fn no_fake_lossless_provenance_is_available() {
        let task = task();
        assert_eq!(
            task.source_quality_provenance,
            SourceQualityProvenance::ProviderEncoded
        );
        assert!(!task.transcoded);
    }

    #[test]
    fn spotify_and_local_queue_requests_are_rejected() {
        assert!(validate_download_provider(ProviderKind::Spotify).is_err());
        assert!(validate_download_provider(ProviderKind::Local).is_err());
    }

    #[test]
    fn migration_service_snapshot_starts_with_default_concurrency() {
        let path = TempDatabasePath::new("download-service-snapshot");
        let database = Database::open(path.path()).unwrap();
        let service = DownloadService::with_task_root(
            database,
            MediaToolManager::with_yt_dlp_override(PathBuf::from("missing")),
            path.path().with_extension("downloads"),
            None,
        )
        .unwrap();
        assert_eq!(
            service.snapshot().unwrap().max_concurrent,
            DEFAULT_MAX_CONCURRENT
        );
    }

    #[tokio::test]
    async fn scheduler_completes_and_finalizes_a_provider_download() {
        let database_path = TempDatabasePath::new("download-service-complete");
        let database = Database::open(database_path.path()).unwrap();
        let destination = tempfile::tempdir().unwrap();
        configure_downloads(&database, destination.path());
        let cache = tempfile::tempdir().unwrap();
        let runner = FakeDownloadRunner::new(vec![FakeBehavior::Complete]);
        let service = DownloadService::with_runner(
            database,
            ready_media_tools(),
            cache.path().to_path_buf(),
            runner.clone(),
            None,
        )
        .unwrap();

        let queued = service
            .queue_search_result_download(search_result(ProviderKind::Youtube), DownloadMode::Audio)
            .unwrap();
        let completed = wait_for_state(&service, queued.id, DownloadState::Completed).await;
        let output = completed.output_path.expect("completed output path");
        assert!(output.is_file());
        assert_eq!(
            completed.source_quality_provenance,
            SourceQualityProvenance::ProviderEncoded
        );
        assert!(!completed.transcoded);
        assert!(!cache.path().join(queued.id.to_string()).exists());
        assert_eq!(runner.starts.load(Ordering::Relaxed), 1);
        service.shutdown_async().await.unwrap();
    }

    #[tokio::test]
    async fn scheduler_honors_concurrency_and_active_cancellation() {
        let database_path = TempDatabasePath::new("download-service-concurrency");
        let database = Database::open(database_path.path()).unwrap();
        let destination = tempfile::tempdir().unwrap();
        configure_downloads(&database, destination.path());
        let cache = tempfile::tempdir().unwrap();
        let runner = FakeDownloadRunner::new(vec![
            FakeBehavior::DelayedComplete,
            FakeBehavior::DelayedComplete,
        ]);
        let service = DownloadService::with_runner(
            database,
            ready_media_tools(),
            cache.path().to_path_buf(),
            runner.clone(),
            None,
        )
        .unwrap();
        service.set_download_concurrency(2).unwrap();
        let first = service
            .queue_search_result_download(search_result(ProviderKind::Youtube), DownloadMode::Audio)
            .unwrap();
        let second = service
            .queue_search_result_download(
                search_result(ProviderKind::Soundcloud),
                DownloadMode::Audio,
            )
            .unwrap();
        wait_for_state(&service, first.id, DownloadState::Completed).await;
        wait_for_state(&service, second.id, DownloadState::Completed).await;
        assert_eq!(runner.max_active.load(Ordering::Relaxed), 2);
        service.shutdown_async().await.unwrap();

        let database_path = TempDatabasePath::new("download-service-cancel");
        let database = Database::open(database_path.path()).unwrap();
        let destination = tempfile::tempdir().unwrap();
        configure_downloads(&database, destination.path());
        let cache = tempfile::tempdir().unwrap();
        let runner = FakeDownloadRunner::new(vec![FakeBehavior::WaitForCancellation]);
        let service = DownloadService::with_runner(
            database,
            ready_media_tools(),
            cache.path().to_path_buf(),
            runner,
            None,
        )
        .unwrap();
        let queued = service
            .queue_search_result_download(search_result(ProviderKind::Youtube), DownloadMode::Audio)
            .unwrap();
        wait_for_state(&service, queued.id, DownloadState::Downloading).await;
        service.cancel_download(queued.id).unwrap();
        let cancelled = wait_for_state(&service, queued.id, DownloadState::Cancelled).await;
        assert_eq!(cancelled.error_code, Some(DownloadErrorCode::Cancelled));
        assert!(!cache.path().join(queued.id.to_string()).exists());
        service.shutdown_async().await.unwrap();
    }

    #[tokio::test]
    async fn failed_download_can_be_retried_without_reusing_output() {
        let database_path = TempDatabasePath::new("download-service-retry");
        let database = Database::open(database_path.path()).unwrap();
        let destination = tempfile::tempdir().unwrap();
        configure_downloads(&database, destination.path());
        let cache = tempfile::tempdir().unwrap();
        let runner = FakeDownloadRunner::new(vec![FakeBehavior::Complete, FakeBehavior::Fail]);
        let service = DownloadService::with_runner(
            database,
            ready_media_tools(),
            cache.path().to_path_buf(),
            runner,
            None,
        )
        .unwrap();
        let queued = service
            .queue_search_result_download(search_result(ProviderKind::Youtube), DownloadMode::Audio)
            .unwrap();
        wait_for_state(&service, queued.id, DownloadState::Failed).await;
        let retry = service.retry_download(queued.id).unwrap();
        assert_eq!(retry.retry_count, 1);
        let completed = wait_for_state(&service, queued.id, DownloadState::Completed).await;
        assert!(completed.output_path.is_some());
        service.shutdown_async().await.unwrap();
    }

    #[test]
    fn service_startup_recovery_requeues_and_cleans_owned_task_cache() {
        let database_path = TempDatabasePath::new("download-service-recovery");
        let database = Database::open(database_path.path()).unwrap();
        let destination = tempfile::tempdir().unwrap();
        configure_downloads(&database, destination.path());
        let cache = tempfile::tempdir().unwrap();
        let mut interrupted = task();
        interrupted.destination_directory = destination.path().to_path_buf();
        interrupted.transition(DownloadState::Resolving).unwrap();
        let id = interrupted.id;
        DownloadRepository::new(&database)
            .insert(&interrupted)
            .unwrap();
        let owned = cache.path().join(id.to_string());
        fs::create_dir_all(&owned).unwrap();
        fs::write(owned.join("partial.webm"), b"partial").unwrap();

        let service = DownloadService::with_runner(
            database,
            ready_media_tools(),
            cache.path().to_path_buf(),
            FakeDownloadRunner::new(vec![]),
            None,
        )
        .unwrap();
        let recovered = service
            .snapshot()
            .unwrap()
            .tasks
            .into_iter()
            .find(|task| task.id == id)
            .unwrap();
        assert_eq!(recovered.state, DownloadState::Queued);
        assert!(!owned.exists());
    }

    #[test]
    fn queue_rejects_spotify_and_non_track_results_without_creating_tasks() {
        let database_path = TempDatabasePath::new("download-service-rejections");
        let database = Database::open(database_path.path()).unwrap();
        let destination = tempfile::tempdir().unwrap();
        configure_downloads(&database, destination.path());
        let service = DownloadService::with_task_root(
            database,
            ready_media_tools(),
            tempfile::tempdir().unwrap().path().to_path_buf(),
            None,
        )
        .unwrap();
        let error = service
            .queue_search_result_download(search_result(ProviderKind::Spotify), DownloadMode::Audio)
            .unwrap_err();
        assert!(matches!(
            error,
            DownloadError::Invalid {
                code: DownloadErrorCode::UnsupportedProvider,
                ..
            }
        ));
        let mut artist = search_result(ProviderKind::Youtube);
        artist.entity_kind = SearchEntityKind::Artist;
        assert!(service
            .queue_search_result_download(artist, DownloadMode::Audio)
            .is_err());
        assert!(service.snapshot().unwrap().tasks.is_empty());
    }

    #[test]
    fn bounded_detail_does_not_retain_unlimited_diagnostics() {
        assert_eq!(
            bounded_detail("x".repeat(MAX_ERROR_DETAIL_LENGTH + 1))
                .unwrap()
                .len(),
            MAX_ERROR_DETAIL_LENGTH
        );
    }

    #[test]
    fn runner_output_type_is_sendable_for_scheduler_boundary() {
        fn assert_send<T: Send>() {}
        assert_send::<YtDlpDownloadProcessOutput>();
    }

    #[test]
    fn collision_attempts_are_bounded() {
        assert_eq!(MAX_COLLISION_ATTEMPTS, 1000);
    }
}
