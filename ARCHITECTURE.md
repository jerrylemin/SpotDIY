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
        +-- PlaybackService (serialized controller / persistent queue + snapshots)
                |
                +-- PlaybackBackend -> MpvBackend -> Windows named pipe -> one mpv.exe
        +-- DownloadService (persistent task scheduler / owned children)
                |
                +-- TokioYtDlpProcessRunner -> yt-dlp -> task temp -> safe finalization
                `-- MediaToolManager -> FFmpeg discovery/probe for video merge/remux
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

Standard storage targets `%LOCALAPPDATA%\SpotDIY\spotdiy.sqlite3`; managed
download task temp storage is `%LOCALAPPDATA%\SpotDIY\cache\downloads\<DownloadTaskId>`
and final output uses the persisted user-selected `downloads_directory`. The
current application opens the database path through an explicit
`Database::open(path)` seam.
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

## Plan 05 provider search boundary

```text
React SearchPage / useSearch
        |
        v
typed SearchRequest + SearchId events
        |
        v
SearchService
  |-- LocalSourceAdapter -> SQLite library records
  |-- YoutubeSourceAdapter -> bounded yt-dlp metadata process
  |-- SoundcloudSourceAdapter -> bounded yt-dlp metadata process
  `-- SpotifySourceAdapter -> PKCE-authenticated catalog metadata (isolated lens)
```

`SearchService` owns the provider registry, active SearchId, cancellation,
per-provider timeouts, partial section events, exact completion, stale-event
identity, provider-local sorting, and a bounded TTL cache. Unified `ALL`,
`TRACKS`, `ARTISTS`, and `ALBUMS` requests never include Spotify; `LOCAL`,
`YOUTUBE`, `SOUNDCLOUD`, and `SPOTIFY` select only their specified providers.
The frontend buffers events that arrive before the native start response and
rejects events from stale SearchIds. Search results are transient DTOs; no
provider payload, raw subprocess output, token, or credential is persisted.

Spotify authorization is loopback-only on `127.0.0.1` with a dynamic port and
S256 PKCE. Access and refresh tokens stay in the Windows credential seam or
process memory, and catalog search remains disabled until the explicit
development/compliance gate is enabled.

## Plan 06 source fusion and resolver boundary

```text
ephemeral SearchResult + UnifiedTrack
              |
              v
  NFKD normalization -> integer Jaro-Winkler matcher
              |
              +--> typed FusionEvaluation (read-only)
              |
              `--> explicit accept_match -> SourceRepository -> TrackSource

UnifiedTrack + settings preference + runtime readiness
              |
              v
        SourceResolver -> ordered SourceResolution
              |
              v
        PlaybackService -> LibraryService path -> mpv
```

`SourceFusionService` applies the Spotify exclusion, entity/identity checks,
target-specific split overrides, one-target merge overrides, guarded version
comparison, title/artist hard minima, duration bands, and the weighted 8800
automatic threshold. Evaluation and best-match selection do not write SQLite;
only explicit acceptance or override operations write. Accepted remote sources
use backend-owned YouTube/SoundCloud metadata capabilities and never create a
local-file row or change track metadata/preferred source.

`SourceResolver` first honors a currently playable per-track preference, then
the validated provider preference order. Local candidates require availability,
playback capability, and a successful managed `LibraryService` path check;
local quality ranks known lossless codecs, then bit depth, sample rate,
bitrate, and stable `SourceId`. YouTube and SoundCloud remain
`ProviderPlaybackNotImplemented`, Spotify remains `MetadataOnly`, and no
online URL or yt-dlp playback path reaches mpv. Its narrow readiness probe is
the test seam for future provider playback.

## Plan 07 download boundary

```text
SearchResult / persisted TrackSource
              |
              v
typed queue command -> DownloadService -> downloads repository (schema 4)
                              |
                              +--> owned task temp -> bounded yt-dlp child
                              +--> progress/state snapshots -> downloads://state
                              `--> destination-side temp -> collision-safe final file
```

`DownloadService` is the sole owner of persistent download lifecycle. It
validates YouTube/SoundCloud provider identity and canonical URLs, reads the
existing `downloads_directory` setting, creates UUID task roots, and never
passes provider-derived filenames directly to the filesystem. It supports
audio provider encoding and video best-video-plus-best-audio with FFmpeg when
available; missing video tooling fails truthfully. Progress is machine-readable
and throttled for SQLite persistence while snapshots remain revisioned and
bounded for the UI.

