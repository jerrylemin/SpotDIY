# SpotDIY project state

State date: 2026-09-02

## Repository

- Branch: `main`
- Origin: `https://github.com/jerrylemin/SpotDIY`
- Plan 04 feature commit: `536617d` (`feat: add mpv playback service and queue transport`)
- Plan 04 review-fix commit: `af66127` (`fix: harden mpv playback lifecycle and event ordering`)
- Plan 07 implementation commits: `0dbb628`, `22438a0`, and `6012921`.
- Plan 08 implementation commits: `525da8c`, `e5f7161`, `1f31d6a`, and `0a62cad`.
- Plan 09 implementation commits: `1bc7108`, `e4d62d8`, `c25f954`, and `7b1a097`.
- Plan 10 implementation commits: `cc28ba1`, `f2a5995`, `850bc82`, `8c62aed`, and `6eb231d`.
- Plan 11 implementation commits: `e5129a0`, `f5562e1`, `0012a43`, `0026146`, `dba1f24`, `d631a2a`, `d2199d5`, `e072fec`, and `15031bf`.
- Plan 12 implementation commits: `95eb41b`, `b7daac6`, `d9b58c3`, `e4793b6`, and `3d39e1d`.
- Delivery status: Plan 12 implementation and final verification are complete; this document is part of the documentation closure boundary.

## Runtime

- Frontend: React 19, TypeScript 6 strict, Vite 8, TanStack Router/Query, Zustand, Zod.
- Native: Tauri 2, Rust stable MSVC, SQLite WAL, typed serialized DTOs, and runtime frontend parsing.
- Library: `LibraryService` owns persistent folder roots, recursive local indexing, metadata/artwork/fingerprint evidence, watcher reconciliation, and managed-source path validation.
- Playback: `PlaybackService` is the sole serialized controller. It owns the persistent ID-only queue, checkpointed position, immutable snapshots, transport, repeat/shuffle/previous/EOF policy, source switching, recovery, and shutdown.
- Playlists: `PlaylistService` owns durable playlists, seeded Inbox, playlist items, one-shot branches, likes, ratings, tags, and bounded collection reads.
- Inspector: `TrackInspectorService` owns the narrow read-only `get_track_inspector` DTO boundary; local filesystem paths never cross it, and provider URLs are revalidated before exposure.
- Lyrics: `LyricsService` owns local-first precedence, bounded LRC/embedded metadata reads, manual overrides, explicit LRCLIB lookup/cache, and typed lyrics DTOs. Local media reads are read-only through `LibraryService`.
- Bookmarks and loops: `BookmarkService` owns durable bookmarks and A/B presets; `PlaybackService` owns active A/B transport state and clears it at a new-track boundary.
- Backend: `MpvBackend` starts one external `mpv.exe` child over one fresh Windows named pipe and keeps JSON protocol/process details behind the backend boundary. Discovery is `SPOTDIY_MPV_PATH`, then PATH.
- Windows integration: `WindowsIntegrationService` owns native overlay lifecycle, tray actions, global shortcut registration/status, SMTC state, gaming click-through recovery, and output-profile application while keeping the frontend on typed DTOs.
- Tauri playback surface: `get_playback_snapshot`, `play_track`, `enqueue_track`, `play_track_next`, `toggle_play_pause`, `seek_playback`, `next_track`, `previous_track`, `set_playback_volume`, `set_playback_muted`, `set_repeat_mode`, `set_shuffle_enabled`, `get_audio_devices`, `set_audio_device`, `switch_playback_source`, `retry_playback_backend`, `clear_playback_queue`, playlist playback/queue commands, queue workspace mutations, and queue snapshot commands; state events use `playback://state` and `queue://state`.
- Downloads: `DownloadService` owns schema-v4 task persistence, yt-dlp/FFmpeg execution, bounded progress, scheduling, cancellation, retry, restart recovery, destination-side finalization, and `downloads://state` snapshots. Tasks support YouTube and SoundCloud only; Spotify and Local are rejected.
- Tauri download surface: `get_download_snapshot`, `queue_search_result_download`, `queue_source_download`, `cancel_download`, `retry_download`, `set_download_concurrency`, and `open_download_location`.
- Tauri lyrics/playback surface: typed lyrics load/save/delete/import/provider/cache commands, bookmark and A/B preset commands, and `set_ab_loop`/`clear_ab_loop`; lyrics state is presentation-owned while active loop state remains in `playback://state`.
- Build cache: Rust/Tauri output is external at `C:\CargoTarget\SpotDIY`; `src-tauri\target` is absent and the path is not committed.

