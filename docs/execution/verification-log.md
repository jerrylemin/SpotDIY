# SpotDIY verification log

## 2026-08-30 — initial bootstrap

- `pnpm install` — passed; lockfile generated and policy check passed.
- `pnpm typecheck` — passed.
- `pnpm lint` — passed.
- `pnpm test` — passed: 1 test file, 1 test.
- `pnpm build` — passed; Vite production bundle generated.
- `cargo fmt --all` — applied; subsequent `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed.
- `cargo test --all-targets` — passed; 0 Rust tests currently defined.
- `pnpm tauri icon public/spotdiy-mark.svg` — passed; Windows and cross-platform icon assets generated.
- `npm install --global @colbymchenry/codegraph@1.6.0` — passed; CodeGraph installed from the package whose repository is the requested upstream.
- `codegraph init .` and `codegraph status .` — passed; 37 files, 196 nodes, 370 edges, WAL-backed index, up to date.
- `graphify install --project --platform codex` — passed; project skill, AGENTS integration, and PreToolUse hook registered.

Passed after the checks above: `pnpm tauri build` (NSIS installer generated) and packaged launch smoke (release executable stayed running for 5 seconds before the test harness closed it).

- `pnpm audit --prod` — passed; no known vulnerabilities found.
- Final CodeGraph sync/status — passed; 37 files, 196 nodes, 352 edges, index up to date.
- Final Graphify AST update — passed; 672 nodes, 1,441 edges in ignored derived output.

Browser preview smoke also passed: Playwright rendered Home and Search, Ctrl+K opened the command palette, command navigation reached Search, typed queries rendered independent provider sections, and the final console contained no errors beyond React development-tools info.

At the bootstrap boundary, database tests, provider live tests, and the full Playwright visual QA suite were not yet run; the Plan 02 database tests are recorded below. Provider live tests and full visual QA remain later-plan checks.

## 2026-08-30 — Plan 02 verification

- `cargo fmt --manifest-path src-tauri/Cargo.toml` — applied; `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` — passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` — passed: 25 Rust tests, 0 failures. Coverage includes migration 1, ordering, rollback, WAL/FK, busy-reader backup protection, domain invariants, repository round trips/rollback, preferred-source integrity, settings, and IPC status.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` — passed.
- `pnpm typecheck` — passed.
- `pnpm lint` — passed.
- `pnpm test` — passed: 1 file, 5 tests.
- `pnpm build` — passed; Vite production bundle generated. Existing Browserslist and Tailwind content warnings remain non-fatal.
- `pnpm tauri build` with the verified Cargo bin directory inherited in `PATH` — passed; x64 release executable and NSIS installer generated.
- Packaged launch smoke — passed; release executable remained running through the startup window and initialized `%LOCALAPPDATA%\\SpotDIY\\spotdiy.sqlite3`.
- Independent Plan 02 review — PASS after the source-move guard and current frontend tests; no unresolved critical, high, or medium findings.
- Implementation commit — `2ec431b7fcbf31fbb2f2cd3b092b66ad75e81365` (`feat: add unified music domain and sqlite foundation`); the final documentation follow-up is the delivery tip.

## 2026-08-30 - Plan 03 Local Library final verification

- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` - passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` - passed: 53 Rust tests, 0 failures.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` - passed.
- `pnpm test` - passed: 4 files, 18 tests.
- `pnpm typecheck` - passed.
- `pnpm lint` - passed.
- `pnpm build` - passed; existing Browserslist stale-data and Tailwind content warnings remain non-fatal.
- `pnpm tauri build` - passed; generated `src-tauri/target/release/spotdiy.exe` and `src-tauri/target/release/bundle/nsis/SpotDIY_0.1.0_x64-setup.exe`.
- Packaged launch smoke - passed; the release executable reported window title `SpotDIY`, stayed alive for five seconds, closed cleanly, and returned exit code 0.
- Native synthetic-folder smoke - passed against the final release build: 2 supported candidates (1 indexed WAV, 1 corrupt error), partial error retained, restart generation advanced, unchanged rescan preserved identity/fingerprint, watcher create/forced modify/rename/delete/restore reconciled correctly, reveal succeeded, folder removal deleted only library rows, and synthetic media remained.
- Native smoke fixture cleanup - passed; the exact temporary fixture and persisted test folder were removed, and no `spotdiy` process remained.
- Mocked-IPC browser smoke - configuration gap recorded: `pnpm exec playwright --version` reports 1.58.0, but `pnpm exec playwright test --list` fails with `unknown command 'test'`; no Playwright browser project/configuration exists in the repository. Native CDP smoke is the corresponding release evidence.
- Independent read-only review - correctness findings for transient scan errors, watcher recovery, missing-root reactivation, reparse-point policy, and partial-error persistence were fixed and covered by focused tests.
- `graphify update .` - passed; ignored derived output rebuilt with 1,202 nodes, 1,662 edges, and 109 communities. No generated graph artifacts are staged.

