# SpotDIY integration log

## 2026-08-30 — bootstrap

- Remote repository had no branches, so the empty workspace was initialized locally with `main` and the requested origin.
- No worker production branches were integrated; the research wave wrote only isolated Markdown notes.
- Frontend, Rust, Tauri configuration, generated icons, CI, and project memory were created in one bootstrap boundary.
- Bootstrap integration is committed and pushed as `403d923`; session bookkeeping is recorded in the follow-up commit.

## 2026-08-30 — Plan 02 domain and database

- Added bundled `rusqlite`, `chrono`, `url`, and `uuid` dependencies with the lockfile updated by Cargo.
- Integrated typed UUID domain identifiers, unified track/source/version/capability records, and explicit Spotify metadata-only guards.
- Integrated SQLite initialization with WAL, foreign keys, busy timeout, migration 1, schema metadata, optional FTS5 probing, and safe destructive-backup handling.
- Integrated focused track/artist/source repositories with transactional aggregate creation, provider identity uniqueness, local-file metadata, and preferred-source integrity checks.
- Integrated typed ordinary settings persistence and narrow settings IPC; no secret-bearing field or generic SQL command was added.
- Updated the TypeScript domain vocabulary, Zod IPC validation, execution records, project memory, and ADRs. Plan 03 work was not started.
- Implementation was committed as `2ec431b7fcbf31fbb2f2cd3b092b66ad75e81365`; a documentation-only follow-up records the final remote verification.

## 2026-08-30 — Plan 03 Local Library

- Added migration `0002_local_library.sql` without changing migration 1. It persists enabled folder roots and extends `local_files` with managed ownership, normalized paths, scan status, observed generations, measured container/artwork fields, and legacy-compatible nullable values. Path-shaped Plan 02 local provider IDs are rewritten during migration; matching legacy rows can be promoted when their selected path is discovered.
- Added `LibraryService` and focused `folders`, `scanner`, `watcher`, `metadata`, `fingerprint`, and `artwork` modules. Rust owns canonical path/reparse-point checks, recursive no-link traversal, Lofty 0.25.1 extraction, streaming SHA-256, atomic cache writes, transactional aggregate persistence, bounded pages, missing/restore/rename reconciliation, and source-ID reveal validation.
- Added Notify 8.2.0 watcher registration/recovery, 450 ms coalescing, forced watcher scans, conservative reconciliation for uncertain events, durable partial scan errors, unavailable-root source state, and manual re-registration on rescan.
- Added Tauri dialog/opener plugins, the narrow folder/scan/status/page/reveal IPC surface, artwork-only asset scope, typed Zod validation, React Query library hooks, progress cleanup, real paged folder/track UI, quality/provenance/error states, and disabled Plan 04 playback controls.
- Added focused frontend tests and 53 Rust tests covering migration compatibility, path/fingerprint/metadata/artwork helpers, scanner/reconciliation identity, watcher semantics, paging, reveal security, and recovery. `0001_initial.sql` remains unchanged and no user media is mutated.
- The independent final review requested medium corrections; those were integrated before the final verification pass. Derived Graphify output was updated to 1,202 nodes and 1,662 edges and remains ignored.

## 2026-08-31 — Plan 04 Playback Engine

- Integrated `MediaToolManager` discovery/health, typed playback contracts,
  bounded mpv JSON protocol, the external Windows named-pipe backend, the
  serialized `PlaybackService`, transient ID-only queue, managed local source
  resolution, transport/repeat/shuffle/EOF/previous policy, source switching,
  recovery/retry, and bounded shutdown.
- Integrated typed Tauri playback commands and `playback://state`, strict
  Zod parsing/revision handling, functional PlayerBar and local-library
  transport actions, Ctrl+K transport commands, the three-width Playwright
  matrix, real mpv smoke, and packaged lifecycle smoke.
- The backend launches exactly `--no-config --idle=yes --terminal=no
  --input-terminal=no --audio-display=no --input-ipc-server=<fresh pipe>`;
  no provider, download, persistent queue, or later-plan playback behavior was
  added. Plan 04 adds no database migration.
- The packaged harness initially exposed a Windows known-folder isolation
  issue. The final implementation uses a smoke-only
  `SPOTDIY_PACKAGED_DATA_ROOT`, backed up and removed only the harness rows
  written to the normal database, and verified zero remaining harness rows.
- The coherent feature boundary is committed as `536617d` (`feat: add mpv
  playback service and queue transport`), with review remediation committed
  as `af66127` (`fix: harden mpv playback lifecycle and event ordering`).
- The fresh independent recheck returned `PASS` with no critical, high, or
  correctness/security medium findings. The final frontend/native/release,
  real-mpv, and packaged lifecycle gates passed; the documentation closure is
  recorded in this delivery boundary and remains a separate docs commit.

## Plan 05 source adapters and search

