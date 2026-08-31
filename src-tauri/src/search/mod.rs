use super::*;
use crate::domain::ProviderKind;
use crate::search::sort::{all_provider_kinds_for_lens, entities_for_lens, sort_provider_results};
use crate::sources::SourceAdapter;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CACHE_CAPACITY: usize = 100;
const LOCAL_CACHE_TTL: Duration = Duration::from_secs(5);
const ONLINE_CACHE_TTL: Duration = Duration::from_secs(60);
const PROVIDER_CLEANUP_GRACE: Duration = Duration::from_secs(1);

#[derive(Clone, Copy)]
struct ProviderTimeouts {
    local: Duration,
    youtube: Duration,
    soundcloud: Duration,
    spotify: Duration,
}

impl Default for ProviderTimeouts {
    fn default() -> Self {
        Self {
            local: Duration::from_secs(2),
            youtube: Duration::from_secs(15),
            soundcloud: Duration::from_secs(15),
            spotify: Duration::from_secs(10),
        }
    }
}

impl ProviderTimeouts {
    fn for_provider(self, provider: ProviderKind) -> Duration {
        match provider {
            ProviderKind::Local => self.local,
            ProviderKind::Youtube => self.youtube,
            ProviderKind::Soundcloud => self.soundcloud,
            ProviderKind::Spotify => self.spotify,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CacheKey {
    provider: ProviderKind,
    lens: SearchLens,
    entities: Vec<SearchEntityKind>,
    sort_field: SearchSortField,
    sort_direction: SearchSortDirection,
    limit: u8,
    market: Option<String>,
    query: String,
}

impl CacheKey {
    fn from_request(provider: ProviderKind, request: &ProviderSearchRequest) -> Self {
        Self {
            provider,
            lens: request.lens,
            entities: request.entities.clone(),
            sort_field: request.sort_field,
            sort_direction: request.sort_direction,
            limit: request.limit,
            market: request.market.clone(),
            query: request.query.clone(),
        }
    }
}

#[derive(Clone)]
struct CacheEntry {
    stored_at: Instant,
    section: ProviderSearchSection,
}

#[derive(Default)]
struct SearchCache {
    entries: HashMap<CacheKey, CacheEntry>,
}

impl SearchCache {
    fn get(
        &mut self,
        key: &CacheKey,
        provider: ProviderKind,
        now: Instant,
    ) -> Option<ProviderSearchSection> {
        let ttl = cache_ttl(provider);
        if self
            .entries
            .get(key)
            .is_some_and(|entry| now.duration_since(entry.stored_at) <= ttl)
        {
            return self.entries.get(key).map(|entry| entry.section.clone());
        }
        self.entries.remove(key);
        None
    }

    fn insert(&mut self, key: CacheKey, section: ProviderSearchSection, now: Instant) {
        if !self.entries.contains_key(&key) && self.entries.len() >= CACHE_CAPACITY {
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.stored_at)
                .map(|(key, _)| key.clone())
            {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(
            key,
            CacheEntry {
                stored_at: now,
                section,
            },
        );
    }
}

fn cache_ttl(provider: ProviderKind) -> Duration {
    match provider {
        ProviderKind::Local => LOCAL_CACHE_TTL,
        ProviderKind::Youtube | ProviderKind::Soundcloud | ProviderKind::Spotify => {
            ONLINE_CACHE_TTL
        }
    }
}

struct ActiveSearch {
    search_id: SearchId,
    cancellation: SearchCancellation,
}

struct SearchServiceInner {
    registry: HashMap<ProviderKind, Arc<dyn SourceAdapter>>,
    active: Mutex<Option<ActiveSearch>>,
    cache: Mutex<SearchCache>,
    timeouts: ProviderTimeouts,
}

#[derive(Clone)]
pub struct SearchService {
    inner: Arc<SearchServiceInner>,
}

impl SearchService {
    pub fn new<I>(adapters: I) -> Self
    where
        I: IntoIterator<Item = Arc<dyn SourceAdapter>>,
    {
        Self::with_timeouts(adapters, ProviderTimeouts::default())
    }

    fn with_timeouts<I>(adapters: I, timeouts: ProviderTimeouts) -> Self
    where
        I: IntoIterator<Item = Arc<dyn SourceAdapter>>,
    {
        let registry = adapters
            .into_iter()
            .map(|adapter| (adapter.kind(), adapter))
            .collect();
        Self {
            inner: Arc::new(SearchServiceInner {
                registry,
                active: Mutex::new(None),
                cache: Mutex::new(SearchCache::default()),
                timeouts,
            }),
        }
    }

    pub fn start_search(
        &self,
        request: SearchRequest,
        sink: SearchEventSink,
    ) -> Result<SearchStarted, SearchValidationError> {
        let query = request.validate()?;
        let search_id = SearchId::new();
        let cancellation = SearchCancellation::new();
        {
            let mut active = self.inner.active.lock().expect("search state poisoned");
            if let Some(previous) = active.replace(ActiveSearch {
                search_id,
                cancellation: cancellation.clone(),
            }) {
                previous.cancellation.cancel();
            }
        }

        let mut tasks = Vec::new();
        for &provider in all_provider_kinds_for_lens(request.lens) {
            let adapter = self.inner.registry.get(&provider).cloned();
            let provider_request = ProviderSearchRequest {
                search_id,
                query: query.clone(),
                lens: request.lens,
                entities: entities_for_lens(request.lens).to_vec(),
                sort_field: request.sort_field,
                sort_direction: request.sort_direction,
                limit: request.limit,
                market: None,
            };
            let key = CacheKey::from_request(provider, &provider_request);
            let task_sink = sink.clone();
            let task_cancellation = cancellation.clone();
            let inner = self.inner.clone();
            tasks.push(tokio::spawn(async move {
                let section = run_provider(
                    adapter,
                    provider,
                    provider_request,
                    key,
                    task_cancellation,
                    &inner,
                )
                .await;
                task_sink(SearchEvent::ProviderSection(ProviderSearchEvent {
                    search_id,
                    section,
                }));
            }));
        }

        let inner = self.inner.clone();
        tokio::spawn(async move {
            for task in tasks {
                let _ = task.await;
            }
            sink(SearchEvent::Completed(SearchCompleted { search_id }));
            let mut active = inner.active.lock().expect("search state poisoned");
            if active
                .as_ref()
                .is_some_and(|current| current.search_id == search_id)
            {
                active.take();
            }
        });

        Ok(SearchStarted { search_id })
    }

    pub fn cancel_search(&self) -> Option<SearchId> {
        let active = self.inner.active.lock().expect("search state poisoned");
        active.as_ref().map(|current| {
            current.cancellation.cancel();
            current.search_id
        })
    }

    pub fn provider_statuses(&self) -> Vec<SearchProviderStatus> {
        ProviderKind::all()
            .iter()
            .filter_map(|provider| {
                self.inner
                    .registry
                    .get(provider)
                    .map(|adapter| SearchProviderStatus {
                        provider: *provider,
                        runtime_status: adapter.runtime_status(),
                    })
            })
            .collect()
    }
}

async fn run_provider(
    adapter: Option<Arc<dyn SourceAdapter>>,
    provider: ProviderKind,
    request: ProviderSearchRequest,
    key: CacheKey,
    cancellation: SearchCancellation,
    inner: &SearchServiceInner,
) -> ProviderSearchSection {
    if *cancellation.subscribe().borrow() {
        return cancelled_section(provider);
    }
    if let Some(section) =
        inner
            .cache
            .lock()
            .expect("search cache poisoned")
            .get(&key, provider, Instant::now())
    {
        return section;
    }
    let Some(adapter) = adapter else {
        return failed_section(
            provider,
            ProviderSearchErrorCode::Unavailable,
            "provider adapter is not registered",
        );
    };

    let mut cancellation_rx = cancellation.subscribe();
    let timeout = inner.timeouts.for_provider(provider);
    let capabilities = adapter.capabilities();
    let sort_field = request.sort_field;
    let sort_direction = request.sort_direction;
    let limit = usize::from(request.limit);
    let provider_cancellation = SearchCancellation::new();
    let mut provider_search = adapter.search(request, provider_cancellation.clone());
    let mut section = tokio::select! {
        biased;
        _ = wait_for_cancellation(&mut cancellation_rx) => {
            provider_cancellation.cancel();
            let _ = tokio::time::timeout(PROVIDER_CLEANUP_GRACE, &mut provider_search).await;
            cancelled_section(provider)
        },
        _ = tokio::time::sleep(timeout) => {
            provider_cancellation.cancel();
            let _ = tokio::time::timeout(PROVIDER_CLEANUP_GRACE, &mut provider_search).await;
            failed_section(provider, ProviderSearchErrorCode::Timeout, "provider search timed out")
        }
        section = &mut provider_search => section,
    };
    if section.state == ProviderSearchState::Ready {
        sort_provider_results(
            provider,
            capabilities,
            &mut section.results,
            sort_field,
            sort_direction,
        );
        section.results.truncate(limit);
        inner.cache.lock().expect("search cache poisoned").insert(
            key,
            section.clone(),
            Instant::now(),
        );
    }
    section
}

async fn wait_for_cancellation(receiver: &mut tokio::sync::watch::Receiver<bool>) {
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

fn failed_section(
    provider: ProviderKind,
    code: ProviderSearchErrorCode,
    detail: &str,
) -> ProviderSearchSection {
    ProviderSearchSection {
        provider,
        state: ProviderSearchState::Failed,
        results: Vec::new(),
        error: Some(ProviderSearchError {
            code,
            detail: Some(detail.to_owned()),
            retry_after_seconds: None,
        }),
    }
}

fn cancelled_section(provider: ProviderKind) -> ProviderSearchSection {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ProviderKind, SourceCapabilities};
    use crate::search::{
        EngagementKind, ProviderRuntimeStatus, ProviderSearchError, ProviderSearchErrorCode,
        ProviderSearchRequest, ProviderSearchSection, ProviderSearchState, SearchCancellation,
        SearchEntityKind, SearchEvent, SearchLens, SearchRequest, SearchResult,
        SearchSortDirection, SearchSortField,
    };
    use crate::sources::SourceAdapter;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct AdapterSpy {
        calls: Arc<AtomicUsize>,
        requests: Arc<Mutex<Vec<ProviderSearchRequest>>>,
    }

    struct FakeAdapter {
        provider: ProviderKind,
        capabilities: SourceCapabilities,
        status: ProviderRuntimeStatus,
        delay: Duration,
        section: ProviderSearchSection,
        spy: AdapterSpy,
    }

    impl SourceAdapter for FakeAdapter {
        fn kind(&self) -> ProviderKind {
            self.provider
        }

        fn capabilities(&self) -> SourceCapabilities {
            self.capabilities
        }

        fn supported_entities(&self) -> &'static [SearchEntityKind] {
            &[SearchEntityKind::Track]
        }

        fn runtime_status(&self) -> ProviderRuntimeStatus {
            self.status
        }

        fn search(
            &self,
            request: ProviderSearchRequest,
            _cancellation: SearchCancellation,
        ) -> Pin<Box<dyn Future<Output = ProviderSearchSection> + Send + '_>> {
            self.spy.calls.fetch_add(1, Ordering::SeqCst);
            self.spy.requests.lock().unwrap().push(request);
            Box::pin(async move {
                tokio::time::sleep(self.delay).await;
                self.section.clone()
            })
        }
    }

    fn fake_adapter(
        provider: ProviderKind,
        delay: Duration,
        section: ProviderSearchSection,
        capabilities: SourceCapabilities,
        status: ProviderRuntimeStatus,
    ) -> (Arc<dyn SourceAdapter>, AdapterSpy) {
        let spy = AdapterSpy {
            calls: Arc::new(AtomicUsize::new(0)),
            requests: Arc::new(Mutex::new(Vec::new())),
        };
        (
            Arc::new(FakeAdapter {
                provider,
                capabilities,
                status,
                delay,
                section,
                spy: spy.clone(),
            }),
            spy,
        )
    }

    fn ready_adapter(
        provider: ProviderKind,
        delay_ms: u64,
        results: Vec<SearchResult>,
    ) -> (Arc<dyn SourceAdapter>, AdapterSpy) {
        fake_adapter(
            provider,
            Duration::from_millis(delay_ms),
            ready_section(provider, results),
            SourceCapabilities {
                search: true,
                popularity: matches!(provider, ProviderKind::Youtube | ProviderKind::Soundcloud),
                release_date: true,
                ..SourceCapabilities::default()
            },
            ProviderRuntimeStatus::Ready,
        )
    }

    fn failed_adapter(
        provider: ProviderKind,
        delay_ms: u64,
    ) -> (Arc<dyn SourceAdapter>, AdapterSpy) {
        fake_adapter(
            provider,
            Duration::from_millis(delay_ms),
            failed_section(provider, ProviderSearchErrorCode::Failed, "fake failure"),
            SourceCapabilities {
                search: true,
                ..SourceCapabilities::default()
            },
            ProviderRuntimeStatus::Ready,
        )
    }

    fn ready_section(provider: ProviderKind, results: Vec<SearchResult>) -> ProviderSearchSection {
        ProviderSearchSection {
            provider,
            state: ProviderSearchState::Ready,
            results,
            error: None,
        }
    }

    fn result(
        provider: ProviderKind,
        id: &str,
        rank: u32,
        duration_ms: Option<u64>,
        engagement_count: Option<u64>,
        engagement_kind: Option<EngagementKind>,
    ) -> SearchResult {
        SearchResult {
            provider,
            entity_kind: SearchEntityKind::Track,
            provider_item_id: id.to_owned(),
            canonical_url: None,
            title: id.to_owned(),
            artists: vec!["artist".to_owned()],
            album: None,
            duration_ms,
            artwork_url: None,
            published_at: None,
            engagement_count,
            engagement_kind,
            explicit: None,
            local_track_id: None,
            local_source_id: None,
            original_rank: rank,
        }
    }

    fn request(lens: SearchLens) -> SearchRequest {
        SearchRequest {
            query: "signal".into(),
            lens,
            sort_field: SearchSortField::Relevance,
            sort_direction: SearchSortDirection::Descending,
            limit: 25,
        }
    }

    fn event_sink() -> (SearchEventSink, Arc<Mutex<Vec<SearchEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        (
            Arc::new(move |event| captured.lock().unwrap().push(event)),
            events,
        )
    }

    async fn wait_for_completions(events: &Arc<Mutex<Vec<SearchEvent>>>, count: usize) {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let completed = events
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|event| matches!(event, SearchEvent::Completed(_)))
                    .count();
                if completed >= count {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    fn sections_for(
        events: &Arc<Mutex<Vec<SearchEvent>>>,
        search_id: SearchId,
    ) -> Vec<ProviderSearchSection> {
        events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match event {
                SearchEvent::ProviderSection(event) if event.search_id == search_id => {
                    Some(event.section.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn completion_count(events: &Arc<Mutex<Vec<SearchEvent>>>, search_id: SearchId) -> usize {
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| {
                matches!(event, SearchEvent::Completed(completed) if completed.search_id == search_id)
            })
            .count()
    }

    #[tokio::test]
    async fn registry_has_four_adapters_but_all_excludes_spotify() {
        let pairs: Vec<_> = ProviderKind::all()
            .iter()
            .map(|provider| ready_adapter(*provider, 0, Vec::new()))
            .collect();
        let adapters = pairs.iter().map(|(adapter, _)| adapter.clone());
        let service = SearchService::new(adapters);
        assert_eq!(service.provider_statuses().len(), 4);

        let (sink, events) = event_sink();
        service
            .start_search(request(SearchLens::All), sink)
            .unwrap();
        wait_for_completions(&events, 1).await;

        assert_eq!(pairs[0].1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(pairs[1].1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(pairs[2].1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(pairs[3].1.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn artists_and_albums_query_local_only() {
        for lens in [SearchLens::Artists, SearchLens::Albums] {
            let (local, local_spy) = ready_adapter(ProviderKind::Local, 0, Vec::new());
            let (youtube, youtube_spy) = ready_adapter(ProviderKind::Youtube, 0, Vec::new());
            let (soundcloud, soundcloud_spy) =
                ready_adapter(ProviderKind::Soundcloud, 0, Vec::new());
            let service = SearchService::new([local, youtube, soundcloud]);
            let (sink, events) = event_sink();

            service.start_search(request(lens), sink).unwrap();
            wait_for_completions(&events, 1).await;

            assert_eq!(local_spy.calls.load(Ordering::SeqCst), 1, "{lens:?}");
            assert_eq!(youtube_spy.calls.load(Ordering::SeqCst), 0, "{lens:?}");
            assert_eq!(soundcloud_spy.calls.load(Ordering::SeqCst), 0, "{lens:?}");
        }
    }

    #[tokio::test]
    async fn local_finishes_before_slow_youtube_and_emits_first() {
        let (local, _) = ready_adapter(ProviderKind::Local, 1, Vec::new());
        let (youtube, _) = ready_adapter(ProviderKind::Youtube, 60, Vec::new());
        let (soundcloud, _) = ready_adapter(ProviderKind::Soundcloud, 80, Vec::new());
        let service = SearchService::new([local, youtube, soundcloud]);
        let (sink, events) = event_sink();
        let started = service
            .start_search(request(SearchLens::All), sink)
            .unwrap();
        wait_for_completions(&events, 1).await;

        let first = events
            .lock()
            .unwrap()
            .iter()
            .find_map(|event| match event {
                SearchEvent::ProviderSection(event) => Some(event.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(first.section.provider, ProviderKind::Local);
        assert_eq!(first.search_id, started.search_id);
    }

    #[tokio::test]
    async fn youtube_error_does_not_discard_local() {
        let (local, _) = ready_adapter(ProviderKind::Local, 1, Vec::new());
        let (youtube, _) = failed_adapter(ProviderKind::Youtube, 2);
        let (soundcloud, _) = ready_adapter(ProviderKind::Soundcloud, 3, Vec::new());
        let service = SearchService::new([local, youtube, soundcloud]);
        let (sink, events) = event_sink();
        let started = service
            .start_search(request(SearchLens::All), sink)
            .unwrap();
        wait_for_completions(&events, 1).await;
        let sections = sections_for(&events, started.search_id);

        assert_eq!(
            sections
                .iter()
                .find(|section| section.provider == ProviderKind::Local)
                .unwrap()
                .state,
            ProviderSearchState::Ready
        );
        assert_eq!(
            sections
                .iter()
                .find(|section| section.provider == ProviderKind::Youtube)
                .unwrap()
                .state,
            ProviderSearchState::Failed
        );
    }

    #[tokio::test]
    async fn soundcloud_timeout_does_not_discard_other_sections() {
        let (local, _) = ready_adapter(ProviderKind::Local, 1, Vec::new());
        let (youtube, _) = ready_adapter(ProviderKind::Youtube, 2, Vec::new());
        let (soundcloud, _) = ready_adapter(ProviderKind::Soundcloud, 100, Vec::new());
        let service = SearchService::with_timeouts(
            [local, youtube, soundcloud],
            ProviderTimeouts {
                soundcloud: Duration::from_millis(15),
                ..ProviderTimeouts::default()
            },
        );
        let (sink, events) = event_sink();
        let started = service
            .start_search(request(SearchLens::All), sink)
            .unwrap();
        wait_for_completions(&events, 1).await;
        let sections = sections_for(&events, started.search_id);

        assert_eq!(
            sections
                .iter()
                .find(|section| section.provider == ProviderKind::Soundcloud)
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .code,
            ProviderSearchErrorCode::Timeout
        );
        assert!(sections.iter().any(|section| {
            section.provider == ProviderKind::Local && section.state == ProviderSearchState::Ready
        }));
        assert!(sections.iter().any(|section| {
            section.provider == ProviderKind::Youtube && section.state == ProviderSearchState::Ready
        }));
    }

    #[tokio::test]
    async fn new_query_cancels_old_query() {
        let (local, _) = ready_adapter(ProviderKind::Local, 40, Vec::new());
        let (youtube, _) = ready_adapter(ProviderKind::Youtube, 40, Vec::new());
        let (soundcloud, _) = ready_adapter(ProviderKind::Soundcloud, 40, Vec::new());
        let service = SearchService::new([local, youtube, soundcloud]);
        let (sink, events) = event_sink();
        let old = service
            .start_search(request(SearchLens::All), sink.clone())
            .unwrap();
        tokio::task::yield_now().await;
        let new = service
            .start_search(request(SearchLens::Local), sink)
            .unwrap();
        wait_for_completions(&events, 2).await;

        let old_sections = sections_for(&events, old.search_id);
        assert_eq!(old_sections.len(), 3);
        assert!(old_sections
            .iter()
            .all(|section| section.state == ProviderSearchState::Cancelled));
        assert_eq!(completion_count(&events, old.search_id), 1);
        assert_eq!(completion_count(&events, new.search_id), 1);
    }

    #[tokio::test]
    async fn stale_provider_completion_keeps_old_search_id() {
        let (local, _) = ready_adapter(ProviderKind::Local, 30, Vec::new());
        let service = SearchService::new([local]);
        let (sink, events) = event_sink();
        let old = service
            .start_search(request(SearchLens::Local), sink.clone())
            .unwrap();
        tokio::task::yield_now().await;
        let new = service
            .start_search(request(SearchLens::Local), sink)
            .unwrap();
        wait_for_completions(&events, 2).await;

        assert_ne!(old.search_id, new.search_id);
        assert_eq!(sections_for(&events, old.search_id).len(), 1);
        assert_eq!(sections_for(&events, new.search_id).len(), 1);
    }

    #[tokio::test]
    async fn completion_emits_once() {
        let (local, _) = ready_adapter(ProviderKind::Local, 1, Vec::new());
        let (youtube, _) = failed_adapter(ProviderKind::Youtube, 2);
        let (soundcloud, _) = ready_adapter(ProviderKind::Soundcloud, 3, Vec::new());
        let service = SearchService::new([local, youtube, soundcloud]);
        let (sink, events) = event_sink();
        let started = service
            .start_search(request(SearchLens::All), sink)
            .unwrap();
        wait_for_completions(&events, 1).await;
        assert_eq!(completion_count(&events, started.search_id), 1);
    }

    #[tokio::test]
    async fn cancellation_completion_emits_once() {
        let (local, _) = ready_adapter(ProviderKind::Local, 100, Vec::new());
        let service = SearchService::new([local]);
        let (sink, events) = event_sink();
        let started = service
            .start_search(request(SearchLens::Local), sink)
            .unwrap();
        assert_eq!(service.cancel_search(), Some(started.search_id));
        wait_for_completions(&events, 1).await;
        assert_eq!(completion_count(&events, started.search_id), 1);
        assert_eq!(
            sections_for(&events, started.search_id)[0].state,
            ProviderSearchState::Cancelled
        );
    }

    #[tokio::test]
    async fn provider_sort_is_independent() {
        let (local, _) = ready_adapter(
            ProviderKind::Local,
            1,
            vec![
                result(ProviderKind::Local, "local-long", 0, Some(300), None, None),
                result(ProviderKind::Local, "local-short", 1, Some(100), None, None),
            ],
        );
        let (youtube, _) = ready_adapter(
            ProviderKind::Youtube,
            2,
            vec![
                result(ProviderKind::Youtube, "yt-long", 0, Some(200), None, None),
                result(ProviderKind::Youtube, "yt-short", 1, Some(50), None, None),
            ],
        );
        let (soundcloud, _) = ready_adapter(ProviderKind::Soundcloud, 3, Vec::new());
        let service = SearchService::new([local, youtube, soundcloud]);
        let (sink, events) = event_sink();
        let mut search = request(SearchLens::All);
        search.sort_field = SearchSortField::Duration;
        search.sort_direction = SearchSortDirection::Ascending;
        let started = service.start_search(search, sink).unwrap();
        wait_for_completions(&events, 1).await;
        let sections = sections_for(&events, started.search_id);
        let ids = |provider| {
            sections
                .iter()
                .find(|section| section.provider == provider)
                .unwrap()
                .results
                .iter()
                .map(|result| result.provider_item_id.as_str())
                .collect::<Vec<_>>()
        };

        assert_eq!(ids(ProviderKind::Local), ["local-short", "local-long"]);
        assert_eq!(ids(ProviderKind::Youtube), ["yt-short", "yt-long"]);
    }

    #[tokio::test]
    async fn null_sort_values_are_last() {
        let (local, _) = ready_adapter(
            ProviderKind::Local,
            0,
            vec![
                result(ProviderKind::Local, "null", 0, None, None, None),
                result(ProviderKind::Local, "five", 1, Some(5), None, None),
                result(ProviderKind::Local, "ten", 2, Some(10), None, None),
            ],
        );
        let service = SearchService::new([local]);
        let (sink, events) = event_sink();
        let mut search = request(SearchLens::Local);
        search.sort_field = SearchSortField::Duration;
        search.sort_direction = SearchSortDirection::Descending;
        let started = service.start_search(search, sink).unwrap();
        wait_for_completions(&events, 1).await;
        let sections = sections_for(&events, started.search_id);
        let ids: Vec<_> = sections[0]
            .results
            .iter()
            .map(|result| result.provider_item_id.as_str())
            .collect();
        assert_eq!(ids, ["ten", "five", "null"]);
    }

    #[tokio::test]
    async fn relevance_preserves_provider_order() {
        let expected = ["first", "second", "third"];
        let (youtube, _) = ready_adapter(
            ProviderKind::Youtube,
            0,
            expected
                .iter()
                .enumerate()
                .map(|(rank, id)| result(ProviderKind::Youtube, id, rank as u32, None, None, None))
                .collect(),
        );
        let service = SearchService::new([youtube]);
        let (sink, events) = event_sink();
        let started = service
            .start_search(request(SearchLens::Youtube), sink)
            .unwrap();
        wait_for_completions(&events, 1).await;
        let sections = sections_for(&events, started.search_id);
        let ids: Vec<_> = sections[0]
            .results
            .iter()
            .map(|result| result.provider_item_id.as_str())
            .collect();
        assert_eq!(ids, expected);
    }

    #[tokio::test]
    async fn unsupported_engagement_falls_back_to_relevance() {
        let (youtube, _) = fake_adapter(
            ProviderKind::Youtube,
            Duration::ZERO,
            ready_section(
                ProviderKind::Youtube,
                vec![
                    result(
                        ProviderKind::Youtube,
                        "first",
                        0,
                        None,
                        Some(1),
                        Some(EngagementKind::Plays),
                    ),
                    result(
                        ProviderKind::Youtube,
                        "second",
                        1,
                        None,
                        Some(999),
                        Some(EngagementKind::Plays),
                    ),
                ],
            ),
            SourceCapabilities {
                search: true,
                popularity: true,
                ..SourceCapabilities::default()
            },
            ProviderRuntimeStatus::Ready,
        );
        let service = SearchService::new([youtube]);
        let (sink, events) = event_sink();
        let mut search = request(SearchLens::Youtube);
        search.sort_field = SearchSortField::Popularity;
        search.sort_direction = SearchSortDirection::Descending;
        let started = service.start_search(search, sink).unwrap();
        wait_for_completions(&events, 1).await;
        let sections = sections_for(&events, started.search_id);
        let ids: Vec<_> = sections[0]
            .results
            .iter()
            .map(|result| result.provider_item_id.as_str())
            .collect();
        assert_eq!(ids, ["first", "second"]);
    }

    #[tokio::test]
    async fn spotify_is_only_queried_by_spotify_lens_with_gate() {
        let (local, _) = ready_adapter(ProviderKind::Local, 0, Vec::new());
        let (youtube, _) = ready_adapter(ProviderKind::Youtube, 0, Vec::new());
        let (soundcloud, _) = ready_adapter(ProviderKind::Soundcloud, 0, Vec::new());
        let (spotify, spotify_spy) = fake_adapter(
            ProviderKind::Spotify,
            Duration::ZERO,
            ProviderSearchSection {
                provider: ProviderKind::Spotify,
                state: ProviderSearchState::Failed,
                results: Vec::new(),
                error: Some(ProviderSearchError {
                    code: ProviderSearchErrorCode::Unavailable,
                    detail: Some("development gate disabled".to_owned()),
                    retry_after_seconds: None,
                }),
            },
            SourceCapabilities {
                search: true,
                ..SourceCapabilities::default()
            },
            ProviderRuntimeStatus::Unsupported,
        );
        let service = SearchService::new([local, youtube, soundcloud, spotify]);
        let (sink, events) = event_sink();
        service
            .start_search(request(SearchLens::All), sink.clone())
            .unwrap();
        wait_for_completions(&events, 1).await;
        assert_eq!(spotify_spy.calls.load(Ordering::SeqCst), 0);

        service
            .start_search(request(SearchLens::Spotify), sink)
            .unwrap();
        wait_for_completions(&events, 2).await;
        assert_eq!(spotify_spy.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cache_key_includes_lens_sort_direction_limit_and_market() {
        let base = ProviderSearchRequest {
            search_id: SearchId::new(),
            query: "signal".to_owned(),
            lens: SearchLens::All,
            entities: vec![SearchEntityKind::Track],
            sort_field: SearchSortField::Relevance,
            sort_direction: SearchSortDirection::Descending,
            limit: 25,
            market: None,
        };
        let base_key = CacheKey::from_request(ProviderKind::Youtube, &base);
        let variants = [
            ProviderSearchRequest {
                lens: SearchLens::Tracks,
                ..base.clone()
            },
            ProviderSearchRequest {
                entities: vec![SearchEntityKind::Artist],
                ..base.clone()
            },
            ProviderSearchRequest {
                sort_field: SearchSortField::Duration,
                ..base.clone()
            },
            ProviderSearchRequest {
                sort_direction: SearchSortDirection::Ascending,
                ..base.clone()
            },
            ProviderSearchRequest {
                limit: 10,
                ..base.clone()
            },
            ProviderSearchRequest {
                market: Some("US".to_owned()),
                ..base.clone()
            },
            ProviderSearchRequest {
                query: "other".to_owned(),
                ..base.clone()
            },
        ];
        for variant in variants {
            assert_ne!(
                base_key,
                CacheKey::from_request(ProviderKind::Youtube, &variant)
            );
        }
        assert_ne!(
            base_key,
            CacheKey::from_request(ProviderKind::Soundcloud, &base)
        );
    }

    #[test]
    fn cache_never_exceeds_100_entries() {
        let now = Instant::now();
        let mut cache = SearchCache::default();
        for index in 0..=CACHE_CAPACITY {
            let request = ProviderSearchRequest {
                search_id: SearchId::new(),
                query: format!("query-{index}"),
                lens: SearchLens::Youtube,
                entities: vec![SearchEntityKind::Track],
                sort_field: SearchSortField::Relevance,
                sort_direction: SearchSortDirection::Descending,
                limit: 25,
                market: None,
            };
            cache.insert(
                CacheKey::from_request(ProviderKind::Youtube, &request),
                ready_section(ProviderKind::Youtube, Vec::new()),
                now + Duration::from_millis(index as u64),
            );
        }
        assert_eq!(cache.entries.len(), CACHE_CAPACITY);
    }

    #[test]
    fn local_cache_ttl_is_at_most_five_seconds() {
        assert!(cache_ttl(ProviderKind::Local) <= Duration::from_secs(5));
        let now = Instant::now();
        let request = ProviderSearchRequest {
            search_id: SearchId::new(),
            query: "signal".to_owned(),
            lens: SearchLens::Local,
            entities: vec![SearchEntityKind::Track],
            sort_field: SearchSortField::Relevance,
            sort_direction: SearchSortDirection::Descending,
            limit: 25,
            market: None,
        };
        let key = CacheKey::from_request(ProviderKind::Local, &request);
        let mut cache = SearchCache::default();
        cache.insert(
            key.clone(),
            ready_section(ProviderKind::Local, Vec::new()),
            now,
        );
        assert!(cache
            .get(
                &key,
                ProviderKind::Local,
                now + Duration::from_secs(5) + Duration::from_millis(1)
            )
            .is_none());
    }
}
