# SpotDIY feature progress

Statuses are only `NOT STARTED`, `IN PROGRESS`, `BLOCKED`, or `COMPLETE`. `COMPLETE` requires passing targeted tests.

| Feature | Plan | Status | Last commit | Tests | Notes |
|---|---|---|---|---|---|
| Repository/toolchain bootstrap | 01 | COMPLETE | 403d923 | typecheck, lint, frontend build, Rust fmt/clippy/test, Tauri build | Remote was empty; origin configured safely. |
| Tauri/React application shell | 01, 11 | IN PROGRESS | 403d923 | Vitest, Vite build, Rust test, Playwright smoke | Routes and truthful empty states exist. |
| Unified music domain model | 02 | COMPLETE | 2ec431b | 5 Rust tests, 5 frontend tests | Typed IDs, `UnifiedTrack`, artists/albums, version qualifiers, source capabilities, and provider identity rules are implemented. |
| SQLite database and migrations | 02 | COMPLETE | 2ec431b | 8 database tests, 5 repository tests | SQLite WAL/FK initialization, ordered migration 1, backup/checkpoint safety, schema constraints, and repository transactions are covered. |
| Durable application settings | 02 | COMPLETE | 2ec431b | 6 Rust tests, 5 frontend tests | Typed settings snapshot, atomic writes, defaults, first-run state, and ordinary/secret boundary are implemented. |
| Local library indexing | 03 | COMPLETE | Plan 03 delivery | 53 Rust tests, 18 frontend tests, native synthetic smoke | Persistent multi-folder roots, recursive incremental/reconciliation scans, Lofty metadata, SHA-256 identity evidence, artwork cache, Notify watcher recovery, typed IPC, and real paged Library UI. |
| mpv playback backend | 04 | NOT STARTED | — | — | Playback adapter must remain behind PlaybackService. |
| Provider adapters and search | 05 | NOT STARTED | — | — | Independent partial results; live calls opt-in in CI. |
| Source Fusion and resolver | 06 | NOT STARTED | — | — | Conservative weighted matcher with merge guards. |
| Downloads | 07 | NOT STARTED | — | — | yt-dlp/FFmpeg provenance and restart recovery. |
| Playlists, queue, likes, tags | 08 | NOT STARTED | — | — | Queue sections and snapshots included. |
| Lyrics and waveform | 09 | NOT STARTED | — | — | Local LRC/embedded first, LRCLIB optional. |
| UI design system and main player | 10, 11 | IN PROGRESS | Plan 03 delivery | frontend tests/build, native Library smoke | Shell identity and player empty state remain; Plan 03 adds the local folder/track states while playback stays disabled for Plan 04. |
| Overlays and Windows integration | 12 | NOT STARTED | — | — | Overlay feasibility requires Tauri/Win32 validation. |
| Import/export and portable mode | 13 | NOT STARTED | — | — | Transactional `.spotdiy` archive. |
| Smart features and analytics | 14 | NOT STARTED | — | — | Local-only listening analytics. |
| Advanced visual exploration | 15 | NOT STARTED | — | — | Music Map, Galaxy, radial menu, Theme Studio. |
| Quality, performance, release | 16 | IN PROGRESS | Plan 03 delivery | Rust 53 tests/fmt/clippy, frontend 18 tests/typecheck/lint/build, Tauri release and native smoke | Plan 03 gates pass; browser Playwright runner configuration, full visual QA, performance profiling, and clean-install validation remain later checks. |