## Decisions in force

- Keep one Tauri application and keep provider-specific logic behind later adapter boundaries.
- Use explicit Rust DTOs plus strict Zod parsing at the IPC boundary; frontend commands carry typed IDs and values, never local paths, pipe names, request IDs, URLs, or raw mpv JSON.
- Permit playback only for managed, indexed, enabled, available local sources resolved by Rust through `LibraryService`.
- Keep `PlaybackService` as the sole queue owner; durable queue state and snapshots use typed IDs and never expose paths, URLs, or raw queue JSON through IPC.
- Use the exact mpv startup arguments in the Plan 04 specification, positive request IDs, bounded 1 MiB frames, six property observations, generation-scoped events, and bounded quit/kill/reap.
- Keep standard data under `%LOCALAPPDATA%\SpotDIY`; `SPOTDIY_PACKAGED_DATA_ROOT` is a smoke-only isolation seam because Windows known-folder resolution does not follow a child `LOCALAPPDATA` override.
- Keep download task temp files under `%LOCALAPPDATA%\SpotDIY\cache\downloads\<DownloadTaskId>`, create final names inside the user-selected `downloads_directory`, and never expose arbitrary paths through IPC.
- Preserve provider-encoded provenance for YouTube/SoundCloud downloads; no lossy-to-FLAC claim, raw provider payload, credential, token, or automatic library mutation is allowed.
- Resolve lyrics with the explicit precedence manual override, exact local `.lrc` sidecar, embedded timed text, embedded plain text, then cached LRCLIB. Local reads never mutate media or metadata.
- Keep LRCLIB opt-in and metadata-safe: no automatic lookup, no raw provider payload persistence, no full copyrighted lyrics in fixtures or logs, and no provider result is sent to playback.
- Keep bookmarks and A/B loop state ID-based. `PlaybackService` owns loop commands; a new track clears A/B, same-track source switching and recovery restore it, and presets never autoplay.
- Keep player modes as presentation-only Zustand state. Standard, Mini, and Expanded surfaces consume the same `usePlayback()` snapshot; `SourceSwitcher` delegates source changes to `PlaybackService`.
- Derive search and track actions from provider capabilities and runtime availability. Online playback remains disabled, Spotify remains metadata-only, and local reveal remains source-ID based.
- Keep provider playback/search, lyrics, overlays, media keys/SMTC, portable mode, analytics, EQ, normalization, crossfade, ReplayGain, and unrelated refactors outside the Plan 08 boundary.
- Keep Plan 12 Windows integration optional and recoverable: unsupported SMTC and failed shortcut registrations are explicit status values, overlay windows are created lazily, and gaming click-through is session-only with a rescue shortcut.
- Persist only ordinary Windows settings, shortcut bindings, and output profiles in schema 8; do not persist overlay visibility, click-through state, tray state, SMTC runtime handles, native window handles, or media paths.

## Plan 04 verification snapshot

