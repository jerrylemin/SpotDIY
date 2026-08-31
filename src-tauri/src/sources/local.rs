use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;

use rusqlite::{params, Row};

use crate::db::Database;
use crate::domain::{ProviderKind, SourceCapabilities, SourceId, TrackId};
use crate::search::types::{
    ProviderRuntimeStatus, ProviderSearchError, ProviderSearchErrorCode, ProviderSearchRequest,
    ProviderSearchSection, ProviderSearchState, SearchCancellation, SearchEntityKind, SearchResult,
};
use crate::sources::SourceAdapter;

const MAX_LOCAL_RESULTS: usize = 50;
const MAX_LOCAL_CANDIDATES_PER_ENTITY: usize = MAX_LOCAL_RESULTS;
const LOCAL_CAPABILITIES: SourceCapabilities = SourceCapabilities {
    search: true,
    playback: true,
    metadata: true,
    artwork: true,
    lyrics: true,
    downloads: false,
    popularity: false,
    release_date: false,
    lyrics_metadata: true,
};
const SUPPORTED_ENTITIES: &[SearchEntityKind] = &[
    SearchEntityKind::Track,
    SearchEntityKind::Artist,
    SearchEntityKind::Album,
];

const TRACK_SQL: &str = r#"
    SELECT t.id, ts.id, ts.provider_item_id, t.title,
           COALESCE((
               SELECT json_group_array(artist_name)
               FROM (
                   SELECT a.name AS artist_name
                   FROM track_artists ta
                   INNER JOIN artists a ON a.id = ta.artist_id
                   WHERE ta.track_id = t.id
                   ORDER BY ta.artist_order ASC, a.id ASC
               )
           ), '[]'),
           al.title, COALESCE(ts.duration_ms, t.duration_ms)
    FROM local_files lf
    INNER JOIN track_sources ts ON ts.id = lf.source_id
    INNER JOIN tracks t ON t.id = ts.track_id
    LEFT JOIN albums al ON al.id = t.album_id
    WHERE ts.provider_kind = 'local'
      AND lf.library_folder_id IS NOT NULL
      AND lf.index_status = 'indexed'
      AND lower(t.title) LIKE ?1 ESCAPE '\'
    ORDER BY t.title COLLATE NOCASE ASC,
             COALESCE((
                 SELECT a.name
                 FROM track_artists ta
                 INNER JOIN artists a ON a.id = ta.artist_id
                 WHERE ta.track_id = t.id
                 ORDER BY ta.artist_order ASC, a.id ASC
                 LIMIT 1
             ), '') COLLATE NOCASE ASC,
             COALESCE(al.title, '') COLLATE NOCASE ASC,
             ts.id ASC
    LIMIT ?2
"#;

const ARTIST_SQL: &str = r#"
    SELECT t.id, ts.id, ts.provider_item_id, t.title,
           COALESCE((
               SELECT json_group_array(artist_name)
               FROM (
                   SELECT a.name AS artist_name
                   FROM track_artists ta
                   INNER JOIN artists a ON a.id = ta.artist_id
                   WHERE ta.track_id = t.id
                   ORDER BY ta.artist_order ASC, a.id ASC
               )
           ), '[]'),
           al.title, COALESCE(ts.duration_ms, t.duration_ms)
    FROM local_files lf
    INNER JOIN track_sources ts ON ts.id = lf.source_id
    INNER JOIN tracks t ON t.id = ts.track_id
    LEFT JOIN albums al ON al.id = t.album_id
    WHERE ts.provider_kind = 'local'
      AND lf.library_folder_id IS NOT NULL
      AND lf.index_status = 'indexed'
      AND EXISTS (
          SELECT 1
          FROM track_artists ta
          INNER JOIN artists a ON a.id = ta.artist_id
          WHERE ta.track_id = t.id
            AND lower(a.name) LIKE ?1 ESCAPE '\'
      )
    ORDER BY t.title COLLATE NOCASE ASC,
             COALESCE((
                 SELECT a.name
                 FROM track_artists ta
                 INNER JOIN artists a ON a.id = ta.artist_id
                 WHERE ta.track_id = t.id
                 ORDER BY ta.artist_order ASC, a.id ASC
                 LIMIT 1
             ), '') COLLATE NOCASE ASC,
             COALESCE(al.title, '') COLLATE NOCASE ASC,
             ts.id ASC
    LIMIT ?2