## 2026-08-31 — Plan 04 playback verification

The following entries are the pre-remediation baseline retained for audit
history; the final post-fix delivery evidence is recorded in the section
below.

- Cargo/Tauri output was redirected to `C:\CargoTarget\SpotDIY`; the
  repository-local `src-tauri\target` remained absent.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` — passed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` — passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` — passed:
  107 tests, 0 failures, with the real-mpv integration gate skipped when its
  environment variable was unset.
- `pnpm typecheck` — passed. `pnpm test` — passed: 5 files, 24 tests.
- `pnpm build` — passed. `pnpm exec playwright test` — passed: 6 playback
  specs across the 1280, 1920, and 2560 projects; no console errors.
- `pnpm lint` initially reported the packaged harness's caught-error cause;
  `scripts/packaged-playback-smoke.mjs` now preserves `{ cause: error }`, and
  the follow-up lint plus `node --check scripts/packaged-playback-smoke.mjs`
  passed.
- Real Windows mpv smoke — passed: 1 test in 4.17 seconds using local
  `v0.41.0-dev-g41f6a6450` at `.tools\mpv\v0.41.0\mpv.exe`; transport,
  devices, EOF, shutdown, and process exit were observed.
- `pnpm tauri build` — passed with the external target; release executable and
  NSIS bundle were generated under `C:\CargoTarget\SpotDIY`.
- Packaged playback smoke — passed with `-TimeoutSeconds 20`: synthetic
  library indexing, playback flow, graceful close, owned-mpv cleanup, restart
  persistence, and empty transient queue. No temporary profile, relevant
  process, or harness row remained afterward.
- The initial whole-branch independent review returned `FAIL`; its complete
  findings and the one-agent fix wave are recorded in the Plan 04 SDD
  workspace. The post-remediation recheck and final delivery evidence follow
  below.

## 2026-08-31 — Plan 04 final delivery after review fix

- The single fresh read-only reviewer rechecked the committed remediation in
  `af66127`: `CRITICAL None`, `HIGH None`, `MEDIUM None`, and `VERDICT PASS`.
  The only remaining note was low priority: add direct hostile-probe output
  regression tests; the implementation is bounded and the note is
  non-blocking.
- `pnpm typecheck` — passed. `pnpm lint` — passed. `pnpm test` — passed: 6
  files, 26 tests. `pnpm build` — passed; existing stale Browserslist and
  Tailwind content notices remain non-fatal.
- `pnpm exec playwright test` — passed: 9 runs across the 1280, 1920, and
  2560 viewport projects; no console errors.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` — passed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
  --all-features -- -D warnings` — passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` — passed:
  117 tests, 0 failures; the real-mpv test was gated in this run.
- `pnpm tauri build` — passed; release executable and NSIS bundle were built
  under external `C:\CargoTarget\SpotDIY`.
- Real Windows mpv smoke with `SPOTDIY_REAL_MPV_SMOKE=1` — passed: 1 test in
  4.13 seconds using `.tools\mpv\v0.41.0\mpv.exe`, including transport,
  devices, EOF, shutdown, and process exit.
