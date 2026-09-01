# Source Adapters and Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Implement independent, cancellable Local, YouTube, SoundCloud, and isolated Spotify catalog search through typed provider adapters.

**Architecture:** Rust owns provider execution, provider credentials, process boundaries, HTTP, normalization, cancellation, search lifecycle, sorting, and error mapping. React owns only presentation and transient interaction state. Online search results remain ephemeral during Plan 05 and are not persisted into UnifiedTrack/TrackSource records.

**Tech Stack:** Tauri 2, Rust, Tokio, SQLite, reqwest, yt-dlp structured JSON, Spotify Web API PKCE, Windows Credential Manager, React, TanStack Query, Zod, Playwright.

**Spec:** docs/superpowers/specs/2026-08-30-spotdiy-design.md

## Global Constraints

- Cargo output uses `C:\CargoTarget\SpotDIY`; `src-tauri\target` remains absent.
- The database schema remains version 2; Plan 05 creates no migration or search-result table.
- `async-trait = "0.1.92"`, `reqwest = { version = "0.13.4", default-features = false, features = ["json", "form", "query", "rustls"] }`, `keyring = "4.2.0"`, and `base64 = { version = "0.23.1", default-features = false, features = ["std"] }` are the only new dependencies.
- Existing Tokio `1.53.1`, `sha2 0.11.0`, `rand 0.9`, `uuid`, `serde`, `serde_json`, `url`, `thiserror`, and `rusqlite` are reused.
- Search query input is trimmed, rejects empty input before provider execution, and rejects more than 256 Unicode scalar values. The actual provider query keeps its accents, punctuation, and case.
- Search limits are 25 by default and 50 maximum; Spotify sends `limit=10` per `track,artist,album` type with `offset=0`.
- Local, YouTube, and SoundCloud are the only providers in `ALL`, `TRACKS`, `ARTISTS`, and `ALBUMS` according to the exact lens mapping in the spec. YouTube and SoundCloud query only track-like entities. Spotify is queried only inside `SPOTIFY` when `SPOTDIY_ENABLE_SPOTIFY_DEV=1`.
- Spotify uses Authorization Code with PKCE, loopback `127.0.0.1:0`, callback path `/callback`, no client secret, memory-only access token/PKCE state/code/expiry, and Windows Credential Manager for `{client_id, market, refresh_token}`.
- Spotify is disabled by default, excluded from unified lenses and Source Fusion, never persisted as a search result, and never used for playback or source selection.
- YouTube and SoundCloud invoke `yt-dlp` with structured arguments, no shell, 4 MiB stdout bound, 256 KiB retained stderr bound, and a 15-second process bound. Cancellation kills and reaps the owned child.
- Provider timeouts are Local 2 seconds, YouTube 15 seconds, SoundCloud 15 seconds, and Spotify HTTP 10 seconds. One provider timeout or failure does not invalidate other sections.
- Frontend event payloads are strict Zod-validated normalized DTOs. No provider raw JSON, executable path, stderr dump, cookies, headers, direct media URL, OAuth code, PKCE value, or token crosses IPC.
- Local result actions reuse existing typed playback commands and IDs. Online results expose only validated `open_provider_result`; they do not expose Play, Download, URL resolution, or Source Fusion actions.
- Provider sections retain fixed visual order `Local`, `YouTube`, `SoundCloud` in `ALL` while native updates arrive independently. Sorting is provider-local; relevance preserves provider order and nullable values sort last.
- In-memory search cache only: maximum 100 provider entries, YouTube/SoundCloud/Spotify TTL 60 seconds, Local TTL at most 5 seconds or disabled. No cache table, Redis, catalog mirror, or persistent provider cache is added.
- No Plan 06 Source Fusion, online playback, downloading, FFmpeg integration, lyrics, playlists, persistent queue, analytics, AI/ML, or shell redesign is implemented.

## File and interface map

The implementation uses focused modules. `sources/traits.rs` owns shared adapter and cancellation contracts; `sources/yt_dlp.rs` owns the bounded process seam; provider files own normalization; `search/` owns validation, orchestration, cache, and sort; `credentials/` owns only the secure Spotify record; `ipc/` and `lib.rs` own the Tauri boundary; React files own only typed event consumption and presentation.

The shared Rust contracts are:

```rust
#[async_trait]
pub trait SourceAdapter: Send + Sync {
    fn kind(&self) -> ProviderKind;
    fn capabilities(&self) -> SourceCapabilities;
    fn supported_entities(&self) -> &'static [SearchEntityKind];
    fn runtime_status(&self) -> ProviderRuntimeStatus;
    async fn search(
        &self,
        request: ProviderSearchRequest,
        cancellation: SearchCancellation,
    ) -> ProviderSearchSection;
}

pub struct SearchRequest {
    pub query: String,
    pub lens: SearchLens,
    pub sort_field: SearchSortField,
    pub sort_direction: SearchSortDirection,
    pub limit: u8,
}

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
```

The shared result contract contains only `provider`, `entityKind`, `providerItemId`, `canonicalUrl`, `title`, `artists`, `album`, `durationMs`, `artworkUrl`, `publishedAt`, `publishedPrecision`, `engagementCount`, `engagementKind`, `explicit`, `localTrackId`, `localSourceId`, and `originalRank`. Provider-specific raw structures stop at each adapter parser.

