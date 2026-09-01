# SpotDIY test matrix

| Area | Current check | Result / coverage |
|---|---|---|
| Frontend type safety | `pnpm typecheck` | Pass. Strict playback/library/settings DTOs and command arguments are checked. |
| Frontend lint | `pnpm lint` | Pass. |
| Frontend behavior | `pnpm test` | Pass: 26 Vitest tests, including typed playback IPC, stale revisions, bridge ordering/race handling, PlayerBar, local-library actions, and Ctrl+K transport. |
| Frontend build | `pnpm build` | Pass. Existing Browserslist/Tailwind content notices are non-fatal. |
| Rust formatting | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | Pass. |
| Rust quality | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | Pass. |
| Rust behavior | `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` | Pass: 117 tests, including playback contracts, protocol framing/correlation, queue policy, managed source resolution, state races/recovery, and library behavior. |
| Browser playback | `pnpm exec playwright test` | Pass: 9 runs across 1280, 1920, and 2560 viewport projects; includes source-switch queue identity preservation and no console errors. |
| Real mpv | Windows `mpv_smoke` integration gate | Pass with local `v0.41.0-dev-g41f6a6450`; synthetic WAV transport, device, EOF, shutdown, and process-exit behavior verified. |
| Packaged lifecycle | `scripts/packaged-playback-smoke.ps1` | Pass with release executable: local indexing, playback transport, graceful close, owned-mpv cleanup, restart persistence, and empty transient queue. |
| Release | `pnpm tauri build` | Pass. Release executable and NSIS bundle built with external `C:\CargoTarget\SpotDIY`; no repository-local `src-tauri\target`. |
| Provider contracts | Plan 05 native adapter/search tests and metadata smoke | Pass: 250 Rust tests include provider registry, local search, yt-dlp adapters, Spotify PKCE/error mapping, timeouts, cancellation, sorting, and cache bounds; opt-in YouTube/SoundCloud metadata smoke returned 25 entries each. |
| Persistence boundary | Schema and restart smoke | Pass: Plan 04 adds no migration; library roots persist while the transient playback queue restarts empty. |
| Visual follow-up | Playwright screenshots | The matrix exercises responsive widths, long titles, missing artwork, and accessible controls; later full visual/design QA remains part of the UI/release plans. |

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
