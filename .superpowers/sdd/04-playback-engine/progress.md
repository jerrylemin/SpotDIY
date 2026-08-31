# SDD ledger — plan: docs/superpowers/plans/04-playback-engine.md

## Setup

- Starting commit: `facf0cd1774424aa2a4eb78f59a4b3f5400945f6` (`main`), matching `origin/main`.
- Working tree was clean before Plan 04 work. The user-authored brief explicitly authorizes the Plan 04 commits and push to `origin/main`; no provider or Plan 05 work is in scope.
- Spec read: `docs/superpowers/specs/2026-08-30-spotdiy-design.md` (sections 2, 6, 7, 9, 10, 11).
- Research read: `docs/SpotDIY-Vault/Research/mpv-json-ipc.md`; official mpv release verification completed for `v0.41.0`, Windows x64 asset SHA-256 `4e197f729f5071c6772f35fffd96e0f36e3e8a044bd9479b136bb09b7c6a80ff`.
- Read-only repository mapping and mpv/Tokio/lifecycle research were completed by three explorer agents; no explorer edits were integrated.

## Plan scan and rulings

| Scope | Producer / consumer | Scan result | Ruling |
|---|---|---|---|
| Task 1 ↔ Task 2 | Task 1 defines `MediaToolManager`, `PlaybackBackend`, and shared DTOs; Task 2 consumes them and modifies Cargo dependencies | Compatible; Task 2 must not redefine shared contracts | Task 1 owns public contracts; Task 2 owns only the mpv adapter and platform session. |
| Task 1 ↔ Task 3 | Task 1 creates the transient queue; Task 3 extends queue traversal and consumes backend/manager contracts | Compatible if queue remains ID-only and non-persistent | Task 3 may add private queue helpers but no migration, persistence, or path fields. |
| Task 1 ↔ Task 4 | Task 1 exposes tool health; Task 4 places it behind application state | Compatible; health detail must remain frontend-safe | IPC may expose status/detail but never executable paths or raw commands. |
| Task 2 ↔ Task 3 | Task 2 creates `playback/mpv.rs`; Task 3 creates/modifies `playback/mod.rs` and consumes the trait | Shared module boundary is intentional | Keep mpv wire/process details behind `PlaybackBackend`; service owns policy and state. |
| Task 3 ↔ Task 4 | Task 3 publishes `PlaybackService`/snapshot/error types and must switch `lib.rs` from Task 2's inline module to file-backed `pub mod playback;`; Task 4 wires commands, event sink, and exit cleanup | The original file list omitted the required declaration-only module switch | Task 3 may edit only the `playback` module declaration in `lib.rs`; Task 4 owns all state, command, setup, and lifecycle edits there. |
| Task 3 ↔ Task 5 | Task 3 defines command DTOs/snapshot semantics; Task 5 mirrors them in TypeScript | Compatible only with integer milliseconds and opaque IDs | Frontend sends `trackId`/`sourceId` and typed values only; Rust remains authoritative. |
| Task 4 ↔ Task 5 | Task 4 defines native command/event names; Task 5 implements typed frontend wrappers/hooks | Compatible; native and browser-preview paths need the same Zod contract | Task 5 validates every response/event and ignores stale revisions. |
| Task 5 ↔ Task 6 | Task 5 creates `usePlaybackCommands`; Task 6 consumes it from the palette | Compatible; no duplicate transport state | Palette dispatches only existing typed mutations and preserves navigation behavior. |
| Task 5 ↔ Task 7 | Task 5 supplies functional controls; Task 7 drives them through a test-only native harness | Compatible; harness must not alter production runtime | Browser tests mock only the typed invoke/event boundary and do not inject paths. |
| Task 7 ↔ Task 8 | Task 7 adds browser dependency/config; Task 8 adds native smoke/build script | Independent runtime surfaces | Keep browser reports and real media outputs ignored; no shared production behavior. |
| Task 8 ↔ Task 9 | Task 8 adds smoke scripts and ignore rules; Task 9 records evidence and stages docs | Compatible; docs must report only executed checks | Run and record smoke evidence before documentation closure. |
| Task 1 self-check | Manager/version/queue/contract tests match created files and interfaces | Agrees after Cargo filter correction | Run each Rust filter as a separate command. |
| Task 2 self-check | Protocol/session tests cover the adapter created in `playback/mpv.rs` | Agrees; Windows-specific behavior is isolated | Keep parser tests platform-neutral and pipe/process tests Windows-gated. |
| Task 3 self-check | State/queue/path tests cover service, queue, and library files | Agrees after Cargo filter correction | Run playback, queue, and library filters separately plus clippy. |
| Task 4 self-check | IPC tests cover exact commands and event payloads wired in `lib.rs` | Agrees | Preserve startup when mpv is missing. |
| Task 5 self-check | Hook/player/library tests cover the UI files changed | Agrees | Update the existing disabled-play assertions to the new local-only behavior. |
| Task 6 self-check | Palette tests cover the exact transport commands it changes | Agrees | No provider-specific commands. |
| Task 7 self-check | Playwright spec/config/harness form one browser-only surface | Agrees after pinning `@playwright/test@1.62.1` | Install Chromium only if absent; do not commit browser binaries/reports. |
| Task 8 self-check | Native smoke and packaged script cover the runtime artifacts they create | Agrees | Gate actual mpv/packaged execution with the specified environment variables. |
| Task 9 self-check | Docs/ADR/ledger updates record completed evidence and final delivery | Agrees | Do not claim skipped or unexecuted checks. |

