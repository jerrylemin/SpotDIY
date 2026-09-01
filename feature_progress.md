# SpotDIY feature progress

Statuses are only `NOT STARTED`, `IN PROGRESS`, `BLOCKED`, or `COMPLETE`.
`COMPLETE` requires passing targeted tests and the feature's documented
verification gates.

| Feature | Plan | Status | Last commit | Tests | Notes |
|---|---|---|---|---|---|
| Repository/toolchain bootstrap | 01 | COMPLETE | 403d923 | typecheck, lint, frontend build, Rust fmt/clippy/test, Tauri build | Remote was initialized safely. |
| Tauri/React application shell | 01, 11 | IN PROGRESS | 403d923 | Vitest, Vite build, Rust test, Playwright and packaged smoke | Routes, truthful empty states, and the Plan 04 player surface exist; later shell polish remains. |
| Unified music domain model | 02 | COMPLETE | 2ec431b | Rust/frontend domain tests | Typed IDs, `UnifiedTrack`, sources, capabilities, and provider identity rules are implemented. |
| SQLite database and migrations | 02, 03 | COMPLETE | Plan 03 delivery | Rust migration/repository/library tests | SQLite WAL/FK initialization and migrations through schema version 2 are stable; Plan 04 adds no migration. |
| Durable application settings | 02 | COMPLETE | 2ec431b | Rust/frontend settings tests | Typed ordinary settings remain separated from future secret storage. |
| Local library indexing | 03 | COMPLETE | Plan 03 delivery | Rust library tests, frontend tests, packaged smoke | Persistent roots, recursive reconciliation, metadata/artwork, watchers, and managed path ownership are implemented. |
| mpv playback engine and transient queue | 04 | COMPLETE | af66127 | 117 Rust tests, 26 Vitest tests, 9 Playwright runs, real mpv smoke, packaged smoke | External mpv JSON IPC, serialized PlaybackService, queue policy, recovery, typed IPC, controls, and review fixes are implemented. |
| Provider adapters and search | 05 | COMPLETE | `ab6169d` | 250 Rust + 38 Vitest + 45 Playwright; native/live/packaged smoke | Concurrent provider sections, strict SearchId lifecycle, Spotify PKCE gate, and bounded no-persistence search boundary. |
| Source Fusion and resolver | 06 | NOT STARTED | — | — | Conservative weighted matcher with merge guards. |
| Downloads | 07 | NOT STARTED | — | — | yt-dlp/FFmpeg provenance and restart recovery. |
| Playlists, persistent queue, likes, tags | 08 | NOT STARTED | — | — | Persistent queue sections and snapshots belong here. |
| Lyrics and waveform | 09 | NOT STARTED | — | — | Local LRC/embedded first, LRCLIB optional. |
| UI design system and main player | 10, 11 | IN PROGRESS | af66127 | frontend quality gates and Playwright matrix | The functional transport/player slice is delivered; later visual system work remains. |
| Overlays and Windows integration | 12 | NOT STARTED | — | — | Overlay feasibility requires Tauri/Win32 validation. |
| Import/export and portable mode | 13 | NOT STARTED | — | — | Transactional `.spotdiy` archive and deterministic portable startup. |
| Smart features and analytics | 14 | NOT STARTED | — | — | Local-only listening analytics. |
| Advanced visual exploration | 15 | NOT STARTED | — | — | Music Map, Galaxy, radial menu, Theme Studio. |
| Quality, performance, release | 16 | IN PROGRESS | af66127 | Rust/frontend/browser/Tauri gates and smoke evidence | Plan 04 gates and review pass locally; clean-install and later performance/accessibility work remain. |
