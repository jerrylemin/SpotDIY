# SpotDIY Plan 04 — Playback Engine

Status: complete; implementation, review, release, smoke, and documentation gates passed

This plan is the resumable execution record for the attached Plan 04 exact
execution specification. The specification is authoritative; this file does
not introduce an alternate architecture or expand scope.

## Scope and non-goals

Plan 04 adds local playback through one external `mpv.exe` child, a transient
queue, typed Rust/Tauri contracts, functional transport controls, browser
coverage, real mpv smoke coverage, and packaged lifecycle evidence. Rust owns
the process, named pipe, JSON protocol, source-path resolution, state machine,
queue policy, recovery, and shutdown. The frontend receives snapshots and
sends opaque typed IDs only.

The plan does not add provider playback/search, Source Fusion, yt-dlp,
downloads, lyrics, playlists, persistent queue or queue snapshots, Queue
Workspace, smart shuffle, EQ, normalization, crossfade, ReplayGain, media
keys, SMTC, overlays, portable mode, analytics, or unrelated refactors.

## Build invariant

Cargo output must stay outside `src-tauri\target`. Every Rust/Tauri command in
the session must explicitly set the external target, currently
`C:\CargoTarget\SpotDIY`. No machine-specific target path is committed.

## Ordered tasks

### Task 1 — Playback contracts and media-tool discovery [x]

- Add `media_tools/mod.rs` for `SPOTDIY_MPV_PATH` then PATH discovery,
  executable validation, version parsing, refresh, and `Ready`/`Missing`/
  `Broken` health.
- Add typed playback modules and the exact product contracts: opaque
  `QueueEntryId(Uuid)`, `PlaybackPhase`, `RepeatMode`, `PlaybackErrorCode`,
  `PlaybackSnapshot`, backend commands/events, and the synchronous
  `PlaybackBackend: Send + Sync` command-enqueue seam.
- Keep queue entries ID-only and transient; do not add a migration or
  persistence table.

### Task 2 — External mpv backend and JSON IPC [x]

- Implement `MpvBackend` in `playback/mpv.rs` with one persistent child and a
  fresh random Windows named pipe per process.
- Launch exactly with `--no-config --idle=yes --terminal=no
  --input-terminal=no --audio-display=no --input-ipc-server=<fresh pipe>`.
  Do not add `--keep-open`, automatic audio-device arguments, or frontend
  process/path controls.
- Keep JSON request/reply/event structures, positive request IDs, interleaved
  correlation, bounded 1 MiB newline-delimited frames, FileLoaded waiting,
  process-exit monitoring, and bounded quit/kill/reap in `protocol.rs` and
  `mpv.rs` only.
- Poll only pause, time-pos, duration, volume, mute, and seeking observations
  at approximately 250 ms; normalize mpv events to typed backend events.

### Task 3 — Serialized PlaybackService and transient queue [x]

- Make `PlaybackService` the sole authoritative controller for snapshots,
  queue traversal, transport, source selection, and backend orchestration.
- Enforce Play Now replacement, enqueue-without-autoplay, Play Next priority,
  clear-to-idle, EOF-only advancement, repeat Off/One/All, canonical versus
  shuffled order, Fisher-Yates seeded tests/OS-seeded production, and the
  3000 ms Previous restart threshold.
- Resolve only managed, indexed, enabled, available local files through the
  library ownership boundary. Never accept a frontend filesystem path or URL.
- Implement source switching with queue/state preservation, crash/disconnect
  recovery with three attempts after 250/750/1500 ms, stale-generation event
  rejection, manual retry, and bounded shutdown.

### Task 4 — Tauri commands, event, and shutdown [x]

- Wire `AppState`, typed commands, and the `playback://state` event.
- Keep executable paths, pipe names, request IDs, raw mpv JSON, and local
  audio paths backend-only. Validate command errors and snapshots at the
  frontend boundary.
- Preserve startup and library usability when mpv is missing or broken.

### Task 5 — Typed frontend controls and local-library actions [x]

