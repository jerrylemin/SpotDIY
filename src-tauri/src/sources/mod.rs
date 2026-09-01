pub mod local;
pub mod soundcloud;
pub mod spotify;
pub mod traits;
pub mod youtube;
pub use crate::media_tools::yt_dlp;
pub use local::LocalSourceAdapter;
pub use soundcloud::SoundcloudSourceAdapter;
pub use spotify::SpotifySourceAdapter;
pub use traits::SourceAdapter;
pub use youtube::YoutubeSourceAdapter;

use std::sync::Arc;

use crate::domain::ProviderKind;
use crate::media_tools::{MediaToolManager, YtDlpToolStatus};
use crate::search::types::{
    ProviderRuntimeStatus, ProviderSearchError, ProviderSearchErrorCode, ProviderSearchSection,
    ProviderSearchState, SafeUrl, SearchCancellation, SearchResult,
};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ProviderUrlError {
    #[error("invalid provider URL")]
    Invalid,
    #[error("provider URL host is not allowlisted")]
    HostNotAllowed,
}

pub fn validate_provider_url(
    provider: ProviderKind,
    value: &str,
) -> Result<SafeUrl, ProviderUrlError> {
    let url = Url::parse(value).map_err(|_| ProviderUrlError::Invalid)?;
    if url.scheme() != "https" || url.host_str().is_none() {
        return Err(ProviderUrlError::Invalid);
    }
    let allowed = match provider {
        ProviderKind::Youtube => matches!(
            url.host_str(),
            Some("youtube.com" | "www.youtube.com" | "music.youtube.com" | "youtu.be")
        ),
        ProviderKind::Soundcloud => matches!(
            url.host_str(),
            Some("soundcloud.com" | "www.soundcloud.com")
        ),
        ProviderKind::Spotify => matches!(
            url.host_str(),
            Some("open.spotify.com" | "spotify.com" | "www.spotify.com")
        ),
        ProviderKind::Local => false,
    };
    if !allowed {
        return Err(ProviderUrlError::HostNotAllowed);
    }
    SafeUrl::from_url(url).ok_or(ProviderUrlError::Invalid)
}

pub fn sanitize_artwork_url(value: &str) -> Option<SafeUrl> {
    let url = Url::parse(value).ok()?;
    (url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("i.ytimg.com" | "yt3.ggpht.com" | "i1.sndcdn.com" | "i.scdn.co")
        ))
    .then(|| SafeUrl::from_url(url))
    .flatten()
}

pub(crate) struct YtDlpAdapterRuntime {
    media_tools: MediaToolManager,
    runner: Arc<dyn yt_dlp::YtDlpProcessRunner>,
    #[cfg(test)]
    test_status: Option<YtDlpToolStatus>,
}

