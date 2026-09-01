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
evidence is in `docs/execution/verification-log.md`. Plan 05 adds no database
migration and leaves Source Fusion, resolver policy, provider playback,
downloads, and persistent queue behavior for later plans.

## Next atomic task

Plan 06 - Source Fusion and Resolver is not started. Stop here until its
approved specification is active.