- Rust: 117 all-target tests pass; formatting and all-features clippy with warnings denied pass; focused playback, protocol, queue, source-resolution, and shutdown behavior is covered.
- Frontend: typecheck, lint, 26 Vitest tests, and production build pass; PlayerBar, local-library actions, and Ctrl+K transport are functional.
- Browser: 9 Playwright runs pass across the 1280, 1920, and 2560 viewport projects with the browser-only typed IPC adapter.
- Native: real synthetic WAV mpv smoke passes load, position, pause/resume, seek, volume/mute, devices, EOF, shutdown, and process exit.
- Packaged: release executable smoke passes local indexing, playback transport, graceful close, owned-mpv cleanup, restart library persistence, and empty transient queue; no temporary profile or owned process remains.
- Review: the single fresh read-only reviewer rechecked the fixes with `PASS`; critical, high, and correctness/security medium findings are zero. One low-priority request for additional hostile-probe regression coverage remains non-blocking.
- Development mpv: local `.tools\mpv\v0.41.0\mpv.exe`, version `v0.41.0-dev-g41f6a6450`, SHA-256 `6145E63F026451A764077D53FD60860EC9F5C2BC76DCD6E62A88967AC375453D`. The documented official Windows x64 asset verification is recorded separately in the execution log.

## Next slice

Plan 05 — Source Adapters and Search. Do not begin Source Fusion or provider playback in that slice until its own boundary is specified.

## Plan 05 delivery snapshot (2026-09-01)

- Delivery status: COMPLETE. Provider adapters, SearchService lifecycle,
  strict search IPC, frontend search surface, Spotify PKCE gate, browser
  matrix, native smoke, live metadata smoke, and packaged smoke are delivered
  through `ab6169d` plus the documentation closure commit.
- Search execution is concurrent and provider-independent. Local, YouTube, and
  SoundCloud participate in unified lenses; Spotify is isolated to the Spotify
  lens and remains disabled without explicit developer authorization.
- Search IDs, cancellation, timeout/error sections, stale-event rejection,
  exact completion, provider-local sorting, and bounded cache behavior are
  covered by native and frontend tests. Early native events are buffered until
  the start response supplies the active SearchId.
- Spotify uses loopback Authorization Code with S256 PKCE, memory/keyring token
  storage, no client secret, and no Spotify data in SQLite.
- Plan 05 adds no database migration. Cargo/Tauri output remains external at
  `C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` remains absent.

## Next slice after Plan 05

Plan 06 - Source Fusion and Resolver is complete through implementation tip
`afd0149` and the documentation closure that follows. The next slice is Plan
07 only after the external review requested by the delivery workflow.

## Plan 06 delivery snapshot (2026-09-01)

- Deterministic source normalization uses Unicode NFKD, accent/punctuation and
  presentation-noise cleanup, feature-artist extraction, guarded version
  qualifiers including Nightcore, and conservative artist-prefix handling.
- Automatic matching uses Jaro-Winkler integer basis points with title 55%,
  artists 35%, duration 10%, an 8800 threshold, 9000 title/artist hard
  minimums, explicit duration bands, exact guarded-version equality, stable
  ambiguity handling, and typed explanations.
- Migration 3 adds only `user_track_overrides`. Merge overrides have one
  forced target per provider identity; split overrides are target-specific;
  Spotify is rejected; search evaluation remains read-only; explicit accepted
  YouTube/SoundCloud sources are persisted without local-file rows or metadata
  moves.
- `SourceResolver` ranks the preferred playable source first, then validated
  settings/provider order and local quality. Local readiness requires the
  managed library path; YouTube/SoundCloud remain not implemented for
  playback; Spotify remains metadata-only. Playback and source switching use
  the resolver, and unavailable sources carry typed explanations.
- Final evidence: 279 Rust tests, 40 Vitest tests, 45 Playwright runs, strict
  typecheck/lint/fmt/clippy, frontend and Tauri builds, real mpv smoke,
  packaged playback/restart/cleanup smoke, and an explicit v2-to-v3 migration
  smoke all pass. Cargo output remains at `C:\CargoTarget\SpotDIY`; the
  repository-local `src-tauri\target` is absent.

## Prior Plan 06 handoff

Plan 06 was the predecessor boundary; its external review gate was completed before Plan 07 implementation began.

## Plan 07 delivery snapshot (2026-09-01)

