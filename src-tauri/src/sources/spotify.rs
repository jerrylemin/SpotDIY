use serde::Deserialize;
use std::collections::HashSet;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::{oneshot, watch};
use url::Url;

use crate::domain::{ProviderKind, SourceCapabilities};
use crate::search::types::{
    ProviderRuntimeStatus, ProviderSearchErrorCode, ProviderSearchRequest, ProviderSearchSection,
    SafeUrl, SearchCancellation, SearchEntityKind, SearchResult,
};
use crate::sources::{
    cancelled_provider_section, failed_provider_section, is_cancelled, ready_provider_section,
    sanitize_artwork_url, validate_provider_url, SourceAdapter,
};

const SUPPORTED_ENTITIES: &[SearchEntityKind] = &[SearchEntityKind::Track];
const SPOTDL_COMMAND_TIMEOUT: Duration = Duration::from_secs(90);
const SPOTDL_OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
const SPOTDL_MAX_RETRIES: &str = "1";
const SPOTDL_PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Spotify results remain metadata records. The actual audio download is
/// resolved by spotdl to a permitted YouTube/SoundCloud source and then goes
/// through the existing yt-dlp download worker.
pub(crate) const SPOTIFY_SOURCE_CAPABILITIES: SourceCapabilities = SourceCapabilities {
    search: true,
    playback: false,
    metadata: true,
    artwork: true,
    lyrics: false,
    downloads: false,
    popularity: false,
    release_date: true,
    lyrics_metadata: false,
};

/// Capabilities shown for a search result. This is deliberately separate from
/// persisted Spotify source capabilities: a result can start a source-matched
/// download without pretending that Spotify exposes a downloadable stream.
pub(crate) const SPOTIFY_RESULT_CAPABILITIES: SourceCapabilities = SourceCapabilities {
    downloads: true,
    ..SPOTIFY_SOURCE_CAPABILITIES
};

#[derive(Clone, Debug, Eq, PartialEq, Error)]
enum SpotDlProcessError {
    #[error("spotdl was cancelled")]
    Cancelled,
    #[error("spotdl could not be started")]
    Spawn,
    #[error("spotdl timed out")]
    Timeout,
    #[error("spotdl returned an unsuccessful result")]
    Failed,
    #[error("spotdl returned too much output")]
    OutputTooLarge,
    #[error("spotdl process state was unavailable")]
    Join,
}

#[derive(Clone, Debug)]
struct SpotDlOutput {
    stdout: String,
}

trait SpotDlRunner: Send + Sync {
    fn run<'a>(
        &'a self,
        executable: &'a Path,
        args: &'a [String],
        cancellation: SearchCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<SpotDlOutput, SpotDlProcessError>> + Send + 'a>>;
}

#[derive(Clone, Copy, Default)]
struct ProcessSpotDlRunner;

impl SpotDlRunner for ProcessSpotDlRunner {
    fn run<'a>(
        &'a self,
        executable: &'a Path,
        args: &'a [String],
        cancellation: SearchCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<SpotDlOutput, SpotDlProcessError>> + Send + 'a>> {
        Box::pin(run_spotdl_process(executable, args, cancellation))
    }
}