- Packaged smoke with `SPOTDIY_PACKAGED_SMOKE=1` and `-TimeoutSeconds 45` —
  passed: playback flow, restart persistence, transient-queue boundary, and
  owned-process cleanup. No `mpv` or `spotdiy` process remained afterward.
- The external target contained 9,657,154,275 bytes (8.99 GiB);
  `src-tauri\target` was absent. Recovery snapshot:
  `C:\Users\ADMINI~1\AppData\Local\Temp\SpotDIY-Plan04-Recovery-20260831-165653`.
- `codegraph sync .`, `codegraph status .`, and the focused
  `PlaybackService MpvBackend AppState` query — passed; CodeGraph reports 73
  files, 1,789 nodes, 6,421 edges, and an up-to-date index.
- `graphify update .` — passed once after documentation finalization; derived
  output reports 1,933 nodes, 3,493 edges, and 151 communities, built from
  `af66127d`.

## Plan 05 source adapters and search (2026-09-01)

- `cargo test --manifest-path src-tauri/Cargo.toml search::tests -- --nocapture`
  - passed: 17 focused SearchService tests.
- `pnpm typecheck` - passed.
- `pnpm lint` - passed.
- `pnpm test` - passed: 38 Vitest tests across 9 files.
- `pnpm exec playwright test` - passed: 45 runs across the 1280, 1920, and
  2560 viewport projects.
- `pnpm build` - passed. Browserslist and Tailwind content notices remained
  non-fatal.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` - passed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
  --all-features -- -D warnings` - passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` - passed:
  250 unit tests and 1 `mpv_smoke` integration test, 0 failures.
- `scripts/provider-search-smoke.ps1` - passed: five focused native Local/
  playback checks. With `-RunLiveProviders`, YouTube and SoundCloud each
  returned 25 structured metadata entries; Spotify was skipped because no
  developer authorization was available.
- `scripts/provider-search-smoke.ps1 -RunPackaged` - passed after rebuilding
  the release binary: isolated Local indexing/result rendering, missing
  yt-dlp provider failure isolation, concurrent cancellation, Spotify gate,
  and helper-process cleanup.
- `pnpm tauri build` - passed with release executable and NSIS bundle under
  external `C:\CargoTarget\SpotDIY`.
- `graphify update .` - passed once after the final implementation refresh;
  Graphify reports 2,741 nodes, 5,049 edges, and 189 communities.
- `codegraph sync .` and `codegraph status .` - passed; CodeGraph reports 93
  files, 2,762 nodes, 9,607 edges, and an up-to-date index.
- The concrete packaged runtime race was fixed by using the Tauri async
  runtime for SearchService tasks; the frontend now buffers provider events
  that arrive before the native `start_search` response.
- `src-tauri\target` was absent; generated Playwright result output was
  removed after verification. No provider credentials, tokens, raw tool
  output, or provider payloads were retained in the repository.

## Plan 06 source fusion and resolver (2026-09-01)

- `pnpm typecheck` - passed.
- `pnpm lint` - passed.
- `pnpm test` - passed: 40 Vitest tests across 10 files.
- `pnpm exec playwright test` - passed: 45 runs across the 1280, 1920, and
  2560 viewport projects.
- `pnpm build` - passed. Existing Browserslist and Tailwind content notices
  remained non-fatal.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` - passed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
  --all-features -- -D warnings` - passed after the concrete new-code loop/
  Option-style corrections.
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` - passed:
  279 unit tests and 1 `mpv_smoke` integration test, 0 failures.
- Explicit real mpv smoke with `SPOTDIY_REAL_MPV_SMOKE=1` - passed: 1 test;
  synthetic WAV load, transport, seek, device, EOF, and cleanup behavior.
- `pnpm tauri build` - passed. Release executable and NSIS bundle were built
  under external `C:\CargoTarget\SpotDIY`.
- Packaged playback smoke with `SPOTDIY_PACKAGED_SMOKE=1` - passed:
  indexing/playback, restart persistence, graceful close, owned-mpv cleanup,
  and transient queue boundary.
