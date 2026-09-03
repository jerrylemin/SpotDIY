# SpotDIY third-party notices

Release-candidate inventory date: 2026-09-03.

## Project license

SpotDIY does not declare a project license in this repository. No root
`LICENSE` file was added.

## Frontend production dependencies

These packages are bundled into the generated frontend assets. Versions and
license expressions are taken from the installed package metadata; source
links are the package project links.

| Package | Version | License | Project | Distribution |
|---|---:|---|---|---|
| `@dnd-kit/core` | 6.3.1 | MIT | [clauderic/dnd-kit](https://github.com/clauderic/dnd-kit) | bundled |
| `@dnd-kit/sortable` | 10.0.0 | MIT | [clauderic/dnd-kit](https://github.com/clauderic/dnd-kit) | bundled |
| `@dnd-kit/utilities` | 3.2.2 | MIT | [clauderic/dnd-kit](https://github.com/clauderic/dnd-kit) | bundled |
| `@tanstack/react-query` | 5.102.8 | MIT | [TanStack/query](https://github.com/TanStack/query) | bundled |
| `@tanstack/react-router` | 1.170.32 | MIT | [TanStack/router](https://github.com/TanStack/router) | bundled |
| `@tauri-apps/api` | 2.11.1 | Apache-2.0 OR MIT | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | bundled |
| `@tauri-apps/plugin-dialog` | 2.7.2 | MIT OR Apache-2.0 | [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | bundled |
| `@tauri-apps/plugin-opener` | 2.5.4 | MIT OR Apache-2.0 | [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | bundled |
| `motion` | 13.1.1 | MIT | [motiondivision/motion](https://github.com/motiondivision/motion) | bundled |
| `react` | 19.2.8 | MIT | [facebook/react](https://github.com/facebook/react) | bundled |
| `react-dom` | 19.2.8 | MIT | [facebook/react](https://github.com/facebook/react) | bundled |
| `zod` | 4.5.4 | MIT | [colinhacks/zod](https://github.com/colinhacks/zod) | bundled |
| `zustand` | 5.0.15 | MIT | [pmndrs/zustand](https://github.com/pmndrs/zustand) | bundled |

Development-only tooling, including Playwright, axe-core, Vitest, ESLint,
TypeScript, and the Tauri CLI, is not shipped in the application bundle.

## Rust production dependencies

These crates are compiled into the native application unless noted otherwise.
The expressions below are the published metadata returned by
`cargo metadata --locked --format-version 1`.

| Crate | Version | License | Project/source | Distribution |
|---|---:|---|---|---|
| `async-trait` | 0.1.92 | MIT OR Apache-2.0 | [dtolnay/async-trait](https://github.com/dtolnay/async-trait) | native |
| `base64` | 0.23.1 | MIT OR Apache-2.0 | [marshallpierce/rust-base64](https://github.com/marshallpierce/rust-base64) | native |
| `chrono` | 0.4.45 | MIT OR Apache-2.0 | [chronotope/chrono](https://github.com/chronotope/chrono) | native |
| `keyring` | 4.2.0 | MIT OR Apache-2.0 | [open-source-cooperative/keyring-rs](https://github.com/open-source-cooperative/keyring-rs) | native |
| `lofty` | 0.25.1 | MIT OR Apache-2.0 | [Serial-ATA/lofty-rs](https://github.com/Serial-ATA/lofty-rs) | native |
| `notify` | 8.2.0 | CC0-1.0 | [notify-rs/notify](https://github.com/notify-rs/notify) | native |
| `rand` | 0.9.5 | MIT OR Apache-2.0 | [rust-random/rand](https://github.com/rust-random/rand) | native |
| `regex` | 1.13.1 | MIT OR Apache-2.0 | [rust-lang/regex](https://github.com/rust-lang/regex) | native |
| `reqwest` | 0.13.4 | MIT OR Apache-2.0 | [seanmonstar/reqwest](https://github.com/seanmonstar/reqwest) | native |
| `rusqlite` | 0.40.2 | MIT | [rusqlite/rusqlite](https://github.com/rusqlite/rusqlite) | native; bundled SQLite |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | [serde-rs/serde](https://github.com/serde-rs/serde) | native |
| `serde_json` | 1.0.151 | MIT OR Apache-2.0 | [serde-rs/json](https://github.com/serde-rs/json) | native |
| `sha2` | 0.11.0 | MIT OR Apache-2.0 | [RustCrypto/hashes](https://github.com/RustCrypto/hashes) | native |
| `strsim` | 0.11.1 | MIT | [rapidfuzz/strsim-rs](https://github.com/rapidfuzz/strsim-rs) | native |
| `tauri` | 2.11.5 | Apache-2.0 OR MIT | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | native |
| `tauri-plugin-dialog` | 2.7.2 | Apache-2.0 OR MIT | [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | native |
| `tauri-plugin-global-shortcut` | 2.3.2 | Apache-2.0 OR MIT | [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | native |
| `tauri-plugin-opener` | 2.5.4 | Apache-2.0 OR MIT | [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | native |
| `thiserror` | 2.0.20 | MIT OR Apache-2.0 | [dtolnay/thiserror](https://github.com/dtolnay/thiserror) | native |
| `tokio` | 1.53.1 | MIT | [tokio-rs/tokio](https://github.com/tokio-rs/tokio) | native |
| `unicode-normalization` | 0.1.25 | MIT OR Apache-2.0 | [unicode-rs/unicode-normalization](https://github.com/unicode-rs/unicode-normalization) | native |
| `url` | 2.5.8 | MIT OR Apache-2.0 | [servo/rust-url](https://github.com/servo/rust-url) | native |
| `uuid` | 1.26.0 | Apache-2.0 OR MIT | [uuid-rs/uuid](https://github.com/uuid-rs/uuid) | native |
| `walkdir` | 2.5.0 | Unlicense OR MIT | [BurntSushi/walkdir](https://github.com/BurntSushi/walkdir) | native |
| `zip` | 8.6.0 | MIT | [zip-rs/zip2](https://github.com/zip-rs/zip2) | native |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 | [microsoft/windows-rs](https://github.com/microsoft/windows-rs) | Windows native |
| `windows` | 0.62.2 | MIT OR Apache-2.0 | [microsoft/windows-rs](https://github.com/microsoft/windows-rs) | Windows native helper |

`spotdiy-windows-smtc` is a project-owned path dependency and has no
third-party license expression. `tauri-build` 2.6.3 is build-time tooling and
is not shipped in the application. The complete transitive graphs remain
authoritatively pinned by `pnpm-lock.yaml` and `src-tauri/Cargo.lock`; this
file intentionally indexes permissive notices instead of duplicating hundreds
of standard license texts.

## External Windows/media runtimes

The current Tauri bundle does not vendor arbitrary media binaries or a
WebView2 installer. These are managed runtime prerequisites and are not
included in the NSIS artifact unless a future packaging change explicitly
adds them.

| Runtime | Baseline/source | License/terms | Current distribution status |
|---|---|---|---|
| mpv | [mpv](https://github.com/mpv-player/mpv); the historical local smoke binary was `v0.41.0-dev-g41f6a6450` | GPL-2.0-or-later | external managed tool; exact binary notice required before vendoring |
| FFmpeg | [ffmpeg.org](https://ffmpeg.org/) | depends on the selected build configuration (LGPL/GPL options) | external managed tool; exact binary/build configuration required before vendoring |
| yt-dlp | [yt-dlp](https://github.com/yt-dlp/yt-dlp); setup baseline `2026.08.19` | Unlicense plus notices for bundled third-party components | external managed tool; not bundled |
| WebView2 Evergreen Runtime | [Microsoft WebView2](https://learn.microsoft.com/microsoft-edge/webview2/) | Microsoft runtime terms | OS/runtime prerequisite; not bundled by current Tauri config |

No exact external binary is copied into this repository or release artifact by
the current configuration. If distribution changes to vendor one, rerun the
inventory against that exact binary and include its upstream notices before
calling the release complete.

## Audit provenance

The inventory was checked with installed frontend package metadata and
`cargo metadata --locked --format-version 1`. JavaScript audit commands passed
locally at the configured thresholds; RustSec could not be completed on this
machine because the local MSVC installation is missing `msvcrt.lib` (see
`docs/SpotDIY-Vault/Sessions/final-verification.md`).
