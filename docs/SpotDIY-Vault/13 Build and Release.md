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
