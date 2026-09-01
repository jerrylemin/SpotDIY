# SpotDIY session handoff

Date: 2026-09-01
Branch: `main`
Origin: `https://github.com/jerrylemin/SpotDIY`
Plan 04 feature commit: `536617d` (`feat: add mpv playback service and queue transport`)
Plan 04 review-fix commit: `af66127` (`fix: harden mpv playback lifecycle and event ordering`)
Plan 05 delivery commits: `58b5adc`, `cbcb43e`, `facab20`, `0db5eac`,
`6c16747`, `51da804`, `905f322`, `0d52f9b`, `9b2a6d8`, and `ab6169d`.

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

## Plan 05 completion

- Concurrent Local, YouTube, SoundCloud, and isolated Spotify source adapters
  are wired through `SearchService`. Unified lenses exclude Spotify; the
  Spotify lens alone can query Spotify, and only after the explicit PKCE
  development/compliance gate is enabled.
- Search uses typed request/result DTOs, SearchIds, provider-local sorting,
  bounded provider timeouts, partial section updates, exact completion,
  stale-event rejection, and a maximum-100-entry TTL cache. The browser-only
  E2E adapter remains gated by `!isTauriRuntime() && import.meta.env.DEV &&
  VITE_SPOTDIY_E2E === "1"`.
- Verification for this handoff is recorded in
  `docs/execution/verification-log.md`: 250 Rust tests, 38 Vitest tests, 45
  Playwright runs, native synthetic smoke, opt-in YouTube/SoundCloud metadata
  smoke, and isolated packaged search smoke passed. Spotify live smoke was
  skipped because no developer authorization was available.
- Plan 05 adds no migration and stores no provider credentials, tokens, raw
  tool output, or provider payloads in SQLite. The repository-local Cargo
  target remains absent.

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

## Plan 06 completion

Plan 06 is complete through implementation tip `afd0149`, with documentation
closure committed separately. The delivery adds migration 3 and durable
merge/split overrides, a deterministic conservative matcher, explicit remote
source acceptance, a settings-aware SourceResolver, resolver-backed playback,
source availability explanations, and narrow typed fusion/resolution IPC.

The final verification log records 279 Rust tests, 40 Vitest tests, 45
Playwright runs, frontend/native quality gates, the external-target Tauri
release build, real mpv smoke, packaged playback/restart/process cleanup, and
the v2-to-v3 migration smoke. Spotify remains excluded from Plan 06 fusion,
acceptance, overrides, and playback; its Plan 05 PKCE/gate boundary is
unchanged.

## Next atomic task

STOPPED AFTER PLAN 06. Awaiting external ChatGPT GitHub review before Plan 07.