- Delivery status: COMPLETE through implementation tip `6012921`; documentation closure is the final Plan 07 commit.
- Schema version 4 adds only `downloads` and the singleton `download_settings` table. Existing schema-3 data is preserved, YT/SC sources gain downloads capability, and playback capability remains unchanged.
- `DownloadService` persists UUID tasks, owns bounded yt-dlp children, parses machine progress, schedules up to four tasks, cancels/reaps owned processes, retries without duplication, recovers active tasks after restart, retains missing completed outputs, and finalizes across volumes without overwriting existing files.
- Downloads UI exposes task history, progress, bytes, speed, ETA, state, retry count, provider-encoded provenance, output format, errors, output-missing state, folder selection, concurrency, cancel/retry, and trusted folder opening. Search offers Audio/Video only for YouTube/SoundCloud tracks; Spotify and Local remain unavailable.
- Final evidence: 308 Rust unit tests plus the synthetic mpv integration test, 47 Vitest tests, 45 Playwright runs, typecheck/lint/build, fmt/clippy, Tauri release packaging, explicit real-mpv smoke, packaged playback/restart/cleanup smoke, and five native provider-search smoke checks pass. Optional live provider/download smoke was not run; the existing optional packaged-search harness still has an immediate start/cancel race when yt-dlp is intentionally missing.
- Storage remains external at `C:\CargoTarget\SpotDIY` for build output and `%LOCALAPPDATA%\SpotDIY\cache\downloads\<DownloadTaskId>` for owned task temp files. No media, credentials, tokens, or raw provider payloads are committed.

## Historical next slice after Plan 07

Plan 08 is the completed follow-on delivery recorded below.

## Plan 08 delivery snapshot (2026-09-01)

- Schema version 5 adds durable `playlists`, `playlist_items`, branch-base
  snapshots, `likes`, `ratings`, `tags`, `track_tags`, `queue_entries`,
  `queue_state`, `queue_snapshots`, and immutable `queue_snapshot_entries`.
  Existing Plan 07 tracks, sources, overrides, downloads, and settings survive
  the v4-to-v5 migration.
- `PlaylistService` provides normal playlist CRUD, duplicate/remove/reorder,
  deterministic seeded Inbox, one-level one-shot branches with base snapshots,
  selected merge/discard and revision conflicts, likes, ratings 1..5, normalized
  tags, and bounded batch collection state.
- `PlaybackService` remains the sole queue owner. Up Next and Later are ordered
  sections, Autoplay is structurally empty, shuffle affects Later only, position
  checkpoints are throttled, startup restores state without autoplay, first Play
  resumes the saved current item/position, and snapshots restore fresh live IDs.
- Typed Rust/Zod IPC and `queue://state` bridge the native owner to the
  presentation-only queue drawer. Playlists and library rows expose the scoped
  collection actions; browser preview remains deterministic and native-free.
- Final evidence: 318 Rust unit tests plus synthetic and explicit real-mpv
  integration smoke, 51 Vitest tests, 48 Playwright runs, typecheck/lint/build,
  fmt/clippy, Tauri release packaging, packaged playback/restart smoke, explicit
  Plan 08 playlist/collection/queue/snapshot/restart smoke, and the v4-to-v5
  migration smoke all pass.
- CodeGraph and Graphify were refreshed once after implementation. Their final
  counts are recorded in `docs/execution/verification-log.md`.

## Plan 09 delivery snapshot (2026-09-02)

- Schema version 6 adds lyrics records, bookmarks, and A/B loop presets with
  foreign keys, bounded fields, normalized preset names, and migration tests
  preserving Plan 08 collections/queue/snapshots and Plan 07 downloads.
- `LyricsService` implements manual override, exact local `.lrc` sidecar,
  embedded timed text, embedded plain text, and cached LRCLIB precedence.
  Sidecar reads are regular-file, size-bounded, managed-path reads; manual
  import uses the native picker and never accepts an arbitrary frontend path.
- Timed LRC parsing covers integer, 1/2/3-digit fractions, multiple
  timestamps, metadata, signed offsets, inline timestamps, stable ordering,
  malformed-line fallback, and bounded input/cue counts. Embedded ID3 plain
  and SYLT text are read without media mutation.
