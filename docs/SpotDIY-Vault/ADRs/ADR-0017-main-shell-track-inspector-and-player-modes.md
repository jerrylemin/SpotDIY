# ADR-0017: Main shell, Track Inspector, and in-shell player modes

- Status: Accepted
- Date: 2026-09-02
- Scope: Plan 11 Main Shell and Player

## Context

Plan 11 needs richer Home, Search, Library, Now Playing, and Track Inspector
surfaces without creating a second owner for playback, queue, search,
downloads, lyrics, or collections. Persisted local tracks and ephemeral online
search results also have different privacy and action rules. Appearance keys
introduced by Plan 10 must work for databases already shipped at schema 6.

## Decision

`AppShell` is the presentation composition point. Its session-only Zustand state
contains the player mode (`standard`, `mini`, or `expanded`) and inspector
selection (`closed`, persisted track, or ephemeral search result). It owns the
Escape priority order: command palette, inspector, queue drawer, then expanded
player. TanStack Query remains the owner of inspector and service snapshot
query state.

`TrackInspectorService` exposes a narrow read-only `get_track_inspector` DTO,
assembled from existing track and collection repositories. The DTO exposes
metadata, collection state, source availability/capabilities, measured local
quality, version qualifiers, and validated provider URLs. Local filesystem
paths are never included; local reveal continues through the existing
source-ID command. A SearchResult inspector stays ephemeral and cannot persist,
fuse, or play an online result.

`PlayerBar`, `MiniPlayer`, and `NowPlayingPanel` consume the same
`usePlayback()` snapshot. Mode changes are presentation-only and do not
autoplay, seek, switch sources, or mutate the queue. `SourceSwitcher` delegates
to existing playback source-switch commands and keeps unavailable sources
visible with their explanation. Expanded Now Playing reuses the inspector
query for compact quality/provenance facts.

`track-actions.ts` is the pure frontend policy boundary for provider
capabilities and runtime availability. Local persisted tracks expose existing
play/queue/inspect/reveal actions; YouTube and SoundCloud expose only legal
open/download actions when available; Spotify remains metadata-only. Disabled
actions retain a visible reason. Existing service boundaries remain
authoritative.

Migration 7 is the only Plan 11 database change. It rebuilds only
`settings_metadata`, copies every existing row, adds the Plan 10 appearance
allowlist, and updates schema metadata to 7. An independent old-constraint
schema-6 fixture and a Plan-10-shaped schema-6 fixture verify value
preservation, new-key writes, custom-theme activation, and clean foreign-key
checks.

## Consequences

The shell can present real state consistently across routes and player modes,
while action availability and privacy remain explicit. The architecture keeps
one playback/queue owner and does not introduce online playback, provider
behavior, a native mini window, OS overlays, or a second persistence setting.
The packaged Plan 11 smoke proves migration, settings, inspector privacy,
player modes, queue/Lyrics/palette navigation, restart persistence, and
no-autoplay behavior.
