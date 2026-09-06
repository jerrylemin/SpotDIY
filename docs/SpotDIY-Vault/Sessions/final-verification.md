# Plan 16 final verification

Date: 2026-09-04. Release candidate: SpotDIY `0.1.0`.

## Status

`PARTIAL`. The source repair, exact pinned CI native/frontend/package jobs,
local external-target Tauri/NSIS build, packaged artifact, clean
install/uninstall, and functional packaged feature smokes pass. The broad
packaged process-tree playback budget fails, and native VisualExplorer SQL and
timed packaged render-readiness budgets were not measured. No completion claim
is made.

## Fixed blockers

- Preview stop now resets stale `Playing` state, and stale reaper generations
  cannot overwrite a newer preview state.
- Visual genre aggregation is deterministic; native capability positives and
  metadata-only negatives are covered.
- Rust and CI use exact `1.98.1-x86_64-pc-windows-msvc`.
- `windows 0.62.2` is listed in third-party notices.
- CSP removes arbitrary remote `connect-src` while retaining remote artwork
  image loading.
- The packaged search harness now waits for `start_search` before cancelling
  and accepts a naturally completed search's `null` cancellation result.

## Source and CI evidence

- Historical repair commit: `39b79bc63396897b6ddfaf81cce3cb2bd3180c2a`.
- Current working tree is based on `eceabb3c4a898c8de3abf409811e60eb78cc9171`;
  no commit or push was made for the runtime-usability repair.
- GitHub Actions run `33769072435` completed `success` for that exact SHA.
  Rust, Frontend, and Package jobs all passed; Rust fmt, Clippy, tests,
  RustSec audit, typecheck, lint, Vitest, build, Playwright, and NSIS upload
  all ran in the green run.
- `pnpm typecheck`, zero-warning ESLint, `pnpm test` (26 files / 95 tests),
  `pnpm build`, and `pnpm exec playwright test` (82/82) pass locally.
- Local pinned `cargo fmt --all -- --check`, strict all-features Clippy, and
  all-target Rust tests (457 unit tests plus the real-MPV integration test)
  pass with the external Cargo target.
- `pnpm audit --audit-level high` and `pnpm audit --prod --audit-level
  moderate` pass with no known vulnerabilities. CI `cargo audit` passes.
- `cargo metadata --manifest-path src-tauri/Cargo.toml --locked` reports 596
  packages and the dependency notice index is current.

## Release artifact and install

- Artifact: `spotdiy-nsis-39b79bc63396897b6ddfaf81cce3cb2bd3180c2a`, GitHub
  artifact ID `9899808630`, ZIP size `6,472,041` bytes.
- Installer: `SpotDIY_0.1.0_x64-setup.exe`, `6,489,236` bytes.
- SHA-256:
  `D52A17EF5A69F514DFE20C98EAD904543F8FA18599FE0DE74CAFB3B62ACA95CB`.
- `Get-AuthenticodeSignature`: `NotSigned`; no certificate or signing step
  was attempted.
- The current local external-target build produced the same-named installer at
  `C:\CargoTarget\SpotDIY\release\bundle\nsis\`: 6,501,279 bytes, SHA-256
  `0DA477FE2B2163130484DD8346D14871564DB1FC13422D8B2A39F3D2FBC325A4`,
  `NotSigned`.
- Silent clean install exited `0`; the installed executable was exercised.
  Silent uninstall exited `0`, removed the isolated install root, and left
  zero SpotDIY-owned packaged processes.

## Packaged functional evidence

Using the exact CI executable, these passed with app exit code `0` and owned
process cleanup: regular playback/restart; Plan 08 playlists, collections,
queue, and snapshot persistence; Plan 09 lyrics, bookmarks, A/B loop, preset,
queue, restart, and no-autoplay; Plan 11 schema migration, shell, inspector,
appearance, queue, lyrics, restart, and no-autoplay; Plan 12 schema migration,
SMTC, tray, shortcut, overlays, click-through recovery, output profiles, and
restart; Plan 13 Standard/Portable transitions; Plan 14 history, sessions,
Private Session, Temporary Mode, smart playlist preview/mix, analytics, and
restart; and Plan 15 visual routes, native dataset contract, local preview,
Theme Studio, and restart isolation.

The Plan 11/12/14 legacy database fixtures used a temporary Python standard-
library SQLite command shim because `sqlite3.exe` is not installed on the
host. The shim was verification-only, was removed, and added no dependency or
repository artifact. The exact package search smoke also passed local search,
provider failure isolation, Spotify gating, and cancellation boundary.

## Provider, download, and privacy boundaries

- Local search and provider isolation pass in the packaged smoke. Spotify
  search is ready when the local `spotdl` runtime is present; YouTube/SoundCloud
  are explicit missing-`yt-dlp` states in the isolated profile.
- Live YouTube/SoundCloud metadata was not run as part of the acceptance
  fixture. Direct `spotdl` metadata/source-resolution smoke was verified in the
  current checkout; live audio download is `SKIPPED — no approved legal
  live-download fixture`.
- Structured process arguments, path containment, archive validation,
  symlink/reparse rejection, staged rollback, and cleanup ownership are
  covered by the CI Rust suite and packaged storage checks.
- No tracked secrets, runtime database, archive, real media, or generated
  diagnostic artifact was found. Analytics, Private Session, Temporary Mode,
  visual exploration, and preview retain their documented local-only
  boundaries.

## Performance

Frontend layout proxies pass: Music Map median/p95 `123.83/154.02 ms` and
Galaxy median/p95 `14.83/48.24 ms`. Five fresh-profile cold launches pass at
median `0.541 s`, p95/max `0.624 s`.

The full process-tree idle sample was `6.56%` CPU / `448.8 MiB`; the parent
alone was `2.50%` / `40.0 MiB`. The 60-second local playback sample reached
`60059 ms` and peaked at `57.81%` / `522.5 MiB` across SpotDIY, WebView2, and
owned mpv. These exceed the requested `2% / 350 MiB` idle and `10% / 450 MiB`
playback budgets. Native 5,000-track SQL and timed packaged render readiness
remain unmeasured. See
`docs/SpotDIY-Vault/Research/performance-baseline.md`.

## Repository and handoff

- SQLite schema is `11`; `.spotdiy` format is `1`; migrations 9-to-11 preserve
  existing settings rows and foreign keys, and no updater, tag, GitHub Release,
  or Gmail message was created.
- `CARGO_TARGET_DIR` remains external and repository-local
  `src-tauri\target` is absent. `memory.zip` is absent. The actual checkout
  began clean apart from tracked history; no claimed unrelated `.gitignore`
  or Plan 05 deletions were present.
- `graphify update .` is rerun for the current source state; CodeGraph remains
  unavailable.
- Full 27-group evidence is in
  `docs/SpotDIY-Vault/Sessions/full-feature-acceptance.md`.
