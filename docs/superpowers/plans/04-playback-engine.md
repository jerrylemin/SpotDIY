# Plan 04 — playback engine

## Goal

Build a queue-aware playback service behind a backend interface, with mpv JSON IPC, safe state transitions, source switching, and recovery.

## Dependencies

Plans 01–03; mpv research report; managed-tool boundary.

## Exact files

`src-tauri/src/playback/mod.rs`, `src-tauri/src/playback/backend.rs`, `src-tauri/src/playback/mpv.rs`, `src-tauri/src/media_tools/mod.rs`, `src-tauri/src/queue/mod.rs`, `src/components/player/**`, `src/stores/player-store.ts`, and playback tests.

## Interfaces consumed

`UnifiedTrack`, `PlayableSource`, queue state, and `MediaToolManager`.

## Interfaces produced

`PlaybackService`, backend trait, playback DTOs, transport commands, output/volume commands, and source-switch event.

## Tests

Mock JSON IPC, all state transitions, next/previous/repeat/shuffle, missing-source fallback, seek clamping, crash recovery, volume, and timestamp-preserving source switch.

## Acceptance criteria

Local audio plays through mpv when installed; UI never depends directly on mpv details and reports missing-tool errors with recovery context.

## Commit boundary

`feat: add mpv playback service and queue transport`
