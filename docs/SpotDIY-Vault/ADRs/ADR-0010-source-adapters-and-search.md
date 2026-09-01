# ADR-0010: source adapters and concurrent search

- Status: Accepted
- Date: 2026-09-01
- Scope: Plan 05

## Context

SpotDIY needs provider-specific search behavior without coupling the UI to
provider payloads, subprocess output, credentials, or provider-specific
failure timing. Local results must remain useful when an online provider is
unavailable, and each request must be attributable to one SearchId.

## Decision

Use a `SourceAdapter` trait behind a `SearchService` registry. The service
starts the selected providers concurrently and emits independent typed
sections followed by exactly one completion event. The frozen lens mapping is:

| Lens | Providers |
|---|---|
| ALL, TRACKS | Local, YouTube, SoundCloud |
| ARTISTS, ALBUMS, LOCAL | Local |
| YOUTUBE | YouTube |
| SOUNDCLOUD | SoundCloud |
| SPOTIFY | Spotify only |

Every search receives a fresh SearchId. Starting a new search cancels the
previous one; cancellation, stale events, provider timeouts, malformed output,
and missing tools become typed provider sections. Timeouts are Local 2 seconds,
YouTube/SoundCloud 15 seconds, and Spotify 10 seconds. Results are sorted per
provider, then capped by the request limit. A bounded maximum-100-entry cache
uses provider, normalized query, lens, sort, direction, limit, and market in
the key; Local entries live at most 5 seconds and online entries at most 60
seconds.

The frontend mirrors the DTOs with strict Zod parsing, debounces input by 250
ms, rejects stale SearchIds, and preserves ready sections when another
provider fails. Provider results are transient DTOs and are not persisted.

## Consequences

- Provider failures are visible and isolated instead of suppressing usable
  Local results.
- Provider adapters can evolve independently behind one typed contract.
- Source Fusion, resolver policy, provider playback, downloads, and persistent
  queue behavior remain outside this boundary.
- Early native events are buffered by the frontend until the `start_search`
  response supplies the active SearchId.

## Evidence

Native SearchService tests, strict IPC tests, 38 Vitest tests, 45 Playwright
runs, five focused native smoke checks, metadata-only live smoke, and isolated
packaged smoke pass for the Plan 05 delivery.