- Named v2-to-v3 migration smoke - passed:
  `schema_two_fixture_upgrades_to_three_without_losing_tracks_sources_or_settings`.
- `src-tauri\target` remained absent. No media files were modified or deleted;
  no provider credentials, tokens, raw output, or provider payloads were
  retained in SQLite or the repository.

## 2026-09-01 - Plan 07 persistent download manager

- Baseline before implementation was clean at `6380ccd715c1958cb09b0df684f521639e0561db`; local and remote matched, `CARGO_TARGET_DIR` was `C:\CargoTarget\SpotDIY`, and `Test-Path .\src-tauri\target` was `False`.
- Focused implementation checks passed: 24 `downloads::` tests, 33
  `media_tools::` tests, 9 `media_tools::yt_dlp::` tests, 7 new frontend
  download/Downloads/search tests, 13 existing domain/search frontend tests,
  and 11 focused Rust IPC tests.
- `pnpm typecheck` - passed. `pnpm lint` - passed. `pnpm test` - passed:
  47 tests across 13 files.
- `pnpm exec playwright test` - passed: 45 runs across the 1280, 1920, and
  2560 viewport projects. Existing search and playback contracts remain green.
- `pnpm build` - passed. Existing Browserslist, Tailwind content, and chunk
  size notices remain non-fatal.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` - passed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
  --all-features -- -D warnings` - passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` - passed:
  308 Rust unit tests, one synthetic mpv integration test, and zero failures.
- `git diff --check` - passed before documentation edits. `pnpm tauri build` -
  passed; release executable and NSIS bundle were generated under external
  `C:\CargoTarget\SpotDIY`.
- Explicit real mpv smoke with `SPOTDIY_REAL_MPV_SMOKE=1` - passed: one
  synthetic-WAV integration test with owned-process cleanup.
- Packaged playback smoke with `SPOTDIY_PACKAGED_SMOKE=1` - passed:
  synthetic indexing/playback, restart persistence, graceful close, and
  owned-mpv cleanup. No SpotDIY-owned mpv process remained.
- `scripts/provider-search-smoke.ps1 -RunPackaged` passed its five native
  synthetic Local/playback checks and skipped live providers/Spotify as
  configured. Its optional packaged-search branch reached the existing
  immediate `start_search`/`cancel_search` race and returned no active
  SearchId under the deliberately missing-yt-dlp profile; the harness cleaned
  up and no owned process remained. This branch is recorded as a harness
  limitation, not a fabricated pass.
- `graphify update .` passed once after code changes: 3,235 nodes, 6,108
  edges, 210 communities. `codegraph sync .` and `codegraph status .` passed
  once after implementation: 107 files, 3,496 nodes, 12,434 edges, index up
  to date.
- The local tool probes report yt-dlp `2026.08.19` and FFmpeg `8.1.1`; no tool
  binary, downloaded media, runtime database, cache temp, credential, token,
  or generated test artifact is part of the repository changes.
- Repository-local `src-tauri\target` remains absent after all checks.

## 2026-09-01 - Plan 08 playlists, collections, and persistent queue

- Baseline before Plan 08 was clean at `d3bd21e7bf0b4ce937cf9cd1e58d6655ccfd6a46`; Cargo output remained external at `C:\CargoTarget\SpotDIY` and `Test-Path .\src-tauri\target` was `False`.
- Focused native coverage passed: schema v4-to-v5 migration preservation,
  playlist CRUD/duplicate/remove/reorder, Inbox idempotence, branch diff and
  one-shot selected merge/discard, collection normalization/validation, queue
  sections, movement, pin/clear, shuffle policy, snapshot round trip, and
  restart position restore.
- `pnpm typecheck` - passed. `pnpm lint` - passed. `pnpm test` - passed:
  51 tests across 14 files.
- `pnpm exec playwright test` - passed: 48 runs across the 1280, 1920, and
  2560 viewport projects, including the queue drawer sections and truthful
  Autoplay empty state.
- `pnpm build` - passed. Existing Browserslist stale-data, Tailwind content,
  and large-chunk notices remain non-fatal.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` - passed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
  --all-features -- -D warnings` - passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` - passed:
  318 Rust unit tests, one synthetic mpv integration test, and zero failures.
