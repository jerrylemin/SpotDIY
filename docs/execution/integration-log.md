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

## 2026-09-02 - Plan 09 local-first lyrics, bookmarks, and A/B loop

- Integrated migration 6 with `lyrics`, `bookmarks`, and `ab_loop_presets`.
  Foreign keys, bounded text/notes, normalized preset names, loop-gap and
  duration validation, and v5 fixture preservation remain enforced.
- Integrated `LyricsService` with manual, exact sidecar `.lrc`, embedded timed,
  embedded plain, and cached LRCLIB precedence. Managed local reads are
  read-only and bounded; native file selection is the only manual import path.
- Integrated bounded LRC parsing, ID3 plain/SYLT metadata extraction, typed
  lyrics DTOs, synchronized cue selection, manual override/delete, explicit
  LRCLIB lookup/search/select/cache actions, and attribution-safe provider data.
- Integrated `BookmarkService` persistence and typed bookmark/preset IPC.
  `PlaybackService` owns active A/B state and sends only typed set/clear
  commands to mpv; new tracks clear the loop, same-track source/recovery paths
  restore it, and presets do not autoplay.
- Integrated `/lyrics`, player markers/controls, manual edit/import/delete,
  explicit online actions, bookmark and A/B/preset controls, strict Zod IPC,
  and the Plan 09 packaged persistence/restart smoke. Browser preview remains
  native-free and no waveform generation was added.
- The implementation boundary is committed as `1bc7108`, `e4d62d8`,
  `c25f954`, and `7b1a097`; documentation closure follows the final gates.

## 2026-09-02 - Plan 10 UI design system and theme foundations

- Integrated the schema-version-1 custom theme contract with exactly 15
  semantic colors, strict hex values, size/name limits, and WCAG contrast
  validation in both TypeScript/Zod and Rust. Added Dark, Light, System, and
  Custom resolution with root `data-theme` and `data-layout` attributes.
- Integrated persistent `layout_profile` and `custom_theme` ordinary settings
  keys without adding migration 7; native persistence remains in
  `SettingsRepository` and browser preview uses a bounded in-memory adapter.
- Integrated semantic CSS tokens, foundations/primitives, reduced-motion
  configuration, shared accessible components, keyboard context actions,
  InspectorPanel/IconGallery foundations, and custom SpotDIY icon additions.
- Integrated Settings APPEARANCE controls for theme/layout selection,
  custom-theme import/export/reset/status/errors and representative
  `LibraryTrackRow` context actions. Added the three-width Playwright design
  matrix and screenshot-output coverage without committing screenshots.
- The implementation boundary is committed as `cc28ba1`, `f2a5995`,
  `850bc82`, `8c62aed`, and `6eb231d`; documentation closure follows the final
  verification gates.

## 2026-09-02 - Plan 11 main shell, Track Inspector, and player modes

- Restored the historical settings allowlist in migration 1 and added
  migration 7, which rebuilds only `settings_metadata`, copies every row, and
  advances schema metadata to version 7. Independent old-constraint,
  Plan-10-shaped, and fresh fixtures verify settings preservation, appearance
  writes, custom-theme activation, and clean foreign keys.
- Integrated `TrackInspectorService` and `get_track_inspector` as a purpose-
  built read-only DTO. Local paths remain behind source-ID reveal; only
  validated remote provider URLs can appear as canonical URLs.
- Integrated real-data Home sections, persisted and ephemeral inspector
  surfaces, source switching, measured quality/provenance facts, and shared
  capability/runtime action derivation across Search, Library, Playlists, and
  Downloads. Online playback and Spotify downloads remain unavailable.
- Integrated AppShell Escape priority, command-palette navigation, focus
  restoration, and Standard/Mini/Expanded in-shell player modes. All modes
  consume the existing `usePlayback()` snapshot; no playback or queue owner
  was added.
- Added the packaged Plan 11 smoke to the existing playback harness. It seeds
  a real schema-6 database, verifies migration 7 and appearance persistence,
  exercises the shell/inspector/queue/Lyrics paths, and checks restart
  persistence, no autoplay, and owned-mpv cleanup.