## Ordered tasks

### Task 1: Search contracts, adapter boundary, and URL security

**Files:**

- Create: `src-tauri/src/search/types.rs`
- Create: `src-tauri/src/search/sort.rs`
- Create: `src-tauri/src/sources/mod.rs`
- Create: `src-tauri/src/sources/traits.rs`
- Modify: `src-tauri/src/lib.rs` to declare `sources` and `search` modules without wiring runtime state
- Test: Rust unit modules in the files above

**Consumes:** Existing `ProviderKind`, `SourceCapabilities`, `TrackId`, `SourceId`, `Uuid`, `serde`, `thiserror`, and `url`.

**Produces:** `SearchId`, `SearchLens`, `SearchEntityKind`, `SearchSortField`, `SearchSortDirection`, `SearchRequest`, `ProviderSearchRequest`, `SearchResult`, `ProviderSearchSection`, `ProviderSearchState`, `ProviderSearchError`, `ProviderSearchErrorCode`, `EngagementKind`, `PartialDate`, `PartialDatePrecision`, `ProviderRuntimeStatus`, `SearchStarted`, `SearchCompleted`, `SearchCancellation`, `SourceAdapter`, `validate_provider_url`, `sanitize_artwork_url`, and fixed provider/entity lens selection helpers.

- [ ] **Step 1: Write failing tests.** Add concrete tests named `search_request_trims_and_rejects_empty_query`, `search_request_rejects_257_unicode_scalars`, `search_request_rejects_limit_above_50`, `partial_date_preserves_year_and_month_precision`, `cancellation_watch_changes_to_cancelled`, `source_adapter_is_object_safe`, `provider_url_allowlist_rejects_http_javascript_file_data_and_wrong_hosts`, `artwork_allowlist_returns_null_for_unknown_https_cdn`, `all_lens_excludes_spotify`, and `spotify_lens_selects_only_spotify`.

```rust
#[test]
fn search_request_rejects_257_unicode_scalars() {
    let request = SearchRequest { query: "a".repeat(257), ..SearchRequest::test_default() };
    assert!(matches!(request.validate(), Err(SearchValidationError::QueryTooLong { .. })));
}
```

- [ ] **Step 2: Run the focused tests to confirm RED.** Run `cargo test --manifest-path src-tauri/Cargo.toml search::tests sources::tests -- --nocapture` with `$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'`. Expected: compilation/test failure because the new contracts and test helpers do not exist.
- [ ] **Step 3: Implement the minimum contracts.** Use UUID-backed `SearchId`; serialize all native enums and structs with lowercase/camelCase wire names; implement the exact provider lens mapping; implement `SearchCancellation` with `tokio::sync::watch`; keep `SearchResult` free of raw provider data; reject non-HTTPS or non-allowlisted source hosts; allow only `i.ytimg.com`, `yt3.ggpht.com`, `i1.sndcdn.com`, and `i.scdn.co` for artwork.
- [ ] **Step 4: Run the focused tests to confirm GREEN.** Run the same focused cargo command and then `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`. Expected: all Task 1 tests pass and formatting passes.
- [ ] **Step 5: Self-review.** Confirm no Spotify enters `all_provider_kinds_for_lens(All|Tracks|Artists|Albums)`, no URL validator accepts a generic arbitrary host, no result has raw JSON or provider secrets, and all fields use the frontend camelCase boundary.
- [ ] **Step 6: Commit boundary.** Commit the focused contract slice as `feat: add provider search contracts` after the task review is clean.

### Task 2: MediaToolManager yt-dlp status and bounded process runner

**Files:**

