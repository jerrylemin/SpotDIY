# SpotDIY session handoff

Date: 2026-08-31
Branch: `main`
Origin: `https://github.com/jerrylemin/SpotDIY`
Plan 04 feature commit: `536617d` (`feat: add mpv playback service and queue transport`)
Plan 04 review-fix commit: `af66127` (`fix: harden mpv playback lifecycle and event ordering`)

## Completed

- Plans 01–03 remain complete: the Tauri/React shell, typed domain, SQLite
  migrations through schema version 2, durable settings, and managed local
  library are implemented.
- Plan 04 adds `MediaToolManager`, the external Windows mpv backend, bounded
  newline-delimited JSON IPC, typed backend events, the serialized
  `PlaybackService`, transient ID-only queue, local source resolution,
  transport/repeat/shuffle/EOF/previous policy, source switching, recovery,
  retry, and shutdown cleanup.
- Typed Tauri commands and `playback://state` feed strict Zod-validated
  frontend snapshots. PlayerBar, local-library Play Now/Play Next/Add to Queue,
  and Ctrl+K transport actions are functional. The browser adapter is enabled
  only for development E2E runs with `VITE_SPOTDIY_E2E=1` outside Tauri.
- `playwright.config.ts` runs the playback matrix at 1280, 1920, and 2560
  viewport projects. Real synthetic WAV and packaged lifecycle smoke scripts
  are present and do not commit mpv or media artifacts.

## Verification

- Rust all-target tests (117), formatting, clippy with warnings denied,
  frontend typecheck/lint/Vitest (26 tests)/build, and Playwright (9 runs)
  pass locally.
- Real mpv smoke passes load, FileLoaded, position, pause/resume, seek,
  volume/mute, device enumeration, EOF, shutdown, and process exit using the
  local `.tools\mpv\v0.41.0\mpv.exe` development binary.
- Packaged release smoke passes synthetic indexing, Play/Pause/Seek/Resume,
  queue/Next, graceful close, owned-mpv cleanup, restart library persistence,
  and an empty transient queue. The isolated smoke profile and owned process
  are absent after the run; the normal database contains zero harness rows.
- Cargo/Tauri output uses `C:\CargoTarget\SpotDIY`; `src-tauri\target` is
  absent. The release executable and NSIS bundle were built successfully.
- The single fresh read-only reviewer rechecked the fixes with `PASS`; no
  critical, high, or correctness/security medium findings remain. The
  documentation closure is the final Plan 04 record before remote SHA
  verification.

## Known limitations

- The queue is intentionally transient. Persistent queue state and queue
  snapshots belong to Plan 08.
- Provider adapters/search, Source Fusion, downloads, lyrics, playlists,
  overlays, global media keys/SMTC, portable mode, analytics, EQ,
  normalization, crossfade, ReplayGain, and later visual/performance work are
  out of Plan 04 scope.
- The packaged harness uses the smoke-only `SPOTDIY_PACKAGED_DATA_ROOT` seam
  because Windows known-folder resolution ignores a child `LOCALAPPDATA`
  override; standard startup remains `%LOCALAPPDATA%\SpotDIY`.

## Next atomic task

Plan 05 — Source Adapters and Search. Preserve the local playback boundary and
do not begin Source Fusion until its own specification is active.
