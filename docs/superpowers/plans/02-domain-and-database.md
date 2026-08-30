# Plan 02 — domain and database

## Goal

Freeze the unified music model and create SQLite WAL migrations, repository primitives, durable settings, and typed status queries.

## Dependencies

Plan 01; research reports for local-first storage and typed IPC.

## Exact files

`src-tauri/src/domain/mod.rs`, `src-tauri/src/db/mod.rs`, `src-tauri/src/db/repository.rs`, `src-tauri/src/settings/mod.rs`, `src-tauri/src/ipc/mod.rs`, `src-tauri/migrations/0001_initial.sql`, `src-tauri/src/lib.rs`, `src/types/domain.ts`, `src/services/ipc.ts`, and `tests/domain.test.ts`.

## Interfaces consumed

`ProviderKind`, `SourceCapabilities`, and the existing Tauri command registration.

## Interfaces produced

Typed IDs, `UnifiedTrack`, `TrackSource`, `LocalFileSource`, settings repository, migration runner, WAL health, and status DTOs.

## Tests

Clean migration, reopen/WAL, repository CRUD, FTS availability, settings round trip, and migration backup tests.

## Acceptance criteria

A temporary SQLite database migrates transactionally, reopens with WAL enabled, and exposes no secure credential fields.

## Commit boundary

`feat: add unified music domain and sqlite foundation`