"#;

const ALBUM_SQL: &str = r#"
    SELECT t.id, ts.id, ts.provider_item_id, t.title,
           COALESCE((
               SELECT json_group_array(artist_name)
               FROM (
                   SELECT a.name AS artist_name
                   FROM track_artists ta
                   INNER JOIN artists a ON a.id = ta.artist_id
                   WHERE ta.track_id = t.id
                   ORDER BY ta.artist_order ASC, a.id ASC
               )
           ), '[]'),
           al.title, COALESCE(ts.duration_ms, t.duration_ms)
    FROM local_files lf
    INNER JOIN track_sources ts ON ts.id = lf.source_id
    INNER JOIN tracks t ON t.id = ts.track_id
    INNER JOIN albums al ON al.id = t.album_id
    WHERE ts.provider_kind = 'local'
      AND lf.library_folder_id IS NOT NULL
      AND lf.index_status = 'indexed'
      AND lower(al.title) LIKE ?1 ESCAPE '\'
    ORDER BY t.title COLLATE NOCASE ASC,
             COALESCE((
                 SELECT a.name
                 FROM track_artists ta
                 INNER JOIN artists a ON a.id = ta.artist_id
                 WHERE ta.track_id = t.id
                 ORDER BY ta.artist_order ASC, a.id ASC
                 LIMIT 1
             ), '') COLLATE NOCASE ASC,
             al.title COLLATE NOCASE ASC,
             ts.id ASC
    LIMIT ?2
"#;

#[derive(Clone)]
pub struct LocalSourceAdapter {
    database: Database,
    #[cfg(test)]
    executed_queries: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl LocalSourceAdapter {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            #[cfg(test)]
            executed_queries: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    #[cfg(test)]
    fn executed_query_count(&self) -> usize {
        self.executed_queries
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn search_local(&self, request: &ProviderSearchRequest) -> Result<Vec<SearchResult>, String> {
        let query = request.query.trim();
        let limit = usize::from(request.limit).min(MAX_LOCAL_RESULTS);
        if query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let pattern = format!("%{}%", escape_like(&query.to_lowercase()));
        let mut rows = Vec::new();
        for entity in SUPPORTED_ENTITIES {
            if request.entities.contains(entity) {
                rows.extend(self.query_entity(
                    *entity,
                    &pattern,
                    MAX_LOCAL_CANDIDATES_PER_ENTITY,
                )?);
            }
        }

        let mut seen_source_ids = HashSet::new();
        let mut results = rows
            .into_iter()
            .filter(|row| seen_source_ids.insert(row.source_id.clone()))
            .map(LocalSearchRow::into_result)
            .collect::<Result<Vec<_>, _>>()?;
        results.sort_by(|left, right| local_result_order(left, right, query));
        results.truncate(limit);
        for (rank, result) in results.iter_mut().enumerate() {
            result.original_rank = u32::try_from(rank).unwrap_or(u32::MAX);
        }
        Ok(results)
    }

    fn query_entity(
        &self,
        entity: SearchEntityKind,
        pattern: &str,
        limit: usize,
    ) -> Result<Vec<LocalSearchRow>, String> {
        let sql = match entity {
            SearchEntityKind::Track => TRACK_SQL,
            SearchEntityKind::Artist => ARTIST_SQL,
            SearchEntityKind::Album => ALBUM_SQL,
            SearchEntityKind::Playlist => return Ok(Vec::new()),
        };
        let limit = i64::try_from(limit).map_err(|_| "local result limit is invalid".to_owned())?;
        self.database
            .with_connection(|connection| {
                let mut statement = connection.prepare(sql)?;
                let rows = statement
                    .query_map(params![pattern, limit], map_local_search_row)?
                    .collect::<Result<Vec<_>, _>>()?;
                #[cfg(test)]
                self.executed_queries
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Ok(rows)
            })
            .map_err(|_| "local library search failed".to_owned())
    }
}

impl SourceAdapter for LocalSourceAdapter {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Local
    }

    fn capabilities(&self) -> SourceCapabilities {
        LOCAL_CAPABILITIES
    }

    fn supported_entities(&self) -> &'static [SearchEntityKind] {
        SUPPORTED_ENTITIES
    }

