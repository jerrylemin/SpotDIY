use crate::domain::{ProviderKind, SourceId, TrackId};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use tokio::sync::watch;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SearchId(Uuid);

impl SearchId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SearchId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchLens {
    All,
    Tracks,
    Artists,
    Albums,
    Playlists,
    Local,
    Youtube,
    Soundcloud,
    Spotify,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEntityKind {
    Track,
    Artist,
    Album,
    Playlist,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSortField {
    Relevance,
    Popularity,
    Newest,
    Oldest,
    Duration,
    DateAdded,
    Downloaded,
    AudioQuality,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub query: String,
    pub lens: SearchLens,
    pub sort_field: SearchSortField,
    pub sort_direction: SearchSortDirection,
    #[serde(default = "default_limit")]
    pub limit: u8,
}

fn default_limit() -> u8 {
    25
}

impl SearchRequest {
    pub fn validate(&self) -> Result<String, SearchValidationError> {
        let query = self.query.trim();
        if query.is_empty() {
            return Err(SearchValidationError::EmptyQuery);
        }
        let length = query.chars().count();
        if length > 256 {
            return Err(SearchValidationError::QueryTooLong { length });
        }
        if self.limit > 50 {
            return Err(SearchValidationError::LimitTooLarge { limit: self.limit });
        }
        Ok(query.to_owned())
    }

    #[cfg(test)]
    fn test_default() -> Self {
        Self {
            query: "signal".into(),
            lens: SearchLens::All,
            sort_field: SearchSortField::Relevance,
            sort_direction: SearchSortDirection::Descending,
            limit: 25,
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SearchValidationError {
    #[error("search query cannot be empty")]
    EmptyQuery,
    #[error("search query has {length} Unicode scalars; maximum is 256")]
    QueryTooLong { length: usize },
    #[error("search limit {limit} exceeds maximum 50")]
    LimitTooLarge { limit: u8 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSearchRequest {
    pub search_id: SearchId,
    pub query: String,
    pub lens: SearchLens,
    pub entities: Vec<SearchEntityKind>,
    pub sort_field: SearchSortField,
    pub sort_direction: SearchSortDirection,
    #[serde(default = "default_limit")]
    pub limit: u8,
    pub market: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub provider: ProviderKind,
    pub entity_kind: SearchEntityKind,
    pub provider_item_id: String,
    pub canonical_url: Option<SafeUrl>,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub artwork_url: Option<SafeUrl>,
    pub published_at: Option<PartialDate>,
    pub engagement_count: Option<u64>,
    pub engagement_kind: Option<EngagementKind>,
    pub explicit: Option<bool>,
    pub local_track_id: Option<TrackId>,
    pub local_source_id: Option<SourceId>,
    pub original_rank: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngagementKind {
    Views,
    Plays,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialDatePrecision {
    Year,
    Month,
    Day,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialDate {
    value: String,
    precision: PartialDatePrecision,
}

impl PartialDate {
    pub fn new(value: impl Into<String>, precision: PartialDatePrecision) -> Result<Self, String> {
        let value = value.into();
        let valid = match precision {
            PartialDatePrecision::Year => value.len() == 4 && value.parse::<u16>().is_ok(),
            PartialDatePrecision::Month => {
                chrono::NaiveDate::parse_from_str(&format!("{value}-01"), "%Y-%m-%d").is_ok()
            }
            PartialDatePrecision::Day => {
                chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d").is_ok()
            }
        };
        valid
            .then_some(Self { value, precision })
            .ok_or_else(|| "date does not match its precision".to_owned())
    }

    pub fn month(year: i32, month: u8) -> Self {
        Self::new(format!("{year:04}-{month:02}"), PartialDatePrecision::Month)
            .expect("valid month")
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn precision(&self) -> PartialDatePrecision {
        self.precision
    }
}

impl<'de> Deserialize<'de> for PartialDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireDate {
            value: String,
            precision: PartialDatePrecision,
        }
        let wire = WireDate::deserialize(deserializer)?;
        Self::new(wire.value, wire.precision).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeUrl(Url);

impl SafeUrl {
    pub(crate) fn from_url(url: Url) -> Option<Self> {
        let host = url.host_str()?;
        let allowed = matches!(
            host,
            "youtube.com"
                | "www.youtube.com"
                | "music.youtube.com"
                | "youtu.be"
                | "soundcloud.com"
                | "www.soundcloud.com"
                | "open.spotify.com"
                | "spotify.com"
                | "www.spotify.com"
                | "i.ytimg.com"
                | "yt3.ggpht.com"
                | "i1.sndcdn.com"
                | "i.scdn.co"
        );
        (url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && !contains_sensitive_query(&url)
            && allowed)
            .then_some(Self(url))
    }

    pub fn as_url(&self) -> &Url {
        &self.0
    }
}

fn contains_sensitive_query(url: &Url) -> bool {
    url.query_pairs().any(|(key, _)| {
        let key = key.to_ascii_lowercase().replace('-', "_");
        let components: Vec<_> = key.split('_').collect();
        let composite_credential = components.contains(&"token")
            && components.iter().any(|component| {
                matches!(*component, "oauth" | "auth" | "api" | "access" | "refresh")
            });
        composite_credential
            || matches!(
                key.as_str(),
                "auth"
                    | "authorization"
                    | "access_token"
                    | "accesstoken"
                    | "refresh_token"
                    | "refreshtoken"
                    | "client_secret"
                    | "clientsecret"
                    | "api_key"
                    | "apikey"
                    | "cookie"
                    | "password"
                    | "token"
                    | "secret"
                    | "code"
                    | "oauth"
            )
    })
}

impl Serialize for SafeUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for SafeUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let url = Url::parse(&value).map_err(D::Error::custom)?;
        Self::from_url(url).ok_or_else(|| D::Error::custom("URL is not safe for a search result"))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSearchSection {
    pub provider: ProviderKind,
    pub state: ProviderSearchState,
    pub results: Vec<SearchResult>,
    pub error: Option<ProviderSearchError>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSearchState {
    Idle,
    Loading,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ProviderSearchError {
    pub code: ProviderSearchErrorCode,
    pub detail: Option<String>,
    #[serde(rename = "retryAfterSeconds")]
    pub retry_after_seconds: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSearchErrorCode {
    Unavailable,
    Timeout,
    Cancelled,
    RateLimited,
    InvalidResponse,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRuntimeStatus {
    Unknown,
    Ready,
    Missing,
    Unsupported,
    Broken,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchStarted {
    pub search_id: SearchId,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCompleted {
    pub search_id: SearchId,
}

#[derive(Clone, Debug)]
pub struct SearchCancellation {
    sender: watch::Sender<bool>,
}

impl SearchCancellation {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(false);
        Self { sender }
    }
    pub fn cancel(&self) {
        let _ = self.sender.send(true);
    }
    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.sender.subscribe()
    }
}
impl Default for SearchCancellation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_request_trims_and_rejects_empty_query() {
        let request = SearchRequest {
            query: "  ".into(),
            ..SearchRequest::test_default()
        };
        assert!(matches!(
            request.validate(),
            Err(SearchValidationError::EmptyQuery)
        ));
    }

    #[test]
    fn search_request_accepts_and_returns_trimmed_query() {
        let request = SearchRequest {
            query: "  signal  ".into(),
            ..SearchRequest::test_default()
        };
        assert_eq!(request.validate().unwrap(), "signal");
    }

    #[test]
    fn search_request_rejects_257_unicode_scalars() {
        let request = SearchRequest {
            query: "a".repeat(257),
            ..SearchRequest::test_default()
        };
        assert!(matches!(
            request.validate(),
            Err(SearchValidationError::QueryTooLong { .. })
        ));
    }

    #[test]
    fn search_request_rejects_limit_above_50() {
        let request = SearchRequest {
            limit: 51,
            ..SearchRequest::test_default()
        };
        assert!(matches!(
            request.validate(),
            Err(SearchValidationError::LimitTooLarge { .. })
        ));
    }

    #[test]
    fn partial_date_preserves_year_and_month_precision() {
        let date = PartialDate::month(2026, 8);
        assert_eq!(date.value(), "2026-08");
        assert_eq!(date.precision(), PartialDatePrecision::Month);
    }

    #[test]
    fn partial_date_rejects_inconsistent_precision() {
        let json = r#"{"value":"2026-08","precision":"day"}"#;
        assert!(serde_json::from_str::<PartialDate>(json).is_err());
    }

    #[test]
    fn search_request_deserialization_defaults_limit_to_25() {
        let json = r#"{"query":"signal","lens":"all","sortField":"relevance","sortDirection":"descending"}"#;
        let request: SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.limit, 25);
    }

    #[test]
    fn native_enums_use_snake_case_wire_names() {
        assert_eq!(
            serde_json::to_string(&SearchSortField::AudioQuality).unwrap(),
            "\"audio_quality\""
        );
        assert_eq!(
            serde_json::to_string(&ProviderSearchErrorCode::InvalidResponse).unwrap(),
            "\"invalid_response\""
        );
    }

    #[test]
    fn provider_error_fields_keep_camel_case_wire_names() {
        let error = ProviderSearchError {
            code: ProviderSearchErrorCode::Timeout,
            detail: None,
            retry_after_seconds: Some(3),
        };
        let value = serde_json::to_value(error).unwrap();
        assert_eq!(value["retryAfterSeconds"], 3);
        assert!(value.get("retry_after_seconds").is_none());
    }

    #[tokio::test]
    async fn cancellation_watch_changes_to_cancelled() {
        let cancellation = SearchCancellation::new();
        let mut receiver = cancellation.subscribe();
        cancellation.cancel();
        receiver.changed().await.unwrap();
        assert!(*receiver.borrow());
    }
}
