# Plan 12 — overlays and Windows integration

## Goal

Add always-on-top, mini/edge/lyrics/gaming overlays, global shortcuts, tray, media controls, output profiles, and click-through only where reliable.

## Dependencies

Plans 04, 08, 11; Tauri Windows research; WebView2 and Win32 validation.

## Exact files

`src-tauri/src/windows/**`, `src-tauri/src/playback/output.rs`, `src/components/overlay/**`, `src/stores/overlay-store.ts`, `src-tauri/capabilities/**`, and Windows integration tests/smoke scripts.

## Interfaces consumed

Playback/queue events, Tauri window APIs, global shortcut capability, and Windows media control API.

## Interfaces produced

Overlay lifecycle commands, shortcut registry, tray menu, output profile DTOs, and capability/error reporting.

## Tests

Window create/close/reopen, always-on-top, shortcut conflict, tray action, media control mapping, output device change, and click-through fallback.

## Acceptance criteria

Overlays remain optional and recoverable; unsupported Windows behavior is explained rather than claimed.

## Commit boundary

`feat: add Windows overlays and integration surfaces`
