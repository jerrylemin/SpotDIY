# SpotDIY project state

State date: 2026-08-30

## Repository

- Branch: `main`
- Origin: `https://github.com/jerrylemin/SpotDIY`
- Remote state: the Plan 03 delivery is authorized and tracked in the final verification/delivery record; local `main` contains the complete milestone.
- Working tree: Plan 03 implementation, tests, and documentation are the milestone changes; unrelated files remain untouched.

## Runtime

- Frontend: React 19, TypeScript 6 strict, Vite 8, TanStack Router/Query, Zustand, Zod.
- Native: Tauri 2, Rust stable MSVC, typed serialized DTOs plus runtime frontend parsing.
- Current native commands: `get_app_status`, `get_source_capabilities`, `get_settings_snapshot`, `set_setting`, `get_library_folders`, `add_library_folders`, `remove_library_folder`, `get_library_status`, `rescan_library_folder`, `rescan_all_library_folders`, `get_library_page`, and `reveal_local_file`.
- Current persistence: SQLite `spotdiy.sqlite3` under `%LOCALAPPDATA%\SpotDIY`, initialized through migration 2 with WAL, foreign keys, and migration-time integrity checks enabled.
- Persisted foundation: `tracks`, `artists`, `track_artists`, `albums`, `track_sources`, `local_files`, `library_folders`, `settings_metadata`, and `schema_metadata`.
- Rust domain/repository foundation: typed UUID identifiers, `UnifiedTrack`, source capabilities, version qualifiers, focused repositories, and transactional aggregate creation.
- Local library: `LibraryService` owns persistent folder roots, recursive WalkDir scans, Lofty metadata, streaming SHA-256 fingerprints, content-addressed artwork, Notify watchers, missing/restore/rename reconciliation, and bounded page reads. Plan 02 legacy local rows remain preserved but are excluded until a matching discovered path promotes them into managed ownership.

## Decisions in force

- Keep a single Tauri application.
- Keep provider-specific logic in adapters.
- Use explicit DTOs plus Zod at the initial IPC boundary; revisit generated types after compatibility is proven.
- Generate icons from `public/spotdiy-mark.svg`; keep provider colors secondary.
- Keep Spotify catalog sources metadata-only; secure provider credentials belong in Windows Credential Manager, not SQLite.
- Keep portable mode deferred to its later startup plan; the current standard opener accepts an explicit database path and rejects an unsupported persisted portable mode.
- Keep local source identity opaque and stable across rename, restoration, and same-path replacement; path ownership is validated independently before reveal.
- Keep one Notify watcher per enabled root, debounce event bursts, force scans for create/modify/rename, and use full reconciliation for uncertain events or watcher recovery.
- Keep playback disabled in the Plan 03 Library UI; Plan 04 owns the playback command and service boundary.

## Plan 03 verification snapshot

- Rust: 53 tests, formatting, clippy with warnings denied, and all-target tests pass.
- Frontend: 18 Vitest tests across 4 files, typecheck, lint, and production build pass.
- Native: x64 Tauri release build, packaged launch, restart persistence, incremental scan, forced same-size modification, watcher create/rename/delete/restore, reveal validation, folder removal, and media-preservation smoke pass.
- Browser harness: `pnpm exec playwright --version` is available, but `pnpm exec playwright test --list` returns `unknown command 'test'`; no Playwright test project/configuration is present. Native CDP smoke covered the real packaged window instead.

## Next slice

Plan 04 Playback Engine: introduce the playback service/adapter boundary only after the local library source, availability, and reveal contracts remain stable. Provider search and Source Fusion stay later.
