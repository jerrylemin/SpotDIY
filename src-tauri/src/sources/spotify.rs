use serde::Deserialize;
use serde_json::Value;
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
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
use url::Url;

use crate::domain::{ProviderKind, SourceCapabilities};
use crate::search::types::{
    ProviderRuntimeStatus, ProviderSearchErrorCode, ProviderSearchRequest, ProviderSearchSection,
    SafeUrl, SearchCancellation, SearchEntityKind, SearchLens, SearchResult,
};
use crate::sources::yt_dlp::{
    yt_dlp_search_args, TokioYtDlpProcessRunner, YtDlpProcessError, YtDlpProcessRunner,
};
use crate::sources::{
    cancelled_provider_section, failed_provider_section, is_cancelled, ready_provider_section,
    sanitize_artwork_url, validate_provider_url, SourceAdapter,
};

const SUPPORTED_ENTITIES: &[SearchEntityKind] = &[SearchEntityKind::Track];
const SPOTDL_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const SPOTDL_OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
const SPOTDL_MAX_RETRIES: &str = "1";
const SPOTDL_PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Spotify results remain metadata records. The actual audio download is
/// resolved through Spotify's public embed metadata plus a bounded yt-dlp
/// search, then goes through the existing yt-dlp download worker.
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

fn save_args(query: &str, artist_search: bool) -> Vec<String> {
    let mut args = vec![
        "save".to_owned(),
        query.to_owned(),
        "--save-file".to_owned(),
        "-".to_owned(),
        "--headless".to_owned(),
        "--max-retries".to_owned(),
        SPOTDL_MAX_RETRIES.to_owned(),
        "--log-level".to_owned(),
        "ERROR".to_owned(),
        "--use-cache-file".to_owned(),
    ];
    if artist_search {
        args.push("--fetch-albums".to_owned());
    }
    args
}

const SPOTIFY_EMBED_BODY_LIMIT: usize = 2 * 1024 * 1024;
const YOUTUBE_MATCH_DURATION_TOLERANCE_MS: u64 = 30_000;

#[derive(Clone, Debug, Eq, PartialEq)]
struct SpotifyTrackMetadata {
    title: String,
    artists: Vec<String>,
    duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct YoutubeMatchCandidate {
    id: String,
    title: String,
    duration_ms: Option<u64>,
}

fn string_field(value: &serde_json::Map<String, Value>, names: &[&str]) -> Option<String> {
    names
        .iter()
        .filter_map(|name| value.get(*name).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_owned)
}

fn parse_spotify_track_object(value: &Value) -> Option<SpotifyTrackMetadata> {
    let object = value.as_object()?;
    let title = string_field(object, &["title", "name"])?;
    let mut artists = string_field(object, &["subtitle"])
        .map(|artist| vec![artist])
        .unwrap_or_default();
    if artists.is_empty() {
        if let Some(values) = object.get("artists").and_then(Value::as_array) {
            artists = values
                .iter()
                .filter_map(|artist| {
                    artist
                        .as_object()
                        .and_then(|artist| string_field(artist, &["name", "title"]))
                        .or_else(|| artist.as_str().map(str::to_owned))
                })
                .map(|artist| artist.trim().to_owned())
                .filter(|artist| !artist.is_empty())
                .collect();
        }
    }
    let duration_ms = object
        .get("duration")
        .and_then(Value::as_u64)
        .or_else(|| object.get("duration_ms").and_then(Value::as_u64));
    Some(SpotifyTrackMetadata {
        title,
        artists,
        duration_ms,
    })
}

fn find_spotify_track_metadata(value: &Value, depth: u8) -> Option<SpotifyTrackMetadata> {
    if depth == 0 {
        return None;
    }
    if let Some(metadata) = parse_spotify_track_object(value) {
        return Some(metadata);
    }
    match value {
        Value::Object(object) => object
            .values()
            .find_map(|child| find_spotify_track_metadata(child, depth - 1)),
        Value::Array(values) => values
            .iter()
            .find_map(|child| find_spotify_track_metadata(child, depth - 1)),
        _ => None,
    }
}

fn parse_spotify_embed_track_metadata(html: &str) -> Option<SpotifyTrackMetadata> {
    let marker = r#"<script id="__NEXT_DATA__""#;
    let marker_start = html.find(marker)?;
    let content_start = marker_start + html[marker_start..].find('>')? + 1;
    let content_end = content_start + html[content_start..].find("</script>")?;
    let data = serde_json::from_str::<Value>(&html[content_start..content_end]).ok()?;
    find_spotify_track_metadata(&data, 10)
}

async fn fetch_spotify_embed_track(
    spotify_id: &str,
    cancellation: SearchCancellation,
) -> Result<SpotifyTrackMetadata, SpotifyDownloadError> {
    let client = reqwest::Client::builder()
        .user_agent("SpotDIY/0.1 Spotify metadata resolver")
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|_| SpotifyDownloadError::MetadataUnavailable)?;
    let url = format!("https://open.spotify.com/embed/track/{spotify_id}");
    let mut cancellation_rx = cancellation.subscribe();
    let response = tokio::select! {
        result = client.get(url).header("accept", "text/html,application/xhtml+xml").send() => {
            result.map_err(|_| SpotifyDownloadError::MetadataUnavailable)?
        }
        changed = cancellation_rx.changed() => {
            let _ = changed;
            return Err(SpotifyDownloadError::Cancelled);
        }
    };
    if !response.status().is_success() {
        return Err(SpotifyDownloadError::MetadataUnavailable);
    }
    let body = tokio::select! {
        result = response.text() => result.map_err(|_| SpotifyDownloadError::MetadataUnavailable)?,
        changed = cancellation_rx.changed() => {
            let _ = changed;
            return Err(SpotifyDownloadError::Cancelled);
        }
    };
    if body.len() > SPOTIFY_EMBED_BODY_LIMIT {
        return Err(SpotifyDownloadError::MetadataUnavailable);
    }
    parse_spotify_embed_track_metadata(&body).ok_or(SpotifyDownloadError::MetadataUnavailable)
}

