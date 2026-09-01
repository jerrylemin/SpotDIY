# SpotDIY test matrix

| Area | Current check | Result / coverage |
|---|---|---|
| Frontend type safety | `pnpm typecheck` | Pass. Strict playback/library/settings DTOs and command arguments are checked. |
| Frontend lint | `pnpm lint` | Pass. |
| Frontend behavior | `pnpm test` | Pass: 73 Vitest tests across 19 files, including inspector DTO parsing, capability-aware actions, shell state, settings/theme schemas, and existing playback/library/lyrics behavior. |
| Frontend build | `pnpm build` | Pass. Existing Browserslist/Tailwind content notices are non-fatal. |
| Rust formatting | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | Pass. |
| Rust quality | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | Pass. |
| Rust behavior | `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` | Pass: 347 unit tests plus one synthetic mpv integration test, including schema-6-to-7 fixtures, inspector privacy, settings/theme validation, playback, persistence, and existing service behavior. |
| Browser playback | `pnpm exec playwright test` | Pass: 63 tests across 1280, 1920, and 2560 viewport projects; includes real-data Home, persisted/ephemeral inspectors, player modes, command palette, source-aware shell behavior, appearance, context-menu keyboard paths, responsive overflow, and existing playback coverage. |
| Real mpv | Windows `mpv_smoke` integration gate | Pass with local `v0.41.0-dev-g41f6a6450`; synthetic WAV transport, device, EOF, shutdown, and process-exit behavior verified. |
| Packaged lifecycle | `scripts/packaged-playback-smoke.ps1` | Pass with release executable: Plan 09 playback/lyrics persistence plus Plan 11 schema migration, appearance, shell, inspector, queue, restart, no-autoplay, and owned-mpv cleanup. |
| Release | `pnpm tauri build` | Pass. Release executable and NSIS bundle built with external `C:\CargoTarget\SpotDIY`; no repository-local `src-tauri\target`. |
| Provider contracts | Plan 05 native adapter/search tests and metadata smoke | Pass: 250 Rust tests include provider registry, local search, yt-dlp adapters, Spotify PKCE/error mapping, timeouts, cancellation, sorting, and cache bounds; opt-in YouTube/SoundCloud metadata smoke returned 25 entries each. |
| Persistence boundary | Schema and restart smoke | Pass: migration 7 preserves schema-6 settings and adds only the appearance allowlist; queue and appearance persist across restart without autoplay. |
| Visual follow-up | Playwright screenshots | Pass: Plan 11 covers Home/player/inspector shell paths, appearance, focus, reduced motion, context actions, icon rendering, responsive guards, and the 1080-pixel height fallback. |

## Plan 05 verification

- Frontend: `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build` pass;
  Vitest reports 38 tests across 9 files.
- Browser: `pnpm exec playwright test` reports 45 passing runs across the
  1280, 1920, and 2560 viewport projects, including independent loading,
  partial results, stale IDs, lens isolation, strict Spotify disablement,
  long-title overflow, and artwork fallback.
- Native: all-target Rust tests report 250 unit tests plus one integration
  `mpv_smoke` test; fmt and all-features clippy with warnings denied pass.
- Runtime: the provider smoke script passes five focused native checks; its
  opt-in live branch passes YouTube and SoundCloud metadata-only searches with
  25 entries each and skips Spotify without developer authorization. The
  isolated packaged search smoke passes Local indexing/result rendering,
  provider failure isolation, cancellation, Spotify gating, and helper
  process cleanup.

## Plan 06 verification

- Frontend: `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build` pass;
  Vitest reports 40 tests across 10 files. Existing Browserslist and Tailwind
  content notices remain non-fatal.
- Browser: `pnpm exec playwright test` passes 45 runs across the 1280, 1920,
  and 2560 viewport projects.
- Native: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
  all-features clippy with warnings denied, and all-target Rust tests pass;
  the suite reports 279 unit tests plus 1 real `mpv_smoke` integration test.
- Focused Plan 06 coverage includes normalization and all guarded qualifiers,
  conservative matcher/duration/version/ambiguity rules, override precedence,
  v2-to-v3 migration preservation, remote-source idempotence/conflicts, local
  quality ordering, unavailable-source explanations, provider readiness, and
  strict fusion/resolution IPC DTOs.
