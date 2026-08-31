pub mod local;
pub mod soundcloud;
pub mod traits;
pub mod youtube;
pub use crate::media_tools::yt_dlp;
pub use local::LocalSourceAdapter;
pub use traits::SourceAdapter;

use crate::domain::ProviderKind;
use crate::search::types::SafeUrl;
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