async fn run_spotdl_process(
    executable: &Path,
    args: &[String],
    cancellation: SearchCancellation,
) -> Result<SpotDlOutput, SpotDlProcessError> {
    let mut cancellation_rx = cancellation.subscribe();
    if *cancellation_rx.borrow() {
        return Err(SpotDlProcessError::Cancelled);
    }
    let mut child = Command::new(executable)
        .args(args)
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| SpotDlProcessError::Spawn)?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            terminate_spotdl_child(&mut child).await;
            return Err(SpotDlProcessError::Join);
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            terminate_spotdl_child(&mut child).await;
            return Err(SpotDlProcessError::Join);
        }
    };
    let (stdout_tx, mut stdout_rx) = oneshot::channel();
    let (stderr_tx, mut stderr_rx) = oneshot::channel();
    tokio::spawn(async move {
        let _ = stdout_tx.send(read_spotdl_output(stdout).await);
    });
    tokio::spawn(async move {
        let _ = stderr_tx.send(read_spotdl_output(stderr).await);
    });

    let mut stdout_result: Option<Vec<u8>> = None;
    let mut stderr_result: Option<Vec<u8>> = None;
    let mut exit_status = None;
    let deadline = tokio::time::sleep(SPOTDL_COMMAND_TIMEOUT);
    tokio::pin!(deadline);
    let mut poll = tokio::time::interval(SPOTDL_PROCESS_POLL_INTERVAL);
    loop {
        if exit_status.is_none() {
            exit_status = match child.try_wait() {
                Ok(status) => status,
                Err(_) => {
                    terminate_spotdl_child(&mut child).await;
                    return Err(SpotDlProcessError::Join);
                }
            };
        }
        if exit_status.is_some() && stdout_result.is_some() && stderr_result.is_some() {
            let status = exit_status
                .take()
                .expect("exit status was checked before taking it");
            let stdout = stdout_result
                .take()
                .expect("stdout result was checked before taking it");
            let _stderr = stderr_result
                .take()
                .expect("stderr result was checked before taking it");
            return if status.success() {
                Ok(SpotDlOutput {
                    stdout: String::from_utf8_lossy(&stdout).into_owned(),
                })
            } else {
                Err(SpotDlProcessError::Failed)
            };
        }
        tokio::select! {
            result = &mut stdout_rx, if stdout_result.is_none() => {
                match result {
                    Ok(Ok(output)) => stdout_result = Some(output),
                    Ok(Err(error)) => {
                        terminate_spotdl_child(&mut child).await;
                        return Err(error);
                    }
                    Err(_) => {
                        terminate_spotdl_child(&mut child).await;
                        return Err(SpotDlProcessError::Join);
                    }
                }
            }
            result = &mut stderr_rx, if stderr_result.is_none() => {
                match result {
                    Ok(Ok(output)) => stderr_result = Some(output),
                    Ok(Err(error)) => {
                        terminate_spotdl_child(&mut child).await;
                        return Err(error);
                    }
                    Err(_) => {
                        terminate_spotdl_child(&mut child).await;
                        return Err(SpotDlProcessError::Join);
                    }
                }
            }
            _ = wait_for_cancellation(&mut cancellation_rx) => {
                terminate_spotdl_child(&mut child).await;
                return Err(SpotDlProcessError::Cancelled);
            }
            _ = &mut deadline => {
                terminate_spotdl_child(&mut child).await;
                return Err(SpotDlProcessError::Timeout);
            }
            _ = poll.tick() => {}
        }
    }
}

async fn read_spotdl_output<R>(mut reader: R) -> Result<Vec<u8>, SpotDlProcessError>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::with_capacity(SPOTDL_OUTPUT_LIMIT.min(8 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .await
            .map_err(|_| SpotDlProcessError::Join)?;
        if bytes_read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(bytes_read) > SPOTDL_OUTPUT_LIMIT {
            return Err(SpotDlProcessError::OutputTooLarge);
        }
        output.extend_from_slice(&buffer[..bytes_read]);
    }
}

async fn terminate_spotdl_child(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

async fn wait_for_cancellation(receiver: &mut watch::Receiver<bool>) {
    if *receiver.borrow() {
        return;
    }
    while receiver.changed().await.is_ok() {
        if *receiver.borrow() {
            return;
        }
    }
    std::future::pending::<()>().await;
}

/// Finds the local spotdl executable without requiring a Spotify developer
/// application or storing any Spotify credentials.
pub(crate) fn spotdl_executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SPOTDIY_SPOTDL_PATH").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }
    let path_entries = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path_entries) {
        for name in spotdl_executable_names() {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(windows)]
fn spotdl_executable_names() -> &'static [&'static str] {
    &["spotdl.exe", "spotdl"]
}

#[cfg(not(windows))]
fn spotdl_executable_names() -> &'static [&'static str] {
    &["spotdl"]
}

pub(crate) fn spotdl_runtime_status() -> ProviderRuntimeStatus {
    if spotdl_executable().is_some() {
        ProviderRuntimeStatus::Ready
    } else {
        ProviderRuntimeStatus::Missing
    }
}

fn executable_for_adapter(override_path: Option<&Path>) -> Option<PathBuf> {
    override_path
        .map(Path::to_path_buf)
        .or_else(spotdl_executable)
}

