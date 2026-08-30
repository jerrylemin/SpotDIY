# Plan 07 — download manager

## Goal

Persist and execute yt-dlp/FFmpeg download tasks with accurate provenance, progress, concurrency, retry, cancel, and restart recovery.

## Dependencies

Plans 02, 03, 05, and `MediaToolManager`; yt-dlp research.

## Exact files

`src-tauri/src/downloads/mod.rs`, `src-tauri/src/downloads/task.rs`, `src-tauri/src/downloads/progress.rs`, `src-tauri/src/media_tools/mod.rs`, `src/components/downloads/**`, `src/pages/DownloadsPage.tsx`, and download tests.

## Interfaces consumed

`SourceTrack`, destination settings, provider URLs, and media-tool health.

## Interfaces produced

`DownloadService`, persisted `DownloadTask`, state stream, concurrency settings, open-destination action, and inspector provenance.

## Tests

Mock progress parsing, queued/start/progress/complete/cancel/failure/retry/restart, sanitization, destination, metadata, concurrency, and no-fake-lossless labeling.

## Acceptance criteria

Unfinished tasks survive restart; output labels preserve source quality and all process arguments are structured.

## Commit boundary

`feat: add persistent download manager`