- `pnpm tauri build` - passed; the release executable and NSIS bundle were
  generated under external `C:\CargoTarget\SpotDIY`.
- Explicit real mpv smoke with `SPOTDIY_REAL_MPV_SMOKE=1` - passed: one
  synthetic-WAV integration test with owned-process cleanup.
- Packaged playback smoke with `SPOTDIY_PACKAGED_SMOKE=1` - passed: playback,
  persistent restart boundary, graceful close, and owned-mpv cleanup.
- Packaged Plan 08 persistence smoke with `-Plan08Persistence` - passed:
  durable playlist/items, Inbox, like/rating/tag collection, queue sections,
  repeat/shuffle, named snapshot, no-autoplay restart, and saved-position
  resume. No SpotDIY-owned mpv process remained.
- Named migration smoke `schema_four_fixture_upgrades_to_five_preserving_plan_seven_data` - passed.
- `git diff --check` - passed before documentation edits. The optional
  missing-yt-dlp packaged provider-search race was intentionally not triggered;
  live provider/download smoke was not run.
- CodeGraph refresh via `codegraph sync .` and status - passed once after
  implementation: 112 files, 4,000 nodes, 14,771 edges, index up to date.
- Graphify refresh via `graphify update .` - passed once after implementation:
  3,596 nodes, 7,012 edges, and 233 communities.
- Repository-local `src-tauri\target` remains absent; no media, credentials,
  tokens, runtime database, cache temp, or generated test artifact is staged.

## 2026-09-02 - Plan 09 local-first lyrics, bookmarks, and A/B loop

- Baseline: `main` was clean and aligned at `4940ebe6ae473507569093629c8f863ce381f990`; `CARGO_TARGET_DIR` was set to `C:\CargoTarget\SpotDIY` and repository-local `src-tauri\target` was absent.
- Frontend: `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build` -
  passed. Vitest reports 56 tests across 16 files.
- Browser: `pnpm exec playwright test` - passed: 48 runs across the 1280,
  1920, and 2560 viewport projects.
- Native: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
  `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
  --all-features -- -D warnings`, and `cargo test --manifest-path
  src-tauri/Cargo.toml --all-targets` - passed. The final Rust run reports
  337 unit tests plus one synthetic `mpv_smoke` integration test.
- Focused coverage: parser timestamp variants/bounds and malformed fallback,
  embedded plain/SYLT metadata, local/manual/provider precedence, LRCLIB
  validation/rate limits/cache, bookmark persistence, safe A/B bounds,
  source/recovery restoration, and new-track clearing - passed.
- Named migration smoke `schema_five_fixture_upgrades_to_six_preserving_plan_eight_collections_and_queue` - passed; the fixture also preserves Plan 07 downloads.
- `SPOTDIY_REAL_MPV_SMOKE=1 cargo test --manifest-path src-tauri/Cargo.toml
  --test mpv_smoke -- --nocapture` - passed with owned-process cleanup.
- `pnpm tauri build` - passed; release output is under external
  `C:\CargoTarget\SpotDIY`. The regular packaged playback/restart/cleanup
  smoke, packaged Plan 08 persistence smoke, and packaged Plan 09 lyrics,
  bookmarks, A/B, preset, restart, queue, no-autoplay, and cleanup smoke all
  passed; zero SpotDIY-owned mpv processes remained.
- `git diff --check` - passed. The live LRCLIB smoke was optional and skipped.
  Non-fatal existing build notices remain for Browserslist data, Tailwind
  content discovery, and a large frontend chunk.
- `codegraph sync .` plus status - passed once after implementation: 121 files,
  4,472 nodes, 16,503 edges, index up to date. `graphify update .` - passed
  once after implementation: 3,883 nodes, 7,668 edges, 237 communities.
