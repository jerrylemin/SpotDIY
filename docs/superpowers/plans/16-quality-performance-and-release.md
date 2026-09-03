# Plan 16 — quality, performance, and release

## Goal

Complete risk-based QA, performance baselines, accessibility/security audits, third-party notices, clean-state verification, Windows installer packaging, and final documentation.

## Dependencies

All feature plans and a stable release candidate.

## Exact files

`.github/workflows/ci.yml`, `scripts/**`, `tests/e2e/**`, `docs/SpotDIY-Vault/Research/performance-baseline.md`, `docs/SpotDIY-Vault/Sessions/final-verification.md`, `THIRD_PARTY_NOTICES.md`, `README.md`, `setup_and_run.md`, and release configuration.

## Interfaces consumed

Packaged Tauri application, all service contracts, CI toolchain, and clean Windows profile.

## Interfaces produced

Installer artifact, evidence logs, performance metrics, security review, dependency audit, and release checklist.

## Tests

Full frontend/Rust/CI suite, Playwright mocked-IPC screenshots, Tauri launch smoke test, clean install, local playback, provider/search/download smoke, import/export, overlay, restart, and portable-mode verification.

## Acceptance criteria

No unverified PASS claims, no known load-bearing failures, no secrets/copyrighted fixtures, clean Git tree, and documented installer/data paths.

## Commit boundary

`chore: verify and package SpotDIY release`

## Implementation record — 2026-09-03

Plan 15 release blockers were repaired in the working tree: PreviewService
now serializes preview and all normal audio-transition entrypoints through a
shared gate; visual artist/album identity is stable and capability flags are
native-derived; and visual actions remain truthful for metadata-only tracks.

The release workflow now uses full action SHAs, exact Node/pnpm pins, frozen
installs, zero-warning lint, frontend/Rust/audit/package jobs, and NSIS-only
artifact upload. The three Fast Refresh warnings were fixed, secondary routes
were lazy-loaded after the measured large-chunk warning, and axe/keyboard
coverage plus a deterministic 5,000-track layout benchmark were added.

Current evidence is `PARTIAL`: frontend/browser/a11y/JS-audit/layout gates
pass, but native compilation, RustSec, Tauri packaging, clean install, and
packaged acceptance are blocked by missing MSVC headers/libraries. The primary
worktree intentionally retains the three unrelated pre-existing changes.
No migration 10, updater, tag, GitHub Release, root LICENSE, or reviewer loop
was added. Plan 16 is committed and pushed as `5dfdd1e`.

## Final execution record — 2026-09-03

The release blockers were repaired in `39b79bc63396897b6ddfaf81cce3cb2bd3180c2a`:
preview stale-state/reaper ordering, deterministic visual genre ordering,
capability-positive coverage, exact Rust toolchain alignment, CSP, the
`windows 0.62.2` notice, and the packaged search harness race. The repair SHA
passed GitHub Actions run `33769072435`, including Rust, RustSec, frontend, and
NSIS package jobs.

The exact CI installer was clean-installed, exercised, uninstalled, and used
for regular, provider, Plans 08/09/11/12/13/14/15, and Standard/Portable
packaged smokes. The full 27-group acceptance matrix is recorded in
`docs/SpotDIY-Vault/Sessions/full-feature-acceptance.md`.

Plan 16 remains `PARTIAL`, not complete: the broad process-tree idle and
60-second playback samples exceed their CPU/memory budgets, while native
VisualExplorer SQL and timed packaged render readiness remain unmeasured. No
tag, release, email, reviewer loop, speculative refactor, or Plan 17 was
created.
