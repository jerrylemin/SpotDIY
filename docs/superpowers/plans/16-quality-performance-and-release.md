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
