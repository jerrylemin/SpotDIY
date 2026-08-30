# Active work

## Current boundary

Plan 02, unified domain and SQLite foundation, is implemented and verified. Plan 03 Local Library is now implemented and verified locally. The native core owns typed unified-track records, WAL-backed migration initialization through schema version 2, focused repositories, durable ordinary settings, typed status/settings IPC, persistent folder roots, incremental scanning/reconciliation, metadata/artwork/fingerprints, watcher recovery, and safe reveal.

Verification includes 53 Rust tests, 18 frontend tests across four files, Rust formatting and clippy, TypeScript typecheck/lint/build, a Tauri x64 release build, packaged launch, native CDP Library smoke, and Graphify synchronization. The standard launch creates the expected local database at `%LOCALAPPDATA%\\SpotDIY\\spotdiy.sqlite3`; application artwork is cached under `%LOCALAPPDATA%\\SpotDIY\\cache\\artwork`.

Portable startup, FTS-backed search, provider adapters, playback, downloads, lyrics, playlists, queue, import/export, and analytics remain later-plan work. Portable mode is represented as future metadata but rejected by the current standard startup path until its deterministic executable-location selection is implemented. Plan 03 intentionally leaves playback controls disabled.

The browser test runner gap is explicit: `pnpm exec playwright --version`
works, but `pnpm exec playwright test --list` returns `unknown command 'test'`
because no Playwright browser project/configuration is present. Native packaged
CDP smoke is the current desktop evidence.

## Next atomic task

Plan 04 Playback Engine: define the playback service and adapter boundary over
the persistent local source/availability contracts. Keep provider logic behind
capability-bearing adapters and do not begin Source Fusion in that slice.
