# SpotDIY test matrix

| Area | Current check | Required expansion |
|---|---|---|
| Frontend type safety | `pnpm typecheck` | Strict DTO coverage for every IPC command. |
| Frontend lint | `pnpm lint` | Keep no unused values and hook rules clean. |
| Frontend behavior | `pnpm test` — 4 files, 18 tests | Browser-preview defaults, provider labels, native status/settings validation, library dialog/page/progress behavior, pagination, quality, unavailable rows, removal confirmation, and playback exclusion. |
| Frontend build | `pnpm build` | Browser production bundle and Tauri build wiring. |
| Rust formatting | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | Run on every native slice. |
| Rust quality | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | Passed after Plan 03 scanner/watcher/recovery changes. |
| Rust behavior | `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` — 53 tests | Migration 2/legacy promotion, WAL/FK, path/reparse safety, metadata/artwork/fingerprint helpers, recursive scanner, unchanged/forced/rename/missing/restore/ambiguous identity behavior, watcher coalescing/recovery, service paging/reveal, repositories, settings, and IPC status. |
| Provider contracts | Not started | Mock malformed responses, timeouts, cancellation, rate limits, missing metrics/artwork. |
| Visual QA | Browser runner unavailable; native CDP smoke passed | `pnpm exec playwright --version` returns 1.58.0, but `pnpm exec playwright test --list` returns `unknown command 'test'`; no Playwright config/project is present. The packaged native window was exercised through its CDP endpoint. |
| Release | `pnpm tauri build` and packaged launch smoke | x64 release executable and NSIS installer build; executable startup creates the standard LocalAppData database. Synthetic native smoke also proves restart persistence, watcher changes, reveal validation, folder removal, and media preservation. Full clean install, playback, search, import/export, and visual screenshot QA remain later. |