- Runtime: explicit real synthetic-WAV mpv smoke passes; `pnpm tauri build`
  passes with output under `C:\CargoTarget\SpotDIY`; packaged playback,
  restart persistence, graceful shutdown, and owned-mpv cleanup smoke passes.
- Storage: schema version 3, one `user_track_overrides` table plus its indexes,
  no provider-search bulk persistence, no Spotify fusion rows, no secrets or
  tokens in SQLite, no media mutation, and no repository-local
  `src-tauri\target`.

## Plan 07 verification

- Frontend: `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build` pass;
  Vitest reports 47 tests across 13 files, including strict download DTOs,
  event validation, queue actions, filtering, folder selection, and search
  provider download actions.
- Browser: `pnpm exec playwright test` passes 45 runs across the 1280, 1920,
  and 2560 viewport projects. Existing search/playback browser contracts stay
  green after the Downloads UI and AppStatus changes.
- Native: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
  all-features Clippy with warnings denied, and `cargo test --all-targets`
  pass. The final Rust run reports 308 unit tests plus the synthetic mpv
  integration test.
- Focused Plan 07 coverage includes schema 3-to-4 preservation, repository
  round trips, state transitions, queue ordering, concurrency limits, machine
  progress parsing, bounded argv/output, FFmpeg availability, cancellation,
  retry, restart recovery, output-missing history, sanitization, collisions,
  cross-volume finalization, no-fake-lossless provenance, and cleanup.
- Runtime: explicit synthetic real-mpv smoke, packaged playback/restart/owned
  process cleanup smoke, five native provider-search smoke checks, and the
  external-target `pnpm tauri build` pass. Optional live provider/download
  smoke was not run. The optional packaged provider-search harness remains
  blocked by its existing immediate `start_search`/`cancel_search` race when
  yt-dlp is deliberately missing; no owned process remained after the run.
- Storage: schema version 4 adds only `downloads` and `download_settings`;
  build output remains at `C:\CargoTarget\SpotDIY`, task temp roots are
  owned under `%LOCALAPPDATA%\SpotDIY\cache\downloads`, and the repository
  `src-tauri\target` path remains absent.

## Plan 08 verification

- Frontend: `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build` pass;
  Vitest reports 51 tests across 14 files, including strict playlist/collection
  and queue DTOs, stale queue-event handling, and browser-preview isolation.
- Browser: `pnpm exec playwright test` passes 48 runs across the 1280, 1920,
  and 2560 viewport projects, including the queue drawer's four sections,
  truthful Autoplay empty state, and accessible drag handles.
- Native: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
  all-features Clippy with warnings denied, and `cargo test --all-targets`
  pass. The final suite reports 318 unit tests plus one synthetic `mpv_smoke`
  integration test; explicit real mpv smoke also passes.
- Focused Plan 08 coverage includes schema v4-to-v5 preservation, playlist
  duplicates/order and Inbox idempotence, branch base/revision/one-shot merge
  behavior, collection normalization and batch bounds, queue section policy,
  movement/pinning/clear, Later-only shuffle, snapshot immutability, restart
  restore, and saved-position resume.
- Runtime: `pnpm tauri build`, the regular packaged playback/restart/cleanup
  smoke, and the explicit packaged Plan 08 playlist/collection/queue/snapshot/
  restart smoke pass. The optional missing-yt-dlp provider-search race and live
  provider/download smoke were intentionally not run.
- Storage: schema version 5 adds durable playlist/collection/queue/snapshot
  tables with foreign-key and ownership checks. Build output remains at
  `C:\CargoTarget\SpotDIY`; no repository-local `src-tauri\target`, media,
  credentials, tokens, or raw provider payloads are retained.

## Plan 09 verification

- Frontend: `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build` pass;
  Vitest reports 56 tests across 16 files, including lyrics DTOs, local-first
  source actions, synchronized cue selection, bookmark and A/B controls.
- Browser: `pnpm exec playwright test` passes 48 runs across the 1280, 1920,
  and 2560 viewport projects.
- Native: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
  all-features Clippy with warnings denied, and `cargo test --all-targets`
  pass. The final suite reports 337 unit tests plus one synthetic `mpv_smoke`
  integration test; explicit real mpv smoke also passes.