- No copyrighted lyrics, media, credentials, tokens, raw provider payloads,
  runtime database, cache temp, or generated test artifact is part of the
  repository changes; `src-tauri\target` remains absent.

## 2026-09-02 - Plan 10 final verification

- `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build` - passed. Vitest
  reports 70 tests across 18 files. Lint retains three non-fatal Fast Refresh
  warnings; build retains non-fatal Browserslist, Tailwind content, and large
  chunk notices.
- `pnpm exec playwright test` - passed: 51 tests across the 1280, 1920, and
  2560 viewport projects. Plan 10 coverage verifies theme/layout controls,
  custom-theme validation and reset, focus, context-menu keyboard paths,
  reduced motion, responsive overflow, icon rendering, and the 1080 height
  guard. Screenshots use Playwright output paths and are not committed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
  `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
  --all-features -- -D warnings`, and
  `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` - passed.
  The final Rust run reports 343 unit tests plus one synthetic mpv integration
  test.
- `pnpm tauri build` - passed; release executable and NSIS output are under
  external `C:\CargoTarget\SpotDIY`. A packaged settings smoke passed startup,
  default Dark/Comfortable values, Dark/Light/Custom writes, all layout
  profiles, custom-theme persistence across restart, reset, and clean close.
  The packaged Plan 09 playback/lyrics persistence smoke also passed; no owned
  mpv process remained.
- `git diff --check` - passed. Storage remains schema 6 with ordinary
  `layout_profile` and `custom_theme` settings keys; no migration 7 was added.
  CodeGraph was refreshed once and reports 138 files, 4,655 nodes, and 17,004
  edges. Graphify was refreshed once with the code-only update and reports 4,023
  nodes, 7,913 edges, and 245 communities. Repository-local `src-tauri\target`
  remains absent.

## 2026-09-02 - Plan 11 final verification

- `pnpm typecheck` - passed. `pnpm lint` - passed with the same three
  pre-existing Fast Refresh warnings in `SpotIcon.tsx` and
  `theme-controller.tsx`. `pnpm test` - passed: 73 tests across 19 files.
- `pnpm exec playwright test` - passed: 63 tests across the 1280, 1920, and
  2560 viewport projects. Coverage includes populated Home, persisted and
  ephemeral inspectors, Standard/Mini/Expanded player modes, source-aware
  actions, command-palette navigation, Escape priority, and existing
  design/search/playback paths.
- `pnpm build` - passed: 712 modules transformed. Non-fatal notices remain for
  stale Browserslist data, empty Tailwind content configuration, an ineffective
  dynamic import, and a frontend chunk over 500 kB.
- Migration regressions - passed: the independent real old-constraint schema-6
  fixture and Plan-10-shaped schema-6 fixture both upgrade to schema 7 without
  losing old settings; fresh startup reaches schema 7; appearance keys work,
  custom theme activates, and foreign-key checks remain clean.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and
  `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
  --all-features -- -D warnings` - passed. The strict lint correction is in
  `e072fec`.
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` - passed:
  347 unit tests and one synthetic mpv integration test. Explicit
  `SPOTDIY_REAL_MPV_SMOKE=1` `mpv_smoke` - passed: one real Windows mpv test.
- `pnpm tauri build` - passed. Release executable and NSIS bundle are under
  external `C:\CargoTarget\SpotDIY`. Existing packaged Plan 09
  playback/lyrics/bookmark/A-B/persistence smoke - passed. Added packaged
  Plan 11 smoke - passed: schema-6 startup migration, appearance persistence,
  live Home, inspector path privacy, shell modes, queue/Lyrics/palette,
  restart persistence, no autoplay, and owned-mpv cleanup.
- `git diff --check` - passed before documentation closure. Final Graphify
  refresh reports 4,131 nodes, 8,235 edges, and 247 communities; CodeGraph was
  refreshed once for the shell/player/inspector dependency query. No live
  provider/LRCLIB/download smoke was required. Repository-local
  `src-tauri\target` remains absent.

## 2026-09-02 - Plan 12 final verification

- `pnpm typecheck` - passed. `pnpm lint` - passed with the same three
  pre-existing Fast Refresh warnings in `SpotIcon.tsx` and
  `theme-controller.tsx`. `pnpm test` - passed: 78 tests across 21 files.
- `pnpm exec playwright test` - passed: 69 tests across the 1280, 1920, and
  2560 viewport projects. Plan 12's focused contract also passed all six
  cases, covering browser-preview settings/overlay state and native-only
  command-palette gating.
- `pnpm build` - passed. Non-fatal notices remain for stale Browserslist data,
  empty Tailwind content configuration, an ineffective dynamic import, and a
  frontend chunk over 500 kB.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and
  `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
  --all-features -- -D warnings` - passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` - passed:
  365 unit tests and one real-mpv synthetic WAV integration test. The named
  `schema_seven_settings_rows_are_copied_unchanged_by_schema_eight` migration
  test also passed independently.
