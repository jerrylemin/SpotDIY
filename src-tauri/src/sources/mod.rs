pub mod traits;
pub use traits::SourceAdapter;

use crate::domain::ProviderKind;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ProviderUrlError {
    #[error("invalid provider URL")]
    Invalid,
    #[error("provider URL host is not allowlisted")]
    HostNotAllowed,
}

pub fn validate_provider_url(provider: ProviderKind, value: &str) -> Result<Url, ProviderUrlError> {
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
    allowed
        .then_some(url)
        .ok_or(ProviderUrlError::HostNotAllowed)
}

pub fn sanitize_artwork_url(value: &str) -> Option<Url> {
    let url = Url::parse(value).ok()?;
    (url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("i.ytimg.com" | "yt3.ggpht.com" | "i1.sndcdn.com" | "i.scdn.co")
        ))
    .then_some(url)
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
}
