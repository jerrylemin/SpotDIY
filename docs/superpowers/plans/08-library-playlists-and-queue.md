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

`feat: add playlists inbox and queue workspace`
