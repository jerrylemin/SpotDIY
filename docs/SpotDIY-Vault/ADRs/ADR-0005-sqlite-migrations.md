# ADR-0005: SQLite migrations and WAL initialization

- Status: Accepted
- Date: 2026-08-30
- Scope: Plan 02 persistence foundation

## Context

Later library, provider, fusion, queue, and import/export work needs a durable local store with predictable startup behavior. The approved design selects SQLite with WAL and explicit migrations, while Plan 02 must avoid pulling future feature tables forward.

## Decision

SpotDIY uses `rusqlite` with the bundled SQLite build. `Database::open(path)` receives an explicit path so the application chooses standard or a future portable location without changing repositories. Startup creates the parent directory, sets a five-second busy timeout, enables and verifies foreign keys, enables and verifies WAL, and uses `synchronous = FULL`.

Migrations are embedded SQL records with strictly increasing versions. The complete migration list is validated before any migration runs; each pending migration executes in an immediate transaction and advances SQLite `user_version` only after its SQL succeeds. A foreign-key check runs after migration. Destructive migrations must checkpoint WAL with `TRUNCATE`, inspect the returned busy status, and copy the main database only when the checkpoint completed. FTS5 is probed as an optional capability rather than making the initial schema depend on it.

The initial migration contains only the Plan 02 foundation: `tracks`, `artists`, `track_artists`, `albums`, `track_sources`, `local_files`, `settings_metadata`, and `schema_metadata`, plus focused indexes and integrity constraints.

## Consequences

Repositories operate against a small, visible SQL schema and remain independent of future table additions. Migration failures roll back the active migration, invalid ordering fails before partial application, and WAL backups do not silently omit frames blocked by readers. Future schema changes must be added as ordered migrations.
