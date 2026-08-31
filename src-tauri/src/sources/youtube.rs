use serde_json::{Map, Value};
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;

use crate::domain::{ProviderKind, SourceCapabilities};
use crate::search::types::{
    EngagementKind, ProviderRuntimeStatus, ProviderSearchErrorCode, ProviderSearchRequest,
    ProviderSearchSection, SearchCancellation, SearchEntityKind, SearchResult,
};
use crate::sources::{
    cancelled_provider_section, failed_provider_section, is_cancelled, ready_provider_section,
    sanitize_artwork_url, validate_provider_url, SourceAdapter, YtDlpAdapterRuntime,
};

const SUPPORTED_ENTITIES: &[SearchEntityKind] = &[SearchEntityKind::Track];
const YOUTUBE_CAPABILITIES: SourceCapabilities = SourceCapabilities {
    search: true,
    playback: false,
    metadata: true,
    artwork: true,
    lyrics: false,
    downloads: false,
    popularity: true,
    release_date: false,
    lyrics_metadata: false,
};

pub struct YoutubeSourceAdapter {
    runtime: YtDlpAdapterRuntime,
}

impl YoutubeSourceAdapter {
    pub fn new(media_tools: crate::media_tools::MediaToolManager) -> Self {
        Self {
            runtime: YtDlpAdapterRuntime::new(media_tools),
        }
    }

    #[cfg(test)]
    fn with_runner_for_tests(
        status: crate::media_tools::YtDlpToolStatus,
        runner: std::sync::Arc<dyn crate::sources::yt_dlp::YtDlpProcessRunner>,
    ) -> Self {
        Self {
            runtime: YtDlpAdapterRuntime::with_runner_for_tests(status, runner),
        }
    }
}

impl SourceAdapter for YoutubeSourceAdapter {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Youtube
    }

    fn capabilities(&self) -> SourceCapabilities {
        YOUTUBE_CAPABILITIES
    }

    fn supported_entities(&self) -> &'static [SearchEntityKind] {
        SUPPORTED_ENTITIES
    }

    fn runtime_status(&self) -> ProviderRuntimeStatus {
        self.runtime.runtime_status()
    }

    fn search(
        &self,
        request: ProviderSearchRequest,
        cancellation: SearchCancellation,
    ) -> Pin<Box<dyn Future<Output = ProviderSearchSection> + Send + '_>> {
        Box::pin(async move {
            if is_cancelled(&cancellation) {
                return cancelled_provider_section(ProviderKind::Youtube);
            }
            if request.limit == 0
                || request.query.trim().is_empty()
                || !request.entities.contains(&SearchEntityKind::Track)
            {
                return ready_provider_section(ProviderKind::Youtube, Vec::new());
            }
            match self
                .runtime
                .execute_structured_search(
                    ProviderKind::Youtube,
                    request.query.trim(),
                    cancellation,
                )
                .await
            {
                Ok(stdout) => match parse_youtube_results(&stdout, usize::from(request.limit)) {
                    Ok(results) => ready_provider_section(ProviderKind::Youtube, results),
                    Err(error) => failed_provider_section(
                        ProviderKind::Youtube,
                        error,
                        Some("yt-dlp returned an invalid structured response".into()),
                    ),
                },
                Err(section) => section,
            }
        })
    }
}

