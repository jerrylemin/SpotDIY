# mpv JSON IPC and Windows process management

Date: 2026-08-30
Scope: Current stable mpv control patterns for a Rust/Tauri desktop backend on Windows. This is a research note, not a production implementation.

## Primary sources (URLs)

- [mpv reference manual index](https://mpv.io/manual/index.html) — identifies the stable manual as mpv v0.41.0 at the research date.
- [mpv stable manual](https://mpv.io/manual/stable/) — [using mpv from other programs](https://mpv.io/manual/stable/#using-mpv-from-other-programs-or-scripts), [JSON IPC](https://mpv.io/manual/stable/#json-ipc), [input commands](https://mpv.io/manual/stable/#list-of-input-commands), [properties](https://mpv.io/manual/stable/#property-list), [events](https://mpv.io/manual/stable/#list-of-events), and [audio options/properties](https://mpv.io/manual/stable/#audio).
- mpv source: [`DOCS/man/ipc.rst`](https://github.com/mpv-player/mpv/blob/master/DOCS/man/ipc.rst), [`DOCS/man/options.rst`](https://github.com/mpv-player/mpv/blob/master/DOCS/man/options.rst), [`DOCS/man/mpv.rst`](https://github.com/mpv-player/mpv/blob/master/DOCS/man/mpv.rst), [`DOCS/man/lua.rst`](https://github.com/mpv-player/mpv/blob/master/DOCS/man/lua.rst), and [`include/mpv/client.h`](https://github.com/mpv-player/mpv/blob/master/include/mpv/client.h).
- [mpv release notes](https://github.com/mpv-player/mpv/blob/master/RELEASE_NOTES) and [official installation page](https://mpv.io/installation/) — release/build and Windows packaging context.
- Rust [`std::process::Command`](https://doc.rust-lang.org/std/process/struct.Command.html), [`std::process::Child`](https://doc.rust-lang.org/std/process/struct.Child.html), [`std::process`](https://doc.rust-lang.org/stable/std/process/index.html), and Windows [`CommandExt`](https://doc.rust-lang.org/std/os/windows/process/trait.CommandExt.html).
- Tokio [`process::Command`](https://docs.rs/tokio/latest/tokio/process/struct.Command.html), [`process::Child`](https://docs.rs/tokio/latest/tokio/process/struct.Child.html), [Windows named pipes](https://docs.rs/tokio/latest/tokio/net/windows/named_pipe/index.html), [`ClientOptions`](https://docs.rs/tokio/latest/tokio/net/windows/named_pipe/struct.ClientOptions.html), and [`NamedPipeClient`](https://docs.rs/tokio/latest/tokio/net/windows/named_pipe/struct.NamedPipeClient.html).
- Microsoft [process creation flags](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags), [named-pipe client](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-client), [pipe names](https://learn.microsoft.com/en-us/windows/win32/ipc/pipe-names), [TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess), and [job objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).

## Current API behavior

### Recommended integration shape

The Tauri/Rust backend should own one `mpv.exe` child, one long-lived duplex IPC connection, and the child/connection lifecycle. UI commands should cross a typed backend boundary; they should not open pipes or run processes directly. A single IPC reader should classify replies versus events, while writes should be serialized. Correlate every request with a unique signed 64-bit `request_id`.

### Launch arguments and Windows process setup

A reasonable starting command line for an audio/video controller is:

```text
mpv.exe --no-config --idle=yes --input-ipc-server=\\.\pipe\spotdiy-mpv-<random> --terminal=no --keep-open=yes --audio-device=auto
```

This is a baseline, not a required fixed argument list:

- `--input-ipc-server` makes mpv listen for JSON IPC. Generate a short, unpredictable per-launch suffix and pass the full Windows name `\\.\pipe\...`; do not reuse a fixed name across concurrent instances.
- `--idle=yes` keeps mpv alive with no file loaded. `--keep-open=yes` is optional and makes the last file pause at EOF instead of making mpv exit; it does not prevent exits caused by errors or unusual conditions.
- `--no-config` avoids a user `mpv.conf` changing process behavior. If user-configurable mpv settings are required, use a controlled app-owned `--config-dir` policy instead; do not assume both choices are equivalent. `--no-config` also makes config-directory placeholders unavailable, so scripts must tolerate that.
- `--terminal=no` is appropriate when all standard streams are intentionally null. For diagnostics, use `--input-terminal=no`, capture stderr, and continuously drain it. A captured pipe that is not drained can block the child.
- `--audio-device=auto` is already the default; stating it explicitly documents the policy. Do not add `--ao` unless the app intentionally forces a specific audio-output driver.
- Only add `--force-window` or `--wid=<HWND>` when the app deliberately embeds/parents an mpv video window. `--wid` uses a Windows `HWND`; audio-only mode should not create a window unnecessarily.
- Use mpv's modern `--option=value` form. In Rust, pass each option and value through `.arg()`/`.args()` rather than building a shell command line. For a user-controlled media path, pass it as a separate argument after `--`, or use a structured JSON `loadfile` command; never invoke `cmd.exe` merely to start mpv.

For a Tauri async backend, Tokio's `tokio::process::Command` mirrors the standard process builder. Configure stdio explicitly: use `Stdio::null()` for unused streams, or pipe stderr only when a task will drain and bound/log it. `std::process::Command` is suitable for synchronous code. On Windows, `CommandExt::creation_flags` can pass `CREATE_NO_WINDOW` to avoid a console window for a console-subsystem mpv build; use the named Win32 constant from the chosen Windows bindings rather than a magic number. This flag does not suppress an intentional mpv GUI/video window.

### Named-pipe/socket behavior

`--input-ipc-server=<filename>` is a server endpoint. On Unix it is a filesystem socket path; on Windows it is a named-pipe namespace path. mpv automatically adds `\\.\pipe\` when the prefix is omitted, but passing the full name avoids ambiguity. A Windows pipe name is not an ordinary file and its existence is not a sufficient readiness test.

The Tokio client should open the full name with `tokio::net::windows::named_pipe::ClientOptions::open`. The documented startup cases are:

- `ErrorKind::NotFound`: mpv has not created the server yet (or has already exited).
- `ERROR_PIPE_BUSY`: the server exists but is not currently accepting this connection.

Retry those cases with a bounded deadline/backoff, then send a harmless health request such as `get_property` for `volume` or `idle-active` and wait for a successful reply. Do not treat a successful OS-level connect as proof that the whole player is usable.

Keep the client open for the lifetime of the mpv session. mpv destroys the IPC client and unregisters all observed properties when the connection closes, so one-connection-per-command breaks observation. Reconnects must run the observation-registration handshake again. Set the IPC server option at process launch and treat it as immutable; the mpv manual documents problematic runtime writes to the IPC options.

The wire format is newline-delimited JSON:

- Each request, reply, and event is one complete UTF-8 JSON object terminated by `\n`. A literal newline cannot occur inside a message; use normal JSON escaping in strings and compact serialization.
- Requests use `{ "command": ["command_name", ...] }` with native JSON values. Replies include `error` and command-specific `data`; success can have `data: null`.
- `request_id` is optional, must be an integer in the signed 64-bit range, and is echoed verbatim. If omitted, mpv replies with `request_id: 0`; use unique IDs in the app.
- Events can arrive between a request and its reply. Async commands can complete out of order. Use one reader/event loop and a pending-request map; never assume the next line is the reply to the last write.
- mpv warns that unusual broken-encoding strings can result in invalid JSON. A robust parser should bound the maximum line/message size, handle a malformed line without silently treating it as a valid state update, and report the connection unhealthy if framing cannot be recovered.

mpv's IPC protocol is intentionally insecure: there is no authentication or encryption, and commands such as `run` can execute system commands. Keep the endpoint local and do not expose it through TCP or a network relay. A random pipe name reduces collisions, not authorization; verify the Windows ACL/security posture if the threat model includes another local process.

### Get, set, and observe properties

Use native JSON types with the property APIs rather than parsing human-oriented terminal output.

```json
{"command":["get_property","pause"],"request_id":101}
{"command":["set_property","pause",true],"request_id":102}
{"command":["observe_property",10,"time-pos"],"request_id":103}
{"command":["unobserve_property",10],"request_id":104}
```

`set_property_string` is an alias that accepts native values and strings; it is not necessary when the Rust side already has a boolean or number. An observation produces `property-change` events containing the numeric observation `id`, property `name`, and new `data`. Use the numeric ID as the stable routing key. mpv's shared property-observation mechanism can provide an initial change notification; still issue explicit `get_property` requests after connection so bootstrap does not depend on event timing. Re-register all observations after reconnect.

Useful core state for a controller is:

| Property | Use | Caveat |
| --- | --- | --- |
| `pause` | authoritative paused/playing state | Boolean; set explicitly for idempotent UI actions. |
| `time-pos` | current position in seconds | May be unavailable when no file is loaded. |
| `duration` | duration in seconds | May be unavailable for live/unknown-duration media. |
| `percent-pos` | percentage fallback/display | Not a substitute for seconds when duration is unknown. |
| `volume` / `mute` | app-level mixer state | `volume` is mpv's internal/software volume; `mute` is separate. |
| `seeking` / `eof-reached` | transient UI state | Treat as asynchronous playback state. |
| `audio-device-list` / `audio-device` | device picker and selection | See output-device section below. |
| `current-ao` | active audio-output driver | It is the driver, not the concrete device. |

The observation stream is asynchronous and may be coalesced. Do not assume one event per rendered frame; throttle high-frequency position updates before emitting them to the frontend. `request_log_messages` is for human diagnostics, not a stable state API.

### Commands for seek, pause, and volume

Use these JSON commands as the small stable control surface:

| Intent | JSON command | Behavior |
| --- | --- | --- |
| Relative seek | `{"command":["seek",10.0,"relative"]}` | Seek forward 10 seconds. Add `"exact"` when exact seeking is required and acceptable. |
| Absolute seek | `{"command":["seek",90.0,"absolute"]}` | Seek to 90 seconds. Other documented modes include `absolute-percent`, `relative-percent`, and `keyframes`. |
| Set pause | `{"command":["set_property","pause",true]}` | Idempotently pause. Use `false` to resume. |
| Toggle pause | `{"command":["cycle","pause"]}` | Toggle semantics when the UI action is explicitly a toggle. |
| Set volume | `{"command":["set_property","volume",65.0]}` | Set mpv's internal volume. |
| Change volume | `{"command":["add","volume",5.0]}` | Add a relative amount; omit the amount only when the documented default of 1 is desired. |
| Toggle/set mute | `{"command":["cycle","mute"]}` or `set_property` | Keep mute independent of volume. |

The command reply confirms command acceptance, not necessarily that a seek or audio reconfiguration has settled. For seek UI, reconcile `time-pos`, `seeking`, and playback events. For a slider, debounce writes and reconcile the next property event rather than assuming local state is authoritative.

### Process lifecycle and crash recovery

Maintain two independent signals: the IPC reader and a task awaiting the child process. Either can detect failure first.

1. Spawn mpv, retain the child handle, and connect to the fresh pipe name within a bounded deadline.
2. Complete a health request, register observations, then load media and apply session state. Keep the last known media identifier, position, pause, volume, mute, and selected audio-device in the Rust controller; do not rely on a live IPC connection surviving a restart.
3. For graceful shutdown, send the mpv `quit` command, stop accepting new UI commands, and await the IPC `shutdown` event and child exit with a timeout. Tokio's `Child::kill().await` is the forceful fallback; with `std::process::Child`, call `kill()` and then `wait()`/`try_wait()` so the process is reaped and its final status observed. `kill_on_drop(true)` can be a safety net, but Tokio documents explicit wait/kill as the stronger lifecycle guarantee.
4. If the pipe returns EOF, repeated I/O errors, a health timeout, or the child exits without the expected shutdown path, mark the session disconnected and fail pending requests. Do not send commands through the stale client. Confirm the child status; if it is still running but unresponsive, force-terminate it.
5. A restart uses a new pipe name and a new process handle. Reconnect, health-check, re-register observations, wait for `file-loaded`, and restore only valid state. Restore position only after the file is loaded and only when the media is seekable. Use bounded exponential backoff and a retry limit; surface a persistent failure instead of creating an infinite restart loop.

On Windows, forceful process termination does not automatically terminate child processes. If the selected mpv build/configuration can create helper descendants and they must be killed with mpv, a Job Object with an appropriate kill-on-close policy is the OS-level option; this is an additional native integration, not a default requirement for a simple mpv child. The mpv client header also documents that mpv can start subprocesses in some configurations, so this should be an explicit product decision.

### Output-device considerations

- Query `audio-device-list` and show `description` to users; persist the corresponding `name`, which is the opaque value mpv accepts. Re-enumerate at startup because device IDs can change. Keep `auto` as a fallback.
- Setting `audio-device` maps to `--audio-device` and schedules an audio-output reload. It does not tell the app which device is actually in use, and writing it while no audio output is active does not enable audio. Use `current-ao` only to identify the current driver, and treat device selection/reload as asynchronous.
- The default `--audio-device=auto` tries the default device through mpv's output-driver preference order. Do not pair `--ao` with `--audio-device` unless deliberately forcing both policy layers; mpv warns this can be confusing.
- `volume` is the app-facing mpv mixer. `ao-volume`/`ao-mute` are audio-output/system-level controls whose availability depends on the active output API; they are not portable replacements for `volume`.
- `--audio-exclusive=yes` can lock out other applications and only applies to some audio outputs, including WASAPI. Leave it off by default and expose it as an explicit user preference if required.
- `--audio-fallback-to-null=yes` can keep playback moving when a selected device cannot open, but it can also produce silent playback. Use it only if the UI clearly reports the null-output state; otherwise a device-open failure should be visible and trigger a re-enumeration/fallback policy.
- HDMI/receiver paths can have channel-layout and wake-up quirks. Keep `--audio-channels=auto-safe` (the default) unless there is a tested product requirement for explicit layouts or passthrough; for direct HDMI hardware, mpv recommends an explicit whitelist such as `7.1,5.1,stereo`. `--audio-stream-silence=yes` can address some receivers that drop the first audio after pause/seek, but mpv documents it as a specialized workaround rather than a general default.

## Rejected alternatives

- **Terminal/stdin control:** mpv describes terminal output and text input as human-oriented and recommends IPC for interactive control. It is difficult to correlate replies, does not provide typed property events, and is fragile across user configuration and versions.
- **A new pipe connection per command:** this loses property observations when the connection closes and creates extra startup/busy races. One persistent connection is the documented model.
- **Fixed pipe name or filesystem polling:** fixed names collide across app instances, while named pipes are not normal files. Use a per-launch name, actual connect retries, and an IPC health request.
- **`--input-ipc-client=fd://N`:** this is a valid documented alternative, but on Windows it requires a pre-connected, inheritable, duplex, overlapped named-pipe server handle wrapped with `_open_osfhandle`. That handle-inheritance path is more complex than letting mpv own the server and Tokio connect as the client.
- **libmpv embedding:** mpv recommends libmpv for true in-process embedding, but it changes the boundary to native FFI/rendering/event-loop integration and removes the process-isolation benefits of a separately managed child. No libmpv/Tauri context or Cargo manifest was present in this repository, so it is outside this JSON-IPC/process-management choice.
- **Forcing `--ao=wasapi` or exclusive mode by default:** this narrows device compatibility and can prevent other applications from using audio. Start with mpv's automatic driver/device selection and capability-driven settings.
- **Using `ao-volume` for the app slider:** it is output/API-dependent and can represent system/device volume. Use `volume` for portable player volume; expose device/system volume separately only when that behavior is intentional.

## Risks

- **Local command authority:** mpv JSON IPC has no authentication/encryption and exposes powerful commands. Keep the pipe local, use an unpredictable name, avoid logging the full capability-bearing name, and review Windows pipe ACLs if another local process is in scope.
- **Reply/event races:** events can interleave with replies and async command replies can reorder. A single reader, unique request IDs, a pending map, serialized writes, and cancellation/timeouts are required.
- **Connection lifetime:** closing/replacing a connection unregisters observations. A reconnect must reset pending requests and repeat the observation handshake.
- **Startup and shutdown races:** `NotFound`, `ERROR_PIPE_BUSY`, early child exit, and a pipe that accepts before playback is fully ready all occur at different layers. Use deadlines and a health request, not sleeps alone.
- **Process leaks and deadlocks:** Rust does not automatically wait for a dropped `Child`; Tokio's default drop behavior also leaves the process running. Explicitly await/kill and reap. Never capture a stream without draining it; prefer null stdio when logs are not needed.
- **Forceful recovery:** kill is not graceful and may lose the last position or leave helper processes. Snapshot state at controlled points, restore only after `file-loaded`, and use a Windows Job Object only if descendant cleanup is a real requirement.
- **Malformed input:** mpv documents occasional invalid JSON from broken-encoding strings. Bound input, handle parse failures explicitly, and treat unrecoverable framing as a failed session rather than updating state from partial data.
- **Audio churn:** unplugged devices, driver changes, exclusive-mode conflicts, HDMI wake-up, and unsupported channel layouts can make an apparently successful property write ineffective. Re-enumerate, observe reconfiguration/errors, and make fallback/silent-output state visible.
- **Video-window coupling:** an external `HWND` and a separate mpv process add lifetime/resize/focus races. Keep window embedding optional and test it independently of the audio/IPC controller.
- **Version drift:** optional properties, events, output drivers, and edge-case behavior can vary by mpv build. Capability-check optional features and test the exact binary shipped with the app.

## Version assumptions

- Research date is 2026-08-30. The official mpv manual index identified v0.41.0 as the stable release; this note targets that stable manual, not Git master behavior.
- The exact packaged `mpv.exe` is the real compatibility boundary. Validate its path, signature/package provenance, and `mpv --version` during development or packaging. JSON IPC `get_version` reports the client API version provided by the remote mpv instance; it is not a substitute for recording the binary version.
- Rust API references were checked against the current stable standard-library documentation (shown as Rust 1.98.0). Tokio references were checked against the current docs.rs API (shown as Tokio 1.53.1). Pin and test the versions selected by the eventual Cargo manifest; this repository had no Cargo manifest/version pin to reuse during this research.
- The design intentionally uses the stable, documented subset: launch-time `--input-ipc-server`, newline-framed JSON, request IDs, core property commands/observations, Tokio Windows named pipes, and explicit process wait/kill. It does not depend on nightly Rust process APIs or development-only mpv changes.
