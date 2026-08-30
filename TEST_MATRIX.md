# SpotDIY test matrix

| Area | Current check | Required expansion |
|---|---|---|
| Frontend type safety | `pnpm typecheck` | Strict DTO coverage for every IPC command. |
| Frontend lint | `pnpm lint` | Keep no unused values and hook rules clean. |
| Frontend behavior | `pnpm test` — 5 tests | Browser-preview defaults, provider labels, native status validation, and settings failure boundaries. |
| Frontend build | `pnpm build` | Browser production bundle and Tauri build wiring. |
| Rust formatting | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | Run on every native slice. |
| Rust quality | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | Add typed error and service checks. |
| Rust behavior | `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` — 25 tests | Migration ordering/rollback/backup, WAL/FK, domain invariants, repository round trips/rollback, preferred-source integrity, settings, and IPC status. |
| Provider contracts | Not started | Mock malformed responses, timeouts, cancellation, rate limits, missing metrics/artwork. |
| Visual QA | Not started | Playwright mocked-IPC screenshots at approved viewports. |
| Release | `pnpm tauri build` and packaged launch smoke | x64 release executable and NSIS installer build; executable startup creates the standard LocalAppData database. Full clean install, playback, search, import/export, and restart acceptance remain later. |
