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

## Delivered evidence

Plan 07 is complete through three implementation commits:

- `0dbb628` - `feat: add persistent download task model`
- `22438a0` - `feat: add managed media download execution`
- `6012921` - `feat: add download manager interface`

Schema 4, the persistent repository/service, bounded yt-dlp and FFmpeg
execution, state/event IPC, Downloads UI, search actions, tool status, and
focused lifecycle/security tests are delivered. Final verification records
308 Rust unit tests plus synthetic mpv, 47 Vitest tests, 45 Playwright runs,
strict quality gates, Tauri packaging, real-mpv smoke, packaged playback smoke,
and five native provider-search smoke checks. Live provider/download smoke is
opt-in and was not run; the optional packaged provider-search harness retains
an immediate start/cancel race when yt-dlp is intentionally missing.

The documentation closure commit is `docs: close Plan 07 download manager
delivery`. Plan 08 is not started.
