# Active work

## Current boundary

Plan 02, unified domain and SQLite foundation, is implemented and verified locally. The native core now owns typed unified-track records, WAL-backed migration initialization, focused repositories, durable ordinary settings, and typed status/settings IPC.

Verification includes 25 Rust tests, 5 frontend tests, Rust formatting and clippy, TypeScript typecheck/lint/build, a Tauri x64 release build, and a packaged launch smoke. The standard launch smoke created the expected local database at `%LOCALAPPDATA%\\SpotDIY\\spotdiy.sqlite3`.

Portable startup, FTS-backed search, local folder selection/indexing, provider adapters, playback, downloads, lyrics, playlists, queue, import/export, and analytics remain later-plan work. Portable mode is represented as future metadata but rejected by the current standard startup path until its deterministic executable-location selection is implemented.

## Next atomic task

Plan 03 Local Library: folder selection, recursive incremental scan, metadata extraction, local source persistence, and filesystem watching. Keep provider logic behind capability-bearing adapters.