- Modify: `src-tauri/src/media_tools/mod.rs`
- Modify: `src-tauri/src/sources/yt_dlp.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Test: media-tool and process-runner unit modules

**Consumes:** Task 1 `SearchCancellation`, `ProviderSearchErrorCode`, and existing mpv manager health/discovery behavior.

**Produces:** `YtDlpToolStatus`, `MediaToolManager::refresh_yt_dlp`, `MediaToolManager::yt_dlp_status`, `MediaToolManager::require_yt_dlp`, `YtDlpProcessRunner`, `TokioYtDlpProcessRunner`, `YtDlpProcessOutput`, `YtDlpProcessError`, exact discovery priority `test override -> SPOTDIY_YTDLP_PATH -> PATH`, and shared bounded probe behavior for mpv and yt-dlp.

- [ ] **Step 1: Write failing tests.** Add tests named `yt_dlp_path_override_has_priority`, `yt_dlp_missing_status_has_no_executable`, `yt_dlp_version_below_minimum_is_unsupported`, `bounded_probe_rejects_oversized_stdout`, `bounded_probe_rejects_oversized_stderr`, `bounded_probe_times_out_and_reaps`, `bounded_probe_handles_invalid_utf8`, `bounded_probe_rejects_malformed_version`, `bounded_probe_rejects_nonzero_exit`, `yt_dlp_runner_records_exact_argv_without_shell`, `metacharacters_remain_one_argument`, `runner_bounds_stdout_at_4_mib`, `runner_bounds_stderr_at_256_kib`, and `runner_cancellation_kills_and_reaps_owned_child`.

```rust
#[tokio::test]
async fn metacharacters_remain_one_argument() {
    let fake = RecordingRunner::default();
    fake.run("C:/yt-dlp.exe", &yt_dlp_search_args("a & b | c"), SearchCancellation::new()).await.unwrap();
    assert_eq!(fake.argv()[..].last().unwrap(), "ytsearch25:a & b | c");
    assert!(!fake.shell_invoked());
}
```

- [ ] **Step 2: Run the focused tests to confirm RED.** Run `cargo test --manifest-path src-tauri/Cargo.toml media_tools::tests sources::yt_dlp::tests -- --nocapture` with the external target. Expected: failure for absent yt-dlp APIs and runner.
- [ ] **Step 3: Implement the bounded runner.** Add `yt-dlp 2026.08.19` minimum-version classification; probe using direct process spawn and concurrent stdout/stderr readers; retain at most 4 MiB stdout and 256 KiB stderr; apply the 15-second deadline; on cancellation, timeout, or overflow kill, wait, and reap only the owned child; convert invalid UTF-8 with replacement; never construct a shell command line. Refactor mpv probing only where the same bounded primitive removes duplication and preserve all existing mpv status behavior.
- [ ] **Step 4: Run the focused tests to confirm GREEN.** Run the same cargo command, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`, and `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`. Expected: all new media/process tests and the existing 117 baseline tests remain clean under clippy/fmt.
- [ ] **Step 5: Self-review.** Inspect the process code for `Command::new` plus `.arg` calls only, no `sh`, `cmd`, `start`, or interpolated command line; verify probe and search output are bounded before storage; verify the manager still keeps executable paths backend-only.
- [ ] **Step 6: Commit boundary.** Commit as `feat: add bounded yt-dlp media tool runner` after the task review is clean.

### Task 3: Local search adapter and deterministic SQLite matching

**Files:**

- Create: `src-tauri/src/sources/local.rs`
- Modify: `src-tauri/src/sources/mod.rs`
- Modify: `src-tauri/src/library/mod.rs` only if a crate-visible database/search seam is required
- Test: `src-tauri/src/sources/local.rs` and existing library test helpers

**Consumes:** Task 1 contracts, `LibraryService`, `Database`, existing schema version 2, and current local `TrackId`/`SourceId` records.

**Produces:** `LocalSourceAdapter::new`, `SourceAdapter` implementation for Local, and bounded parameterized SQL for Track, Artist, and Album results without FTS or migration 3.

- [ ] **Step 1: Write failing tests.** Add tests named `local_exact_title_ranks_first`, `local_title_prefix_precedes_substring`, `local_artist_match_returns_track`, `local_album_match_returns_track`, `local_matching_is_case_insensitive`, `local_like_wildcards_are_literal`, `local_empty_query_returns_no_provider_request`, `local_tie_order_is_title_then_stable_id`, `local_limit_is_bounded`, `local_multiple_artists_are_retained_in_order`, `local_track_returns_track_and_source_ids`, `local_unavailable_source_is_not_playable`, and `local_page_uses_one_bounded_query_per_entity_kind`.

```rust
#[tokio::test]
async fn local_like_wildcards_are_literal() {
    let fixture = local_fixture_with_title("100% Signal");
    let section = fixture.search("100%", SearchLens::Local).await;
    assert_eq!(section.results[0].title, "100% Signal");
}
```

- [ ] **Step 2: Run the focused tests to confirm RED.** Run `cargo test --manifest-path src-tauri/Cargo.toml sources::local::tests -- --nocapture`. Expected: failure because `LocalSourceAdapter` and local search result normalization are absent.
- [ ] **Step 3: Implement local search.** Use one parameterized query for each requested entity group, escaped `%`, `_`, and `\\` with `ESCAPE '\\\\'`, indexed local rows only, joins/subqueries for ordered artists and album metadata, and a hard result bound. Compute exact/prefix/substring relevance in Rust with deterministic title/artist/album and stable-ID tie-breaks. Populate Local track `localTrackId` and `localSourceId`; leave online-only fields null; preserve unavailable details without fabricating playback.
- [ ] **Step 4: Run the focused tests to confirm GREEN.** Run the local tests, all library tests, and `cargo test --manifest-path src-tauri/Cargo.toml --all-targets`. Expected: local contract tests pass, schema remains 2, and the baseline suite is green.
- [ ] **Step 5: Self-review.** Count SQL statements in the fixture query path; verify no query concatenates user input, no per-result lookup is added, no row is inserted or updated, and no local metadata is sent to an online adapter.
- [ ] **Step 6: Commit boundary.** Commit as `feat: add local source search adapter` after the task review is clean.

### Task 4: YouTube and SoundCloud structured-search adapters

**Files:**

- Create: `src-tauri/src/sources/youtube.rs`
- Create: `src-tauri/src/sources/soundcloud.rs`
- Modify: `src-tauri/src/sources/mod.rs`
- Test: provider parser and fake-runner tests in both adapter files

**Consumes:** Task 1 `SourceAdapter`/normalized DTOs and Task 2 `YtDlpProcessRunner`, tool status, cancellation, and bounded output.