- Integrated the common `SourceAdapter` contract, Local SQLite search,
  yt-dlp-backed YouTube and SoundCloud metadata search, and isolated Spotify
  catalog metadata search.
- Integrated concurrent `SearchService` execution with exact lens mappings,
  SearchId lifecycle, cancellation, provider timeouts, partial events, exact
  completion, stale-event handling, provider-local sorting, and a bounded TTL
  cache.
- Integrated strict Rust/Zod search DTOs, the 250 ms debounced frontend hook,
  provider sections, local playback actions, safe provider URL opening, and
  the development-only browser E2E adapter.
- Integrated Spotify loopback Authorization Code with S256 PKCE, no client
  secret, keyring/memory-only token handling, and the explicit disabled-by-
  default compliance gate. Plan 05 adds no database migration or provider
  persistence.
- Integrated browser, native, metadata-only live, and isolated packaged smoke
  coverage. The packaged harness validates local indexing/result rendering,
  failure isolation, cancellation, Spotify gating, and owned-process cleanup.

## Plan 06 source fusion and resolver

- Integrated migration 3 and the `user_track_overrides` repository with
  transactional one-target Merge replacement, target-specific Split rows, and
  Spotify exclusion.
- Integrated deterministic NFKD normalization, guarded version extraction,
  integer Jaro-Winkler scoring, duration bands, hard title/artist minima,
  weighted automatic matching, stable ambiguity handling, and typed fusion
  explanations.
- Integrated explicit YouTube/SoundCloud match acceptance into
  `TrackSource` persistence. Accepted remote sources use backend capability
  truth, retain validated provider URLs, and never create `local_files` or
  mutate target metadata/preferred-source state.
- Integrated settings-aware `SourceResolver` ranking and its readiness probe
  seam into automatic playback and exact source switching. Local playback
  remains gated by availability, capability, and managed-library path
  resolution; online providers remain non-playable in this plan.
- Integrated strict frontend fusion/resolution DTOs and only the five scoped
  Tauri commands. No Fusion UI, automatic SearchPage grouping, provider URL
  playback, download path, or automatic search persistence was added.
- Plan 06 implementation commits are `cf0248f`, `d4f72a7`, `4161810`, and
  `afd0149`; documentation closure follows the final verification gates.

## 2026-09-01 - Plan 07 persistent download manager

- Integrated schema migration 4 with only the `downloads` task table and the
  singleton `download_settings` row. Existing data is preserved and persisted
  YouTube/SoundCloud sources gain downloads capability without gaining
  playback capability.
- Integrated typed download task/repository/state contracts, UUID task IDs,
  settings-backed destination validation, owned per-task temp roots, filename
  sanitization/collision handling, destination-side atomic finalization, and
  output-missing history.
- Integrated FFmpeg discovery/probe with the existing `MediaToolManager`, a
  direct bounded yt-dlp download runner, machine progress/file records,
  scheduler concurrency 1..4, cancellation/reap, retry, restart recovery,
  progress write throttling, and bounded shutdown cleanup.
- Integrated `DownloadService` into `AppState`, the seven narrow Tauri
  commands, `downloads://state`, strict Zod parsing, TanStack Query snapshot
  updates, Downloads task controls, search-result Audio/Video actions for
  YouTube/SoundCloud only, and Settings tool health.
- The implementation boundary is committed as `0dbb628` (`feat: add
  persistent download task model`), `22438a0` (`feat: add managed media
  download execution`), and `6012921` (`feat: add download manager
  interface`). Documentation closure follows the final verification gates.
- No Plan 08 work, Spotify/Local download path, online playback, automatic
  library mutation, source fusion, raw provider payload, credential, token,
  media artifact, or repository-local Cargo target was integrated.

## 2026-09-01 - Plan 08 playlists, collections, and persistent queue

- Integrated migration 5 with durable playlists/items, protected seeded Inbox,
  one-shot branch base snapshots, likes, ratings, tags, track tags, queue state,
  queue entries, and immutable queue snapshots. Foreign keys, dense positions,
  source/track ownership checks, and Plan 07 data preservation remain enforced.
- Integrated `PlaylistService` for playlist CRUD, duplicate/remove/reorder,
  branch diff/selected merge/discard with revision conflicts, Inbox idempotence,
  1..5 ratings, normalized tags, and bounded batch collection reads.
- Integrated `PlaybackService`-owned persistent queue sections, pin/remove/move/
  clear operations, Later-only shuffle, throttled position checkpoints,
  restart restore without autoplay, first-Play position resume, and fresh-ID
  snapshot restore. No separate `QueueService` was introduced.
- Integrated typed Rust/Zod playlist, collection, playlist playback, queue, and
  snapshot IPC; the presentation-only queue drawer uses `queue://state` and
  dnd-kit handles while browser preview remains native-free.
- The implementation boundary is committed as `525da8c`, `e5f7161`, `1f31d6a`,
  and `0a62cad`; documentation closure follows the final verification gates.