fn parse_youtube_results(
    stdout: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, ProviderSearchErrorCode> {
    let root: Value =
        serde_json::from_str(stdout).map_err(|_| ProviderSearchErrorCode::InvalidResponse)?;
    let entries = root
        .as_object()
        .and_then(|object| object.get("entries"))
        .and_then(Value::as_array)
        .ok_or(ProviderSearchErrorCode::InvalidResponse)?;
    let mut seen_ids = HashSet::new();
    let mut results = Vec::new();
    for (rank, entry) in entries.iter().enumerate() {
        let entry = entry
            .as_object()
            .ok_or(ProviderSearchErrorCode::InvalidResponse)?;
        let Some(id) = string(entry, "id") else {
            continue;
        };
        let Some(title) = string(entry, "title") else {
            continue;
        };
        if id.is_empty() || title.is_empty() || !seen_ids.insert(id.to_owned()) {
            continue;
        }
        let engagement_count = unsigned(entry, "view_count");
        results.push(SearchResult {
            provider: ProviderKind::Youtube,
            entity_kind: SearchEntityKind::Track,
            provider_item_id: id.to_owned(),
            canonical_url: safe_canonical_url(entry, ProviderKind::Youtube),
            title: title.to_owned(),
            artists: artist(entry),
            album: string(entry, "album").map(str::to_owned),
            duration_ms: duration_ms(entry),
            artwork_url: string(entry, "thumbnail").and_then(sanitize_artwork_url),
            published_at: None,
            engagement_count,
            engagement_kind: engagement_count.map(|_| EngagementKind::Views),
            explicit: None,
            local_track_id: None,
            local_source_id: None,
            original_rank: u32::try_from(rank).unwrap_or(u32::MAX),
        });
        if results.len() == limit {
            break;
        }
    }
    Ok(results)
}

fn string<'a>(entry: &'a Map<String, Value>, field: &str) -> Option<&'a str> {
    entry
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn unsigned(entry: &Map<String, Value>, field: &str) -> Option<u64> {
    entry.get(field).and_then(Value::as_u64)
}

fn duration_ms(entry: &Map<String, Value>) -> Option<u64> {
    let seconds = entry.get("duration")?.as_f64()?;
    (seconds.is_finite() && seconds >= 0.0 && seconds <= (u64::MAX as f64) / 1_000.0)
        .then_some((seconds * 1_000.0) as u64)
}

fn artist(entry: &Map<String, Value>) -> Vec<String> {
    ["channel", "uploader", "artist", "creator"]
        .into_iter()
        .find_map(|field| string(entry, field))
        .map(|value| vec![value.to_owned()])
        .unwrap_or_default()
}