**Produces:** `YoutubeSourceAdapter::new`, `SoundcloudSourceAdapter::new`, flat JSON parsers, exact argv builders, and provider-specific normalized sections.

- [ ] **Step 1: Write failing tests.** Add YouTube tests for `youtube_normal_flat_result`, `youtube_missing_thumbnail_view_count_duration_and_channel`, `youtube_malformed_top_level_json`, `youtube_missing_entries`, `youtube_unexpected_entry_type`, `youtube_empty_results`, `youtube_tool_missing`, `youtube_unsupported_version`, `youtube_timeout`, `youtube_cancellation`, `youtube_output_too_large`, and `youtube_metacharacters_stay_in_one_argv_entry`. Add SoundCloud tests for `soundcloud_normal_flat_result`, `soundcloud_plays_map_to_engagement`, `soundcloud_missing_plays_artwork_duration`, `soundcloud_malformed_response`, `soundcloud_empty_results`, `soundcloud_timeout`, `soundcloud_cancellation`, `soundcloud_rate_or_provider_error`, and `soundcloud_tool_failure`.

```rust
#[tokio::test]
async fn youtube_missing_optional_fields_remain_null() {
    let runner = FakeYtDlpRunner::json(r#"{"entries":[{"id":"v1","title":"Title"}]}"#);
    let section = youtube_with(runner).search(test_request(), SearchCancellation::new()).await;
    assert_eq!(section.results[0].engagement_count, None);
    assert_eq!(section.results[0].duration_ms, None);
    assert_eq!(section.results[0].artwork_url, None);
}
```

- [ ] **Step 2: Run the focused tests to confirm RED.** Run `cargo test --manifest-path src-tauri/Cargo.toml sources::youtube::tests sources::soundcloud::tests -- --nocapture`. Expected: failure because the adapters and parsers are absent.
- [ ] **Step 3: Implement provider adapters.** Invoke exactly `--no-config --dump-single-json --flat-playlist --skip-download --no-warnings --socket-timeout 10 ytsearch25:<query>` or `scsearch25:<query>`, with each item as one process argument. Parse only structured JSON; cap normalized results at the request limit; deduplicate within the provider by provider/entity/item ID while preserving first rank; treat metadata as nullable; map YouTube views to `EngagementKind::Views` and SoundCloud plays to `EngagementKind::Plays`; validate canonical URLs and artwork hosts; never resolve formats, direct media URLs, or downloads.
- [ ] **Step 4: Run the focused tests to confirm GREEN.** Run both provider test modules, `cargo test --manifest-path src-tauri/Cargo.toml --all-targets`, and clippy with warnings denied. Expected: all parser/error/cancellation tests pass and no baseline regression appears.
- [ ] **Step 5: Self-review.** Verify that a tool error produces a typed provider state and bounded redacted diagnostic excerpt, malformed optional fields do not discard valid entries, and no stderr, header, cookie, or resolved URL reaches `SearchResult`.
- [ ] **Step 6: Commit boundary.** Commit as `feat: add yt-dlp source adapters` after the task review is clean.

### Task 5: Provider-independent SearchService, cache, sorting, and events

**Files:**

- Modify: `src-tauri/src/search/mod.rs`
- Modify: `src-tauri/src/search/types.rs`
- Modify: `src-tauri/src/search/sort.rs`
- Modify: `src-tauri/src/sources/traits.rs`
- Test: SearchService unit tests with fake adapters

**Consumes:** Tasks 1-4 adapter implementations and shared DTOs.

**Produces:** `SearchService::new`, `SearchService::start_search`, `SearchService::cancel_search`, `SearchService::provider_statuses`, `SearchEventSink`, independent provider tasks, fixed lens selection, 2/15-second provider timeouts, cancellation completion, stale SearchId tagging, provider-local sort, and bounded in-memory cache.

- [ ] **Step 1: Write failing tests.** Add tests named `registry_has_four_adapters_but_all_excludes_spotify`, `local_finishes_before_slow_youtube_and_emits_first`, `youtube_error_does_not_discard_local`, `soundcloud_timeout_does_not_discard_other_sections`, `new_query_cancels_old_query`, `stale_provider_completion_keeps_old_search_id`, `completion_emits_once`, `cancellation_completion_emits_once`, `provider_sort_is_independent`, `null_sort_values_are_last`, `relevance_preserves_provider_order`, `unsupported_engagement_falls_back_to_relevance`, `spotify_is_only_queried_by_spotify_lens_with_gate`, `cache_key_includes_lens_sort_direction_limit_and_market`, `cache_never_exceeds_100_entries`, and `local_cache_ttl_is_at_most_five_seconds`.

```rust
#[tokio::test]
async fn local_finishes_before_slow_youtube_and_emits_first() {
    let service = test_search_service(slow_youtube(), ready_local(), ready_soundcloud());
    let started = service.start_search(test_request(SearchLens::All), sink.clone()).unwrap();
    let first = next_provider_event(&mut events).await;
    assert_eq!(first.provider, ProviderKind::Local);
    assert_eq!(first.search_id, started.search_id);
}
```