Cancellation kills and reaps only the child owned by that task. Restart
recovery requeues only interrupted active states and cleans only their owned
task temp roots. Finalization validates a regular output inside that root,
copies through a destination-side temporary file, renames without overwrite,
persists the trusted path, and cleans the owned temp root. Plan 07 does not
create library tracks, move library media, fuse sources, or provide online
playback; Spotify and Local download requests are rejected.

## Plan 08 durable collections and queue boundary

```text
React playlists/library/queue surfaces
              |
              v
typed playlist + collection + queue IPC / queue://state
              |
              +--> PlaylistService -> playlists, Inbox, branches, likes, ratings, tags
              `--> PlaybackService -> queue sections, checkpoints, snapshots -> SQLite schema 5
                                      |
                                      `--> SourceResolver -> LibraryService path -> mpv
```

`PlaylistService` owns durable normal playlists, deterministic system Inbox,
playlist items, and one-level lightweight branches. A branch stores its parent
revision and base item snapshot; diff, selected merge, and discard are one-shot
operations with explicit revision/conflict errors. Likes, 1..5 ratings, tags,
and batch collection reads remain bounded and track-identity based.

`PlaybackService` remains the sole owner of queue state; there is no separate
`QueueService`. Queue entries contain only opaque IDs and optional requested
source IDs. Up Next precedes Later, Autoplay is structurally empty, shuffle
reorders Later without replaying current/history, and current/consumed entries
are protected. Queue changes and approximately one-second position checkpoints
persist through the typed repository. Startup restores queue state without
autoplay; the first Play resumes the saved current item and position. Queue
snapshots are immutable records whose restore creates fresh live queue IDs and
does not autoplay.

## Plan 09 lyrics, bookmarks, and A/B boundary

```text
React LyricsPage / PlayerBar controls
              |
              v
typed lyrics + bookmark + loop IPC
              |
              +--> LyricsService -> local metadata / managed .lrc -> schema 6 cache
              +--> explicit LRCLIB lookup (metadata-only candidates)
              +--> BookmarkService -> bookmarks / A-B presets
              `--> PlaybackService -> SetAbLoop/ClearAbLoop -> mpv ab-loop-a/b
```

`LyricsService` applies the deterministic precedence manual override, exact
sidecar `.lrc`, embedded timed text, embedded plain text, then cached LRCLIB.
Local reads use the existing `LibraryService` managed-path boundary, verify a
regular non-link file, enforce bounded input, and never mutate media or library
metadata. Embedded plain and ID3 SYLT text are read-only metadata evidence.

LRCLIB is an explicit user action behind an HTTPS-only, bounded, rate-gated
provider boundary. Only validated metadata and selected lyric text enter the
local cache; raw provider responses, credentials, and automatic background
lookups do not cross the boundary. Frontend commands use typed track/source
identifiers and native file selection for manual import.

`BookmarkService` persists bounded notes and positions and normalized named
loop presets in schema 6. `PlaybackService` remains the sole owner of active
A/B state: setting A/B and clearing the loop are typed backend commands, a new
track clears the active loop, same-track source switching and recovery restore
it, and applying a preset never starts playback. The synchronized lyrics page
selects the active cue by playback position and shows source/attribution state;
waveform generation is outside this boundary.

## Plan 10 UI design-system boundary

The frontend design system is token-first. `tokens.css` defines the semantic
surface, text, accent, status, focus, and waveform variables; `foundations.css`
and `primitives.css` provide the shared interaction and component layer.
`ThemeController` resolves Dark, Light, System, or validated Custom themes and
applies `data-theme="dark|light|custom"` plus
`data-layout="comfortable|compact|dense"` to the document root. System-theme
changes are subscribed through `matchMedia`, and invalid custom data falls back
to dark with a recoverable Settings error.

The custom theme contract is schema version 1, contains exactly 15 semantic
color tokens, accepts strict `#RRGGBB` values, and is checked for byte, name,
and WCAG contrast limits in both Zod and Rust. `layout_profile` and
`custom_theme` are ordinary typed settings keys in schema 6; no migration 7 was
introduced. The browser E2E adapter is an in-memory preview seam, while native
settings continue through `SettingsRepository`.

Shared Button, IconButton, Surface, StatusChip, Field, SegmentedControl,
Tooltip, EmptyState, ProviderBadge, and ContextActionMenu primitives centralize
labels, focus, disabled explanations, keyboard actions, and reduced motion.
InspectorPanel/IconGallery is a development/design surface. Context actions
are caller-supplied and are adopted by `LibraryTrackRow`; permanent navigation,
full Track Inspector, and Plan 11 player refinement remain outside this boundary.
