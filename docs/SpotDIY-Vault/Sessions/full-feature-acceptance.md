# Plan 16 full feature acceptance

Date: 2026-09-03. This matrix covers the 27 requested feature groups. A
functional `PASS` does not hide a separate performance-budget failure; optional
live-provider and live-download checks are recorded as skipped in the notes.

| Subsystem | Feature | Test level | Evidence | Result | Notes |
|---|---|---|---|---|---|
| 1 | App bootstrap / shell | CI package, packaged smoke, E2E | `scripts/packaged-playback-smoke.ps1 -Plan11Shell`; Playwright 76/76 | PASS | Clean startup and close verified. |
| 2 | Database / migration | CI native, packaged migration | Rust CI `cargo test`; packaged Plan 11/12/14 migration smokes | PASS | Schema 9; no migration 10. Legacy fixtures used a temporary Python stdlib SQLite shim. |
| 3 | Local library | CI native, packaged, E2E | `src-tauri/src/library`; regular and Plan 15 package scans; local package search | PASS | Synthetic WAV indexing, metadata, artwork/path boundaries, and restart persistence pass. |
| 4 | Search providers | CI native/frontend, packaged | `src-tauri/src/search`; `tests/search-*.test.*`; `scripts/packaged-search-smoke.mjs` | PASS | Local/provider isolation, missing yt-dlp states, Spotify gate, and cancellation boundary pass; live upstream checks skipped. |
| 5 | Source fusion / resolution | CI native/frontend | `src-tauri/src/fusion`, `src-tauri/src/sources/resolver.rs`, `tests/fusion-ipc.test.ts` | PASS | Deterministic normalization, matching, overrides, and capability-aware resolution are covered. |
| 6 | Main playback | CI native, 60-second package, E2E | `src-tauri/src/playback`; regular package smoke; 60-second harness reached `60059 ms` | PASS | Playback continuity, seek, pause/resume, shutdown, and owned-mpv cleanup pass. Performance budget is separate and fails. |
| 7 | Persistent queue | CI native/frontend, packaged restart | `src-tauri/src/queue`; Plan 08 package smoke; `tests/playlist-queue-ipc.test.ts` | PASS | Queue ownership, ordering, snapshots, restart, and no-autoplay pass. |
| 8 | Playlists / collections | CI native/frontend, packaged | Plan 08 package smoke; `tests/playlist-queue-ipc.test.ts` | PASS | Playlists, Inbox, collections, likes, ratings, and tags pass. |
| 9 | Playlist branches | CI native | Playlist branch tests in the Rust CI suite and Plan 08 persistence smoke | PASS | One-shot/base/revision and merge behavior covered. |
| 10 | Lyrics | CI native/frontend, packaged | `src-tauri/src/lyrics`; `tests/lyrics-*.test.*`; Plan 09 package smoke | PASS | Local-first lyrics, cues, bookmarks, A/B loop, presets, restart, and no-autoplay pass. |
| 11 | Download manager | CI native/frontend | `src-tauri/src/downloads.rs`; `tests/download-*.test.*`; Rust CI | PASS | Queue, bounded progress, cancel/retry/recovery, provenance, and path safety pass; live download skipped. |
| 12 | Track inspector | CI native/frontend, packaged | `src-tauri/src/inspector`; Plan 11 package smoke; inspector tests | PASS | Local path privacy, provider URL validation, metadata, and capability display pass. |
| 13 | Player modes | Packaged, E2E | Plan 11 package smoke; Playwright shell/player coverage | PASS | Standard, Mini, Expanded, shared snapshot, and keyboard/Escape paths pass. |
| 14 | Windows native integration | CI native, packaged | `src-tauri/src/windows`; Plan 12 package smoke; `tests/playwright/plan12-windows-integration.spec.ts` | PASS | SMTC READY, tray, shortcut, overlays, click-through recovery, output profiles, restart, and cleanup pass. |
| 15 | Backup / restore | CI native, packaged storage | `src-tauri/src/backup`; Rust archive/staging/rollback tests; Plan 13 package smoke | PASS | Format 1 integrity, bounds, path trust, staging, rollback, media/sidecar handling pass. |
| 16 | Standard / Portable storage | Packaged, CI native | `scripts/packaged-backup-storage-smoke.ps1` | PASS | Standard-to-Portable-to-Standard restart transitions, exact roots, markers, and retained DBs pass. |
| 17 | Listening history / analytics | CI native, packaged | `src-tauri/src/analytics`; Plan 14 package smoke; `tests/smart-analytics-ipc.test.ts` | PASS | Qualified plays, sessions, aggregates, and restart persistence pass. |
| 18 | Private session | CI native, packaged | Plan 14 package smoke; analytics privacy tests | PASS | No history/session writes across the private boundary. |
| 19 | Temporary mode | CI native, packaged | Plan 14 package smoke; analytics/privacy tests | PASS | Temporary queue/state behavior and restart boundary pass. |
| 20 | Smart playlists | CI native/frontend, packaged | `src-tauri/src/smart`; Plan 14 package smoke; smart analytics tests | PASS | Validated rules, preview/mix, persistence, and bounded SQL pass. |
| 21 | Smart shuffle | CI native, packaged | Smart shuffle tests; Plan 14 package smoke | PASS | Deterministic non-ML scoring and anti-repetition pass. |
| 22 | Music Map | CI native/frontend, packaged, E2E | `src-tauri/src/visual_explorer`; Plan 15 package smoke; `tests/visual-exploration.test.ts`; Playwright | PASS | Dataset contract, SVG interaction, selection/actions, and bounded layout pass; timed native SQL/render budgets remain unmeasured. |
| 23 | Library Galaxy | CI native/frontend, packaged, E2E | `src-tauri/src/visual_explorer`; Plan 15 package smoke; visual tests; Playwright | PASS | Canvas interaction, navigator, selection/actions, and bounded layout pass; timed render budget remains unmeasured. |
| 24 | Radial / drag actions | Frontend, packaged | Plan 15 package smoke; Playwright visual interaction coverage | PASS | Keyboard radial fallback, drag targets, queue actions, and capability gating pass. |
| 25 | Local preview | CI native, packaged | `src-tauri/src/preview`; Plan 15 package smoke | PASS | Local-only preview, interlock, cancellation, limits, restart isolation, and cleanup pass. |
| 26 | Theme Studio | Frontend, packaged | Plan 15 package smoke; Playwright visual coverage | PASS | 15 tokens, validation, preview/save/reset, import/export, and restart route pass. |
| 27 | Dynamic artwork accent | CI native/frontend, packaged | Theme/accent tests; Plan 15 package smoke; visual E2E | PASS | Bounded 32x32 sampling and contrast fallback pass. |

## Totals

Functional feature rows: `27 PASS`, `0 FAIL`, `0 BLOCKED`, `0 SKIPPED`.

Optional external checks: live YouTube/SoundCloud metadata `SKIPPED` because
`yt-dlp` is unavailable; Spotify `SKIPPED` because authorization is not
available; live download `SKIPPED` because no approved legal fixture exists.

Separate Plan 16 release gates remain `PARTIAL`: broad process-tree idle and
playback performance exceed their budgets, and native VisualExplorer SQL plus
timed packaged render readiness have no measurements. See
`docs/SpotDIY-Vault/Research/performance-baseline.md`.
