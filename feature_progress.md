# SpotDIY feature progress

Statuses are only `NOT STARTED`, `IN PROGRESS`, `BLOCKED`, or `COMPLETE`.
`COMPLETE` requires passing targeted tests and the feature's documented
verification gates.

| Feature | Plan | Status | Last commit | Tests | Notes |
|---|---|---|---|---|---|
| Repository/toolchain bootstrap | 01 | COMPLETE | 403d923 | typecheck, lint, frontend build, Rust fmt/clippy/test, Tauri build | Remote was initialized safely. |
| Tauri/React application shell | 01, 11 | COMPLETE | `15031bf` | 73 Vitest, 63 Playwright, Tauri build, packaged Plan 11 smoke | Real-data Home, persisted/ephemeral inspectors, source-aware actions, command palette, Escape priority, and Standard/Mini/Expanded in-shell player modes are delivered. |
| Unified music domain model | 02 | COMPLETE | 2ec431b | Rust/frontend domain tests | Typed IDs, `UnifiedTrack`, sources, capabilities, and provider identity rules are implemented. |
| SQLite database and migrations | 02, 03, 06, 07, 08, 09, 11, 12 | COMPLETE | `95eb41b` | Migration 7/8 legacy/fresh fixtures, repository and FK tests | SQLite WAL/FK initialization and migrations through schema version 8 are stable; migrations 7 and 8 rebuild only `settings_metadata` and preserve prior rows. |
| Durable application settings | 02 | COMPLETE | 2ec431b | Rust/frontend settings tests | Typed ordinary settings remain separated from future secret storage. |
| Local library indexing | 03 | COMPLETE | Plan 03 delivery | Rust library tests, frontend tests, packaged smoke | Persistent roots, recursive reconciliation, metadata/artwork, watchers, and managed path ownership are implemented. |
| mpv playback engine and transient queue | 04 | COMPLETE | af66127 | 117 Rust tests, 26 Vitest tests, 9 Playwright runs, real mpv smoke, packaged smoke | External mpv JSON IPC, serialized PlaybackService, queue policy, recovery, typed IPC, controls, and review fixes are implemented. |
| Provider adapters and search | 05 | COMPLETE | `ab6169d` | 250 Rust + 38 Vitest + 45 Playwright; native/live/packaged smoke | Concurrent provider sections, strict SearchId lifecycle, Spotify PKCE gate, and bounded no-persistence search boundary. |
| Source Fusion and resolver | 06 | COMPLETE | `afd0149` | 279 Rust + 40 Vitest + 45 Playwright; native/package smoke | Conservative NFKD matcher, durable merge/split overrides, explicit YT/SC acceptance, preference-aware SourceResolver, and typed IPC are delivered. |
| Downloads | 07 | COMPLETE | `6012921` | 308 Rust + 47 Vitest + 45 Playwright; fmt/clippy/build/package and smoke | Persistent yt-dlp/FFmpeg task execution, truthful provenance, progress, concurrency, cancel/retry, restart recovery, safe finalization, typed IPC, and Downloads UI. |
| Playlists, persistent queue, likes, tags | 08 | COMPLETE | `0a62cad` | 318 Rust + 51 Vitest + 48 Playwright; fmt/clippy/build/package and persistence smoke | Durable playlists, seeded Inbox, one-shot branches, likes/ratings/tags, typed collection IPC, PlaybackService-owned queue sections, snapshots, restart restore, and queue drawer are delivered. |
| Lyrics, bookmarks, and A/B loop | 09 | COMPLETE | `7b1a097` | 337 Rust + 56 Vitest + 48 Playwright; parser/provider/package persistence smoke | Local-first LRC/embedded lyrics, explicit LRCLIB, synchronized display, durable bookmarks/presets, and PlaybackService-owned A/B loop are delivered. Waveform generation is not claimed. |
| UI design system | 10 | COMPLETE | `6eb231d` | 70 Vitest, 51 Playwright; typecheck/lint/build | Semantic themes, layout profiles, accessible primitives, context actions, inspector/gallery foundation, and Settings APPEARANCE are delivered. |
| Main-player refinement | 11 | COMPLETE | `15031bf` | 73 Vitest, 63 Playwright, packaged Plan 11 smoke | Source switcher, quality/provenance, Track Inspector, capability-aware actions, Home dashboard, command palette, Escape priority, and three in-shell modes are delivered. |
| Overlays and Windows integration | 12 | COMPLETE | `3d39e1d` | 365 Rust + 78 Vitest + 69 Playwright; fmt/clippy/build/package/smoke | Schema 8 settings, native overlays/tray/shortcuts/SMTC, click-through recovery, output profiles, browser/native/package coverage, and restart persistence are delivered. |
| Import/export and portable mode | 13 | NOT STARTED | — | — | Transactional `.spotdiy` archive and deterministic portable startup. |
| Smart features and analytics | 14 | NOT STARTED | — | — | Local-only listening analytics. |
| Advanced visual exploration | 15 | NOT STARTED | — | — | Music Map, Galaxy, radial menu, Theme Studio. |
| Quality, performance, release | 16 | IN PROGRESS | `3d39e1d` | 365 Rust + 78 Vitest + 69 Playwright; fmt/clippy/build/package/smoke | Plan 12 verification gates pass locally; clean-install and later performance/accessibility work remain. |
