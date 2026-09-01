# Plan 09 — lyrics

## Goal

Load embedded/sidecar lyrics first, add optional LRCLIB lookup, parse timed lines, synchronize playback, and support bookmarks/A-B loop.

## Dependencies

Plans 02–04; lyrics research report and playback timestamps.

## Exact files

`src-tauri/src/lyrics/mod.rs`, `src-tauri/src/lyrics/parser.rs`, `src-tauri/src/lyrics/providers.rs`, `src-tauri/src/bookmarks/mod.rs`, `src/components/lyrics/**`, `src/pages/LyricsPage.tsx`, and tests.

## Interfaces consumed

`UnifiedTrack`, local file metadata, playback position, and lyrics cache repository.

## Interfaces produced

`LyricsService`, `LyricsDocument`, timed-line DTOs, synced display state, bookmarks, and A/B loop commands.

## Tests

LRC timestamp variants, embedded tags, malformed lines, provider failures, cache behavior, line synchronization, bookmark persistence, and safe loop bounds.

## Acceptance criteria

No-lyrics states provide import/search/edit actions; timed lyrics follow playback and copyright/provider attribution remains explicit.

## Commit boundary

Implementation boundary: `1bc7108` (`feat: add local-first lyrics service`),
`e4d62d8` (`feat: add optional LRCLIB lyrics lookup`), `c25f954` (`feat: add
playback bookmarks and ab loop`), and `7b1a097` (`feat: add synchronized
lyrics interface`). Final evidence is recorded in
`docs/execution/verification-log.md`; waveform generation and live LRCLIB
smoke remain outside this delivery.
