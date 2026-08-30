# Windows integration

Standard mode targets `%LOCALAPPDATA%\SpotDIY`; portable mode keeps its data beside the executable. Future overlay, global shortcut, tray, media controls, output profile, and always-on-top work must be capability- and OS-tested. Gaming click-through is conditional on reliable current Tauri/Win32 behavior.

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
