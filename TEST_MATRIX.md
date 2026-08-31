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
| Provider contracts | Not started | Plan 05+; no provider playback/search is part of Plan 04. |
| Persistence boundary | Schema and restart smoke | Pass: Plan 04 adds no migration; library roots persist while the transient playback queue restarts empty. |
| Visual follow-up | Playwright screenshots | The matrix exercises responsive widths, long titles, missing artwork, and accessible controls; later full visual/design QA remains part of the UI/release plans. |
