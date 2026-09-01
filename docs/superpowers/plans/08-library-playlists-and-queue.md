# Plan 08 — playlists and queue

## Goal

Implement durable playlists, branches, Inbox, likes/ratings/tags, queue sections, snapshots, and drag/reorder behavior.

## Dependencies

Plans 02–04 and unified track identity.

## Exact files

`src-tauri/src/playlists/mod.rs`, `src-tauri/src/queue/mod.rs`, `src-tauri/src/db/repository.rs`, `src/components/queue/**`, `src/components/library/**`, `src/pages/PlaylistsPage.tsx`, `src/stores/queue-store.ts`, and tests.

## Interfaces consumed

`UnifiedTrack`, `QueueEntry`, `PlaybackService`, and database repositories.

## Interfaces produced

Playlist/branch/Inbox commands, queue workspace DTOs, snapshot save/restore, likes, ratings, tags, and order operations.

## Tests

Playlist order, branch merge/discard, duplicate/remove, queue section movement, pin/clear, snapshot round trip, and restart persistence.

## Acceptance criteria

Queue context and playlists persist locally and remain understandable without a version-control engine.

## Commit boundary

1. `feat: add durable playlists and library collections`
2. `feat: add playlist branches and collection interface`
3. `feat: add persistent queue workspace`
4. `feat: add queue workspace interface`
5. `docs: close Plan 08 playlists and queue delivery`

## Delivered evidence (2026-09-01)

- Schema 5, durable playlists/items, seeded Inbox, one-shot branches with
  revision conflicts, likes/ratings/tags, bounded collection reads, and typed
  playlist/collection IPC are delivered.
- `PlaybackService` is the sole persistent queue owner. Up Next/Later policy,
  Later-only shuffle, pin/remove/move/clear, throttled checkpoints, immutable
  named snapshots, no-autoplay restart, first-Play position resume, and the
  dnd-kit queue drawer are delivered.
- Implementation commits are `525da8c`, `e5f7161`, `1f31d6a`, and `0a62cad`.
  Final evidence is 318 Rust tests plus real/synthetic mpv smoke, 51 Vitest
  tests, 48 Playwright runs, quality gates, Tauri packaging, packaged Plan 08
  persistence smoke, and v4-to-v5 migration smoke.
- STOPPED AFTER PLAN 08. Awaiting external ChatGPT GitHub review before Plan 09.
