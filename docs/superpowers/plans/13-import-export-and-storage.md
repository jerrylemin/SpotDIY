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

## Delivery evidence

Implemented in `7579312`, `d287f65`, `6c2b026`, `bdf04f0`, and `5e70fdf`, with the Plan 12
shortcut repair in `3ca57a4`. No schema 9 migration was added. The final
storage resolver selects Standard or Portable before database open; the
executable-adjacent marker is authoritative and Portable has no AppData
fallback. The archive is deterministic format 1 with online SQLite snapshots,
manifest/checksum validation, trusted optional media, artwork, and exact
same-stem sidecars. Import is bounded and secure, staged without active DB
mutation, previewed, committed through a pending descriptor, applied on
restart, and recoverable with database/media rollback.

Final checks pass: 393 Rust unit tests plus real-mpv, 81 Vitest, 69 Playwright,
frontend typecheck/lint/test/build, Rust fmt/strict Clippy/all-target tests,
Tauri release/NSIS build, regular/Plan 11/Plan 12 packaged smokes, and the
isolated packaged Plan 13 Standard/Portable restart smoke. CodeGraph is current
at 176 files, 5,781 nodes, and 21,456 edges; Graphify reports 4,723 nodes,
9,470 edges, and 277 communities. Lint/build retain only the documented
pre-existing warnings. The requested Gmail completion
message `Plan 13 finished` was sent to `jerryle.minh.3@gmail.com`.