- Focused Plan 09 coverage includes schema v5-to-v6 preservation, LRC
  timestamp variants and bounds, malformed-line fallback, embedded plain/SYLT
  metadata, local/manual/provider precedence, LRCLIB validation/rate limits,
  cache behavior, bookmark validation/persistence, loop bounds, source/recovery
  restoration, and new-track clearing.
- Runtime: `pnpm tauri build`, the regular packaged playback/restart/cleanup
  smoke, the packaged Plan 08 persistence smoke, and the packaged Plan 09
  lyrics/bookmark/A-B/preset/restart/queue/no-autoplay smoke pass. No owned
  mpv process remained. Live LRCLIB smoke was optional and skipped.
- Storage: schema version 6 adds `lyrics`, `bookmarks`, and `ab_loop_presets`;
  local reads are read-only, media and raw provider payloads are not retained,
  build output remains at `C:\CargoTarget\SpotDIY`, and
  `src-tauri\target` remains absent.

## Plan 10 verification

- Frontend: `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build` pass.
  Vitest reports 70 tests across 18 files. Lint has three non-fatal Fast Refresh
  warnings; build has non-fatal Browserslist, Tailwind content, and chunk-size
  notices.
- Browser: `pnpm exec playwright test` passes 51 tests across the 1280, 1920,
  and 2560 viewport projects. Design coverage verifies Dark/Light/Custom theme
  controls, invalid import recovery, export/reset, all layout profiles, focus,
  keyboard and pointer context actions, reduced motion, responsive overflow,
  long-content handling, and the 1080-pixel height guard. Screenshots are
  written to Playwright output paths and are not repository artifacts.
- Native: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
  all-features Clippy with warnings denied, and
  `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` pass. The
  final run reports 343 Rust unit tests plus one synthetic mpv integration test.
- Runtime: `pnpm tauri build` passes with release output under external
  `C:\CargoTarget\SpotDIY`. The packaged settings smoke proves startup,
  default Dark/Comfortable values, Dark/Light/Custom writes, all three layout
  profiles, custom-theme persistence across restart, reset, and clean close.
  The packaged Plan 09 playback/lyrics persistence smoke also passes and leaves
  no owned mpv process.
- Storage and graphs: schema version remains 6; `layout_profile` and
  `custom_theme` are ordinary settings keys and no migration 7 was added.
  CodeGraph was refreshed once and is up to date at 138 files, 4,655 nodes,
  and 17,004 edges. Graphify was refreshed once at 4,023 nodes, 7,913 edges,
  and 245 communities. Repository-local `src-tauri\target` remains absent.

## Plan 11 verification

- Migration: the old-constraint schema-6 fixture, Plan-10-shaped schema-6
  fixture, and fresh database all migrate to schema 7. Existing settings are
  retained, new appearance keys work afterward, custom theme activation works,
  and foreign-key checks remain clean.
- Frontend: `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build` pass;
  Vitest reports 73 tests across 19 files. Lint retains three pre-existing Fast
  Refresh warnings. Build notices for Browserslist, Tailwind content, an
  ineffective dynamic import, and a large chunk are non-fatal.
- Browser: `pnpm exec playwright test` passes 63 tests across the 1280, 1920,
  and 2560 viewport projects. Plan 11 coverage includes populated Home,
  persisted and ephemeral inspectors, player modes, Escape coordination, and
  source-aware shell actions; existing design/search/playback coverage remains
  green.
- Native: fmt, all-features Clippy with warnings denied, and all-target tests
  pass: 347 Rust unit tests plus one passing real-mpv integration test.
- Runtime: `pnpm tauri build` passes with release output under external
  `C:\CargoTarget\SpotDIY`. The explicit real mpv smoke, packaged Plan 09
  playback/lyrics persistence smoke, and packaged Plan 11 migration/shell/
  restart smoke all pass with clean owned-process shutdown.
- Graphs and safety: final Graphify output is 4,131 nodes, 8,235 edges, and
  247 communities; CodeGraph was refreshed once for the shell/player/inspector
  dependency query. No online playback, Spotify download, provider behavior,
  media mutation, secret, or repository-local `src-tauri\target` is included.
