# ADR-0015: Local-first lyrics and playback-owned A/B loop

- Status: Accepted
- Date: 2026-09-02

## Context

SpotDIY needs synchronized lyrics and lightweight playback annotations without
turning local media into a mutation surface or allowing provider data to enter
the playback process. The existing `LibraryService` owns managed local paths,
and `PlaybackService` is the sole serialized transport and queue owner.

## Decision

- Resolve lyrics in this order: manual override, exact local `.lrc` sidecar,
  embedded timed text, embedded plain text, then cached LRCLIB.
- Keep local metadata and sidecar reads read-only, bounded, and behind the
  managed `LibraryService` path boundary. Manual file import uses the native
  picker rather than accepting an arbitrary frontend path.
- Make LRCLIB an explicit user action only. Use an HTTPS-only, bounded,
  rate-gated, metadata-safe provider boundary and retain only validated cache
  records; do not persist raw provider responses or credentials.
- Persist lyrics records, bookmarks, and normalized A/B presets in SQLite
  schema 6 with typed IDs and validation. Keep notes and loop bounds bounded.
- Keep active A/B state in `PlaybackService` and send typed set/clear commands
  to mpv. Clear A/B for a new track, restore it for same-track source switching
  and backend recovery, and never autoplay a preset.

## Consequences

Local evidence is deterministic and available without network access. Provider
lookup is discoverable and attributable but cannot silently run in the
background. The frontend can display synchronized cues and controls without
seeing paths, URLs, raw provider payloads, or mpv protocol details. Waveform
generation remains a separate future boundary.

## Verification

Plan 09 passes 337 Rust unit tests plus one synthetic mpv integration test, 56
Vitest tests, 48 Playwright runs, strict frontend/native gates, real-mpv and
packaged persistence smokes, and the named v5-to-v6 migration smoke. Live
LRCLIB smoke was optional and skipped.
