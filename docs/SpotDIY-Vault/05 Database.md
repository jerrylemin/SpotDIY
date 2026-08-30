# Database

## Plan 02 foundation

SQLite is the runtime source of truth for local data. `Database::open(path)` creates the parent directory, sets a five-second busy timeout, enables and verifies foreign keys, enables and verifies WAL, applies `synchronous = FULL`, runs ordered embedded migrations, checks foreign-key integrity, and probes FTS5 availability. The standard Tauri startup path resolves `%LOCALAPPDATA%\\SpotDIY\\spotdiy.sqlite3` through `app.path().local_data_dir()`; tests use isolated temporary files.

Migration 1 introduces only the durable foundation required by Plan 02:

- `tracks`
- `artists`
- `track_artists`
- `albums`
- `track_sources`
- `local_files`
- `settings_metadata`
- `schema_metadata`

Known access paths have indexes for normalized titles, updated tracks, provider identity, track sources, and artist relationships. Provider identity is globally unique by exact `(provider_kind, provider_item_id)`; local paths are unique. Preferred-source triggers prevent cross-track references and source moves that would invalidate a preferred source. Spotify audio and lyric capabilities are schema-rejected.

Each pending migration runs in an immediate transaction and advances `user_version` only after success. The full migration list is validated before application. Destructive migrations checkpoint WAL and inspect the busy result before copying the main file, so an active reader cannot produce an incomplete backup. FTS5 is optional and does not alter the initial migration contract.

Future logical tables such as playlists, queue, downloads, lyrics, history, caches, and overrides belong in later migrations. Secrets never belong in SQLite. See [ADR-0005](ADRs/ADR-0005-sqlite-migrations.md).
