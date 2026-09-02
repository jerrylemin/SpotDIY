# Build and release

CI targets Windows and runs frozen pnpm install, typecheck, lint, frontend
tests/build, Rust fmt, clippy, and tests. Release packaging uses the Tauri
NSIS target. Third-party notices must cover Tauri dependencies and media tools.
GitHub Releases are not published until explicitly requested.

## Plan 04 build cache and smoke

Cargo/Tauri output for the MEGA workspace is redirected to the external
`C:\CargoTarget\SpotDIY` target directory through the user/session
`CARGO_TARGET_DIR` environment variable. No machine-specific Cargo config is
committed and `src-tauri\target` must remain absent.

The Plan 04 release gate is `pnpm tauri build` followed by the gated packaged
smoke script. It runs against the generated release executable, a temporary
isolated profile, and two generated WAV files. The smoke proves local library
indexing, Play/Pause/Seek/Resume/Next, graceful close, owned mpv cleanup,
restart persistence, and an empty transient queue. The local development mpv
binary is `v0.41.0-dev-g41f6a6450`; its SHA-256 and the official Windows asset
verification are recorded in `docs/execution/verification-log.md`.

Plan 04 final delivery used the external target, passed the release build, and
passed both the real synthetic-mpv smoke and the isolated packaged playback/
restart/owned-process cleanup smoke. No repository-local `src-tauri\target`,
SpotDIY-owned `mpv.exe`, temporary profile, or harness database row remained.

## Plan 05 release and packaged search smoke

The Plan 05 release gate uses the same external
`C:\CargoTarget\SpotDIY` target and `pnpm tauri build`. The generated release
executable is exercised by `scripts/provider-search-smoke.ps1 -RunPackaged`
with a temporary data root, synthetic WAV fixtures, missing yt-dlp/mpv paths,
and a WebView2 CDP connection. The smoke confirms Local indexing and search,
independent online-provider failure sections, cancellation, the Spotify gate,
and cleanup of the packaged process and helper processes. The temporary profile
is removed after the run and no repository-local Cargo target is retained.

## Plan 13 release and storage smoke

Plan 13 continues to redirect Cargo/Tauri output to the external
`C:\CargoTarget\SpotDIY` target. `pnpm tauri build` produces the release
executable and NSIS bundle there; `src-tauri\target` remains absent.

The packaged Plan 13 smoke copies the release executable into an isolated
temporary application directory, starts it with a temporary data root, and
drives only typed Tauri storage commands over the packaged WebView bridge. It
proves Standard startup, restart-gated Standard-to-Portable preparation,
Portable startup from the executable marker and `Database` path, exact
portable directories, Portable-to-Standard preparation, marker removal, final
Standard restart, and retention of both databases. Export/import OS dialogs
are intentionally not bypassed by the smoke; archive and restore behavior is
covered by the native suite.
