# ADR-0009 — External mpv process + JSON IPC playback architecture

Status: accepted
Date: 2026-08-31

## Context

SpotDIY needs dependable local playback on Windows while keeping provider
adapters, the React webview, and the local filesystem outside the media-process
boundary. The playback implementation must remain replaceable, must work when
mpv is absent, and must not expose executable paths, pipe names, request IDs,
raw JSON, or local audio paths to the frontend.

## Decision

Use one external stable `mpv.exe` child per `PlaybackService`. Discover it from
`SPOTDIY_MPV_PATH` first and PATH second. `MediaToolManager` reports `Ready`,
`Missing`, or `Broken` and keeps the executable path backend-only.

`MpvBackend` creates one fresh random Windows named pipe and starts mpv with
exactly:

```text
--no-config --idle=yes --terminal=no --input-terminal=no --audio-display=no --input-ipc-server=<fresh pipe>
```

The backend owns newline-delimited JSON framing with a 1 MiB frame limit,
positive request IDs, reply correlation, interleaved events/replies, and
process-exit monitoring. It observes pause, time-pos, duration, volume, mute,
and seeking at approximately 250 ms and converts them to typed
`BackendEvent` values. FileLoaded is awaited for loads. Quit, kill, and process
reap are bounded by the backend shutdown policy.

`PlaybackService` is the only state mutator. A serialized controller receives
user commands and backend events through `tokio::mpsc` and publishes the
latest `PlaybackSnapshot` through `tokio::watch` and the `playback://state`
event. `PlaybackBackend` is enqueue-only; a bounded worker owns synchronous
mpv request/reply work. Backend generations remain stamped through delivery,
and the controller rejects stale events. All frontend commands carry typed IDs
or values and are validated with strict Zod schemas.

Only managed, indexed, enabled, available local sources can play. Rust
resolves and validates source ownership and current regular-file existence;
the frontend never selects an arbitrary path or URL. Backend generations make
stale events harmless. Disconnect/crash recovery retries after 250, 750, and
1500 ms, then exposes manual retry and `RecoveryExhausted`.

The queue is ID-only and transient. Canonical order and active shuffle order
are separate; repeat, EOF, Previous, Next, Play Now, Play Next, Add to Queue,
and Clear Queue are service policies. Persistent queue state and queue
snapshots are explicitly deferred to Plan 08.

## Dependencies

- Tokio `1.53.1`, direct, with `process`, `net`, `io-util`, `sync`, `time`,
  `rt-multi-thread`, and `macros`, for process/pipe I/O, channels, watches,
  timers, and the serialized backend worker. Tokio is MIT/Apache-2.0.
- `rand` `0.9.5` with `small_rng`, only for Fisher-Yates queue shuffle and OS
  seeding in production. rand is MIT/Apache-2.0.
- `windows-sys` `0.61.2` with the documented Foundation and Threading
  features, only for Windows process/named-pipe constants. windows-sys is
  MIT/Apache-2.0.
- Existing `uuid` supplies queue-entry IDs and the per-process pipe suffix.
  No async-trait, libmpv binding, wrapper crate, ORM, or alternate state
  framework is added.

## Consequences

The process boundary is easy to test with a fake backend and a real synthetic
WAV smoke while preserving a single authoritative state machine. mpv remains
an external runtime dependency and its output-device behavior is platform
dependent; missing/broken health is surfaced without preventing library use.
The transient queue intentionally does not survive restart. Standard data
remains under `%LOCALAPPDATA%\SpotDIY`; the packaged smoke may set the
smoke-only `SPOTDIY_PACKAGED_DATA_ROOT` because Windows known-folder
resolution ignores a child `LOCALAPPDATA` override.

## Verification record

The local development binary is `.tools\mpv\v0.41.0\mpv.exe`, reporting
`v0.41.0-dev-g41f6a6450`, SHA-256
`6145E63F026451A764077D53FD60860EC9F5C2BC76DCD6E62A88967AC375453D`. The
official Windows x64 asset verification is recorded in
`docs/execution/verification-log.md`. The final delivery tip is `af66127`:
117 Rust tests, 26 Vitest tests, 9 Playwright runs, all release quality gates,
real-mpv synthetic-media smoke, and packaged restart/cleanup smoke passed.