- Implementation and smoke commits are `e5129a0`, `f5562e1`, `0012a43`,
  `0026146`, `dba1f24`, `d631a2a`, `d2199d5`, `e072fec`, and `15031bf`.
- Final Graphify code-only output is 4,131 nodes, 8,235 edges, and 247
  communities. CodeGraph was refreshed once for the shell/player/inspector
  dependency query; derived graph files remain ignored.

## 2026-09-02 - Plan 12 Windows overlays and system integration

- Integrated schema 8 ordinary settings for Windows integration, nine global
  shortcut bindings, and normalized output profiles. Migration 8 rebuilds only
  `settings_metadata`, copies schema-7 rows unchanged, and preserves the
  foreign-key boundary.
- Integrated `WindowsIntegrationService` with lazy Mini, Edge, Lyrics, and
  Gaming overlay windows, exact native labels/dimensions, always-on-top state,
  safe edge placement, tray actions, per-binding shortcut status, and typed
  browser/native IPC. The overlay capability remains narrow.
- Integrated Windows SMTC through the isolated
  `spotdiy-windows-smtc` WinRT helper, with bounded metadata projection and
  typed transport commands. Added session-only Gaming click-through with the
  `Ctrl+Alt+Shift+G` rescue path and truthful failure states.
- Integrated serialized output-device/profile apply and rollback through
  `PlaybackService`; profile changes preserve track, queue, position, and phase.
  Added Settings and command-palette controls plus four native overlay React
  surfaces; browser preview remains native-free.
- Added regular, Plan 11, and dedicated Plan 12 packaged smoke coverage. The
  Plan 12 live run reported `SMTC READY`, a registered controlled shortcut,
  overlay reuse/topmost, click-through recovery, output-profile apply/restore,
  schema-8 restart persistence, and zero owned mpv processes.
- Implementation commits are `95eb41b`, `b7daac6`, `d9b58c3`, `e4793b6`, and
  `3d39e1d`; documentation closure follows this verified integration.

## 2026-09-02 - Plan 13 import/export and portable storage

- Integrated deterministic startup storage resolution before SQLite open. The
  exact executable-adjacent `SpotDIY.portable` marker selects Portable; no
  marker selects Standard; portable setup failures are explicit and never fall
  back to AppData. Mode switches copy databases with online WAL-safe backups,
  change the settings row only as a runtime mirror, and create/remove the
  marker last with restart required.
- Integrated `BackupService` with format-1 `.spotdiy` export/import. ZIP paths,
  compression, bounds, case collisions, symlinks, manifest bytes, declared
  payloads, hashes, schema, integrity, and foreign keys are validated before
  import staging. Export includes only the database and explicitly selected
  trusted local audio, same-stem sidecars, and active artwork cache.
- Integrated pending restore descriptors, restart-before-apply, applying-state
  recovery, active database rollback via online snapshot, created-media
  tracking, and missing-file preview. Native dialogs retain ownership of
  destination, archive, and Standard audio restore folder selection.
- Integrated typed frontend IPC, `useBackup`, the Settings Backup section,
  import preview/confirm/cancel, storage status, and restart-required mode
  controls. Browser preview remains path-safe and native-free.
- Fixed the packaged startup path by resolving the current executable's parent
  directly; the release executable now passes regular, Plan 11, Plan 12, and
  Plan 13 isolated packaged smokes.
- Implementation commits are `7579312`, `d287f65`, `6c2b026`, `bdf04f0`, and
  `5e70fdf`;
  Plan 12 shortcut repair is `3ca57a4`.
- `codegraph sync .` reported the index already current at 176 files, 5,781
  nodes, and 21,456 edges; `graphify update .` reports 4,723 nodes, 9,470
  edges, and 277 communities.

## 2026-09-03 - Plan 14 smart features and local analytics

- Integrated schema 9 with exactly four new tables: `track_genres`,
  `listening_sessions`, `play_history`, and `smart_playlists`. Existing
  albums release dates are reused; local tag extraction supplies bounded,
  normalized genres and validated release dates.
