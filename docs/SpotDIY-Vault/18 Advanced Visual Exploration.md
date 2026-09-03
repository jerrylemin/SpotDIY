# Advanced Visual Exploration

## Status

Plan 15 is complete on 2026-09-03. Schema 9 remains the latest database
schema; no migration 10 was added.

## Native contract

`VisualExplorerService` returns one read-only, parameter-bound dataset for
Music Map and Library Galaxy. Requests accept query, genre, artist, liked-only,
and a 1..5,000 limit (default 2,000). Responses report total/returned counts,
truncation, deterministic order, aggregate play facts, quality, provider
count, and only trusted existing artwork-cache references. Local media paths,
raw provider URLs, credentials, and network results are excluded.

## Surfaces

- Music Map: deterministic GENRES -> ARTISTS -> ALBUMS -> TRACKS SVG graph with
  pan/zoom/reset, filters, selection, shared actions, and a 200-item Map
  Navigator.
- Library Galaxy: deterministic artist/genre clusters with golden-angle and
  TrackId-hashed positions, bounded 2D Canvas rendering, hover/focus/click,
  filters, shared actions, and a 200-item Galaxy Navigator.
- Shared actions: Play Now, Play Next, Queue, Inbox, Inspect, Lyrics, Reveal
  Local, and Preview/Cancel Preview. Radial overflow uses the linear More
  menu; dnd-kit targets have keyboard alternatives.

## Preview and theme

`PreviewService` resolves only indexed managed local sources, interlocks with
active main playback, runs one cancellable eight-second sample at no more than
35% volume, and records no queue/history/analytics/SMTC state. Theme Studio
uses schema v1 with 15 tokens and draft/session/persistent boundaries. Dynamic
Accent is session-only and samples at most 32x32 client-side pixels with
contrast fallback. Existing Comfortable/Compact/Dense profiles are reused.

## Evidence

430 Rust unit tests plus one real-mpv integration test, 88 Vitest tests, 70
Playwright tests, strict native/frontend gates, Tauri release/NSIS packaging,
and the packaged Plan 15 visual/restart smoke pass. See
`docs/execution/verification-log.md` and `docs/SpotDIY-Vault/12 Testing.md`.

## Plan 16 repairs

Visual points now carry deterministic artist-ID/name pairs and album IDs, so
same-label entities do not merge. They also carry native `canPlayback`,
`canPreview`, and `canRevealLocal` capabilities; unavailable actions remain
disabled with explicit reasons. Preview and Windows/main transport share one
serialization gate, preserving the eight-second preview rule and the
no-history/no-analytics boundary.
