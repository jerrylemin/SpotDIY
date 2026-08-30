# SpotDIY session handoff

Date: 2026-08-30
Branch: `main`
Origin: `https://github.com/jerrylemin/SpotDIY`
Plan 03 delivery: this milestone; the final commit SHA and remote equality are reported in the delivery result.

## Completed

- Bootstrap architecture, workflow, Tauri/React shell, typed status contract, CI, project memory, icons, and research were completed in the earlier milestone.
- Plan 02 unified music domain and SQLite foundation remains complete.
- Plan 03 Local Library implementation is complete in the current worktree and is ready for the authorized `origin/main` delivery after the final fresh gates.
- The native core now has UUID-backed `TrackId`, `ArtistId`, `AlbumId`, `SourceId`, `LibraryFolderId`, and `ArtworkId`; `ProviderKind`; `UnifiedTrack`; `Artist`; `Album`; `TrackSource`; `LocalFileSource`; `VersionInfo`; explicit `SourceCapabilities`; and typed Library folder/page/scan DTOs.
- SQLite uses bundled rusqlite, WAL, foreign keys, a busy timeout, synchronous FULL, ordered migrations through version 2, schema metadata, and optional FTS5 probing. Standard startup stores `spotdiy.sqlite3` under `%LOCALAPPDATA%\\SpotDIY`.
- Migration 1 remains unchanged. Migration 2 adds persistent `library_folders` and managed local-file folder/path/status/artwork fields, preserves legacy Plan 02 rows, rewrites path-shaped legacy provider IDs, and validates foreign keys inside each migration transaction.
- `LibraryService` persists canonical folder roots, rejects duplicate/nested/reparse roots, scans supported audio recursively without following links, extracts Lofty metadata/quality/artwork, hashes with streaming SHA-256, reconciles missing/restored/renamed files, retains opaque identities, exposes bounded pages, and validates ownership before reveal. Artwork is cached under the app-owned `%LOCALAPPDATA%\\SpotDIY\\cache\\artwork` scope.
- Durable settings provide typed theme, downloads directory, source-preference order, first-run, and storage-mode state. Ordinary settings are separated from future secret settings; no credential is stored in SQLite or sent through current IPC.
- IPC now exposes the Plan 03 folder, scan, status, page, reveal, and `library://scan-progress` contracts in addition to the Plan 02 commands, with explicit TypeScript types and Zod validation.

## Verification

- Rust: 53 tests passed; `cargo fmt -- --check` and clippy with warnings denied passed after the final recovery/path fixes.
- Frontend: 18 Vitest tests across 4 files, typecheck, lint, and production build passed.
- Tauri: x64 release executable and NSIS installer built; packaged launch smoke passed. The synthetic native sequence covered add, initial scan, restart persistence, unchanged scan, watcher changes, same-size forced modification, rename identity, missing/restore, reveal, removal, and media preservation.
- Independent read-only review completed. Its correctness-relevant findings were validated and fixed: transient I/O versus missing state, watcher failure recovery, durable partial errors, root unavailability/recovery, and reparse-point handling.
- Graphify was updated to 1,202 nodes and 1,662 edges; derived graph files remain ignored.
- Browser harness gap: `pnpm exec playwright --version` is available, but `pnpm exec playwright test --list` returns `unknown command 'test'`; no Playwright browser project/configuration exists. Native CDP smoke was used for the packaged window.

## Known limitations

- Portable startup is intentionally deferred to its later plan. `Database::open(path)` is path-injected now, and persisted portable mode is rejected until deterministic executable-location selection exists.
- Provider adapters/search, Source Fusion, playback, downloads, lyrics, playlists, queue, import/export, visual QA, and performance work remain later plans. Library playback buttons intentionally remain disabled until Plan 04.
- FTS5 is reported as a capability; search indexing is not implemented in Plan 02.

## Exact next atomic task

Plan 04 Playback Engine: define `PlaybackService` and its adapter boundary against the now-persistent local source/availability contracts. Do not add provider search or Source Fusion in that slice.
