use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use url::Url;

use super::{LyricsCandidate, LyricsLookup};

pub const LRCLIB_BASE_URL: &str = "https://lrclib.net";
pub const LRCLIB_BODY_LIMIT: usize = 2 * 1024 * 1024;
pub const LRCLIB_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const LRCLIB_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
pub const LRCLIB_MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(300);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LyricsHttpResponse {
    pub status: u16,
    pub retry_after_seconds: Option<u64>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LyricsProviderError {
    #[error("LRCLIB returned no matching lyrics")]
    NotFound,
    #[error("LRCLIB rate limit exceeded")]
    RateLimited { retry_after_seconds: Option<u64> },
    #[error("LRCLIB response was too large")]
    OversizedResponse,
    #[error("LRCLIB returned an invalid response")]
    InvalidResponse,
    #[error("LRCLIB request timed out")]
    Timeout,
    #[error("LRCLIB network request failed")]
    Network,
    #[error("LRCLIB URL failed its HTTPS/host policy")]
    UnsafeUrl,
}

#[async_trait]
pub trait LyricsHttpTransport: Send + Sync {
    async fn get(
        &self,
        url: Url,
        user_agent: &str,
    ) -> Result<LyricsHttpResponse, LyricsProviderError>;
}

#[derive(Clone)]
pub struct ReqwestLyricsTransport {
    client: reqwest::Client,
}

impl ReqwestLyricsTransport {
    pub fn new() -> Result<Self, LyricsProviderError> {
        let client = reqwest::Client::builder()
            .connect_timeout(LRCLIB_CONNECT_TIMEOUT)
            .timeout(LRCLIB_REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| LyricsProviderError::Network)?;
        Ok(Self { client })
    }
}

impl Default for ReqwestLyricsTransport {
    fn default() -> Self {
        Self::new().expect("LRCLIB HTTP client configuration is static")
    }
}

#[async_trait]
impl LyricsHttpTransport for ReqwestLyricsTransport {
    async fn get(
        &self,
        url: Url,
        user_agent: &str,
    ) -> Result<LyricsHttpResponse, LyricsProviderError> {
        let response = self
            .client
            .get(url)
            .header(reqwest::header::USER_AGENT, user_agent)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    LyricsProviderError::Timeout
                } else {
                    LyricsProviderError::Network
                }
            })?;
        if response
            .content_length()
            .is_some_and(|length| length > LRCLIB_BODY_LIMIT as u64)
        {
            return Err(LyricsProviderError::OversizedResponse);
        }
        let retry_after_seconds = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok());
        let status = response.status().as_u16();
        let mut response = response;
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| {
            if error.is_timeout() {
                LyricsProviderError::Timeout
            } else {
                LyricsProviderError::Network
            }
        })? {
            if body.len().saturating_add(chunk.len()) > LRCLIB_BODY_LIMIT {
                return Err(LyricsProviderError::OversizedResponse);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(LyricsHttpResponse {
            status,
            retry_after_seconds,
            body,
        })
    }
}

#[derive(Clone)]
pub struct LrclibProvider {
    transport: Arc<dyn LyricsHttpTransport>,
    gate: Arc<Mutex<ProviderGate>>,
    user_agent: String,
}

#[derive(Debug)]
struct ProviderGate {
    last_request_at: Option<Instant>,
    not_before: Option<Instant>,
}

impl LrclibProvider {
    pub fn new() -> Result<Self, LyricsProviderError> {
        Self::with_transport(Arc::new(ReqwestLyricsTransport::default()))
    }

