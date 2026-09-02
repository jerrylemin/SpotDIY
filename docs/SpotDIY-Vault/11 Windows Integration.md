# Windows integration

Standard mode targets `%LOCALAPPDATA%\SpotDIY`; portable mode keeps its data beside the executable. Plan 12 delivers the capability- and OS-tested overlay, global shortcut, tray, media control, output profile, and always-on-top boundary. Gaming click-through remains conditional on reliable current Tauri/Win32 behavior and is recoverable when unavailable.

## Plan 03 local files

The native folder picker uses the Tauri 2 dialog plugin with directory and
multiple-selection enabled. Rust canonicalizes and validates every returned
path, rejects duplicate/nested/reparse roots, persists the selected roots, and
does not expose arbitrary filesystem access to the frontend.

One Notify 8.2.0 recursive watcher is held per enabled folder. A 450 ms
coalescer handles create/modify/remove/reliable rename events, routes uncertain
events to forced reconciliation, and distinguishes watcher/channel failure so
the handler can re-register before recovering. Missing startup roots remain
persisted and mark their sources unavailable; a later rescan can re-register a
restored root without asking the user to select it again.

`reveal_local_file` accepts only a typed source ID. Before the official opener
reveals the item, Rust verifies local-source ownership, enabled folder
containment, canonical path equality, and current file existence. The Tauri
asset protocol is enabled only for the application artwork cache under
`%LOCALAPPDATA%\SpotDIY\cache\artwork`; selected music roots are outside its
scope.

## Plan 04 playback process

Playback uses one external `mpv.exe` child per `PlaybackService`, discovered by
`SPOTDIY_MPV_PATH` and then PATH. SpotDIY creates a fresh random Windows named
pipe and starts mpv with `--no-config --idle=yes --terminal=no
--input-terminal=no --audio-display=no --input-ipc-server=<fresh pipe>`. The
pipe is backend-only; the webview never sees the executable, pipe, request
IDs, local audio paths, or raw JSON. Frames are bounded and the child is
monitored for disconnect/exit; shutdown attempts quit and then bounded
kill/reap. Discovery uses a bounded `mpv.exe --no-config --version` probe with
finite process and output budgets; timeout cleanup targets only that probe
child.

The packaged playback smoke uses a temporary profile and the smoke-only
`SPOTDIY_PACKAGED_DATA_ROOT` environment variable. This is necessary because
Windows known-folder resolution can ignore a child `LOCALAPPDATA` override;
normal production startup remains `%LOCALAPPDATA%\SpotDIY` and portable mode
is still deferred. The harness identifies only mpv children launched with the
SpotDIY pipe suffix and never terminates unrelated mpv processes.

## Plan 12 Windows integration

`WindowsIntegrationService` owns four lazy Tauri overlay windows: Mini,
Edge, Lyrics, and Gaming. Their native labels are `overlay-mini`,
`overlay-edge`, `overlay-lyrics`, and `overlay-gaming`; Rust owns exact sizes,
safe placement, close/reopen reuse, and always-on-top configuration. The
overlay capability is intentionally limited to event listen/unlisten and the
window close/always-on-top operations required by the surfaces.

The tray menu and nine typed global shortcuts dispatch to the same native
actions used by the Settings and command-palette surfaces. Global shortcuts are
disabled by default and expose registered, conflict, invalid, or failed status
per binding. Gaming click-through is session-only and uses
`Ctrl+Alt+Shift+G` as the rescue path; standard always-on-top behavior may still
be covered by exclusive fullscreen applications.

Windows SMTC is enabled by default, but the status is explicitly `ready`,
`disabled`, `unsupported`, or `failed` with detail. Its WinRT bridge lives in
the isolated `spotdiy-windows-smtc` helper crate, and media commands map back
to `PlaybackService`. Output profiles persist only normalized device, volume,
and mute values in schema 8; apply/rollback preserves playback context. Native
window visibility, handles, tray state, SMTC runtime objects, and click-through
are not persisted.

The packaged Plan 12 smoke proves fresh schema-8 startup, native tray/status,
live SMTC, controlled shortcut registration, overlay reuse/topmost, click-
through recovery, output-profile apply/restore, restart persistence, and zero
owned mpv processes. Browser preview remains native-free and truthful.