- Integrated `AnalyticsRecorder` with `PlaybackService` observation,
  qualified-play batching, 30-minute session grouping, local aggregate
  queries, heatmap, Taste Timeline, Time Machine, and reopen-as-queue actions.
  Paused time, skipped/unqualified plays, Private Session, and Temporary Mode
  follow the documented persistence boundaries.
- Integrated typed smart-playlist rule validation/CRUD/preview and
  parameter-bound allowlisted SQL, plus deterministic non-ML Smart Shuffle
  with familiarity/variety/freshness/discovery signals and anti-repetition.
- Integrated `/analytics`, the Playlists smart-rule surface, command-palette
  and listening-mode controls, strict frontend DTO validation, and a Plan 14
  packaged harness. The harness parses successfully; native/package execution
  remains blocked by the local MSVC/SDK and browser availability.
- Hardened Plan 13 restore staging with component-by-component trusted-root
  checks and cleanup ownership proof; added a symlink `imports` regression.
- Phase commits are `f516eee` (trusted staging), `6a02daa` (history and
  sessions), `ab01f3e` (smart playlists), `aedd3a8` (shuffle and Temporary
  Mode), `4527b02` (analytics interface), and `dbcdb2f` (packaged coverage).
  The final docs commit closes the record. The commits are local only while native
  and package gates remain blocked; three unrelated pre-existing worktree
  changes remain unstaged.

## 2026-09-03 - Plan 15 advanced visual exploration

- Repaired the Plan-14 Private Session transition in `1403955`: the active
  interval now closes through the shared transition classifier, preserving
  qualified/stopped/skipped outcomes and preventing later private intervals
  from being recorded.
- Integrated `VisualExplorerService` and typed/Zod IPC for one bounded,
  read-only schema-9 dataset with deterministic ordering, filters, aggregates,
  quality, truncation, and trusted artwork-cache references only. No media
  paths, raw provider URLs, credentials, or network calls cross the boundary.
- Integrated deterministic Music Map SVG and Library Galaxy Canvas routes with
  pan/zoom/reset, selection, filters, bounded DOM navigators, shared actions,
  radial More fallback, and dnd-kit/keyboard queue targets.
- Integrated separate local `PreviewService` with TrackId resolution,
  playback interlock, 8-second/35%-volume limits, owned-process cancellation,
  shutdown, and injectable backend tests. Preview never mutates playback
  history, analytics, queue, SMTC, or providers.
- Integrated Theme Studio's schema-v1/15-token draft workflow, import/export,
  session preview, Save & Activate, clone actions, contrast-safe 32x32 dynamic
  accent, and existing layout profiles; added browser, native, packaged, and
  restart coverage.
- Implementation commits are `1403955`, `d73e755`, `e442767`, `977176e`,
  `aee1dad`, and `2612804`. Graphify final output is 5,517 nodes, 12,505
  edges, and 268 communities; CodeGraph is unavailable.

## 2026-09-03 - Plan 16 quality and release implementation

- Closed the Plan 15 preview/main audio race with a shared PreviewService gate
  used by Tauri transport, Windows SMTC/shortcut/tray transport, output
  profile/device changes, source switching, backend retry, and operations that
  replace or start playback. Queue-only mutations remain independent.
- Added ordered artist IDs, album IDs, and native set-based visual capabilities
  without a schema migration or N+1 frontend/provider lookup. Music Map and
  Galaxy now keep stable identity separate from display labels.
- Removed all three Fast Refresh warnings by moving runtime icon/theme exports
  to non-component modules. Lazy-loaded all page routes after the measured
  large main chunk warning; the final main chunk is 404.04 kB minified.
- Added axe-core 4.13.0 route/keyboard coverage, immutable-SHA CI gates with
  exact Node/pnpm pins, a deterministic 5,000-track layout benchmark, and
  third-party dependency notices. No speculative product feature or migration
  was added.
- Plan 16 is committed and pushed as `5dfdd1e`; the primary checkout's
  unrelated `.gitignore` change and two deleted Plan 05 reports were not
  touched.