    fn runtime_status(&self) -> ProviderRuntimeStatus {
        ProviderRuntimeStatus::Ready
    }

    fn search(
        &self,
        request: ProviderSearchRequest,
        cancellation: SearchCancellation,
    ) -> Pin<Box<dyn Future<Output = ProviderSearchSection> + Send + '_>> {
        Box::pin(async move {
            let cancellation_rx = cancellation.subscribe();
            if *cancellation_rx.borrow() {
                return cancelled_section();
            }
            match self.search_local(&request) {
                Ok(_results) if *cancellation_rx.borrow() => cancelled_section(),
                Ok(results) => ready_section(results),
                Err(_) => failed_section(),
            }
        })
    }
}

#[derive(Debug)]
struct LocalSearchRow {
    track_id: String,
    source_id: String,
    provider_item_id: String,
    title: String,
    artists_json: String,
    album: Option<String>,
    duration_ms: Option<i64>,
}

impl LocalSearchRow {
    fn into_result(self) -> Result<SearchResult, String> {
        let track_id = TrackId::parse_str(&self.track_id)
            .map_err(|_| "local library search returned an invalid track identifier".to_owned())?;
        let source_id = SourceId::parse_str(&self.source_id)
            .map_err(|_| "local library search returned an invalid source identifier".to_owned())?;
        let artists = serde_json::from_str(&self.artists_json)
            .map_err(|_| "local library search returned invalid artist metadata".to_owned())?;
        let duration_ms = self
            .duration_ms
            .map(|value| {
                u64::try_from(value)
                    .map_err(|_| "local library search returned an invalid duration".to_owned())
            })
            .transpose()?;
        Ok(SearchResult {
            provider: ProviderKind::Local,
            entity_kind: SearchEntityKind::Track,
            provider_item_id: self.provider_item_id,
            canonical_url: None,
            title: self.title,
            artists,
            album: self.album,
            duration_ms,
            artwork_url: None,
            published_at: None,
            engagement_count: None,
            engagement_kind: None,
            explicit: None,
            local_track_id: Some(track_id),
            local_source_id: Some(source_id),
            original_rank: 0,
        })
    }
}

fn map_local_search_row(row: &Row<'_>) -> rusqlite::Result<LocalSearchRow> {
    Ok(LocalSearchRow {
        track_id: row.get(0)?,
        source_id: row.get(1)?,
        provider_item_id: row.get(2)?,
        title: row.get(3)?,
        artists_json: row.get(4)?,
        album: row.get(5)?,
        duration_ms: row.get(6)?,
    })
}

fn local_result_order(
    left: &SearchResult,
    right: &SearchResult,
    query: &str,
) -> std::cmp::Ordering {
    local_match_rank(left, query)
        .cmp(&local_match_rank(right, query))
        .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        .then_with(|| {
            left.artists
                .first()
                .map(|artist| artist.to_lowercase())
                .cmp(&right.artists.first().map(|artist| artist.to_lowercase()))
        })
        .then_with(|| {
            left.album
                .as_ref()
                .map(|album| album.to_lowercase())
                .cmp(&right.album.as_ref().map(|album| album.to_lowercase()))
        })
        .then_with(|| {
            left.local_source_id
                .map(|source_id| source_id.to_string())
                .cmp(&right.local_source_id.map(|source_id| source_id.to_string()))
        })
}

