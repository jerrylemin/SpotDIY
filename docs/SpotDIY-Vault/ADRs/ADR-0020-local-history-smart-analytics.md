# ADR-0020: local listening history and deterministic smart analytics

- Status: Accepted
- Date: 2026-09-03
- Plan: 14

## Context

SpotDIY needs useful history, sessions, analytics, and smart queues without
turning a local music player into a telemetry or recommendation service.
Playback already has one authoritative queue owner, and Plan 13 establishes
the local SQLite/portable storage boundary. New behavior must preserve those
boundaries, avoid sensitive filesystem/provider data, and remain deterministic
enough to test.

## Decision

Keep listening history and analytics entirely in the local SQLite database.
Record only typed track/source/session facts, local calendar fields, outcome,
qualified status, and listened milliseconds. A play qualifies after 30 seconds
of monotonic playing time, or halfway through a validated track shorter than
60 seconds. Paused time and backend/recovery gaps do not advance the clock.
Use a fixed 30-minute inactivity gap to start a new listening session.

Use in-memory Private Session and Temporary Mode state. Private activity never
writes history or session rows. Temporary Mode snapshots the durable queue,
allows transient queue mutations, excludes its activity from persistence, and
restores the durable queue without autoplay. Temporary Mode keeps Private
Session enabled for its lifetime.

Store local genres in `track_genres`; reuse validated album release dates and
never guess metadata from filenames or paths. Smart playlists are persisted
typed rule trees with bounded depth/node count. Compile only allowlisted
fields/operators into parameter-bound SQL; never accept raw SQL, paths, or
provider URLs from IPC. Smart Shuffle is a seeded, deterministic, non-ML
weighted heuristic using familiarity, variety, freshness, and discovery
signals with recent-track and recent-artist anti-repetition windows. Seeds are
not persisted.

`PlaybackService` remains the sole queue/transport owner. Analytics queries,
reopen-as-queue actions, smart CRUD/preview, and listening-mode commands cross
the native boundary only through typed DTOs and frontend validation.

## Consequences

Analytics is private-by-default and has no telemetry/network dependency.
Qualified listening is comparable across sessions, while short tracks can
still qualify without inventing a remote recommendation signal. Private and
Temporary modes have clear data-loss-safe boundaries, but Temporary Mode is
intentionally session-only. Smart results update with local library changes
and are reproducible for a supplied seed; they are not personalized by a
remote model.

## Verification

Focused source tests cover migration 8-to-9, metadata normalization,
qualification, pause/recovery behavior, session grouping, skip outcomes,
Private/Temporary exclusion, queue restoration, rule evaluation and
parameter binding, shuffle anti-repetition, analytics aggregates, and trusted
Plan 13 staging. Frontend checks pass. Native/package/browser verification is
pending the local MSVC/SDK and Chromium runtime blockers.