- LRCLIB is an explicit HTTPS-only, bounded, rate-gated, metadata-only
  provider boundary. Bookmarks persist notes and positions; A/B commands stay
  inside `PlaybackService`, clear at a new-track boundary, restore on same-track
  source/recovery paths, and presets do not autoplay.
- Plan 09 final evidence: 337 Rust unit tests plus one synthetic mpv integration test,
  56 Vitest tests, 48 Playwright runs, typecheck/lint/build, Rust fmt/clippy,
  Tauri release packaging, explicit real-mpv smoke, packaged Plan 08 and Plan
  09 persistence smokes, and the named v5-to-v6 migration smoke all pass.
- CodeGraph and Graphify were each refreshed once after implementation. Build
  output remains external at `C:\CargoTarget\SpotDIY`; repository-local
  `src-tauri\target` remains absent. Live LRCLIB smoke was optional and skipped.

## Plan 10 delivery snapshot

Plan 10 is complete through implementation commits `cc28ba1`, `f2a5995`,
`850bc82`, `8c62aed`, and `6eb231d`. The delivery adds the validated 15-token
semantic theme contract, Dark/Light/System/Custom themes, persistent
Comfortable/Compact/Dense layout profiles, root theme/layout attributes,
system-theme synchronization, reduced-motion handling, accessible shared
primitives, keyboard context actions, the InspectorPanel/IconGallery
foundation, and Settings APPEARANCE controls for import/export/reset/status.
Library track actions provide the representative context-menu adoption.

Final Plan 10 evidence is 343 Rust unit tests plus one synthetic mpv integration
test, 70 Vitest tests across 18 files, and 51 Playwright tests across the 1280,
1920, and 2560 viewport projects. Typecheck, lint, build, Rust fmt, all-features
Clippy with warnings denied, all-target Rust tests, Tauri release packaging,
and `git diff --check` pass. Packaged settings smoke proves default values,
Dark/Light/Custom writes, all three layout profiles, custom-theme persistence
across restart, and reset behavior; the packaged Plan 09 playback/lyrics smoke
also passes.

Storage remains schema version 6 with `layout_profile` and `custom_theme` as
ordinary settings keys; no migration 7 was added. CodeGraph was refreshed once
and is up to date at 138 files, 4,655 nodes, and 17,004 edges. Graphify was
refreshed once with the code-only update at 4,023 nodes, 7,913 edges, and 245
communities. Build output remains external at `C:\CargoTarget\SpotDIY`; the
repository-local `src-tauri\target` remains absent.

## Next slice after Plan 10

Plan 11 main-player refinement and Track Inspector work are complete. Plan 12
overlay and Windows integration work is now complete. Waveform generation is
not claimed by Plan 09, Plan 10, Plan 11, or Plan 12.

## Plan 11 delivery snapshot

Plan 11 is complete through `15031bf`. Migration 7 repairs compatibility for
shipped schema-6 databases by rebuilding `settings_metadata` with the Plan 10
appearance keys, copying every existing value, and preserving the historical
0001 migration contract. The real legacy constraint fixture, Plan-10-shaped
schema-6 fixture, and fresh database all reach schema 7 without foreign-key
drift or lost settings.

The delivered shell adds real-data Home dashboard sections, a read-only Track
Inspector and ephemeral SearchResult inspector, source switching, measured
quality/provenance facts, capability-aware actions, command-palette routes,
centralized Escape priority, and Standard/Mini/Expanded in-shell player modes.
No new queue/playback/search/download/lyrics owner or provider behavior was
introduced; online playback and Spotify download remain unavailable.

Final evidence: 347 Rust unit tests plus one passing real-mpv integration test,
73 Vitest tests across 19 files, 63 Playwright tests across 1280/1920/2560
viewport projects, typecheck, lint, build, fmt, strict all-features Clippy,
Tauri release packaging, packaged Plan 09 persistence smoke, and packaged Plan
11 migration/shell/restart smoke. Lint retains three pre-existing Fast Refresh
warnings; build retains Browserslist, Tailwind content, dynamic-import, and
large-chunk notices.