- [ ] **Step 2: Run the focused tests to confirm RED.** Run `cargo test --manifest-path src-tauri/Cargo.toml search::tests -- --nocapture`. Expected: failure because SearchService orchestration is absent.
- [ ] **Step 3: Implement orchestration.** Validate `SearchRequest`, cancel the prior active watch sender when a new request begins, generate a new SearchId, select adapters according to the exact lens map, spawn independent Tokio tasks, wrap each adapter in its provider timeout, emit `ProviderSearchSection` as each task ends, and emit exactly one `SearchCompleted` after all selected tasks report. A cancelled/expired provider returns its own section state; other sections remain valid. Sort only within each section, preserving provider-native relevance order; use stable original rank then provider item ID; keep null values last. Add a 100-entry timestamped cache with the specified per-provider TTL and no persistence.
- [ ] **Step 4: Run the focused tests to confirm GREEN.** Run the SearchService tests, all Rust tests, and fmt/clippy. Expected: independent timing, cancellation, sorting, cache, Spotify isolation, and one-completion assertions pass.
- [ ] **Step 5: Self-review.** Trace a new query, old query cancellation, stale event, timeout, and completion path; verify Spotify is never included in `ALL`, `TRACKS`, `ARTISTS`, or `ALBUMS`, and verify SearchService has no playback, persistence, fusion, or download call.
- [ ] **Step 6: Commit boundary.** Commit as `feat: add concurrent multi-source search` after the task review is clean.

### Task 6: Spotify PKCE, secure credentials, transport, and isolated adapter

**Files:**

- Create: `src-tauri/src/credentials/mod.rs`
- Modify: `src-tauri/src/sources/spotify.rs`
- Modify: `src-tauri/src/sources/mod.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Test: credential, PKCE, callback, transport, parser, and gate unit modules

**Consumes:** Task 1 DTO/security contracts, Task 5 `SourceAdapter` and lens isolation, existing `uuid`, `sha2`, `url`, Tokio, and the attached PKCE requirements.

**Produces:** `CredentialStore`, `MemoryCredentialStore`, `KeyringCredentialStore`, redacted `SpotifyCredentialRecord`, `SpotifyAuthService`, `SpotifyHttpTransport`, `ReqwestSpotifyTransport`, `SpotifySourceAdapter`, `SpotifySetupStatus`, `SpotifyAuthState`, market validation, PKCE URL/callback/token exchange, refresh-token rotation, 401-once retry, and `SPOTDIY_ENABLE_SPOTIFY_DEV` gate.

- [ ] **Step 1: Write failing tests.** Add tests named `pkce_verifier_has_43_to_128_allowed_characters`, `pkce_challenge_is_sha256_urlsafe_without_padding`, `oauth_state_is_fresh`, `callback_requires_exact_path`, `callback_rejects_state_mismatch`, `callback_rejects_oauth_error`, `callback_times_out_at_120_seconds`, `loopback_binds_only_127_0_0_1_with_dynamic_port`, `authorization_requests_no_scopes`, `market_requires_two_ascii_letters_and_uppercases`, `credential_round_trip_uses_memory_store`, `credential_debug_redacts_refresh_token`, `credential_store_failure_fails_closed`, `token_exchange_uses_pkce_without_secret`, `refresh_rotates_refresh_token`, `spotify_search_uses_exact_endpoint_and_limit_10`, `spotify_401_refreshes_once_then_retries_once`, `spotify_403_maps_forbidden`, `spotify_429_maps_rate_limit_and_retry_after`, `spotify_quota_exceeded_maps_separately`, `spotify_malformed_json_is_typed_error`, `spotify_partial_release_date_preserves_precision`, `spotify_optional_fields_are_nullable`, and `disabled_gate_performs_no_network_or_auth`.

```rust
#[test]
fn credential_debug_redacts_refresh_token() {
    let record = SpotifyCredentialRecord::new("public-client", "VN", "secret-refresh-token").unwrap();
    let debug = format!("{record:?}");
    assert!(!debug.contains("secret-refresh-token"));
    assert!(debug.contains("redacted"));
}
```

- [ ] **Step 2: Run the focused tests to confirm RED.** Run `cargo test --manifest-path src-tauri/Cargo.toml credentials::tests sources::spotify::tests -- --nocapture`. Expected: failure because the credential/auth/Spotify transport implementation is absent.
- [ ] **Step 3: Implement secure PKCE and transport.** Use one serialized keyring entry with service `SpotDIY` and username `spotify-pkce`; production reads/writes Windows Credential Manager and tests use memory only; no plaintext fallback exists. Generate UUID-random state and a 64-character UUID-hex verifier; compute SHA-256 and `URL_SAFE_NO_PAD`; bind `127.0.0.1:0`; accept only `/callback`; exchange with `grant_type=authorization_code`, `code`, `redirect_uri`, `client_id`, and `code_verifier`; request no scopes; bound listener lifetime to 120 seconds; return a minimal safe browser response. Build one reqwest client with 10-second timeout, no cookies, HTTPS-only Spotify endpoints, and restrictive redirects. Search with `GET https://api.spotify.com/v1/search`, `type=track,artist,album`, configured uppercase market, `limit=10`, `offset=0`; parse only nullable normalized fields and typed partial dates; set Spotify popularity capability false. Map 401/403/429/quota/5xx/timeout without sleeps or retry storms; refresh once and retry one 401 once.
- [ ] **Step 4: Run the focused tests to confirm GREEN.** Run the Spotify tests, all Rust tests, fmt, and clippy. Expected: credential secrets stay out of Debug/error/DTO paths, the dev gate prevents network/auth by default, and all transport mappings pass.
- [ ] **Step 5: Self-review.** Search the changed Rust source for `client_secret`, `access_token`, `refresh_token`, `authorization_code`, `code_verifier`, and `Authorization`; confirm occurrences are only typed field names, test placeholders, or internal redaction/transport code; confirm none are logged, serialized into SQLite, or returned to frontend. Confirm no Spotify result is passed to a non-Spotify adapter.
- [ ] **Step 6: Commit boundary.** Commit as `feat: add isolated spotify catalog adapter` after the task review is clean.