- `pnpm tauri build` - passed. Release executable and NSIS bundle are under
  external `C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` is
  absent.
- Regular `scripts/packaged-playback-smoke.ps1` - passed for playback,
  restart/no-autoplay, graceful shutdown, and owned-mpv cleanup. Packaged Plan
  11 smoke - passed for schema-6-to-7 migration, appearance, shell, inspector,
  queue, and restart persistence.
- Dedicated `scripts/packaged-windows-integration-smoke.ps1` - passed. The live
  Windows run reported `SMTC READY`, registered the controlled shortcut,
  verified tray state, overlay reuse/topmost, Gaming click-through recovery,
  output-profile apply/restore without playback-context mutation, schema 8,
  restart persistence, and zero owned mpv processes. The independent SMTC
  restart check also reported `SMTC READY after restart`.
- `git diff --check` - passed before documentation closure. CodeGraph is
  current at 166 files, 5,349 nodes, and 19,394 edges; Graphify is current at
  4,467 nodes, 8,952 edges, and 262 communities. No runtime database, media,
  credentials, tokens, raw provider payloads, or generated test artifacts are
  retained in the repository.

## 2026-09-02 - Plan 13 final verification

- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` - passed.
  `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
  --all-features -- -D warnings` - passed. `cargo test --manifest-path
  src-tauri/Cargo.toml --all-targets` - passed: 393 Rust unit tests plus one
  real-mpv synthetic WAV integration test.
- Backup-focused native coverage passed for deterministic archive metadata and
  byte equality, exact manifest checksums, online WAL snapshots, secure ZIP
  bounds/path/compression/symlink/case checks, staged import isolation,
  migration/integrity/FK validation, missing references, audio and sidecar
  restore, artwork trust boundaries, crash recovery, rollback, and both
  Standard/Portable transitions.
- `pnpm typecheck` - passed. `pnpm lint` - passed with the same three
  pre-existing Fast Refresh warnings in `SpotIcon.tsx` and
  `theme-controller.tsx`. `pnpm test` - passed: 81 tests across 22 files.
- `pnpm exec playwright test` - passed: 69 tests across the 1280, 1920, and
  2560 viewport projects. `pnpm build` - passed. Existing non-fatal notices
  remain for stale Browserslist data, empty Tailwind content configuration,
  ineffective dynamic import, and the large frontend chunk.
- `pnpm tauri build` - passed. Release executable and NSIS bundle are under
  external `C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target`
  remains absent.
- `scripts/packaged-playback-smoke.ps1` - passed. Packaged Plan 11 shell
  smoke - passed. `scripts/packaged-windows-integration-smoke.ps1` - passed.
  `scripts/packaged-backup-storage-smoke.ps1` - passed: isolated release
  Standard startup, Standard-to-Portable preparation, Portable restart,
  Portable-to-Standard preparation, final Standard restart, exact portable
  directories, marker removal, and retained databases.
- `graphify update .` - passed; final code-only graph reports 4,723 nodes,
  9,470 edges, and 277 communities. `git diff --check` - passed before
  documentation closure. `memory.zip` remains untracked and was not touched;
  no runtime data or secrets are retained in the repository.
- `codegraph sync .` and `codegraph status .` - passed; the index is up to date
  at 176 files, 5,781 nodes, and 21,456 edges.

## 2026-09-03 - Plan 14 final verification

- `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build` passed. Vitest
  reports 83 tests across 23 files. Lint retains three pre-existing Fast
  Refresh warnings; frontend build notices remain non-fatal.
- `pnpm exec playwright test` passed: 69 tests across the 1280, 1920, and
  2560 viewport projects.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` passed.
  `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` passed:
  420 Rust unit tests plus one real-mpv synthetic WAV integration test.
  Strict all-target/all-feature Clippy with `-D warnings` passed.