- Add strict TypeScript/Zod playback DTOs, revision filtering, native IPC
  wrappers, and the browser-only E2E adapter gated by
  `!Tauri && import.meta.env.DEV && VITE_SPOTDIY_E2E=1`.
- Make PlayerBar functional for idle/loading/playing/paused/seeking/ended,
  recovery/failure, volume/mute, repeat/shuffle, output device, progress,
  source label, and long-title/missing-artwork layouts.
- Make local rows expose Play Now, Play Next, and Add to Queue while retaining
  visible unavailable rows and honest error/retry states.

### Task 6 — Command palette transport [x]

- Connect Ctrl+K to typed Play/Pause, Next Track, Previous Track, and Clear
  Queue actions without changing existing navigation or adding provider
  commands.

### Task 7 — Playwright matrix [x]

- Pin `@playwright/test` at `1.62.1` and run the Vite browser project through
  the typed mock boundary only.
- Cover the required playback states, queue/repeat/shuffle, controls,
  devices, source labels, local-library actions, keyboard focus, stale
  revisions, long titles, missing artwork, and no console errors at 1280,
  1920, and 2560 widths.

### Task 8 — Real mpv and packaged smoke [x]

- Gate real execution with `SPOTDIY_REAL_MPV_SMOKE=1` or
  `SPOTDIY_RUN_MPV_SMOKE=1`, use generated synthetic WAV media, and verify
  load, position, pause/resume, seek, volume/mute, devices, EOF, shutdown,
  and process exit.
- Gate packaged execution with `SPOTDIY_PACKAGED_SMOKE=1`. Use an isolated
  temporary profile, index two synthetic WAV files, exercise playback and
  restart, prove the library persists while the transient queue is empty, and
  prove no SpotDIY-owned mpv child remains. The harness must never globally
  kill unrelated mpv processes or touch the normal user database.

### Task 9 — Documentation, review, graph, and delivery [x]

- Update the required root, execution, and vault documents and add one ADR:
  `ADR-0009-external-mpv-json-ipc.md`, titled “External mpv process + JSON
  IPC playback architecture”. Queue persistence remains Plan 08.
- Run the complete final verification set, refresh Graphify and CodeGraph,
  dispatch a fresh read-only whole-branch reviewer, resolve correctness
  findings, then create the exact feature and docs commits requested by the
  specification and push `origin/main` without force. Completed with review
  fix `af66127` and the documentation closure commit recorded below.

## Acceptance checks

Plan 04 acceptance checks were verified after the review-fix commit.

- `MediaToolManager` discovers and classifies mpv safely; missing/broken mpv
  does not prevent library startup.
- One persistent child, one fresh pipe, bounded frames, unique IDs,
  interleaved replies/events, typed events, generation safety, recovery,
  retry, and clean quit/kill/reap are covered.
- `PlaybackService` owns all listed state, queue, source, device, recovery,
  EOF, and shutdown semantics; only managed local sources can play.
- Tauri and frontend contracts are typed, strict, revision-aware, and path
  free; UI actions are functional and accessible.
- Playwright, real mpv, packaged lifecycle, full quality gates, independent
  review, docs, graph refresh, clean Git state, and remote SHA equality are
  recorded with exact evidence.

## Verification commands

Use `$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY';` before every Rust or
Tauri command. The final record must include exact outcomes for:

```text
git diff --check
pnpm typecheck
pnpm lint
pnpm test
pnpm build
pnpm exec playwright test
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
pnpm tauri build
SPOTDIY_REAL_MPV_SMOKE=1 cargo test --manifest-path src-tauri/Cargo.toml --test mpv_smoke -- --nocapture
SPOTDIY_PACKAGED_SMOKE=1 powershell -NoProfile -ExecutionPolicy Bypass -File scripts/packaged-playback-smoke.ps1
graphify update .
codegraph sync .
codegraph status .
```

## Commit boundaries

- Feature: `feat: add mpv playback service and queue transport`
- Review fix: `fix: harden mpv playback lifecycle and event ordering`
- Documentation closure: `docs: close Plan 04 playback delivery`

Do not commit or push until the review and final gates pass. Never force-push
or reset away unrelated work.
