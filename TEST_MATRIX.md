# SpotDIY test matrix

| Area | Current check | Required expansion |
|---|---|---|
| Frontend type safety | `pnpm typecheck` | Strict DTO coverage for every IPC command. |
| Frontend lint | `pnpm lint` | Keep no unused values and hook rules clean. |
| Frontend behavior | `pnpm test` | Router, command palette, state, accessibility, provider sections. |
| Frontend build | `pnpm build` | Browser production bundle and Tauri build wiring. |
| Rust formatting | `cargo fmt --all -- --check` | Run on every native slice. |
| Rust quality | `cargo clippy --all-targets --all-features -- -D warnings` | Add typed error and service checks. |
| Rust behavior | `cargo test --all-targets` | Migrations, fusion, queue, playback, backup, provider parser tests. |
| Provider contracts | Not started | Mock malformed responses, timeouts, cancellation, rate limits, missing metrics/artwork. |
| Visual QA | Not started | Playwright mocked-IPC screenshots at approved viewports. |
| Release | Not started | Clean Windows install, launch, local playback, source search, import/export, restart. |
