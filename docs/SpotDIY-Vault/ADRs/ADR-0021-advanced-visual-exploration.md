# ADR-0021: Advanced visual exploration boundaries

- Status: Accepted
- Date: 2026-09-03

## Context

Plan 15 needs visual exploration, local audio preview, and theme authoring
without creating a second library model, leaking filesystem/provider details,
or letting presentation features own playback and persistence.

## Decision

1. Keep SQLite at schema 9 and expose visual data through one native,
   read-only, parameter-bound `VisualExplorerService` query. Cap requests at
   5,000 tracks (default 2,000), return deterministic ordering and aggregates,
   and expose only trusted artwork-cache references; never return local media
   paths, raw provider URLs, credentials, or network results.
2. Build Music Map as a deterministic bounded SVG relationship graph and
   Library Galaxy as a deterministic bounded 2D Canvas layout. Both routes use
   real local data, pan/zoom/reset/filter/selection, and a 200-item DOM
   navigator. Rendering does not depend on a continuous animation loop.
3. Reuse the existing capability/action policy and `ContextActionMenu` for
   Play Now, Play Next, Queue, Inbox, Inspect, Lyrics, Reveal Local, and
   Preview. Cap radial menus at eight visible entries with a linear More
   fallback; provide keyboard alternatives for drag targets and ensure
   canceled/invalid drops do not mutate queue state.
4. Keep local preview in a separate `PreviewService`. Resolve by TrackId
   through the managed local library, interlock with active main playback, cap
   samples at eight seconds and 35% volume, own cancellation/shutdown, and
   forbid queue/history/analytics/SMTC/provider side effects. Use an injectable
   backend seam so tests do not sleep for the runtime duration.
5. Make Theme Studio draft-first with schema v1 and exactly 15 tokens. Preview
   changes in the current session, persist only on Save & Activate, validate
   import/export, and reuse existing layout profiles. Keep Dynamic Accent
   session-only, sample at most 32x32 client-side artwork pixels, enforce
   contrast, and fall back to the existing accent pair.

## Consequences

The visual surfaces stay bounded and keyboard-usable while sharing existing
domain, capability, library, playback, and theme ownership. Large collections
may require filters because the DOM navigator is intentionally capped at 200
items and the visual dataset at 5,000 tracks. Richer preview processing,
network artwork, ML clustering, and new persistence are explicitly deferred.

## Verification

The implementation passes the native/frontend suites, 70 Playwright tests,
external Tauri release/NSIS packaging, and the packaged Plan 15 visual/restart
smoke. Schema 9 remains current and no migration 10 was introduced.