    pub fn with_transport(
        transport: Arc<dyn LyricsHttpTransport>,
    ) -> Result<Self, LyricsProviderError> {
        Ok(Self {
            transport,
            gate: Arc::new(Mutex::new(ProviderGate {
                last_request_at: None,
                not_before: None,
            })),
            user_agent: format!(
                "SpotDIY/{} (https://github.com/jerrylemin/SpotDIY)",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub(crate) async fn find_best(
        &self,
        lookup: &LyricsLookup,
    ) -> Result<LrclibRecord, LyricsProviderError> {
        let mut url = endpoint("/api/get")?;
        append_lookup(&mut url, lookup, true);
        self.request_record(url).await
    }

    pub(crate) async fn search(
        &self,
        lookup: &LyricsLookup,
    ) -> Result<Vec<LyricsCandidate>, LyricsProviderError> {
        let mut url = endpoint("/api/search")?;
        append_lookup(&mut url, lookup, false);
        let records: Vec<LrclibRecord> = self.request_json(url).await?;
        records
            .into_iter()
            .take(20)
            .map(LrclibRecord::candidate)
            .collect()
    }

    pub(crate) async fn get(
        &self,
        provider_record_id: i64,
    ) -> Result<LrclibRecord, LyricsProviderError> {
        if provider_record_id <= 0 {
            return Err(LyricsProviderError::InvalidResponse);
        }
        let url = endpoint(&format!("/api/get/{provider_record_id}"))?;
        self.request_record(url).await
    }

    async fn request_record(&self, url: Url) -> Result<LrclibRecord, LyricsProviderError> {
        self.request_json::<LrclibRecord>(url).await?.validated()
    }

    async fn request_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: Url,
    ) -> Result<T, LyricsProviderError> {
        validate_url(&url)?;
        let response = self.gated_get(url).await?;
        match response.status {
            200 => serde_json::from_slice(&response.body)
                .map_err(|_| LyricsProviderError::InvalidResponse),
            404 => Err(LyricsProviderError::NotFound),
            429 => Err(LyricsProviderError::RateLimited {
                retry_after_seconds: response.retry_after_seconds,
            }),
            _ => Err(LyricsProviderError::InvalidResponse),
        }
    }

    async fn gated_get(&self, url: Url) -> Result<LyricsHttpResponse, LyricsProviderError> {
        validate_url(&url)?;
        let mut gate = self.gate.lock().await;
        let now = Instant::now();
        let mut wait_until = gate.not_before;
        if let Some(last_request_at) = gate.last_request_at {
            let interval_end = last_request_at + LRCLIB_MIN_REQUEST_INTERVAL;
            wait_until = Some(wait_until.map_or(interval_end, |value| value.max(interval_end)));
        }
        if let Some(wait_until) = wait_until {
            if wait_until > now {
                tokio::time::sleep(wait_until - now).await;
            }
        }
        gate.last_request_at = Some(Instant::now());
        let response = self.transport.get(url, &self.user_agent).await?;
        if response.body.len() > LRCLIB_BODY_LIMIT {
            return Err(LyricsProviderError::OversizedResponse);
        }
        if response.status == 429 {
            gate.not_before = response
                .retry_after_seconds
                .map(|seconds| Instant::now() + Duration::from_secs(seconds));
        } else {
            gate.not_before = None;
        }
        Ok(response)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LrclibRecord {
    pub id: Option<i64>,
    pub track_name: Option<String>,
    pub artist_name: Option<String>,
    pub album_name: Option<String>,
    pub duration: Option<f64>,
    pub instrumental: Option<bool>,
    pub plain_lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
}

impl LrclibRecord {
    pub(crate) fn validated(self) -> Result<Self, LyricsProviderError> {
        if self.id.is_none_or(|value| value <= 0)
            || self
                .track_name
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            || self
                .artist_name
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(LyricsProviderError::InvalidResponse);
        }
        if self
            .duration
            .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(LyricsProviderError::InvalidResponse);
        }
        Ok(self)
    }

    pub(crate) fn candidate(self) -> Result<LyricsCandidate, LyricsProviderError> {
        let record = self.validated()?;
        Ok(LyricsCandidate {
            provider_record_id: record.id.expect("validated ID"),
            track_name: record.track_name.expect("validated track name"),
            artist_name: record.artist_name.expect("validated artist name"),
            album_name: record.album_name,
            duration_ms: duration_ms(record.duration)?,
            instrumental: record.instrumental.unwrap_or(false),
            has_plain: record
                .plain_lyrics
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
            has_synced: record
                .synced_lyrics
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        })
    }
}

pub(crate) fn duration_ms(duration: Option<f64>) -> Result<Option<u64>, LyricsProviderError> {
    duration
        .map(|value| {
            let millis = (value * 1_000.0).round();
            if !millis.is_finite() || millis < 0.0 || millis > u64::MAX as f64 {
                return Err(LyricsProviderError::InvalidResponse);
            }
            Ok(millis as u64)
        })
        .transpose()
}

fn endpoint(path: &str) -> Result<Url, LyricsProviderError> {
    Url::parse(&format!("{LRCLIB_BASE_URL}{path}")).map_err(|_| LyricsProviderError::UnsafeUrl)
}

fn validate_url(url: &Url) -> Result<(), LyricsProviderError> {
    if url.scheme() != "https"
        || url.host_str() != Some("lrclib.net")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(LyricsProviderError::UnsafeUrl);
    }
    Ok(())
}

fn append_lookup(url: &mut Url, lookup: &LyricsLookup, include_duration: bool) {
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("track_name", &lookup.track_name);
        query.append_pair("artist_name", &lookup.artist_name);
        if let Some(album_name) = lookup.album_name.as_deref() {
            query.append_pair("album_name", album_name);
        }
        if include_duration {
            if let Some(duration_ms) = lookup.duration_ms {
                query.append_pair("duration", &(duration_ms as f64 / 1_000.0).to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    #[derive(Default)]
    struct MockTransport {
        requests: StdMutex<Vec<(Url, String)>>,
        responses: StdMutex<Vec<LyricsHttpResponse>>,
    }

    #[async_trait]
    impl LyricsHttpTransport for MockTransport {
        async fn get(
            &self,
            url: Url,
            user_agent: &str,
        ) -> Result<LyricsHttpResponse, LyricsProviderError> {
            self.requests
                .lock()
                .unwrap()
                .push((url, user_agent.to_owned()));
            self.responses
                .lock()
                .unwrap()
                .pop()
                .ok_or(LyricsProviderError::Network)
        }
    }

    fn response(status: u16, body: &str) -> LyricsHttpResponse {
        LyricsHttpResponse {
            status,
            retry_after_seconds: None,
            body: body.as_bytes().to_vec(),
        }
    }

    fn lookup() -> LyricsLookup {
        LyricsLookup {
            track_name: "Synthetic Track".to_owned(),
            artist_name: "Synthetic Artist".to_owned(),
            album_name: Some("Synthetic Album".to_owned()),
            duration_ms: Some(181_000),
        }
    }

    #[tokio::test]
    async fn uses_exact_host_user_agent_and_structured_queries() {
        let transport = Arc::new(MockTransport::default());
        transport.responses.lock().unwrap().push(response(
            200,
            r#"{"id":7,"trackName":"Synthetic Track","artistName":"Synthetic Artist","albumName":"Synthetic Album","duration":181,"plainLyrics":"plain","syncedLyrics":"[00:01.00]line"}"#,
        ));
        let provider = LrclibProvider::with_transport(transport.clone()).unwrap();
        let record = provider.find_best(&lookup()).await.unwrap();
        assert_eq!(record.id, Some(7));
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0.host_str(), Some("lrclib.net"));
        assert_eq!(requests[0].0.scheme(), "https");
        assert_eq!(
            requests[0]
                .0
                .query_pairs()
                .find(|(key, _)| key == "track_name")
                .unwrap()
                .1,
            "Synthetic Track"
        );
        assert!(requests[0].1.starts_with("SpotDIY/"));
        assert!(requests[0]
            .1
            .contains("https://github.com/jerrylemin/SpotDIY"));
    }

    #[tokio::test]
    async fn search_is_metadata_only_and_404_is_typed() {
        let transport = Arc::new(MockTransport::default());
        transport.responses.lock().unwrap().push(response(
            200,
            r#"[{"id":7,"trackName":"Synthetic Track","artistName":"Synthetic Artist","plainLyrics":"secret"}]"#,
        ));
        let provider = LrclibProvider::with_transport(transport).unwrap();
        let candidates = provider.search(&lookup()).await.unwrap();
        assert_eq!(candidates[0].provider_record_id, 7);
        assert!(candidates[0].has_plain);

        let transport = Arc::new(MockTransport::default());
        transport
            .responses
            .lock()
            .unwrap()
            .push(response(404, "{}"));
        let provider = LrclibProvider::with_transport(transport).unwrap();
        assert_eq!(
            provider.find_best(&lookup()).await,
            Err(LyricsProviderError::NotFound)
        );
    }

    #[tokio::test]
    async fn rate_limit_carries_retry_after_without_retrying() {
        let transport = Arc::new(MockTransport::default());
        transport
            .responses
            .lock()
            .unwrap()
            .push(LyricsHttpResponse {
                status: 429,
                retry_after_seconds: Some(4),
                body: Vec::new(),
            });
        let provider = LrclibProvider::with_transport(transport.clone()).unwrap();
        assert_eq!(
            provider.find_best(&lookup()).await,
            Err(LyricsProviderError::RateLimited {
                retry_after_seconds: Some(4)
            })
        );
        assert_eq!(transport.requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rejects_oversized_bodies_before_json_is_parsed() {
        let transport = Arc::new(MockTransport::default());
        let body = LyricsHttpResponse {
            status: 200,
            retry_after_seconds: None,
            body: vec![0; LRCLIB_BODY_LIMIT + 1],
        };
        transport.responses.lock().unwrap().push(body);
        let provider = LrclibProvider::with_transport(transport).unwrap();
        assert_eq!(
            provider.find_best(&lookup()).await,
            Err(LyricsProviderError::OversizedResponse)
        );
    }

    #[test]
    fn rejects_unsafe_provider_endpoints_before_transport() {
        for value in [
            "http://lrclib.net/api/get",
            "https://example.test/api/get",
            "https://lrclib.net:444/api/get",
            "https://user:pass@lrclib.net/api/get",
        ] {
            let url = Url::parse(value).unwrap();
            assert_eq!(validate_url(&url), Err(LyricsProviderError::UnsafeUrl));
        }
    }
}