fn safe_canonical_url(
    entry: &Map<String, Value>,
    provider: ProviderKind,
) -> Option<crate::search::types::SafeUrl> {
    string(entry, "webpage_url").and_then(|url| validate_provider_url(provider, url).ok())
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    use crate::domain::ProviderKind;
    use crate::media_tools::yt_dlp::{YtDlpProcessError, YtDlpProcessOutput, YtDlpProcessRunner};
    use crate::media_tools::YtDlpToolStatus;
    use crate::search::types::{
        EngagementKind, ProviderRuntimeStatus, ProviderSearchErrorCode, ProviderSearchRequest,
        ProviderSearchState, SearchCancellation, SearchEntityKind, SearchId, SearchLens,
        SearchSortDirection, SearchSortField,
    };
    use crate::sources::SourceAdapter;

    use super::YoutubeSourceAdapter;

    #[derive(Clone)]
    struct FakeYtDlpRunner {
        response: Result<YtDlpProcessOutput, YtDlpProcessError>,
        calls: Calls,
    }

    type Calls = Arc<Mutex<Vec<(String, Vec<String>)>>>;

    impl FakeYtDlpRunner {
        fn json(stdout: &str) -> Self {
            Self {
                response: Ok(YtDlpProcessOutput {
                    stdout: stdout.to_owned(),
                    stderr: String::new(),
                    exit_code: Some(0),
                }),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn failure(error: YtDlpProcessError) -> Self {
            Self {
                response: Err(error),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Vec<(String, Vec<String>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl YtDlpProcessRunner for FakeYtDlpRunner {
        fn run<'a>(
            &'a self,
            executable: &'a str,
            args: &'a [String],
            _cancellation: SearchCancellation,
        ) -> Pin<Box<dyn Future<Output = Result<YtDlpProcessOutput, YtDlpProcessError>> + Send + 'a>>
        {
            self.calls
                .lock()
                .unwrap()
                .push((executable.to_owned(), args.to_vec()));
            let response = self.response.clone();
            Box::pin(async move { response })
        }
    }

    fn ready_status() -> YtDlpToolStatus {
        YtDlpToolStatus {
            status: ProviderRuntimeStatus::Ready,
            executable: Some("C:/yt-dlp.exe".into()),
            version: Some("2026.08.19".into()),
            detail: None,
        }
    }

    fn status(status: ProviderRuntimeStatus) -> YtDlpToolStatus {
        YtDlpToolStatus {
            status,
            executable: None,
            version: None,
            detail: Some("provider diagnostic".into()),
        }
    }

    fn youtube_with(runner: FakeYtDlpRunner) -> YoutubeSourceAdapter {
        YoutubeSourceAdapter::with_runner_for_tests(ready_status(), Arc::new(runner))
    }

    fn youtube_with_status(
        tool_status: YtDlpToolStatus,
        runner: FakeYtDlpRunner,
    ) -> YoutubeSourceAdapter {
        YoutubeSourceAdapter::with_runner_for_tests(tool_status, Arc::new(runner))
    }

    fn test_request() -> ProviderSearchRequest {
        ProviderSearchRequest {
            search_id: SearchId::new(),
            query: "signal".into(),
            lens: SearchLens::Youtube,
            entities: vec![SearchEntityKind::Track],
            sort_field: SearchSortField::Relevance,
            sort_direction: SearchSortDirection::Descending,
            limit: 2,
            market: None,
        }
    }

    #[tokio::test]
    async fn youtube_normal_flat_result() {
        let section = youtube_with(FakeYtDlpRunner::json(
            r#"{"entries":[{"id":"v1","title":"Signal","channel":"Channel","duration":123,"view_count":42,"webpage_url":"https://www.youtube.com/watch?v=v1","thumbnail":"https://i.ytimg.com/vi/v1/hqdefault.jpg"}]}"#,
        ))
        .search(test_request(), SearchCancellation::new())
        .await;

        assert_eq!(section.state, ProviderSearchState::Ready);
        assert_eq!(section.results.len(), 1);
        assert_eq!(section.results[0].provider, ProviderKind::Youtube);
        assert_eq!(section.results[0].entity_kind, SearchEntityKind::Track);
        assert_eq!(section.results[0].artists, ["Channel"]);
        assert_eq!(section.results[0].duration_ms, Some(123_000));
        assert_eq!(section.results[0].engagement_count, Some(42));
        assert_eq!(
            section.results[0].engagement_kind,
            Some(EngagementKind::Views)
        );
        assert_eq!(
            section.results[0]
                .canonical_url
                .as_ref()
                .unwrap()
                .as_url()
                .as_str(),
            "https://www.youtube.com/watch?v=v1"
        );
    }

    #[tokio::test]
    async fn youtube_missing_thumbnail_view_count_duration_and_channel() {
        let section = youtube_with(FakeYtDlpRunner::json(
            r#"{"entries":[{"id":"v1","title":"Title","duration":"unknown","view_count":{},"thumbnail":"https://evil.example/image.jpg"}]}"#,
        ))
        .search(test_request(), SearchCancellation::new())
        .await;

        assert_eq!(section.results[0].artists, Vec::<String>::new());
        assert_eq!(section.results[0].engagement_count, None);
        assert_eq!(section.results[0].duration_ms, None);
        assert_eq!(section.results[0].artwork_url, None);
    }

    #[tokio::test]
    async fn youtube_malformed_top_level_json() {
        let section = youtube_with(FakeYtDlpRunner::json("not json"))
            .search(test_request(), SearchCancellation::new())
            .await;

        assert_eq!(section.state, ProviderSearchState::Failed);
        assert_eq!(
            section.error.unwrap().code,
            ProviderSearchErrorCode::InvalidResponse
        );
    }

    #[tokio::test]
    async fn youtube_missing_entries() {
        let section = youtube_with(FakeYtDlpRunner::json(r#"{"id":"search"}"#))
            .search(test_request(), SearchCancellation::new())
            .await;

        assert_eq!(section.state, ProviderSearchState::Failed);
        assert_eq!(
            section.error.unwrap().code,
            ProviderSearchErrorCode::InvalidResponse
        );
    }

    #[tokio::test]
    async fn youtube_unexpected_entry_type() {
        let section = youtube_with(FakeYtDlpRunner::json(r#"{"entries":["not-an-entry"]}"#))
            .search(test_request(), SearchCancellation::new())
            .await;

        assert_eq!(section.state, ProviderSearchState::Failed);
        assert_eq!(
            section.error.unwrap().code,
            ProviderSearchErrorCode::InvalidResponse
        );
    }

    #[tokio::test]
    async fn youtube_empty_results() {
        let section = youtube_with(FakeYtDlpRunner::json(r#"{"entries":[]}"#))
            .search(test_request(), SearchCancellation::new())
            .await;

        assert_eq!(section.state, ProviderSearchState::Ready);
        assert!(section.results.is_empty());
    }

    #[tokio::test]
    async fn youtube_deduplicates_before_capping_and_preserves_first_rank() {
        let section = youtube_with(FakeYtDlpRunner::json(
            r#"{"entries":[{"id":"v1","title":"First"},{"id":"v1","title":"Duplicate"},{"id":"v2","title":"Second"},{"id":"v3","title":"Third"}]}"#,
        ))
        .search(test_request(), SearchCancellation::new())
        .await;

        assert_eq!(
            section
                .results
                .iter()
                .map(|result| result.provider_item_id.as_str())
                .collect::<Vec<_>>(),
            ["v1", "v2"]
        );
        assert_eq!(section.results[0].original_rank, 0);
        assert_eq!(section.results[1].original_rank, 2);
    }

    #[tokio::test]
    async fn youtube_tool_missing() {
        let runner = FakeYtDlpRunner::json(r#"{"entries":[]}"#);
        let section = youtube_with_status(status(ProviderRuntimeStatus::Missing), runner.clone())
            .search(test_request(), SearchCancellation::new())
            .await;

        assert_eq!(section.state, ProviderSearchState::Failed);
        assert_eq!(
            section.error.unwrap().code,
            ProviderSearchErrorCode::Unavailable
        );
        assert!(runner.calls().is_empty());
    }

    #[tokio::test]
    async fn youtube_unsupported_version() {
        let section = youtube_with_status(
            status(ProviderRuntimeStatus::Unsupported),
            FakeYtDlpRunner::json(r#"{"entries":[]}"#),
        )
        .search(test_request(), SearchCancellation::new())
        .await;

        assert_eq!(section.state, ProviderSearchState::Failed);
        assert_eq!(
            section.error.unwrap().code,
            ProviderSearchErrorCode::Unavailable
        );
    }

    #[tokio::test]
    async fn youtube_timeout() {
        let section = youtube_with(FakeYtDlpRunner::failure(YtDlpProcessError::Timeout))
            .search(test_request(), SearchCancellation::new())
            .await;

        assert_eq!(section.state, ProviderSearchState::Failed);
        assert_eq!(
            section.error.unwrap().code,
            ProviderSearchErrorCode::Timeout
        );
    }

    #[tokio::test]
    async fn youtube_cancellation() {
        let section = youtube_with(FakeYtDlpRunner::failure(YtDlpProcessError::Cancelled))
            .search(test_request(), SearchCancellation::new())
            .await;

        assert_eq!(section.state, ProviderSearchState::Cancelled);
        assert_eq!(
            section.error.unwrap().code,
            ProviderSearchErrorCode::Cancelled
        );
    }

    #[tokio::test]
    async fn youtube_output_too_large() {
        let section = youtube_with(FakeYtDlpRunner::failure(YtDlpProcessError::StdoutTooLarge))
            .search(test_request(), SearchCancellation::new())
            .await;

        assert_eq!(section.state, ProviderSearchState::Failed);
        assert_eq!(
            section.error.unwrap().code,
            ProviderSearchErrorCode::InvalidResponse
        );
    }

    #[tokio::test]
    async fn youtube_metacharacters_stay_in_one_argv_entry() {
        let runner = FakeYtDlpRunner::json(r#"{"entries":[]}"#);
        let mut request = test_request();
        request.query = "a & b | c".into();
        let _ = youtube_with(runner.clone())
            .search(request, SearchCancellation::new())
            .await;

        assert_eq!(
            runner.calls()[0].1,
            [
                "--no-config",
                "--dump-single-json",
                "--flat-playlist",
                "--skip-download",
                "--no-warnings",
                "--socket-timeout",
                "10",
                "ytsearch25:a & b | c",
            ]
        );
    }
}
