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

## Fix round 1: production lifecycle coverage

Replaced the synthetic probe fixtures and fake runner with controlled real-child tests. The test binary launches itself by direct executable path with an exact helper-test filter; helper tests are inert during normal test runs and only emit, block, or exit nonzero in that controlled child process. No shell script, shell command, or additional dependency is used.

- `bounded_probe_rejects_oversized_stdout` and `bounded_probe_rejects_oversized_stderr` invoke `run_bounded_probe` against children that write more than the 64 KiB probe cap.
- `bounded_probe_times_out_and_reaps` invokes `run_bounded_probe` against a genuinely blocking child, asserts `Timeout`, and confirms return in under one second after the 100 ms test deadline.
- Invalid UTF-8, malformed-version, and nonzero-exit cases now obtain bytes and exit status from `run_bounded_probe`, then execute the production yt-dlp classification path. The invalid-byte test asserts the replacement character is present after lossy decoding.
- `runner_cancellation_kills_and_reaps_owned_child` invokes `TokioYtDlpProcessRunner` with a genuinely blocking controlled child, cancels after it has been allowed to start, asserts `Cancelled`, and bounds completion to one second.
- The argv tests inspect `yt_dlp_command`, the production `Command::new`/`.args` builder used by the Tokio runner; they no longer rely on a hardcoded `shell_invoked()` result.

### Fix-round verification

```powershell
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml --lib media_tools:: -- --nocapture
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

All commands exited 0. The focused command ran 26 media-tool and runner tests with 0 failures; clippy passed with warnings denied; formatting passed. `src-tauri/target` remains absent.

### Fix-round self-review

- Test lifecycle assertions now cross real `Command::new` process boundaries and production `kill`/`wait` paths rather than setting synthetic flags.
- The production runner still uses direct `Command::new` plus `.args`, concurrent bounded readers, lossy decoding after bounds checks, and owned-child-only cleanup.
- The test-only child protocol is confined to `#[cfg(test)]`; production behavior and dependencies are unchanged.

## Fix round 2: controlled-child marker isolation

Controlled helper tests now require the explicit `SPOTDIY_TASK2_CONTROLLED_CHILD` marker with their module-specific expected value in addition to the exact helper-test arguments. The probe test parent supplies its marker only through the internal bounded-probe command construction path; the Tokio runner parent supplies its marker only through the test-only runner configuration before the production command is spawned. Ordinary test execution and direct `--exact` helper runs do not set either marker, so every blocking/overflow helper returns immediately.

Both modules include regression tests that verify an otherwise matching exact invocation is rejected when the marker is absent or wrong and accepted only with the expected marker.

### Fix-round 2 verification

```powershell
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml --lib media_tools:: -- --nocapture
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml --lib media_tools::tests::controlled_probe_blocks -- --exact --nocapture
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo test --manifest-path src-tauri/Cargo.toml --lib media_tools::yt_dlp::tests::controlled_runner_blocks -- --exact --nocapture
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
$env:CARGO_TARGET_DIR='C:\CargoTarget\SpotDIY'; cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

All commands exited 0. The focused suite ran 28 tests with 0 failures. Each direct exact helper invocation ran one test and returned immediately, confirming the absent marker leaves it inert. Clippy passed with warnings denied, formatting passed, and `src-tauri/target` remains absent.

### Fix-round 2 self-review

- The marker is injected only on the controlled child command; normal production probe and runner invocations receive no test environment setting.
- Both controlled helper gates require the expected marker as well as the exact helper-test arguments, preventing an ordinary direct exact run from entering an infinite loop.
- The existing real-child coverage still uses direct `Command::new`/`.args`, concurrent bounded drains, lossy invalid-byte decoding, and owned-child `kill`/`wait` cleanup.