fn normalize_match_text(value: &str) -> String {
    let mut normalized = String::new();
    for character in value
        .nfkd()
        .filter(|character| !is_combining_mark(*character))
    {
        let character = match character {
            'Đ' | 'đ' => 'd',
            character => character,
        };
        if character.is_alphanumeric() {
            normalized.extend(character.to_lowercase());
        } else if character.is_whitespace() {
            normalized.push(' ');
        } else if character != '\'' && character != '’' {
            normalized.push(' ');
        }
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn spotify_title_core(title: &str) -> &str {
    title
        .split_once(" - ")
        .map(|(core, _)| core)
        .unwrap_or(title)
}

fn title_plausibly_matches(candidate: &str, expected: &str) -> bool {
    let candidate = normalize_match_text(candidate);
    let expected = normalize_match_text(spotify_title_core(expected));
    if candidate.is_empty() || expected.is_empty() {
        return false;
    }
    if expected.chars().count() >= 4 {
        candidate.contains(&expected)
    } else {
        candidate.split_whitespace().any(|word| word == expected)
    }
}

fn artist_tokens(artists: &[String]) -> Vec<String> {
    artists
        .iter()
        .flat_map(|artist| {
            artist
                .replace(" feat. ", ",")
                .replace(" feat ", ",")
                .replace(" ft. ", ",")
                .replace(" ft ", ",")
                .split(',')
                .map(str::trim)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .map(|artist| normalize_match_text(&artist))
        .filter(|artist| !artist.is_empty())
        .collect()
}

fn parse_youtube_match_candidates(stdout: &str) -> Option<Vec<YoutubeMatchCandidate>> {
    let root = serde_json::from_str::<Value>(stdout).ok()?;
    let entries = root.get("entries")?.as_array()?;
    Some(
        entries
            .iter()
            .filter_map(|entry| {
                let object = entry.as_object()?;
                let id = object.get("id")?.as_str()?.trim();
                let title = object.get("title")?.as_str()?.trim();
                if id.is_empty() || title.is_empty() {
                    return None;
                }
                let duration_ms = object
                    .get("duration")
                    .and_then(Value::as_f64)
                    .filter(|duration| duration.is_finite() && *duration >= 0.0)
                    .map(|duration| (duration * 1000.0).round() as u64);
                Some(YoutubeMatchCandidate {
                    id: id.to_owned(),
                    title: title.to_owned(),
                    duration_ms,
                })
            })
            .collect(),
    )
}

fn choose_sunnify_youtube_match(
    stdout: &str,
    metadata: &SpotifyTrackMetadata,
) -> Result<String, SpotifyDownloadError> {
    let candidates =
        parse_youtube_match_candidates(stdout).ok_or(SpotifyDownloadError::InvalidResponse)?;
    let title_matches = candidates
        .iter()
        .filter(|candidate| title_plausibly_matches(&candidate.title, &metadata.title))
        .collect::<Vec<_>>();
    if title_matches.is_empty() {
        return Err(SpotifyDownloadError::NoPlayableMatch);
    }
    let tokens = artist_tokens(&metadata.artists);
    let artist_matches = title_matches
        .iter()
        .copied()
        .filter(|candidate| {
            let title = normalize_match_text(&candidate.title);
            tokens.iter().any(|artist| title.contains(artist))
        })
        .collect::<Vec<_>>();
    let pool = if artist_matches.is_empty() {
        title_matches
    } else {
        artist_matches
    };
    let chosen = if let Some(expected) = metadata.duration_ms {
        let candidate = pool
            .iter()
            .min_by_key(|candidate| {
                candidate
                    .duration_ms
                    .map(|duration| duration.abs_diff(expected))
                    .unwrap_or(u64::MAX)
            })
            .copied()
            .ok_or(SpotifyDownloadError::NoPlayableMatch)?;
        if candidate.duration_ms.is_some_and(|duration| {
            duration.abs_diff(expected) > YOUTUBE_MATCH_DURATION_TOLERANCE_MS
        }) {
            return Err(SpotifyDownloadError::NoPlayableMatch);
        }
        candidate
    } else {
        pool.first()
            .copied()
            .ok_or(SpotifyDownloadError::NoPlayableMatch)?
    };
    let candidate_url = format!("https://www.youtube.com/watch?v={}", chosen.id);
    validate_provider_url(ProviderKind::Youtube, &candidate_url)
        .map(|url| url.as_url().as_str().to_owned())
        .map_err(|_| SpotifyDownloadError::InvalidResponse)
}

async fn resolve_sunnify_audio_url(
    metadata: SpotifyTrackMetadata,
    yt_dlp_path: &Path,
    cancellation: SearchCancellation,
) -> Result<String, SpotifyDownloadError> {
    let query = format!("{} {} audio", metadata.title, metadata.artists.join(" "));
    let args = yt_dlp_search_args(&query);
    let executable = yt_dlp_path.to_string_lossy().into_owned();
    let output = TokioYtDlpProcessRunner::default()
        .run(&executable, &args, cancellation)
        .await
        .map_err(|error| match error {
            YtDlpProcessError::Cancelled => SpotifyDownloadError::Cancelled,
            YtDlpProcessError::Timeout => SpotifyDownloadError::Timeout,
            YtDlpProcessError::Spawn => SpotifyDownloadError::YtDlpUnavailable,
            _ => SpotifyDownloadError::Failed,
        })?;
    choose_sunnify_youtube_match(&output.stdout, &metadata)
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
            let query = request.query.trim();
            let artist_search = request.lens == SearchLens::Artists;
            // The provider lens is also commonly used for plain artist names.
            // Ask spotdl for the albums behind the matched songs so a Spotify
            // provider search does not collapse an artist query to one track.
            let expand_results = artist_search || request.lens == SearchLens::Spotify;
            let query = if artist_search && !query.to_ascii_lowercase().starts_with("artist:") {
                format!("artist:{query}")
            } else {
                query.to_owned()
            };
            let args = save_args(&query, expand_results);
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
    #[error("Spotify metadata could not be read from its public page")]
    MetadataUnavailable,
    #[error("yt-dlp is not installed")]
    YtDlpUnavailable,
    #[error("Spotify audio matching was cancelled")]
    Cancelled,
    #[error("Spotify audio matching timed out")]
    Timeout,
    #[error("Spotify audio matching failed")]
    Failed,
    #[error("Spotify returned no confident YouTube audio match")]
    NoPlayableMatch,
    #[error("Spotify audio matching returned an invalid response")]
    InvalidResponse,
}

pub(crate) async fn resolve_spotify_download_url(
    spotify_url: &str,
    title: Option<&str>,
    artists: &[String],
    duration_ms: Option<u64>,
    yt_dlp_path: Option<&Path>,
    cancellation: SearchCancellation,
) -> Result<String, SpotifyDownloadError> {
    let id = spotify_track_id(spotify_url).map_err(|_| SpotifyDownloadError::InvalidSpotifyUrl)?;
    let metadata = match title.filter(|title| !title.trim().is_empty()) {
        Some(title) => SpotifyTrackMetadata {
            title: title.trim().to_owned(),
            artists: artists.to_vec(),
            duration_ms,
        },
        None => fetch_spotify_embed_track(&id, cancellation.clone()).await?,
    };
    let yt_dlp_path = yt_dlp_path.ok_or(SpotifyDownloadError::YtDlpUnavailable)?;
    resolve_sunnify_audio_url(metadata, yt_dlp_path, cancellation).await
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
        assert!(calls[0].contains(&"--fetch-albums".to_owned()));
        assert!(!calls[0].contains(&"--no-cache".to_owned()));
        assert!(!calls[0]
            .iter()
            .any(|arg| arg.contains("client-secret") || arg.contains("client-id")));
    }

    #[tokio::test]
    async fn artist_search_uses_spotdl_artist_query_and_fetches_more_results() {
        let runner = Arc::new(FakeSpotDlRunner::json(
            r#"[{"name":"Hello","artists":["Adele"],"song_id":"1Yk0cQdMLx5RzzFTYwmuld","url":"https://open.spotify.com/track/1Yk0cQdMLx5RzzFTYwmuld"}]"#,
        ));
        let adapter = SpotifySourceAdapter::with_runner_for_tests(runner.clone());
        let mut artist_request = request("Adele");
        artist_request.lens = SearchLens::Artists;
        let section = adapter
            .search(artist_request, SearchCancellation::new())
            .await;

        assert_eq!(section.results.len(), 1);
        let calls = runner.calls();
        assert!(calls[0].contains(&"artist:Adele".to_owned()));
        assert!(calls[0].contains(&"--fetch-albums".to_owned()));
    }

    #[test]
    fn spotify_public_embed_metadata_is_extracted_without_credentials() {
        let html = r#"<script id="__NEXT_DATA__" type="application/json">{
          "props":{"pageProps":{"state":{"data":{"entity":{
            "title":"Hello",
            "subtitle":"Adele",
            "duration":295000
          }}}}}
        }</script>"#;
        assert_eq!(
            parse_spotify_embed_track_metadata(html),
            Some(SpotifyTrackMetadata {
                title: "Hello".to_owned(),
                artists: vec!["Adele".to_owned()],
                duration_ms: Some(295_000),
            })
        );
    }

    #[test]
    fn sunnify_matching_prefers_title_artist_and_duration() {
        let metadata = SpotifyTrackMetadata {
            title: "Hello".to_owned(),
            artists: vec!["Adele".to_owned()],
            duration_ms: Some(295_000),
        };
        let stdout = r#"{
          "entries":[
            {"id":"wrong","title":"Adele - Hello (Live)","duration":120},
            {"id":"right","title":"Adele - Hello (Official Audio)","duration":295},
            {"id":"other","title":"Hello - Another Artist","duration":295}
          ]
        }"#;
        assert_eq!(
            choose_sunnify_youtube_match(stdout, &metadata).unwrap(),
            "https://www.youtube.com/watch?v=right"
        );
    }

    #[test]
    fn sunnify_matching_rejects_a_wrong_recording() {
        let metadata = SpotifyTrackMetadata {
            title: "Hello".to_owned(),
            artists: vec!["Adele".to_owned()],
            duration_ms: Some(295_000),
        };
        let stdout = r#"{"entries":[{"id":"live","title":"Adele - Hello Live","duration":420}]}"#;
        assert_eq!(
            choose_sunnify_youtube_match(stdout, &metadata),
            Err(SpotifyDownloadError::NoPlayableMatch)
        );
    }
}
