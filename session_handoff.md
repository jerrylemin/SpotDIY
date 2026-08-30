# SpotDIY session handoff

Date: 2026-08-30
Branch: `main`
Origin: `https://github.com/jerrylemin/SpotDIY`

## Completed

- Bootstrap architecture, workflow, Tauri/React shell, typed status contract, CI, project memory, icons, and research were completed in the earlier milestone.
- Plan 02 unified music domain and SQLite foundation is complete in the current worktree.
- The native core now has UUID-backed `TrackId`, `ArtistId`, `AlbumId`, and `SourceId`; `ProviderKind`; `UnifiedTrack`; `Artist`; `Album`; `TrackSource`; `LocalFileSource`; `VersionInfo`; and explicit `SourceCapabilities`.
- SQLite uses bundled rusqlite, WAL, foreign keys, a busy timeout, synchronous FULL, ordered migration 1, schema metadata, and optional FTS5 probing. Standard startup stores `spotdiy.sqlite3` under `%LOCALAPPDATA%\\SpotDIY`.
- Migration 1 contains `tracks`, `artists`, `track_artists`, `albums`, `track_sources`, `local_files`, `settings_metadata`, and `schema_metadata`. Focused repositories provide aggregate track persistence, artist/source reads, provider identity uniqueness, local metadata, and transaction rollback.
- Durable settings provide typed theme, downloads directory, source-preference order, first-run, and storage-mode state. Ordinary settings are separated from future secret settings; no credential is stored in SQLite or sent through current IPC.
- IPC now exposes `get_app_status`, `get_source_capabilities`, `get_settings_snapshot`, and `set_setting`, with explicit TypeScript types and Zod validation.

## Verification

- Rust: 25 tests passed; fmt check and clippy with warnings denied passed.
- Frontend: typecheck, lint, 5 Vitest tests, and production build passed.
- Tauri: x64 release executable and NSIS installer built; packaged launch smoke passed and initialized the standard LocalAppData database.
- Independent read-only review PASS: no unresolved critical, high, or medium findings after the final source-move guard and current frontend tests.

## Known limitations

- Portable startup is intentionally deferred to its later plan. `Database::open(path)` is path-injected now, and persisted portable mode is rejected until deterministic executable-location selection exists.
- Provider adapters/search, local library indexing, Source Fusion, playback, downloads, lyrics, playlists, queue, import/export, visual QA, and performance work remain later plans.
- FTS5 is reported as a capability; search indexing is not implemented in Plan 02.

## Exact next atomic task

Plan 03 Local Library: folder selection, recursive incremental indexing, metadata extraction, local source persistence, and filesystem watching. Do not start provider search or playback before the local library seam is established.
