use crate::domain::{ProviderKind, SourceId, TrackId};
use serde::{Deserialize, Serialize};
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub enum SearchEntityKind {
    Track,
    Artist,
    Album,
    Playlist,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
    pub limit: u8,
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
    pub limit: u8,
    pub market: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub provider: ProviderKind,
    pub entity_kind: SearchEntityKind,
    pub provider_item_id: String,
    pub canonical_url: Option<Url>,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub artwork_url: Option<Url>,
    pub published_at: Option<String>,
    pub published_precision: Option<PartialDatePrecision>,
    pub engagement_count: Option<u64>,
    pub engagement_kind: Option<EngagementKind>,
    pub explicit: Option<bool>,
    pub local_track_id: Option<TrackId>,
    pub local_source_id: Option<SourceId>,
    pub original_rank: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EngagementKind {
    Views,
    Plays,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PartialDatePrecision {
    Year,
    Month,
    Day,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialDate {
    pub value: String,
    pub precision: PartialDatePrecision,
}

impl PartialDate {
    pub fn month(year: i32, month: u8) -> Self {
        Self {
            value: format!("{year:04}-{month:02}"),
            precision: PartialDatePrecision::Month,
        }
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
#[serde(rename_all = "lowercase")]
pub enum ProviderSearchState {
    Idle,
    Loading,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSearchError {
    pub code: ProviderSearchErrorCode,
    pub detail: Option<String>,
    pub retry_after_seconds: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderSearchErrorCode {
    Unavailable,
    Timeout,
    Cancelled,
    RateLimited,
    InvalidResponse,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
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
        assert_eq!(date.value, "2026-08");
        assert_eq!(date.precision, PartialDatePrecision::Month);
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
