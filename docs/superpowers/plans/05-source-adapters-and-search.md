# Plan 05 — source adapters and search

## Goal

Implement common provider adapters for Local, YouTube, SoundCloud, and Spotify catalog metadata plus independent concurrent search with partial results.

## Dependencies

Plans 01–04; provider research reports; secure credential storage for Spotify.

## Exact files

`src-tauri/src/sources/mod.rs`, `src-tauri/src/sources/traits.rs`, `src-tauri/src/sources/local.rs`, `src-tauri/src/sources/youtube.rs`, `src-tauri/src/sources/soundcloud.rs`, `src-tauri/src/sources/spotify.rs`, `src-tauri/src/search/mod.rs`, `src/pages/SearchPage.tsx`, `src/components/search/**`, and provider contract tests.

## Interfaces consumed

`ProviderKind`, capabilities, `SearchQuery`, `MediaToolManager`, and keyring settings.

## Interfaces produced

`SourceAdapter`, `SearchService`, normalized `SearchPage`, provider errors, cancellation, timeout, and rate-limit mapping.

## Tests

Mock parser/HTTP responses, empty query, cancellation, malformed payload, missing metric/artwork, timeout, rate limit, and provider identifiers. No live CI calls.

## Acceptance criteria

Each provider renders an independent status/section; Spotify setup is visible and Spotify audio is never represented as downloadable.

## Commit boundary

`feat: add multi-source search adapters`