fn save_args(query: &str) -> Vec<String> {
    vec![
        "save".to_owned(),
        query.to_owned(),
        "--save-file".to_owned(),
        "-".to_owned(),
        "--no-cache".to_owned(),
        "--headless".to_owned(),
        "--max-retries".to_owned(),
        SPOTDL_MAX_RETRIES.to_owned(),
        "--log-level".to_owned(),
        "WARNING".to_owned(),
    ]
}

fn url_args(query: &str) -> Vec<String> {
    vec![
        "url".to_owned(),
        query.to_owned(),
        "--no-cache".to_owned(),
        "--headless".to_owned(),
        "--max-retries".to_owned(),
        SPOTDL_MAX_RETRIES.to_owned(),
        "--log-level".to_owned(),
        "WARNING".to_owned(),
    ]
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
enum SpotifyQueryError {
    #[error("a Spotify track URL is required")]
    InvalidTrackUrl,
}

fn valid_spotify_id(value: &str) -> bool {
    let length = value.chars().count();
    (1..=128).contains(&length)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn spotify_track_id(value: &str) -> Result<String, SpotifyQueryError> {
    let url = Url::parse(value).map_err(|_| SpotifyQueryError::InvalidTrackUrl)?;
    validate_provider_url(ProviderKind::Spotify, value)
        .map_err(|_| SpotifyQueryError::InvalidTrackUrl)?;
    let mut segments = url
        .path_segments()
        .ok_or(SpotifyQueryError::InvalidTrackUrl)?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    while segments
        .first()
        .is_some_and(|segment| segment.starts_with("intl-"))
    {
        segments.remove(0);
    }
    if segments.first() == Some(&"embed") {
        segments.remove(0);
    }
    if segments.len() != 2 || segments[0] != "track" || !valid_spotify_id(segments[1]) {
        return Err(SpotifyQueryError::InvalidTrackUrl);
    }
    Ok(segments[1].to_owned())
}

fn canonical_spotify_url(id: &str) -> Result<SafeUrl, SpotifyQueryError> {
    if !valid_spotify_id(id) {
        return Err(SpotifyQueryError::InvalidTrackUrl);
    }
    let value = format!("https://open.spotify.com/track/{id}");
    validate_provider_url(ProviderKind::Spotify, &value)
        .map_err(|_| SpotifyQueryError::InvalidTrackUrl)
}

#[derive(Debug, Deserialize)]
struct SpotDlSong {
    name: Option<String>,
    artists: Option<Vec<String>>,
    artist: Option<String>,
    album_name: Option<String>,
    duration: Option<f64>,
    date: Option<String>,
    song_id: Option<String>,
    url: Option<String>,
    cover_url: Option<String>,
    explicit: Option<bool>,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse_partial_date(value: Option<String>) -> Option<crate::search::types::PartialDate> {
    let value = value?.trim().to_owned();
    match value.len() {
        4 => crate::search::types::PartialDate::new(
            value,
            crate::search::types::PartialDatePrecision::Year,
        )
        .ok(),
        7 => crate::search::types::PartialDate::new(
            value,
            crate::search::types::PartialDatePrecision::Month,
        )
        .ok(),
        10 => crate::search::types::PartialDate::new(
            value,
            crate::search::types::PartialDatePrecision::Day,
        )
        .ok(),
        _ => None,
    }
}

fn duration_ms(value: Option<f64>) -> Option<u64> {
    let seconds = value?;
    (seconds.is_finite() && seconds >= 0.0 && seconds <= (u64::MAX as f64) / 1_000.0)
        .then_some((seconds * 1_000.0) as u64)
}

fn parse_spotdl_results(
    stdout: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, ProviderSearchErrorCode> {
    let start = stdout
        .find('[')
        .ok_or(ProviderSearchErrorCode::InvalidResponse)?;
    let end = stdout
        .rfind(']')
        .filter(|end| *end >= start)
        .ok_or(ProviderSearchErrorCode::InvalidResponse)?;
    let songs = serde_json::from_str::<Vec<SpotDlSong>>(&stdout[start..=end])
        .map_err(|_| ProviderSearchErrorCode::InvalidResponse)?;
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for song in songs {
        let Some(title) = non_empty(song.name) else {
            continue;
        };
        let id = song.song_id.filter(|id| valid_spotify_id(id)).or_else(|| {
            song.url
                .as_deref()
                .and_then(|url| spotify_track_id(url).ok())
        });
        let Some(id) = id else {
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let canonical_url = canonical_spotify_url(&id).ok();
        let mut artists = song
            .artists
            .unwrap_or_default()
            .into_iter()
            .filter_map(|artist| non_empty(Some(artist)))
            .collect::<Vec<_>>();
        if artists.is_empty() {
            if let Some(artist) = non_empty(song.artist) {
                artists.push(artist);
            }
        }
        results.push(SearchResult {
            provider: ProviderKind::Spotify,
            entity_kind: SearchEntityKind::Track,
            provider_item_id: id,
            canonical_url,
            title,
            artists,
            album: non_empty(song.album_name),
            duration_ms: duration_ms(song.duration),
            artwork_url: song.cover_url.as_deref().and_then(sanitize_artwork_url),
            published_at: parse_partial_date(song.date),
            engagement_count: None,
            engagement_kind: None,
            explicit: song.explicit,
            local_track_id: None,
            local_source_id: None,
            original_rank: u32::try_from(results.len()).unwrap_or(u32::MAX),
        });
        if results.len() == limit {
            break;
        }
    }
    Ok(results)
}

pub struct SpotifySourceAdapter {
    runner: Arc<dyn SpotDlRunner>,
    #[cfg(test)]
    executable_override: Option<PathBuf>,
}

impl SpotifySourceAdapter {
    pub fn new() -> Self {
        Self {
            runner: Arc::new(ProcessSpotDlRunner),
            #[cfg(test)]
            executable_override: None,
        }
    }

    #[cfg(test)]
    fn with_runner_for_tests(runner: Arc<dyn SpotDlRunner>) -> Self {
        Self {
            runner,
            executable_override: Some(PathBuf::from("spotdl")),
        }
    }

    fn executable(&self) -> Option<PathBuf> {
        #[cfg(test)]
        if self.executable_override.is_some() {
            return executable_for_adapter(self.executable_override.as_deref());
        }
        executable_for_adapter(None)
    }
}

impl Default for SpotifySourceAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceAdapter for SpotifySourceAdapter {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Spotify
    }

    fn capabilities(&self) -> SourceCapabilities {
        SPOTIFY_SOURCE_CAPABILITIES
    }

    fn supported_entities(&self) -> &'static [SearchEntityKind] {
        SUPPORTED_ENTITIES
    }

    fn runtime_status(&self) -> ProviderRuntimeStatus {
        spotdl_runtime_status()
    }

    fn search(
        &self,
        request: ProviderSearchRequest,
        cancellation: SearchCancellation,
    ) -> Pin<Box<dyn Future<Output = ProviderSearchSection> + Send + '_>> {
        Box::pin(async move {
            if is_cancelled(&cancellation) {
                return cancelled_provider_section(ProviderKind::Spotify);
            }
            if request.limit == 0
                || request.query.trim().is_empty()
                || !request.entities.contains(&SearchEntityKind::Track)
            {
                return ready_provider_section(ProviderKind::Spotify, Vec::new());
            }
            let Some(executable) = self.executable() else {
                return failed_provider_section(
                    ProviderKind::Spotify,
                    ProviderSearchErrorCode::Unavailable,
                    Some("spotdl is not installed. Install spotdl to search and download Spotify tracks.".into()),
                );
            };
            let args = save_args(request.query.trim());
            match self.runner.run(&executable, &args, cancellation).await {
                Ok(output) => {
                    match parse_spotdl_results(&output.stdout, usize::from(request.limit)) {
                        Ok(results) => ready_provider_section(ProviderKind::Spotify, results),
                        Err(code) => failed_provider_section(
                            ProviderKind::Spotify,
                            code,
                            Some("spotdl returned invalid Spotify metadata.".into()),
                        ),
                    }
                }
                Err(SpotDlProcessError::Cancelled) => {
                    cancelled_provider_section(ProviderKind::Spotify)
                }
                Err(SpotDlProcessError::Timeout) => failed_provider_section(
                    ProviderKind::Spotify,
                    ProviderSearchErrorCode::Timeout,
                    Some("Spotify search timed out.".into()),
                ),
                Err(SpotDlProcessError::Spawn) => failed_provider_section(
                    ProviderKind::Spotify,
                    ProviderSearchErrorCode::Unavailable,
                    Some("spotdl could not be started.".into()),
                ),
                Err(_) => failed_provider_section(
                    ProviderKind::Spotify,
                    ProviderSearchErrorCode::Failed,
                    Some("spotdl could not read Spotify metadata.".into()),
                ),
            }
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub(crate) enum SpotifyDownloadError {
    #[error("a Spotify track URL is required")]
    InvalidSpotifyUrl,
    #[error("spotdl is not installed")]
    SpotDlUnavailable,
    #[error("spotdl was cancelled")]
    Cancelled,
    #[error("spotdl timed out")]
    Timeout,
    #[error("spotdl could not resolve a playable source")]
    Failed,
    #[error("spotdl returned an invalid playable source")]
    InvalidResponse,
}

pub(crate) async fn resolve_spotify_download_url(
    spotify_url: &str,
    cancellation: SearchCancellation,
) -> Result<String, SpotifyDownloadError> {
    let id = spotify_track_id(spotify_url).map_err(|_| SpotifyDownloadError::InvalidSpotifyUrl)?;
    let path = spotdl_executable().ok_or(SpotifyDownloadError::SpotDlUnavailable)?;
    resolve_spotify_download_url_with_runner(
        &path,
        &id,
        Arc::new(ProcessSpotDlRunner),
        cancellation,
    )
    .await
}

async fn resolve_spotify_download_url_with_runner(
    executable: &Path,
    spotify_id: &str,
    runner: Arc<dyn SpotDlRunner>,
    cancellation: SearchCancellation,
) -> Result<String, SpotifyDownloadError> {
    let spotify_url =
        canonical_spotify_url(spotify_id).map_err(|_| SpotifyDownloadError::InvalidSpotifyUrl)?;
    let output = runner
        .run(
            executable,
            &url_args(spotify_url.as_url().as_str()),
            cancellation,
        )
        .await
        .map_err(|error| match error {
            SpotDlProcessError::Cancelled => SpotifyDownloadError::Cancelled,
            SpotDlProcessError::Timeout => SpotifyDownloadError::Timeout,
            SpotDlProcessError::Spawn => SpotifyDownloadError::SpotDlUnavailable,
            _ => SpotifyDownloadError::Failed,
        })?;
    for line in output
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(url) = Url::parse(line) else {
            continue;
        };
        let value = url.as_str();
        for provider in [ProviderKind::Youtube, ProviderKind::Soundcloud] {
            if let Ok(safe) = validate_provider_url(provider, value) {
                return Ok(safe.as_url().as_str().to_owned());
            }
        }
    }
    Err(SpotifyDownloadError::InvalidResponse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::types::{SearchId, SearchLens, SearchSortDirection, SearchSortField};
    use std::sync::Mutex;

    #[derive(Clone)]
    struct FakeSpotDlRunner {
        response: Result<SpotDlOutput, SpotDlProcessError>,
        calls: Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl FakeSpotDlRunner {
        fn json(value: &str) -> Self {
            Self {
                response: Ok(SpotDlOutput {
                    stdout: value.to_owned(),
                }),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl SpotDlRunner for FakeSpotDlRunner {
        fn run<'a>(
            &'a self,
            _executable: &'a Path,
            args: &'a [String],
            _cancellation: SearchCancellation,
        ) -> Pin<Box<dyn Future<Output = Result<SpotDlOutput, SpotDlProcessError>> + Send + 'a>>
        {
            self.calls.lock().unwrap().push(args.to_vec());
            let response = self.response.clone();
            Box::pin(async move { response })
        }
    }

    fn request(query: &str) -> ProviderSearchRequest {
        ProviderSearchRequest {
            search_id: SearchId::new(),
            query: query.to_owned(),
            lens: SearchLens::Spotify,
            entities: vec![SearchEntityKind::Track],
            sort_field: SearchSortField::Relevance,
            sort_direction: SearchSortDirection::Descending,
            limit: 25,
            market: None,
        }
    }

    #[test]
    fn spotify_track_urls_are_canonicalized_and_sensitive_queries_are_rejected() {
        let id = spotify_track_id(
            "https://open.spotify.com/intl-en/track/2wZAvkgiOE5tyrnqhB69KA?si=share",
        )
        .unwrap();
        assert_eq!(id, "2wZAvkgiOE5tyrnqhB69KA");
        assert_eq!(
            canonical_spotify_url(&id).unwrap().as_url().as_str(),
            "https://open.spotify.com/track/2wZAvkgiOE5tyrnqhB69KA"
        );
        for value in [
            "http://open.spotify.com/track/2wZAvkgiOE5tyrnqhB69KA",
            "https://evil.example/track/2wZAvkgiOE5tyrnqhB69KA",
            "https://open.spotify.com/album/4aawyAB9vmqN3uQ7FjRGTy",
            "https://open.spotify.com/track/2wZAvkgiOE5tyrnqhB69KA?access_token=secret",
        ] {
            assert_eq!(
                spotify_track_id(value),
                Err(SpotifyQueryError::InvalidTrackUrl)
            );
        }
    }

    #[test]
    fn spotdl_json_is_normalized_without_popularity_or_credentials() {
        let results = parse_spotdl_results(
            r#"[
              {
                "name": "Hello",
                "artists": ["Adele"],
                "album_name": "25",
                "duration": 295,
                "date": "2015-11-20",
                "song_id": "1Yk0cQdMLx5RzzFTYwmuld",
                "url": "https://open.spotify.com/track/1Yk0cQdMLx5RzzFTYwmuld",
                "cover_url": "https://i.scdn.co/image/cover",
                "explicit": false,
                "popularity": 99
              }
            ]"#,
            25,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Hello");
        assert_eq!(results[0].artists, vec!["Adele"]);
        assert_eq!(results[0].duration_ms, Some(295_000));
        assert_eq!(results[0].engagement_count, None);
        assert_eq!(
            results[0].canonical_url.as_ref().unwrap().as_url().as_str(),
            "https://open.spotify.com/track/1Yk0cQdMLx5RzzFTYwmuld"
        );
    }

    #[tokio::test]
    async fn spotdl_output_is_bounded_before_process_completion() {
        use tokio::io::AsyncWriteExt;

        let (mut writer, reader) = tokio::io::duplex(8 * 1024);
        let writer_task = tokio::spawn(async move {
            let _ = writer.write_all(&vec![0_u8; SPOTDL_OUTPUT_LIMIT + 1]).await;
        });
        assert_eq!(
            read_spotdl_output(reader).await,
            Err(SpotDlProcessError::OutputTooLarge)
        );
        let _ = writer_task.await;
    }

    #[tokio::test]
    async fn adapter_searches_with_spotdl_save_and_no_client_configuration() {
        let runner = Arc::new(FakeSpotDlRunner::json(
            r#"[{"name":"Hello","artists":["Adele"],"song_id":"1Yk0cQdMLx5RzzFTYwmuld","url":"https://open.spotify.com/track/1Yk0cQdMLx5RzzFTYwmuld"}]"#,
        ));
        let adapter = SpotifySourceAdapter::with_runner_for_tests(runner.clone());
        let section = adapter
            .search(request("Adele Hello"), SearchCancellation::new())
            .await;
        assert_eq!(
            section.state,
            crate::search::types::ProviderSearchState::Ready
        );
        assert_eq!(section.results[0].title, "Hello");
        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0]
            .windows(2)
            .any(|window| window == ["--save-file", "-"]));
        assert!(calls[0].contains(&"--no-cache".to_owned()));
        assert!(!calls[0]
            .iter()
            .any(|arg| arg.contains("client-secret") || arg.contains("client-id")));
    }

    #[tokio::test]
    async fn download_resolution_accepts_only_youtube_or_soundcloud_urls() {
        let runner = Arc::new(FakeSpotDlRunner::json(
            "https://www.youtube.com/watch?v=3fNbfdACbzE\n",
        ));
        let url = resolve_spotify_download_url_with_runner(
            Path::new("spotdl"),
            "1Yk0cQdMLx5RzzFTYwmuld",
            runner,
            SearchCancellation::new(),
        )
        .await
        .unwrap();
        assert_eq!(url, "https://www.youtube.com/watch?v=3fNbfdACbzE");
    }
}
