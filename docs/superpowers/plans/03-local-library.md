# Plan 03 — local library

## Goal

Implement selected-folder management, recursive incremental scan, metadata/artwork/quality extraction, fingerprinting, watcher updates, and library queries.

## Dependencies

Plans 01–02; approved local storage ADR.

## Exact files

`src-tauri/src/library/mod.rs`, `src-tauri/src/library/scanner.rs`, `src-tauri/src/library/metadata.rs`, `src-tauri/src/library/watcher.rs`, `src-tauri/src/db/repository.rs`, `src-tauri/src/ipc/mod.rs`, `src/components/library/**`, `src/pages/LibraryPage.tsx`, and library tests.

## Interfaces consumed

`UnifiedTrack`, `LocalFileSource`, settings, and database repositories.

## Interfaces produced

`LibraryService`, folder commands, paged library query, rescan status, open-location action, and quality/provenance DTOs.

## Tests

Synthetic audio fixtures, unchanged-file skip, add/remove/rename, tags/artwork, invalid format, path validation, and watcher debounce tests.

## Acceptance criteria

A user can choose multiple folders, scan recursively, reopen the app with indexed metadata, and see accurate empty/indexed states without duplicate rescans.

## Commit boundary

`feat: add incremental local library indexing`
