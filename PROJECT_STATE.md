# SpotDIY project state

State date: 2026-08-30

## Repository

- Branch: `main`
- Origin: `https://github.com/jerrylemin/SpotDIY`
- Remote state: `origin/main` is the verified delivery branch; the Plan 02 implementation commit is `2ec431b7fcbf31fbb2f2cd3b092b66ad75e81365` and is ready to be pushed with the documentation follow-up.
- Working tree: Plan 02 implementation is committed; documentation bookkeeping is the remaining local change before push.

## Runtime

- Frontend: React 19, TypeScript 6 strict, Vite 8, TanStack Router/Query, Zustand, Zod.
- Native: Tauri 2, Rust stable MSVC, typed serialized DTOs plus runtime frontend parsing.
- Current native commands: `get_app_status`, `get_source_capabilities`, `get_settings_snapshot`, and `set_setting`.
- Current persistence: SQLite `spotdiy.sqlite3` under `%LOCALAPPDATA%\SpotDIY`, initialized through migration 1 with WAL and foreign keys enabled.
- Persisted foundation: `tracks`, `artists`, `track_artists`, `albums`, `track_sources`, `local_files`, `settings_metadata`, and `schema_metadata`.
- Rust domain/repository foundation: typed UUID identifiers, `UnifiedTrack`, source capabilities, version qualifiers, focused repositories, and transactional aggregate creation.

## Decisions in force

- Keep a single Tauri application.
- Keep provider-specific logic in adapters.
- Use explicit DTOs plus Zod at the initial IPC boundary; revisit generated types after compatibility is proven.
- Generate icons from `public/spotdiy-mark.svg`; keep provider colors secondary.
- Keep Spotify catalog sources metadata-only; secure provider credentials belong in Windows Credential Manager, not SQLite.
- Keep portable mode deferred to its later startup plan; the current standard opener accepts an explicit database path and rejects an unsupported persisted portable mode.

## Next slice

Plan 03 Local Library: folder selection, recursive incremental indexing, metadata extraction, and filesystem watching. Do not implement provider search before the local library seam is established.
