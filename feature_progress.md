# SpotDIY feature progress

Statuses are only `NOT STARTED`, `IN PROGRESS`, `BLOCKED`, or `COMPLETE`.
`COMPLETE` requires passing targeted tests and the feature's documented
verification gates.

| Feature | Plan | Status | Last commit | Tests | Notes |
|---|---|---|---|---|---|
| Repository/toolchain bootstrap | 01 | COMPLETE | 403d923 | typecheck, lint, frontend build, Rust fmt/clippy/test, Tauri build | Remote was initialized safely. |
| Tauri/React application shell | 01, 11 | IN PROGRESS | 403d923 | Vitest, Vite build, Rust test, Playwright and packaged smoke | Routes, truthful empty states, and the Plan 04 player surface exist; later shell polish remains. |
| Unified music domain model | 02 | COMPLETE | 2ec431b | Rust/frontend domain tests | Typed IDs, `UnifiedTrack`, sources, capabilities, and provider identity rules are implemented. |
| SQLite database and migrations | 02, 03, 06, 07 | COMPLETE | `0dbb628` | Rust migration/repository/library/download tests | SQLite WAL/FK initialization and non-destructive migrations through schema version 4 are stable; Plan 07 adds only `downloads` and `download_settings`. |
| Durable application settings | 02 | COMPLETE | 2ec431b | Rust/frontend settings tests | Typed ordinary settings remain separated from future secret storage. |
| Local library indexing | 03 | COMPLETE | Plan 03 delivery | Rust library tests, frontend tests, packaged smoke | Persistent roots, recursive reconciliation, metadata/artwork, watchers, and managed path ownership are implemented. |
| mpv playback engine and transient queue | 04 | COMPLETE | af66127 | 117 Rust tests, 26 Vitest tests, 9 Playwright runs, real mpv smoke, packaged smoke | External mpv JSON IPC, serialized PlaybackService, queue policy, recovery, typed IPC, controls, and review fixes are implemented. |
| Provider adapters and search | 05 | COMPLETE | `ab6169d` | 250 Rust + 38 Vitest + 45 Playwright; native/live/packaged smoke | Concurrent provider sections, strict SearchId lifecycle, Spotify PKCE gate, and bounded no-persistence search boundary. |
| Source Fusion and resolver | 06 | COMPLETE | `afd0149` | 279 Rust + 40 Vitest + 45 Playwright; native/package smoke | Conservative NFKD matcher, durable merge/split overrides, explicit YT/SC acceptance, preference-aware SourceResolver, and typed IPC are delivered. |
| Downloads | 07 | COMPLETE | `6012921` | 308 Rust + 47 Vitest + 45 Playwright; fmt/clippy/build/package and smoke | Persistent yt-dlp/FFmpeg task execution, truthful provenance, progress, concurrency, cancel/retry, restart recovery, safe finalization, typed IPC, and Downloads UI. |
| Playlists, persistent queue, likes, tags | 08 | NOT STARTED | — | — | Persistent queue sections and snapshots belong here. |
| Lyrics and waveform | 09 | NOT STARTED | — | — | Local LRC/embedded first, LRCLIB optional. |
| UI design system and main player | 10, 11 | IN PROGRESS | af66127 | frontend quality gates and Playwright matrix | The functional transport/player slice is delivered; later visual system work remains. |
| Overlays and Windows integration | 12 | NOT STARTED | — | — | Overlay feasibility requires Tauri/Win32 validation. |
| Import/export and portable mode | 13 | NOT STARTED | — | — | Transactional `.spotdiy` archive and deterministic portable startup. |
| Smart features and analytics | 14 | NOT STARTED | — | — | Local-only listening analytics. |
| Advanced visual exploration | 15 | NOT STARTED | — | — | Music Map, Galaxy, radial menu, Theme Studio. |
| Quality, performance, release | 16 | IN PROGRESS | `6012921` | 308 Rust + 47 Vitest + 45 Playwright; fmt/clippy/build/smoke | Plan 07 verification gates pass locally; clean-install and later performance/accessibility work remain. |
