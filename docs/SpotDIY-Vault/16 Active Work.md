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

## Next atomic task

Plan 10/11 visual system and main-player refinement.