Graphify's final code-only refresh reports 4,131 nodes, 8,235 edges, and 247
communities. CodeGraph was refreshed once for the shell/player/inspector
dependency query. Build output remains external at `C:\CargoTarget\SpotDIY`;
repository-local `src-tauri\target` remains absent.

## Plan 12 delivery snapshot

Plan 12 is complete through implementation commits `95eb41b`, `b7daac6`,
`d9b58c3`, `e4793b6`, and `3d39e1d`. Migration 8 expands the typed ordinary
settings allowlist for Windows integration, nine shortcut bindings, and output
profiles while preserving all schema-7 settings rows and advancing the latest
schema to 8.

The native boundary adds lazy Mini, Edge, Lyrics, and Gaming overlay windows
with exact labels/dimensions and always-on-top state, a tray menu, truthful
global shortcut registration/conflict/failure statuses, Windows SMTC media
commands and metadata projection, session-only Gaming click-through with a
rescue path, and bounded output-device/profile apply with rollback. The SMTC
WinRT bridge is isolated in `src-tauri/crates/spotdiy-windows-smtc`; the
frontend uses typed IPC, browser-preview adapters, Settings controls, and
command-palette actions without native-only leakage.

Final evidence: 365 Rust unit tests plus one passing real-mpv integration test,
78 Vitest tests, 69 Playwright tests across 1280/1920/2560, typecheck, lint,
production build, Rust fmt, strict all-features Clippy, schema 7-to-8 migration
coverage, Tauri release packaging, regular playback and Plan 11 packaged
smokes, and the dedicated Plan 12 packaged smoke. The live packaged check
reported `SMTC READY`, a registered controlled shortcut, overlay reuse and
topmost state, click-through recovery, output-profile apply/restore, restart
persistence, and zero owned mpv processes. Lint/build retain only documented
non-fatal warnings. CodeGraph reports 166 files, 5,349 nodes, and 19,394
edges; Graphify reports 4,467 nodes, 8,952 edges, and 262 communities. Build
output remains external at `C:\CargoTarget\SpotDIY`; repository-local
`src-tauri\target` remains absent.

## Plan 13 delivery snapshot

Plan 13 is complete through implementation commits `7579312`, `d287f65`,
`6c2b026`, `bdf04f0`, and `5e70fdf`. The Plan 12 shortcut persistence repair remains in
`3ca57a4` and was verified before Plan 13 work. SQLite remains at schema 8;
the storage-mode row is a runtime mirror, while deterministic startup selects
Standard or Portable from the executable-adjacent `SpotDIY.portable` marker.

The native boundary now owns WAL-safe online database snapshots, deterministic
format-1 `.spotdiy` archives with manifest and SHA-256 integrity metadata,
trusted local-media/artwork/sidecar selection, bounded secure ZIP validation,
staged import preview, restart-gated commit, crash recovery, rollback backups,
and Standard/Portable mode transitions. Portable mode uses exact executable-
relative `Data`, `Music`, `Covers`, `Lyrics`, `Database`, `Cache`, and `Config`
roots without AppData fallback. Settings exposes typed export, import preview,
cancel/confirm, storage status, and restart-required mode switching.

Final evidence: 393 Rust unit tests plus one passing real-mpv integration test,
81 Vitest tests across 22 files, 69 Playwright tests across the three viewport
projects, frontend typecheck/lint/test/build, Rust fmt/strict all-features
Clippy/all-target tests, Tauri release/NSIS packaging, regular/Plan 11/Plan 12
packaged smokes, and the new isolated Plan 13 packaged Standard/Portable
restart smoke. Lint retains three pre-existing Fast Refresh warnings; build
retains the existing Browserslist, Tailwind content, ineffective dynamic
import, and large-chunk notices. CodeGraph is current at 176 files, 5,781
nodes, and 21,456 edges; Graphify's final code-only refresh reports 4,723
nodes, 9,470 edges, and 277 communities. Build output remains external
at `C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` remains
absent. The requested Gmail completion message `Plan 13 finished` was sent to
`jerryle.minh.3@gmail.com`.
