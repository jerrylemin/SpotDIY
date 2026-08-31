# Task 2 Report: yt-dlp media tool runner

## Result

Implemented the backend-only yt-dlp status lifecycle and a bounded Tokio process runner. yt-dlp discovery uses the exact priority test override, `SPOTDIY_YTDLP_PATH`, then `PATH`; versions below `2026.08.19` are classified as unsupported. The runner invokes the executable directly with structured arguments, retains at most 4 MiB of stdout and 256 KiB of stderr, enforces a 15-second deadline, decodes output with replacement characters, and kills, waits for, and reaps its owned child on cancellation, timeout, overflow, and process-check failures.

The existing mpv probe now shares the bounded synchronous probe implementation while retaining its 3-second and 64 KiB behavior.

## RED

The brief-prescribed command was run with the required external target:

```powershell
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml media_tools::tests sources::yt_dlp::tests -- --nocapture
```

It exited 1 before compilation because Cargo accepts one test-name filter: `error: unexpected argument 'sources::yt_dlp::tests' found`.

The valid equivalent focused RED command was then run:

```powershell
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml media_tools::tests -- --nocapture
```

It exited 1 with the expected missing-API errors, including absent `RecordingRunner`, `yt_dlp_search_args`, `TokioYtDlpProcessRunner`, `YtDlpProcessError`, `MediaToolManager::with_yt_dlp_override`, and bounded-probe helpers.

## GREEN

Focused media and runner tests:

```powershell
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml --lib media_tools:: -- --nocapture
```

Passed: 19 tests, 0 failed.

Full task-adjacent verification:

```powershell
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml --all-targets
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

Passed: 148 unit tests and 1 mpv smoke test; clippy exited 0 with warnings denied; formatting exited 0. `src-tauri/target` remained absent.

## Files changed

- `src-tauri/src/media_tools/mod.rs`
- `src-tauri/src/sources/yt_dlp.rs`

`src-tauri/Cargo.toml` and `src-tauri/Cargo.lock` required no changes because the existing Tokio feature set already includes process, I/O, time, synchronization, and runtime support.

## Self-review

- Process launch sites use `Command::new` with `.args`; no shell program, command-line interpolation, `sh`, `cmd`, or `start` invocation is used.
- Both probe streams are read concurrently and bounded before conversion or storage; the yt-dlp runner also reads stdout and stderr concurrently and applies independent bounds before lossy UTF-8 conversion.
- Cancellation, timeout, and overflow terminate and await only the child created by the runner/probe.
- Executable paths remain in non-serialized backend status structures and do not enter the existing frontend-facing diagnostics.
- Existing mpv health/discovery tests and the full test suite remain green.

## Concern

Task 4 owns `src-tauri/src/sources/mod.rs`, so Task 2 compiles the new runner through an owned path module under `media_tools`. Task 4 should re-export that existing module from `sources` when it wires the YouTube and SoundCloud adapters, rather than creating a second copy of the runner types.