### Task 7: AppState, dynamic provider status, Tauri commands, and native events

**Files:**

- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/ipc/mod.rs`
- Modify: `src-tauri/src/media_tools/mod.rs` only for status accessors required by IPC
- Modify: `src-tauri/src/sources/mod.rs` only for production registry construction
- Test: IPC serialization/status tests and native command integration tests

**Consumes:** Tasks 1-6 public contracts, existing `Database`, `LibraryService`, `PlaybackService`, `MediaToolManager`, Tauri opener plugin, and current AppState startup.

**Produces:** `AppState::search`, one shared Spotify auth service, production adapter registry, dynamic `ProviderStatus`, `start_search`, `cancel_search`, `get_spotify_setup_status`, `begin_spotify_authorization`, `disconnect_spotify`, `open_provider_result`, `search://provider-update`, `search://complete`, and `spotify://auth-state`.

- [ ] **Step 1: Write failing tests.** Add tests named `provider_status_reports_local_folder_configuration`, `provider_status_shares_one_ytdlp_state_for_youtube_and_soundcloud`, `provider_status_reports_spotify_compliance_disabled_without_network`, `spotify_popularity_capability_is_false`, `startup_succeeds_without_mpv_ytdlp_spotify_or_network`, `search_ipc_serializes_only_normalized_fields`, `search_commands_reject_secret_arguments_at_compile_boundary`, `open_provider_result_rejects_wrong_host`, `open_provider_result_rejects_non_https`, and `stale_search_events_are_tagged_with_search_id`.

```rust
#[test]
fn provider_status_reports_spotify_compliance_disabled_without_network() {
    let status = provider_statuses(&fixture_runtime_without_spotify());
    let spotify = status.iter().find(|item| item.kind == ProviderKind::Spotify).unwrap();
    assert!(!spotify.available);
    assert_eq!(spotify.runtime_status, ProviderRuntimeStatus::Disabled);
}
```

- [ ] **Step 2: Run the focused tests to confirm RED.** Run `cargo test --manifest-path src-tauri/Cargo.toml ipc::tests -- --nocapture`. Expected: failure because status and commands have no runtime/search wiring.
- [ ] **Step 3: Implement native integration.** Instantiate one `MediaToolManager`, one local adapter, two yt-dlp adapters sharing it, one isolated Spotify adapter/auth service, and one `SearchService`; retain the same manager clone used by `PlaybackService`. Make startup tolerate missing/broken tools, disabled Spotify, offline network, and missing mpv. Derive Local configured from enabled folders, YouTube/SoundCloud availability from the shared yt-dlp status, and Spotify status from gate plus local credential state without network. Add only the narrow commands named by the specification; do not add generic provider execution or URL commands. Validate provider URLs before calling the existing opener and emit only normalized payloads.
- [ ] **Step 4: Run the focused tests to confirm GREEN.** Run IPC tests, all Rust tests, fmt, clippy, and a Tauri compile check with the external target. Expected: command registration compiles; status is truthful; existing playback ownership and behavior remain unchanged.
- [ ] **Step 5: Self-review.** Inspect `AppState` and startup for duplicate managers, secret-bearing command parameters, path/URL leakage, provider aggregation that accidentally includes Spotify, or any playback/service mutation outside existing local commands.
- [ ] **Step 6: Commit boundary.** Commit as `feat: add native search and provider integration` after the task review is clean.

### Task 8: Typed frontend IPC, search hook, SearchPage, and provider setup UI

**Files:**

- Modify: `src/types/domain.ts`
- Modify: `src/services/ipc.ts`
- Create: `src/hooks/useSearch.ts`
- Create: `src/components/search/SearchControls.tsx`
- Create: `src/components/search/ProviderSearchSection.tsx`
- Create: `src/components/search/SearchResultCard.tsx`
- Modify: `src/pages/SearchPage.tsx`
- Modify: `src/pages/SettingsPage.tsx`
- Modify: `src/styles/globals.css` only for scoped search/provider styles
- Test: `tests/search-ipc.test.ts`, `tests/use-search.test.tsx`, `tests/search-page.test.tsx`

**Consumes:** Tasks 1 and 5-7 wire contracts/events, current typed playback/library IPC, current `useAppStatus`, existing UI components/styles, and current browser-only E2E adapter gate.

