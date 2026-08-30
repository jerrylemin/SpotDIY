# Plan 13 — import/export and storage

## Goal

Implement deterministic standard/portable storage, `.spotdiy` export/import, checksum validation, rollback backups, and missing-file reporting.

## Dependencies

Plans 02, 03, 08, and 09; local-first storage ADR.

## Exact files

`src-tauri/src/backup/**`, `src-tauri/src/settings/**`, `src-tauri/src/app/storage.rs`, `src/pages/SettingsPage.tsx`, `src/components/backup/**`, and backup/storage tests.

## Interfaces consumed

Database snapshot, settings excluding secure secrets, file references, themes/layouts, lyrics, and caches.

## Interfaces produced

Manifest/schema/checksum format, export options, transactional import result, rollback path, and portable-mode resolver.

## Tests

Round-trip archive, checksum failure, unsupported version, traversal rejection, import transaction rollback, missing files, portable path resolution, and secure-secret exclusion.

## Acceptance criteria

Failed imports cannot damage the active library and portable mode never silently writes the main database to AppData.

## Commit boundary

`feat: add local backup restore and portable storage`