### Rulings

- Ruling: keep the explicitly authorized implementation on the existing `main` checkout — the user’s attached execution brief authorizes the nine Plan 04 commits and push, and the initial checkout is clean — cost if wrong: direct task commits would need a later branch migration.
- Ruling: run Cargo focused tests in separate invocations — Cargo accepts one test filter positionally, so the original multi-filter commands were invalid — cost if wrong: none beyond extra process startup.
- Ruling: pin `@playwright/test` to `1.62.1` — this is the verified current stable version at execution start — cost if wrong: a newer release may exist later, but the recorded dependency remains reproducible.
- Ruling: use `--keep-open=no` for the mpv generation — official behavior and local research show this yields the required genuine `end-file`/`eof` lifecycle — cost if wrong: EOF handling would need an adapter-only policy adjustment.
- Ruling: permit Task 3 to replace the Task 2 inline playback module with `pub mod playback;` in `lib.rs` — without that declaration Rust ignores the required `playback/mod.rs`, so the service cannot compile; Task 4 remains owner of every other `lib.rs` change — cost if wrong: the shared module declaration would need a targeted follow-up, not a behavior rollback.
- Ruling: follow the attached specification's exact mpv startup arguments and omit `--keep-open=no` and automatic audio-device arguments — the attachment is the binding authority and the current EOF path is normalized from mpv's end-file event — cost if wrong: the adapter's EOF classification would need a focused runtime correction.
- Ruling: add `SPOTDIY_PACKAGED_DATA_ROOT` only for the packaged smoke environment — Windows' known-folder API ignored the child `LOCALAPPDATA` override, so the harness needed an explicit isolated database root without changing normal production startup — cost if wrong: the smoke-only environment seam would need removal or a narrower test launcher.
- Ruling: after backing up the real database, remove only rows whose folder path matched the verified `SpotDIY-Plan04-Packaged-*` harness prefix using the same relationship semantics as `LibraryService::remove_folder` — the failed harness attempts had written only their own exact temporary roots — cost if wrong: restoring the backup would be required to recover accidentally selected user rows.

## Task status

- Task 1: complete. Implementer commits `fa049bd` and focused fix `bce78eb`; independent task review requested generic health diagnostics and stable Play Next ordering, both addressed and approved by scoped re-review.
- Task 2: complete. Implementer commits `f224667`, `3c8f6cf`, and `e01784e`; focused protocol/session tests, fmt, and clippy passed. Independent review findings on bounded forced shutdown, bounded writes, and kill-on-write-failure were addressed and approved. The final branch review also found no correctness issue in this area.
- Task 3: complete in the integrated working tree. Playback service, transient queue, source-resolution boundary, recovery, and focused state tests are present; the final whole-branch review passed.
- Task 4: complete in the integrated working tree. Tauri commands, `playback://state`, startup fallback, and exit cleanup are wired; the final whole-branch review passed.
- Task 5: complete in the integrated working tree. Typed frontend IPC, revision filtering, PlayerBar, and local-library controls are present; final browser gates and the whole-branch review passed.
- Task 6: complete in the integrated working tree. Ctrl+K transport actions are present and covered by frontend tests.
- Task 7: complete in the integrated working tree. The pinned Playwright matrix covers 1280/1920/2560 projects and passed its browser run.
- Task 8: complete in the integrated working tree. Real mpv smoke passed; the packaged playback/restart/cleanup smoke passed after the isolated data-root correction.
- Task 9: complete in the integrated working tree. Required docs, ADR,
  final graph refresh, independent review, feature/docs commits, and remote
  verification are recorded in the final delivery sequence.
- The initial final review returned `FAIL`; the two medium findings were fixed
  in `af66127`. The same reviewer rechecked the committed remediation and
  returned `PASS` with zero critical, high, or correctness/security medium
  findings. One low-priority hostile-probe regression-test suggestion remains
  non-blocking.
- Final local gates passed: 117 Rust tests, 26 Vitest tests, 9 Playwright runs,
  fmt, all-features clippy, Tauri release build, real-mpv smoke, and packaged
  playback/restart/cleanup smoke.
