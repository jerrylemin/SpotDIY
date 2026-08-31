# SpotDIY architecture

SpotDIY is a single Tauri 2 application with a React frontend and a Rust
native core. The frontend owns presentation and interaction state; Rust owns
filesystem, database, process, provider, and Windows integration boundaries.

```text
React routes/components
        |
        v
Tauri commands / IPC DTOs
        |
        v
Rust services -- SQLite WAL / local filesystem / managed tools / providers
        |
        +-- PlaybackService (serialized controller / transient queue)
                |
                +-- PlaybackBackend -> MpvBackend -> Windows named pipe -> one mpv.exe
```

Frontend state uses Zustand for command palette, player presentation, overlay,
and layout interaction state. TanStack Query owns asynchronous backend data
such as search pages, library pages, downloads, lyrics, settings, and
analytics. Authoritative records must not be duplicated across stores.

The current native persistence seam is `Database` plus focused
`TrackRepository`, `ArtistRepository`, `SourceRepository`, and
`SettingsRepository` modules. Database initialization resolves the
caller-provided path, creates the parent directory, enables and verifies
WAL/foreign keys, applies ordered migrations, probes FTS5 availability, and
exposes typed settings/status commands. `LibraryService` is the Plan 03 owner
for managed folder roots and local-file lifecycle; it uses the same thread-safe
database wrapper for short transactional writes and keeps filesystem/hash/
metadata/artwork I/O outside those transactions.

The local-library flow is:

```text
native folder picker -> add_library_folders
        -> canonical folder validation and persistent library_folders row
        -> Notify watcher registration + background WalkDir scan
        -> Lofty metadata / SHA-256 / artwork cache
        -> transactional tracks, sources, artists, albums, and local_files upsert
        -> library://scan-progress -> React Query status/page refresh
```

One watcher is retained per enabled root. Ordinary scans use size/mtime
evidence to skip unchanged files; watcher create/modify/rename events force
re-reading. Full reconciliation marks confirmed missing files unavailable,
reactivates restored paths, and preserves source/track IDs for an unambiguous
fingerprint rename. Uncertain filesystem events request reconciliation, while
backend/channel failure requests watcher re-registration. Reveal accepts only
a source ID and revalidates managed-folder containment and current canonical
path before invoking the scoped opener.

Provider adapters report capability sets and normalize provider results into
shared DTOs. `SourceFusionService` will match sources into `UnifiedTrack`
records, while `SourceResolver` will select playable sources according to the
user's ordered preferences. Spotify catalog sources remain metadata-only.

Standard storage targets `%LOCALAPPDATA%\SpotDIY\spotdiy.sqlite3`; the current
application opens that path through an explicit `Database::open(path)` seam.
Portable startup and its beside-executable layout remain a later-plan concern,
and persisted portable mode is rejected until that startup path exists. Secrets
use Windows Credential Manager, never SQLite or source control.

## Plan 04 playback boundary

The frontend sends only typed track/source IDs and transport values. Tauri
commands call `PlaybackService`; the service resolves the selected source
through `LibraryService`, which verifies managed-folder ownership, indexed
local-file state, availability, and current regular-file existence. The
service publishes one revisioned `PlaybackSnapshot` through `tokio::watch` and
emits `playback://state`; stale snapshots are ignored by the frontend.

The controller is the only component that mutates playback state. It receives
user commands and typed backend events through a serialized command loop. The
`PlaybackBackend` contract is enqueue-only (`send`, `health`, `shutdown`), so
the controller never waits synchronously for an mpv reply; the bounded worker
owns that work. Events retain their backend generation through delivery and
the controller rejects stale events. The backend starts one persistent
external `mpv.exe` process with the exact Plan 04 arguments and a fresh random
Windows named pipe. Its JSON reader/writer owns request IDs, bounded frames,
typed protocol errors, reply correlation, six property observations,
child-exit monitoring, and bounded quit/kill/reap. No pipe path, executable
path, request ID, raw JSON, or local audio path crosses the frontend boundary.

Lifecycle event queues are bounded and critical events await capacity; position
samples may be coalesced at approximately 4 Hz. Source-switch failure restores
the prior identity, source, timestamp, pause state, and queue entry before
entering normal recovery when rollback itself fails. mpv version probing uses
`--no-config --version` with a finite process/output budget and only cleans up
the probe child.

The queue is intentionally transient and ID-only. Canonical ordering remains
stable when shuffle changes; repeat and EOF traversal are service policies.
Persistent queue state and queue snapshots belong to Plan 08.