**Produces:** strict frontend mirrors for all search/auth/status DTOs; `startSearch`, `cancelSearch`, `subscribeToSearchProviderUpdates`, `subscribeToSearchCompleted`, `getSpotifySetupStatus`, `beginSpotifyAuthorization`, `disconnectSpotify`, `openProviderResult`; `useSearch`; functional SearchPage; truthful provider setup controls.

- [ ] **Step 1: Write failing tests.** Add tests named `search_ipc_rejects_malformed_provider_update`, `search_ipc_rejects_secret_fields`, `empty_search_page_has_no_provider_request`, `search_debounce_waits_250ms`, `clear_cancels_and_resets_results`, `lens_selection_changes_provider_selection`, `local_loading_then_ready`, `youtube_later_completion_does_not_block_local`, `soundcloud_error_is_independent`, `timeout_and_rate_limit_render_distinct_states`, `stale_search_id_is_ignored`, `sort_control_changes_request`, `views_and_plays_labels_are_distinct`, `missing_metrics_are_omitted`, `unsupported_lens_is_explicit`, `local_result_play_now_uses_ids`, `online_result_has_no_play_action`, `open_source_uses_validated_command`, `spotify_compliance_disabled_is_truthful`, and `spotify_dev_lens_isolated`.

```tsx
it("ignores a stale provider update", async () => {
  const view = render(<SearchPage />);
  await typeSearch("new query");
  emitProviderUpdate({ searchId: oldId, provider: "local", state: "ready", results: [] });
  expect(screen.queryByText("old result")).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run the focused tests to confirm RED.** Run `pnpm test -- tests/search-ipc.test.ts tests/use-search.test.tsx tests/search-page.test.tsx`. Expected: failure because the schemas, hook, result components, and page behavior are absent.
- [ ] **Step 3: Implement typed frontend behavior.** Mirror Rust enums exactly with no `any` or broad feature-boundary records; use strict Zod schemas for every native event and response. Make `useSearch` own 250 ms debounce, active SearchId, event cleanup, stale-ID rejection, cancellation, transient sections, sort controls, retry, and clear. Keep fixed provider section order. Render loading, ready, empty, setup-required, unavailable, unsupported-lens, rate-limited, quota-exceeded, error, cancelled, and disabled states distinctly; omit absent metrics. Render Local Play Now/Play Next/Add to Queue/Open File Location through existing ID-based IPC; render online Open source only; render Spotify attribution, contained artwork, and Open on Spotify only in the isolated lens. Add a browser-only E2E search adapter behind `!isTauriRuntime() && import.meta.env.DEV && VITE_SPOTDIY_E2E === "1"`; production browser preview has no fake provider results.
- [ ] **Step 4: Run the focused tests to confirm GREEN.** Run the three focused test files, then `pnpm test`, `pnpm typecheck`, `pnpm lint`, and `pnpm build`. Expected: search UI tests pass; all existing 26 baseline tests remain green; no console or type/lint errors occur.
- [ ] **Step 5: Self-review.** Check keyboard tab semantics, labelled input/Clear/sort/retry/external links, polite loading announcements, non-color-only errors, long-title overflow, artwork fallback, no Play button for online rows, and no Spotify result outside the Spotify lens.
- [ ] **Step 6: Commit boundary.** Commit as `feat: add multi-source search interface` after the task review is clean.

### Task 9: Browser matrix, smoke coverage, documentation, graph refresh, and delivery evidence

**Files:**

- Create: `tests/playwright/search.spec.ts`
- Create: `scripts/provider-search-smoke.ps1`
- Create: `docs/SpotDIY-Vault/ADRs/ADR-0010-source-adapters-and-search.md`
- Create: `docs/SpotDIY-Vault/ADRs/ADR-0011-spotify-pkce-compliance-isolation.md`
- Modify: `PROJECT_STATE.md`
- Modify: `feature_progress.md`
- Modify: `project_structure.md`
- Modify: `session_handoff.md`
- Modify: `ARCHITECTURE.md`
- Modify: `DECISION_LOG.md`
- Modify: `TEST_MATRIX.md`
- Modify: `docs/execution/agent-ledger.md`
- Modify: `docs/execution/integration-log.md`
- Modify: `docs/execution/verification-log.md`
- Modify: `docs/SpotDIY-Vault/04 Unified Source Model.md`
- Modify: `docs/SpotDIY-Vault/07 Provider Integrations.md`
- Modify: `docs/SpotDIY-Vault/12 Testing.md`
- Modify: `docs/SpotDIY-Vault/13 Build and Release.md`
- Modify: `docs/SpotDIY-Vault/16 Active Work.md`
- Modify: `docs/SpotDIY-Vault/17 Session Handoff.md`
- Modify: `docs/SpotDIY-Vault/Research/spotify-web-api.md` only to mark the old Client Credentials recommendation as superseded for this gated Plan 05 implementation
- Test: `tests/playwright/search.spec.ts`, opt-in provider smoke, native synthetic local search smoke, packaged search smoke

**Consumes:** All Tasks 1-8 behavior, current Playwright viewport projects, existing packaged playback smoke conventions, and the attached verification/definition-of-done requirements.

**Produces:** 12 Playwright search scenarios across all existing viewport projects; opt-in metadata-only yt-dlp smoke; isolated synthetic local/native smoke; packaged search smoke; ADRs; exact verification records; CodeGraph and Graphify evidence; clean artifact/secret checks.

- [ ] **Step 1: Write failing tests and smoke checks.** Add the 12 named Playwright scenarios `empty_search`, `independent_provider_loading`, `local_before_youtube`, `youtube_completion`, `soundcloud_error`, `partial_results_remain_usable`, `sort_interaction`, `lens_switching`, `stale_event_ignored`, `spotify_compliance_disabled`, `long_title_overflow`, and `provider_artwork_fallback`; add native synthetic local exact-title/artist/album/clear/Play Now coverage; add opt-in yt-dlp YouTube/SoundCloud metadata-only checks; add packaged isolated-profile checks for Local, truthful missing yt-dlp, cancellation cleanup, default Spotify disabled, and no owned helper process.

```typescript
test("partial provider results remain usable", async ({ page }) => {
  await page.goto("/search");
  await page.getByLabel("Search music").fill("signal");
  await expect(page.getByText("Local result")).toBeVisible();
  await expect(page.getByRole("button", { name: "Open source" })).toBeVisible();
});
```

- [ ] **Step 2: Run the focused tests to confirm RED.** Run `pnpm exec playwright test tests/playwright/search.spec.ts`, the native smoke command with its explicit opt-in variable, and the packaged smoke command with its explicit isolation variable. Expected: the new search assertions fail before the test adapter/smoke implementation exists; no normal user database is touched.
- [ ] **Step 3: Implement tests, smoke scripts, and documentation.** Use only the existing browser typed IPC harness pattern; do not alter production runtime to support mocks. Keep live provider calls opt-in and record truthful upstream failure/skip details without raw payloads. Document yt-dlp version/path policy, PKCE and keyring storage, disabled-by-default Spotify compliance gate, no Spotify in ALL/fusion/playback, no search-result persistence, schema version 2, exact limits/timeouts, CSP image hosts, and known provider limitations. Create ADR-0010 for SourceAdapter/SearchService and ADR-0011 for Spotify PKCE/compliance isolation.
- [ ] **Step 4: Run the focused checks to confirm GREEN.** Run `pnpm exec playwright test`, the local/native smoke, the opt-in yt-dlp smoke if `yt-dlp --version` is verified, the packaged search smoke, `graphify update .`, `codegraph sync .`, and `codegraph status .`. Expected: all browser scenarios pass across 1280, 1920, and 2560 projects; skipped live checks are documented with reason; graph health is recorded; repository-local Cargo target remains absent.
- [ ] **Step 5: Self-review.** Inspect `git status --short`, `git diff --check`, the complete diff, staged-file patterns, and secret scan terms `Authorization:`, `Bearer`, `access_token`, `refresh_token`, `client_secret`, `password`, `api_key`, `SPOTIFY_CLIENT_SECRET`, and `cookie`. Reject binaries, archives, databases, provider dumps, tokens, callback dumps, personal paths, and generated Playwright artifacts.
- [ ] **Step 6: Commit boundary.** Commit coherent test/documentation closure as `test: expand provider search coverage` followed by `docs: close Plan 05 source search delivery` after final review and all required gates are clean.

## Self-review coverage map

The plan covers shared typed contracts and URL security in Task 1; process bounds and hostile probes in Task 2; Local SQL semantics and no-persistence behavior in Task 3; structured YouTube/SoundCloud parsing and cleanup in Task 4; independent concurrency, stale IDs, sorting, cache, and event lifecycle in Task 5; PKCE, keyring, token handling, Spotify isolation, and rate-limit mapping in Task 6; AppState, truthful status, IPC security, and startup resilience in Task 7; Zod, debounce, SearchPage, actions, accessibility, and provider UI in Task 8; and browser/native/package/documentation/graph/delivery evidence in Task 9. Plan 06 Source Fusion remains outside every task.

## Final verification commands

Run each command freshly with the external Cargo target before claiming completion:

```powershell
$env:CARGO_TARGET_DIR = "C:\CargoTarget\SpotDIY"
pnpm typecheck
pnpm lint
pnpm test
pnpm exec playwright test
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
git diff --check
pnpm tauri build
Test-Path .\src-tauri\target
```

Expected final checks are exit code 0, exact test counts recorded in `docs/execution/verification-log.md`, `Test-Path .\src-tauri\target` equal to `False`, and Cargo artifacts under `C:\CargoTarget\SpotDIY`. Only after those gates and the independent whole-branch review pass may the controller run `git push origin main` and verify local SHA equals `origin/main` with a clean worktree.

## Execution evidence (2026-09-01)

Tasks 1-9 are implemented in the shared `main` worktree. The delivered feature
range includes the provider adapter registry, SearchService, Spotify PKCE
isolation, strict frontend search UI, browser coverage, native/live/packaged
smoke scripts, and the Plan 05 documentation boundary. Verification reports
250 Rust unit tests plus one integration test, 38 Vitest tests, 45 Playwright
runs, successful native/live/packaged smoke, and an external-target release
build. No Plan 06 implementation or database migration was added.
