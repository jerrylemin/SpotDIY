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

## Plan 03 migration 2

Migration 2 leaves migration 1 unchanged and adds `library_folders` with an
opaque folder ID, canonical display path, case-insensitive normalized path key,
enabled flag, scan status/generation, scan timestamps/errors, and aggregate
counts derived from managed local files. It extends `local_files` with nullable
legacy-compatible folder/path, normalized-key, container, index-status,
status-detail, last-seen/indexed timestamps and generation, and artwork cache
references. Folder, generation, page-order, content-fingerprint, and normalized
path indexes support bounded scans and reads; fingerprints are deliberately not
unique-constrained.

Migration 2 rewrites path-shaped Plan 02 local provider item IDs to stable
`legacy-local-*` values. Legacy rows remain intact and outside current managed
folder/status/page counts. If a selected folder later discovers the same path,
the scanner promotes that row into folder ownership instead of violating the
legacy global path uniqueness constraint or creating a duplicate identity.

Library writes keep file I/O outside SQLite transactions and write each track,
artists, album, source, and local-file aggregate atomically. Confirmed missing
files update source availability without deleting metadata; removing a managed
folder deletes only its local source/index rows and safe orphan tracks, never
the user media path. Migration foreign-key checks run inside the migration
transaction and again after startup.

Future logical tables such as playlists, queue, downloads, lyrics, history, caches, and overrides belong in later migrations. Secrets never belong in SQLite. See [ADR-0005](ADRs/ADR-0005-sqlite-migrations.md) and [ADR-0008](ADRs/ADR-0008-local-library-identity-and-reconciliation.md).

## Plan 11 migration 7 compatibility repair

Plan 10 introduced `layout_profile` and `custom_theme` as ordinary settings,
but shipped schema-6 databases still have migration 1's narrower
`settings_metadata` CHECK constraints. Plan 11 restores the historical
`0001_initial.sql` allowlist and adds `0007_appearance_settings.sql` as a
destructive, WAL-safe migration. It creates a replacement
`settings_metadata_v7`, copies every existing row, drops the old table, renames
the replacement, and updates `schema_metadata` to version 7. No other table is
changed and no setting value is rewritten or discarded.

The migration suite explicitly creates an independent old-constraint schema-6
fixture, a schema-6 database with the already-shipped Plan 10-shaped settings
table, and a fresh database. All reach schema 7; old settings survive, the two
appearance keys can be written, a custom theme can become active, and
`foreign_key_check` remains clean. `LATEST_SCHEMA_VERSION` is 7 and there are
no additional Plan 11 database changes.