fn local_match_rank(result: &SearchResult, query: &str) -> u8 {
    let query = query.to_lowercase();
    std::iter::once(result.title.as_str())
        .chain(result.artists.iter().map(String::as_str))
        .chain(result.album.iter().map(String::as_str))
        .map(|value| value.to_lowercase())
        .filter_map(|value| {
            (value == query)
                .then_some(0)
                .or_else(|| value.starts_with(&query).then_some(1))
                .or_else(|| value.contains(&query).then_some(2))
        })
        .min()
        .unwrap_or(3)
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn ready_section(results: Vec<SearchResult>) -> ProviderSearchSection {
    ProviderSearchSection {
        provider: ProviderKind::Local,
        state: ProviderSearchState::Ready,
        results,
        error: None,
    }
}

fn cancelled_section() -> ProviderSearchSection {
    ProviderSearchSection {
        provider: ProviderKind::Local,
        state: ProviderSearchState::Cancelled,
        results: Vec::new(),
        error: Some(ProviderSearchError {
            code: ProviderSearchErrorCode::Cancelled,
            detail: None,
            retry_after_seconds: None,
        }),
    }
}

fn failed_section() -> ProviderSearchSection {
    ProviderSearchSection {
        provider: ProviderKind::Local,
        state: ProviderSearchState::Failed,
        results: Vec::new(),
        error: Some(ProviderSearchError {
            code: ProviderSearchErrorCode::Failed,
            detail: Some("local library search failed".to_owned()),
            retry_after_seconds: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use crate::db::{Database, TempDatabasePath};
    use crate::domain::{AlbumId, ArtistId, LibraryFolderId, ProviderKind, SourceId, TrackId};
    use crate::library::{LibraryError, LibraryService};
    use crate::search::types::{
        ProviderSearchRequest, SearchCancellation, SearchEntityKind, SearchId, SearchLens,
        SearchSortDirection, SearchSortField,
    };
    use crate::sources::SourceAdapter;
    use rusqlite::params;

    use super::LocalSourceAdapter;

    #[tokio::test]
    async fn local_exact_title_ranks_first() {
        let fixture = LocalFixture::new("exact-title");
        fixture.add_track("Signal Echo", &["First Artist"], None, true);
        fixture.add_track("Signal", &["Second Artist"], None, true);

        let section = fixture.search("Signal", SearchLens::Local).await;

        assert_eq!(section.results[0].title, "Signal");
    }

    #[tokio::test]
    async fn local_title_prefix_precedes_substring() {
        let fixture = LocalFixture::new("prefix");
        fixture.add_track("Night Signal", &["Artist"], None, true);
        fixture.add_track("Signal Echo", &["Artist"], None, true);

        let section = fixture.search("Signal", SearchLens::Local).await;

        assert_eq!(section.results[0].title, "Signal Echo");
    }

    #[tokio::test]
    async fn local_artist_match_returns_track() {
        let fixture = LocalFixture::new("artist");
        fixture.add_track("Other Title", &["Signal Artist"], None, true);

        let section = fixture.search("signal artist", SearchLens::Local).await;

        assert_eq!(section.results.len(), 1);
        assert_eq!(section.results[0].entity_kind, SearchEntityKind::Track);
        assert_eq!(section.results[0].title, "Other Title");
    }

    #[tokio::test]
    async fn local_album_match_returns_track() {
        let fixture = LocalFixture::new("album");
        fixture.add_track("Other Title", &["Artist"], Some("Signal Album"), true);

        let section = fixture.search("signal album", SearchLens::Local).await;

        assert_eq!(section.results.len(), 1);
        assert_eq!(section.results[0].entity_kind, SearchEntityKind::Track);
        assert_eq!(section.results[0].title, "Other Title");
    }

    #[tokio::test]
    async fn local_matching_is_case_insensitive() {
        let fixture = LocalFixture::new("case");
        fixture.add_track("SiGnAl", &["Artist"], None, true);

        let section = fixture.search("signal", SearchLens::Local).await;

        assert_eq!(section.results[0].title, "SiGnAl");
    }

    #[tokio::test]
    async fn local_like_wildcards_are_literal() {
        let fixture = LocalFixture::new("wildcards");
        fixture.add_track("100% Signal", &["Artist"], None, true);
        fixture.add_track("100x Signal", &["Artist"], None, true);
        fixture.add_track("Under_score", &["Artist"], None, true);
        fixture.add_track("UnderXscore", &["Artist"], None, true);
        fixture.add_track(r"Slash\Signal", &["Artist"], None, true);

        let percent = fixture.search("100%", SearchLens::Local).await;
        let underscore = fixture.search("under_score", SearchLens::Local).await;
        let backslash = fixture.search("slash\\", SearchLens::Local).await;

        assert_eq!(percent.results.len(), 1);
        assert_eq!(percent.results[0].title, "100% Signal");
        assert_eq!(underscore.results.len(), 1);
        assert_eq!(underscore.results[0].title, "Under_score");
        assert_eq!(backslash.results.len(), 1);
        assert_eq!(backslash.results[0].title, r"Slash\Signal");
    }

    #[tokio::test]
    async fn local_empty_query_returns_no_provider_request() {
        let fixture = LocalFixture::new("empty");
        fixture.add_track("Signal", &["Artist"], None, true);

        let section = fixture.search("", SearchLens::Local).await;

        assert!(section.results.is_empty());
        assert_eq!(fixture.adapter.executed_query_count(), 0);
    }

    #[tokio::test]
    async fn local_tie_order_is_title_then_stable_id() {
        let fixture = LocalFixture::new("tie-order");
        let (_, second_source_id) = fixture.add_track_with_ids(
            TrackId::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
            SourceId::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
            "Signal",
            &["Artist"],
            None,
            true,
        );
        let (_, first_source_id) = fixture.add_track_with_ids(
            TrackId::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            SourceId::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            "Signal",
            &["Artist"],
            None,
            true,
        );

        let section = fixture.search("Signal", SearchLens::Local).await;

        assert_eq!(section.results[0].local_source_id, Some(first_source_id));
        assert_eq!(section.results[1].local_source_id, Some(second_source_id));
    }

    #[tokio::test]
    async fn local_limit_is_bounded() {
        let fixture = LocalFixture::new("limit");
        fixture.add_track("Signal One", &["Artist"], None, true);
        fixture.add_track("Signal Two", &["Artist"], None, true);

        let section = fixture
            .search_with_limit("Signal", SearchLens::Local, 1)
            .await;

        assert_eq!(section.results.len(), 1);
        assert_eq!(fixture.adapter.executed_query_count(), 3);
    }

    #[tokio::test]
    async fn local_relevance_precedes_requested_limit() {
        let fixture = LocalFixture::new("relevance-before-limit");
        fixture.add_track("A Signal Title Candidate", &["Artist"], None, true);
        let (_, exact_title_source_id) = fixture.add_track("Signal", &["Artist"], None, true);
        fixture.add_track("A Artist Candidate", &["Not Signal Artist"], None, true);
        let (_, exact_artist_source_id) =
            fixture.add_track("Z Artist Exact", &["Signal"], None, true);
        fixture.add_track(
            "A Album Candidate",
            &["Artist"],
            Some("Not Signal Album"),
            true,
        );
        let (_, exact_album_source_id) =
            fixture.add_track("Z Album Exact", &["Artist"], Some("Signal"), true);

        let title = fixture
            .search_with_entities(
                "Signal",
                SearchLens::Local,
                vec![SearchEntityKind::Track],
                1,
            )
            .await;
        let artist = fixture
            .search_with_entities(
                "Signal",
                SearchLens::Local,
                vec![SearchEntityKind::Artist],
                1,
            )
            .await;
        let album = fixture
            .search_with_entities(
                "Signal",
                SearchLens::Local,
                vec![SearchEntityKind::Album],
                1,
            )
            .await;

        assert_eq!(
            title.results[0].local_source_id,
            Some(exact_title_source_id)
        );
        assert_eq!(
            artist.results[0].local_source_id,
            Some(exact_artist_source_id)
        );
        assert_eq!(
            album.results[0].local_source_id,
            Some(exact_album_source_id)
        );
    }

    #[tokio::test]
    async fn local_multiple_artists_are_retained_in_order() {
        let fixture = LocalFixture::new("artists-order");
        fixture.add_track("Signal", &["First Artist", "Second Artist"], None, true);

        let section = fixture.search("Signal", SearchLens::Local).await;

        assert_eq!(
            section.results[0].artists,
            vec!["First Artist", "Second Artist"]
        );
    }

    #[tokio::test]
    async fn local_track_returns_track_and_source_ids() {
        let fixture = LocalFixture::new("ids");
        let (track_id, source_id) = fixture.add_track("Signal", &["Artist"], None, true);

        let section = fixture.search("Signal", SearchLens::Local).await;

        assert_eq!(section.results[0].provider, ProviderKind::Local);
        assert_eq!(section.results[0].local_track_id, Some(track_id));
        assert_eq!(section.results[0].local_source_id, Some(source_id));
        assert!(section.results[0].canonical_url.is_none());
        assert!(section.results[0].artwork_url.is_none());
    }

    #[tokio::test]
    async fn local_unavailable_source_is_not_playable() {
        let fixture = LocalFixture::new("unavailable");
        let (track_id, source_id) = fixture.add_track("Signal", &["Artist"], None, false);

        let section = fixture.search("Signal", SearchLens::Local).await;
        let artwork = tempfile::tempdir().unwrap();
        let library = LibraryService::new(fixture.database.clone(), artwork.path()).unwrap();

        assert_eq!(section.results[0].local_track_id, Some(track_id));
        assert_eq!(section.results[0].local_source_id, Some(source_id));
        assert!(matches!(
            library.resolve_playback_path(track_id, source_id),
            Err(LibraryError::SourceUnavailable { .. })
        ));
    }

    #[tokio::test]
    async fn local_page_uses_one_bounded_query_per_entity_kind() {
        let fixture = LocalFixture::new("query-count");
        fixture.add_track("Signal", &["Signal Artist"], Some("Signal Album"), true);

        let section = fixture.search("Signal", SearchLens::Local).await;

        assert!(!section.results.is_empty());
        assert_eq!(fixture.adapter.executed_query_count(), 3);
    }

    struct LocalFixture {
        _database_path: TempDatabasePath,
        database: Database,
        adapter: LocalSourceAdapter,
        folder_id: LibraryFolderId,
    }

    impl LocalFixture {
        fn new(label: &str) -> Self {
            let database_path = TempDatabasePath::new(label);
            let database = Database::open(database_path.path()).unwrap();
            let folder_id = LibraryFolderId::new();
            let now = "2026-08-31T00:00:00Z";
            database
                .with_connection(|connection| {
                    connection.execute(
                        "INSERT INTO library_folders (
                            id, path, normalized_path_key, enabled, scan_status, scan_generation,
                            created_at, updated_at
                         ) VALUES (?1, ?2, ?3, 1, 'complete', 1, ?4, ?4)",
                        params![
                            folder_id.to_string(),
                            "C:/SpotDIY-test-library",
                            "c:/spotdiy-test-library",
                            now,
                        ],
                    )?;
                    Ok(())
                })
                .unwrap();
            let adapter = LocalSourceAdapter::new(database.clone());
            Self {
                _database_path: database_path,
                database,
                adapter,
                folder_id,
            }
        }

        fn add_track(
            &self,
            title: &str,
            artists: &[&str],
            album: Option<&str>,
            available: bool,
        ) -> (TrackId, SourceId) {
            self.add_track_with_ids(
                TrackId::new(),
                SourceId::new(),
                title,
                artists,
                album,
                available,
            )
        }

        fn add_track_with_ids(
            &self,
            track_id: TrackId,
            source_id: SourceId,
            title: &str,
            artists: &[&str],
            album: Option<&str>,
            available: bool,
        ) -> (TrackId, SourceId) {
            let now = "2026-08-31T00:00:00Z";
            self.database
                .with_connection(|connection| {
                    let album_id = album
                        .map(|album_title| {
                            let album_id = AlbumId::new();
                            connection.execute(
                                "INSERT INTO albums (id, title, created_at, updated_at)
                             VALUES (?1, ?2, ?3, ?3)",
                                params![album_id.to_string(), album_title, now],
                            )?;
                            Ok::<_, rusqlite::Error>(album_id)
                        })
                        .transpose()?;
                    connection.execute(
                        "INSERT INTO tracks (
                            id, title, normalized_title, album_id, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                        params![
                            track_id.to_string(),
                            title,
                            title.to_lowercase(),
                            album_id.map(|value| value.to_string()),
                            now,
                        ],
                    )?;
                    for (artist_order, artist_name) in artists.iter().enumerate() {
                        let artist_id = ArtistId::new();
                        connection.execute(
                            "INSERT INTO artists (id, name, created_at, updated_at)
                             VALUES (?1, ?2, ?3, ?3)",
                            params![artist_id.to_string(), artist_name, now],
                        )?;
                        connection.execute(
                            "INSERT INTO track_artists (track_id, artist_id, artist_order)
                             VALUES (?1, ?2, ?3)",
                            params![
                                track_id.to_string(),
                                artist_id.to_string(),
                                i64::try_from(artist_order)
                                    .expect("fixture artist order fits SQLite"),
                            ],
                        )?;
                    }
                    connection.execute(
                        "INSERT INTO track_sources (
                            id, track_id, provider_kind, provider_item_id, available,
                            can_search, can_metadata, can_artwork, can_playback,
                            created_at, updated_at
                         ) VALUES (?1, ?2, 'local', ?3, ?4, 1, 1, 1, 1, ?5, ?5)",
                        params![
                            source_id.to_string(),
                            track_id.to_string(),
                            format!("fixture-{}", source_id),
                            i64::from(available),
                            now,
                        ],
                    )?;
                    connection.execute(
                        "INSERT INTO local_files (
                            source_id, path, library_folder_id, normalized_path_key, index_status,
                            created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, 'indexed', ?5, ?5)",
                        params![
                            source_id.to_string(),
                            format!("C:/SpotDIY-test-library/{source_id}.flac"),
                            self.folder_id.to_string(),
                            format!("c:/spotdiy-test-library/{source_id}.flac"),
                            now,
                        ],
                    )?;
                    Ok(())
                })
                .unwrap();
            (track_id, source_id)
        }

        async fn search(
            &self,
            query: &str,
            lens: SearchLens,
        ) -> crate::search::types::ProviderSearchSection {
            self.search_with_limit(query, lens, 25).await
        }

        async fn search_with_limit(
            &self,
            query: &str,
            lens: SearchLens,
            limit: u8,
        ) -> crate::search::types::ProviderSearchSection {
            self.search_with_entities(
                query,
                lens,
                vec![
                    SearchEntityKind::Track,
                    SearchEntityKind::Artist,
                    SearchEntityKind::Album,
                ],
                limit,
            )
            .await
        }

        async fn search_with_entities(
            &self,
            query: &str,
            lens: SearchLens,
            entities: Vec<SearchEntityKind>,
            limit: u8,
        ) -> crate::search::types::ProviderSearchSection {
            self.adapter
                .search(
                    ProviderSearchRequest {
                        search_id: SearchId::new(),
                        query: query.to_owned(),
                        lens,
                        entities,
                        sort_field: SearchSortField::Relevance,
                        sort_direction: SearchSortDirection::Descending,
                        limit,
                        market: None,
                    },
                    SearchCancellation::new(),
                )
                .await
        }
    }
}
