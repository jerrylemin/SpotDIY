# Active work

## Current boundary

Plans 01–03 and Plan 04 are implemented. The native core owns the typed music
domain, SQLite/WAL storage through schema version 2, durable settings,
persistent managed local folders, recursive indexing/reconciliation,
metadata/artwork/fingerprint evidence, watcher recovery, typed library IPC,
and safe reveal.

Plan 04 now adds the serialized `PlaybackService`, transient ID-only queue,
external mpv JSON IPC backend, local source resolution, transport/repeat/
shuffle/EOF/previous policy, source switching, crash recovery, retry, typed
playback IPC, `playback://state`, functional PlayerBar/library controls, Ctrl+K
transport, Playwright coverage, real mpv smoke, and packaged lifecycle smoke.

The implementation is consolidated in feature commit `536617d` and review-fix
commit `af66127`. The single fresh independent review passed after the fixes;
the final release, real-mpv, and packaged smoke gates are green.

## Verification boundary

117 Rust all-target tests, fmt, clippy, 26 Vitest tests, frontend
typecheck/lint/build, 9 Playwright runs at 1280/1920/2560, real synthetic mpv
smoke, and release packaged playback/restart/cleanup smoke pass locally. Cargo
output is external at
`C:\CargoTarget\SpotDIY`; the repository-local `src-tauri\target` is absent.

The queue is deliberately transient. Persistent queue state and queue snapshots
remain Plan 08 work. Provider adapters/search, Source Fusion, downloads,
lyrics, playlists, overlays, media keys/SMTC, portable mode, analytics, EQ,
normalization, crossfade, ReplayGain, and later visual/performance work remain
outside this boundary.

## Next atomic task

Plan 05 — Source Adapters and Search. Preserve the local playback boundary and
do not begin Source Fusion until its own specification is active.

## Plan 05 delivery

Plan 05 is complete. The `SourceAdapter` registry and `SearchService` now
provide concurrent Local, YouTube, SoundCloud, and isolated Spotify search with
typed SearchIds, cancellation, timeouts, partial updates, exact completion,
stale-event rejection, provider-local sorting, and a bounded TTL cache. The
frontend search surface is strict, debounced, provider-independent, and keeps
Spotify out of unified lenses. Spotify authorization uses loopback S256 PKCE,
keyring/memory-only tokens, no client secret, and an explicit disabled-by-
default gate.

The delivery commits run from `58b5adc` through `ab6169d`; exact test and smoke
evidence is in `docs/execution/verification-log.md`. Plan 05 added no database
migration and left Source Fusion, resolver policy, provider playback,
downloads, and persistent queue behavior outside its boundary. Plan 06 below
records the approved follow-on delivery.

## Plan 06 completion

Plan 06 is complete through implementation tip `afd0149`. It delivers the
conservative deterministic Source Fusion matcher, migration-3 user overrides,
explicit YouTube/SoundCloud source acceptance, the settings-aware
`SourceResolver`, resolver-backed playback/source switching, availability
explanations, and narrow typed IPC. Final verification evidence is recorded in
`docs/execution/verification-log.md`.

The current boundary remains local-only production playback. Spotify remains
metadata-only and excluded from fusion; downloads, playlists, persistent queue,
lyrics, and later visual/performance work remain outside this plan.

## Plan 07 completion

Plan 07 is complete through implementation tip `6012921`. It delivers schema-v4
persistent download tasks, settings-backed destination selection, bounded
yt-dlp/FFmpeg execution, machine progress, concurrency, cancellation, retry,
restart recovery, safe finalization, typed IPC/events, Downloads UI, supported
YouTube/SoundCloud search actions, and truthful tool/provenance status.

The current boundary still excludes Spotify and Local downloads, online
playback, automatic source fusion/library mutation, persistent playback queue,
and all later plans. Final verification evidence is recorded in
`docs/execution/verification-log.md`.

## Plan 08 completion

Plan 08 is complete through implementation commits `525da8c`, `e5f7161`,
`1f31d6a`, and `0a62cad`. It delivers schema-5 durable playlists and
collections, seeded Inbox, one-shot branches, likes/ratings/tags, typed
playlist and collection IPC, PlaybackService-owned persistent queue sections,
throttled checkpoints, immutable snapshots, restart restore, and the queue
drawer. Final verification evidence is recorded in
`docs/execution/verification-log.md`.

## Plan 09 completion

Plan 09 is complete through implementation commits `1bc7108`, `e4d62d8`,
`c25f954`, and `7b1a097`. It delivers schema-6 local-first lyrics, bounded LRC
and embedded metadata handling, explicit LRCLIB lookup/cache, synchronized
lyrics presentation, durable bookmarks and loop presets, and
PlaybackService-owned A/B controls. Final verification and packaged restart,
queue, no-autoplay, and owned-process cleanup evidence are recorded in the
execution logs. Waveform generation is not claimed.

## Plan 10 completion

Plan 10 is complete through implementation commits `cc28ba1`, `f2a5995`,
`850bc82`, `8c62aed`, and `6eb231d`. It delivers semantic tokens, the
Dark/Light/System/Custom theme controller, validated custom theme
import/export/reset, Comfortable/Compact/Dense layout profiles, reduced motion,
accessible shared primitives, keyboard context actions, InspectorPanel and
IconGallery foundations, and Settings APPEARANCE integration with a
representative LibraryTrackRow.

The final boundary keeps storage at schema 6 with ordinary `layout_profile` and
`custom_theme` settings keys and no migration 7. Full Track Inspector, Theme
Studio, mobile UI, and Plan 11 main-player refinement remain out of scope.
Final verification and packaged settings persistence/reset evidence are recorded
in `docs/execution/verification-log.md`.

## Plan 11 completion

Plan 11 is complete through `15031bf`. It adds migration 7 compatibility for
shipped schema-6 settings, a real-data Home dashboard, persisted and ephemeral
Track Inspector surfaces, source switching, measured quality/provenance,
capability-aware actions, command-palette and Escape coordination, and
Standard/Mini/Expanded in-shell player modes. Existing service and queue
ownership remains unchanged; online playback, Spotify download, and Theme
Studio remain outside the boundary. Plan 12 now owns the optional native
Windows overlay and system-integration boundary.

Verification passes with 347 Rust unit tests plus real-mpv integration smoke,
73 Vitest tests, 63 Playwright tests across three viewport projects, frontend
and native quality gates, Tauri packaging, packaged Plan 09 persistence smoke,
and packaged Plan 11 migration/shell/restart smoke. See
`docs/execution/verification-log.md` for the exact run record.

## Next atomic task

## Plan 12 completion

Plan 12 is complete through implementation commits `95eb41b`, `b7daac6`,
`d9b58c3`, `e4793b6`, and `3d39e1d`. It delivers schema-8 Windows settings,
lazy Mini/Edge/Lyrics/Gaming overlays, tray actions, typed global shortcuts,
SMTC with an isolated WinRT bridge, session-only click-through recovery, and
rollback-safe output profiles. Verification evidence is recorded in
`docs/execution/verification-log.md` and `docs/SpotDIY-Vault/12 Testing.md`.

## Next atomic task

Plan 13 import/export and portable mode remains unstarted.
