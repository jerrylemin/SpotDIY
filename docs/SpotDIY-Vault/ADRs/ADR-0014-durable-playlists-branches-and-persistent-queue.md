# durable playlists, lightweight one-shot branches, and PlaybackService-owned persistent queue

- Status: Accepted
- Date: 2026-09-01
- Scope: Plan 08

## Context

SpotDIY needs local-first playlists and library collections without introducing
a version-control engine or duplicating authoritative state in the frontend.
Playback already has a serialized controller and a transient queue, but users
need the queue, current position, and named queue snapshots to survive restart.
The feature must remain compatible with the unified track/source model and the
existing SourceResolver boundary.

## Decision

Use `PlaylistService` for durable normal playlists, playlist items, the seeded
system Inbox, likes, ratings, tags, and bounded collection-state reads. Playlist
items reference `TrackId` and optional requested `SourceId`; duplicate items are
allowed and positions remain dense. A branch is a lightweight one-level copy
with a parent revision and immutable base item snapshot. Diff, selected merge,
and discard are explicit one-shot operations with revision/conflict errors.

Keep queue ownership inside `PlaybackService`; do not add a separate
`QueueService`. Persist only typed queue identities, section, pin state, order,
current entry, repeat/shuffle state, history/traversal order, and checkpointed
position. `Up Next` precedes `Later`; `Autoplay` is structurally empty. Shuffle
changes only the Later traversal order while preserving current and consumed
history. Startup restores the queue without autoplay, and the first Play
resumes the saved current item and position.

Schema migration 5 adds the playlist, collection, queue, and immutable snapshot
tables with foreign keys, indexes, singleton/system-row protection, and source-
belongs-to-track checks. Queue snapshot restore creates fresh live queue IDs and
does not autoplay. Rust owns all mutations and publishes typed `playback://state`
and `queue://state` DTOs; the frontend stores presentation state only.

## Consequences

Playlists, Inbox, collection actions, queue sections, reorder/pin/remove/clear,
and named snapshots are durable and restart-safe. Source resolution remains the
only path to playback, so playlist and queue records never carry filesystem
paths or online URLs. Branches are intentionally one-shot and one level deep;
smart/rule playlists, multi-level history, three-way merges, export/import,
automatic Autoplay, and online playback remain outside Plan 08.

## Verification

Plan 08 passes the schema v4-to-v5 migration test, playlist/branch/collection
unit coverage, persistent queue/restart tests, 318 Rust tests overall, 51
Vitest tests, 48 Playwright runs, Rust/frontend quality gates, Tauri release
packaging, real mpv smoke, packaged playback/restart smoke, and the explicit
packaged playlist/collection/queue/snapshot/resume smoke.