- `pnpm tauri build` passed and produced the release executable and NSIS
  installer under external `C:\CargoTarget\SpotDIY`; repository-local
  `src-tauri\target` remains absent.
- The rebuilt release passed the regular playback/restart smoke, Plan 11
  shell/inspector smoke, Plan 12 Windows integration smoke (schema 8-to-9,
  tray, SMTC, shortcut, overlay, click-through, and output-profile checks),
  Plan 13 Standard/Portable storage smoke, and Plan 14 history/privacy/
  temporary/smart/analytics smoke. All reported clean owned-process shutdown.
- An isolated synthetic schema-9 roundtrip passed in a temporary Rust test:
  the exported `.spotdiy` manifest remained format 1, schema 9, and metadata,
  genres, sessions, history, and smart playlists survived staged import and
  restart apply. A fresh mode service had Private Session and Temporary Mode
  disabled. The temporary test and archive were deleted afterward.
- Plan 13 staging security regressions passed in the final Rust suite, covering
  symlink/reparse/path ownership rejection and safe cleanup. The real mpv
  smoke used the verified `.tools\mpv\v0.41.0\mpv.exe` binary.
- The initial native blocker was an uninitialized Visual Studio Developer
  Shell environment, not a missing installation; the x64 DevShell was loaded
  and no repository code was changed for that repair. Chromium was installed
  with `pnpm exec playwright install chromium`. The only source fixes were a
  Clippy-required settings match guard and the Plan 12 smoke's stale schema-8
  assertion, both verified by the final gate.
- `graphify update .` passed after those code changes: 281 files, 5,329 nodes,
  12,057 edges, and 260 communities. CodeGraph remains unavailable because no
  command/index is present. No runtime SQLite, archive, media, credentials,
   tokens, or generated test diagnostics are retained.

## 2026-09-03 - Plan 15 final verification

- `pnpm typecheck` and `pnpm test` passed; Vitest reports 88 tests across 24
  files. `pnpm lint` passed with the three existing Fast Refresh warnings and
  no errors. `pnpm build` passed with the existing non-fatal dynamic-import and
  large-chunk notices.
- `pnpm exec playwright test` passed: 70 tests across the existing viewport
  projects and the targeted Plan-15 ultrawide project.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` passed;
  strict all-target/all-feature Clippy with `-D warnings` passed; native all-
  target tests passed with 430 unit tests and one real-mpv synthetic-WAV
  integration test.
- `pnpm tauri build` passed with release executable and NSIS installer at
  external `C:\CargoTarget\SpotDIY`. The Plan-15 packaged runner passed both
  visual/restart phases, real local preview, dataset contract, Theme Studio,
  and owned-process cleanup; both packaged app processes exited with code 0.
- `graphify update .` passed and rebuilt 5,517 nodes, 12,505 edges, and 268
  communities. CodeGraph remains unavailable. `git diff --check` passed and
  repository-local `src-tauri\\target` remains absent.
- The Plan-14 transition repair is covered by four native regression tests;
  visual dataset coverage includes empty/filter/order/aggregate/truncation and
  path non-leakage cases; preview coverage includes bounded policy and an
  injectable process seam without an 8-second test sleep.