impl YtDlpAdapterRuntime {
    pub(crate) fn new(media_tools: MediaToolManager) -> Self {
        Self {
            media_tools,
            runner: Arc::new(yt_dlp::TokioYtDlpProcessRunner::default()),
            #[cfg(test)]
            test_status: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_runner_for_tests(
        status: YtDlpToolStatus,
        runner: Arc<dyn yt_dlp::YtDlpProcessRunner>,
    ) -> Self {
        Self {
            media_tools: MediaToolManager::with_yt_dlp_override("test-only-yt-dlp".into()),
            runner,
            test_status: Some(status),
        }
    }

    pub(crate) fn runtime_status(&self) -> ProviderRuntimeStatus {
        self.tool_status().status
    }

    fn tool_status(&self) -> YtDlpToolStatus {
        #[cfg(test)]
        if let Some(status) = &self.test_status {
            return status.clone();
        }
        self.media_tools.yt_dlp_status()
    }

    pub(crate) async fn execute_structured_search(
        &self,
        provider: ProviderKind,
        query: &str,
        cancellation: SearchCancellation,
    ) -> Result<String, ProviderSearchSection> {
        if is_cancelled(&cancellation) {
            return Err(cancelled_provider_section(provider));
        }
        let status = self.tool_status();
        let Some(executable) = ready_executable(&status) else {
            return Err(tool_status_failure_section(provider, status.status));
        };
        let args = structured_search_args(provider, query);
        match self
            .runner
            .run(&executable, &args, cancellation.clone())
            .await
        {
            Ok(_output) if is_cancelled(&cancellation) => Err(cancelled_provider_section(provider)),
            Ok(output) => Ok(output.stdout),
            Err(error) => Err(process_failure_section(provider, error)),
        }
    }
}

pub(crate) fn ready_provider_section(
    provider: ProviderKind,
    results: Vec<SearchResult>,
) -> ProviderSearchSection {
    ProviderSearchSection {
        provider,
        state: ProviderSearchState::Ready,
        results,
        error: None,
    }
}

pub(crate) fn failed_provider_section(
    provider: ProviderKind,
    code: ProviderSearchErrorCode,
    detail: Option<String>,
) -> ProviderSearchSection {
    ProviderSearchSection {
        provider,
        state: ProviderSearchState::Failed,
        results: Vec::new(),
        error: Some(ProviderSearchError {
            code,
            detail,
            retry_after_seconds: None,
        }),
    }
}

pub(crate) fn cancelled_provider_section(provider: ProviderKind) -> ProviderSearchSection {
    ProviderSearchSection {
        provider,
        state: ProviderSearchState::Cancelled,
        results: Vec::new(),
        error: Some(ProviderSearchError {
            code: ProviderSearchErrorCode::Cancelled,
            detail: None,
            retry_after_seconds: None,
        }),
    }
}

pub(crate) fn is_cancelled(cancellation: &SearchCancellation) -> bool {
    *cancellation.subscribe().borrow()
}

fn structured_search_args(provider: ProviderKind, query: &str) -> Vec<String> {
    let expression = match provider {
        ProviderKind::Youtube => format!("ytsearch25:{query}"),
        ProviderKind::Soundcloud => format!("scsearch25:{query}"),
        ProviderKind::Local | ProviderKind::Spotify => unreachable!("yt-dlp search is online-only"),
    };
    [
        "--no-config".to_owned(),
        "--dump-single-json".to_owned(),
        "--flat-playlist".to_owned(),
        "--skip-download".to_owned(),
        "--no-warnings".to_owned(),
        "--socket-timeout".to_owned(),
        "10".to_owned(),
        expression,
    ]
    .to_vec()
}

fn ready_executable(status: &YtDlpToolStatus) -> Option<String> {
    (status.status == ProviderRuntimeStatus::Ready)
        .then_some(status.executable.as_ref())
        .flatten()
        .map(|path| path.to_string_lossy().into_owned())
}

fn tool_status_failure_section(
    provider: ProviderKind,
    status: ProviderRuntimeStatus,
) -> ProviderSearchSection {
    let (code, detail) = match status {
        ProviderRuntimeStatus::Missing => (
            ProviderSearchErrorCode::Unavailable,
            "yt-dlp is unavailable",
        ),
        ProviderRuntimeStatus::Unsupported => (
            ProviderSearchErrorCode::Unavailable,
            "yt-dlp is unsupported",
        ),
        _ => (ProviderSearchErrorCode::Failed, "yt-dlp is unavailable"),
    };
    failed_provider_section(provider, code, Some(detail.to_owned()))
}

fn process_failure_section(
    provider: ProviderKind,
    error: yt_dlp::YtDlpProcessError,
) -> ProviderSearchSection {
    if error == yt_dlp::YtDlpProcessError::Cancelled {
        return cancelled_provider_section(provider);
    }
    let code = match &error {
        yt_dlp::YtDlpProcessError::NonZeroExit { stderr, .. } if is_rate_limited(stderr) => {
            ProviderSearchErrorCode::RateLimited
        }
        _ => error.provider_error_code(),
    };
    let detail = match error {
        yt_dlp::YtDlpProcessError::NonZeroExit { stderr, .. } => redacted_diagnostic(&stderr),
        _ => Some(error.to_string()),
    };
    failed_provider_section(provider, code, detail)
}

fn is_rate_limited(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("429") || lower.contains("rate limit") || lower.contains("too many requests")
}

fn redacted_diagnostic(stderr: &str) -> Option<String> {
    if stderr.trim().is_empty() {
        return Some("yt-dlp search failed".to_owned());
    }
    Some("yt-dlp returned a redacted provider error".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProviderKind;

    #[test]
    fn provider_url_allowlist_rejects_http_javascript_file_data_and_wrong_hosts() {
        for value in [
            "http://www.youtube.com/watch?v=x",
            "javascript:alert(1)",
            "file:///secret",
            "data:text/plain,secret",
            "https://evil.example/video",
        ] {
            assert!(
                validate_provider_url(ProviderKind::Youtube, value).is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn artwork_allowlist_returns_null_for_unknown_https_cdn() {
        assert_eq!(sanitize_artwork_url("https://evil.example/art.jpg"), None);
    }

    #[test]
    fn provider_url_rejects_userinfo_and_sensitive_query_data() {
        assert!(validate_provider_url(
            ProviderKind::Youtube,
            "https://user:pass@youtube.com/watch"
        )
        .is_err());
        assert!(validate_provider_url(
            ProviderKind::Youtube,
            "https://youtube.com/watch?access_token=secret"
        )
        .is_err());
        assert!(validate_provider_url(
            ProviderKind::Youtube,
            "https://youtube.com/watch?v=decoder1234"
        )
        .is_ok());
        assert!(validate_provider_url(
            ProviderKind::Youtube,
            "https://youtube.com/watch?apiKey=secret"
        )
        .is_err());
        for query_name in ["oauth_token", "AUTH-TOKEN", "Api_Token"] {
            let url = format!("https://youtube.com/watch?{query_name}=secret");
            assert!(validate_provider_url(ProviderKind::Youtube, &url).is_err());
        }
    }
}
