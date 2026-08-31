# Plan 04 independent final review

Reviewer: Confucius (`01a056a9-da6d-7e01-932e-260fcb8771fe`)
Range: `e01784e36cbd8d7e3ef08cc3b4f4a7b598682a23..536617d3d58222f6253582b9def6903fb4848224`
Result: FAIL; fix wave required before documentation closure.

## High

- `src-tauri/src/playback/backend.rs:73-103`, `src-tauri/src/playback/mod.rs:529-574,1087-1118`: the required enqueue-only backend/session architecture is not used; the controller directly calls blocking methods and has a 5-second outer shutdown timeout instead of the required 3 seconds.
- `src-tauri/src/playback/backend.rs:28-43`, `src-tauri/src/playback/mod.rs:493-508,924-1026`, `src-tauri/src/playback/mpv.rs:133-155`: generation IDs are removed before events reach the controller, so stale events cannot be rejected at the state-machine boundary.
- `src-tauri/src/playback/mpv.rs:1518-1588`: malformed/oversized frames become generic disconnects, protocol details are discarded, and unknown reply IDs are silently ignored.

## Medium

- `src-tauri/src/playback/mpv.rs:755,1435`: session event channel is unbounded.
- `src-tauri/src/playback/mod.rs:1220-1303`: source-switch rollback through a dead backend does not enter the normal recovery path.
- `src-tauri/src/playback/mod.rs:1316-1322`, `src-tauri/src/playback/types.rs:34-68`: pending-load expiry exposes an internal `backendTimeout` code instead of the specified external contract.
- `src-tauri/src/media_tools/mod.rs:244-257`: mpv version discovery is unbounded and does not use `--no-config`.
- `src/hooks/usePlayback.ts:54-66`, `src/services/ipc.ts:1657-1667`: initial snapshot subscription race and silent invalid-event parse failure.

## Low

- `src/services/ipc.ts:1554-1557`: browser source-switch fixture replaces the whole queue rather than preserving it.
- `tests/playwright/playback.spec.ts:12-117`, `src-tauri/tests/mpv_smoke.rs:12-106`: missing source-switch rollback, stale generation, simultaneous Next/EOF, malformed frame, shutdown-during-load, and real service-path coverage.
- `scripts/packaged-playback-smoke.mjs:60`: lint failure in the reviewed commit; the controller has since made an uncommitted cause-preserving fix, which must be retained in the reviewed fix range.

## Strengths noted by reviewer

Exact startup arguments and fresh UUID pipes, shell-free process spawning,
force-kill/reap cleanup paths, restrictive managed local-path validation,
ID-only frontend playback requests, and substantial queue/native DTO tests.

## Remediation recheck

Reviewer: Mendel (`01a05761-c31f-7f51-88d5-52902c7ac673`)
Range: current `main` after `af66127`
Result: PASS

- CRITICAL: None.
- HIGH: None.
- MEDIUM: None.
- LOW: The probe implementation has no direct regression test for hostile
  oversized output or output-read timeout; bounded implementation and the
  final gates are otherwise green.
