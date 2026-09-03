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
        +-- WindowsIntegrationService (overlays / tray / shortcuts / SMTC / output)
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
`custom_theme` were introduced as ordinary typed settings during Plan 10;
Migration 7 now makes those keys compatible with shipped schema-6 databases.
The browser E2E adapter is an in-memory preview seam, while native settings
continue through `SettingsRepository`.

Shared Button, IconButton, Surface, StatusChip, Field, SegmentedControl,
Tooltip, EmptyState, ProviderBadge, and ContextActionMenu primitives centralize
labels, focus, disabled explanations, keyboard actions, and reduced motion.
InspectorPanel/IconGallery is a development/design surface. Context actions
are caller-supplied and are adopted by `LibraryTrackRow`; permanent navigation
and the full Track Inspector were intentionally outside the Plan 10 boundary.

## Plan 11 main shell and inspector boundary

```text
AppShell
  +--> CommandPalette --> presentation commands / navigation
  +--> TrackInspector or SearchResultInspector
  +--> QueueDrawer
  `--> PlayerBar --> StandardPlayerBar | MiniPlayer | NowPlayingPanel
                         |
                         +--> usePlayback() snapshot and existing commands
                         +--> SourceSwitcher --> PlaybackService source switch
                         `--> useTrackInspector() quality/provenance read
```

`AppShell` is the single presentation composition point for player mode,
inspector selection, queue visibility, command palette visibility, and Escape
priority. `ui-store.ts` contains session-only presentation state; it does not
duplicate playback or queue ownership. All three player modes consume the same
`usePlayback()` snapshot and opening a mode does not autoplay or mutate queue,
source, or position state.

`TrackInspectorService` exposes a purpose-built read-only DTO assembled from
the existing Track and collection repositories. It includes source identity,
availability, capabilities, measured local quality, version qualifiers, and
validated provider URLs. Local filesystem paths are deliberately excluded;
reveal remains the existing source-ID command. Online SearchResults use a
separate ephemeral inspector and cannot persist, fuse, or play online.

`track-actions.ts` is the pure frontend policy boundary for provider and runtime
capabilities. Search, library, playlist, and download surfaces reuse it or
existing service hooks; disabled actions keep their reason visible. YouTube
and SoundCloud downloads remain native/capability-gated, Spotify remains
metadata-only, and Plan 11 did not add provider or backend behavior. Plan 12
adds the separate Windows integration boundary described below.

## Plan 12 Windows integration boundary

`WindowsIntegrationService` is the native owner for optional desktop surfaces
and system controls. It lazily creates exactly four labeled Tauri webview
windows (`overlay-mini`, `overlay-edge`, `overlay-lyrics`, and
`overlay-gaming`) with exact dimensions, safe positioning, and always-on-top
configuration. Reopening an active overlay reuses the existing window. The
overlay capability grants only the event listen/unlisten and window close,
always-on-top permissions needed by those surfaces; the main capability grants
only the additional focus/show permissions needed by the tray and palette path.

The same service owns the tray menu, nine typed global shortcut bindings, and
per-binding registered/conflict/invalid/failed status. The master shortcut
switch is disabled by default; failed registrations do not become claimed
actions. Gaming click-through is session-only and can be recovered with the
reserved `Ctrl+Alt+Shift+G` rescue binding.

SMTC is enabled by default but reports `ready`, `disabled`, `unsupported`, or
`failed` with detail. The WinRT bridge is isolated in the Windows-only
`spotdiy-windows-smtc` helper crate; playback snapshots project only bounded
metadata and typed transport commands. Output profiles are ordinary schema-8
settings and apply through `PlaybackService` without changing track, queue,
position, or playback phase; device/volume/mute failures roll back and report
the recovery result. Overlay visibility, native handles, tray state, SMTC
runtime objects, and click-through state are never persisted.

## Plan 13 backup and portable storage boundary

`StorageLayout` is the startup authority. It inspects only the exact
`SpotDIY.portable` marker beside the running executable, validates every
required directory without following symlinks or reparse points, and resolves
the database before `Database::open`. Standard uses the platform local-data
root; Portable uses executable-relative `Data`, `Music`, `Covers`, `Lyrics`,
`Database`, `Cache`, and `Config` roots. A marker is authoritative, so a
portable failure is explicit and never silently redirected to AppData.

`BackupService` owns the archive and restore lifecycle. `archive.rs` creates a
WAL-safe online database snapshot and a deterministic format-1 ZIP containing
stable JSON, an exact manifest checksum, and only trusted local audio, exact
same-stem sidecars, and active artwork-cache files. `import.rs` validates ZIP
names, compression, bounds, hashes, declared entries, schema, integrity, and
foreign keys before staging. Commit writes a pending descriptor and requires a
restart; startup applies it before normal database open, records created media,
keeps one rollback snapshot, and restores the prior database/deletes only files
created by the failed import on error.

The frontend receives only typed/Zod-validated status, options, previews, and
results. Native Tauri dialogs own destination selection for export, archive
selection for import, and Standard-mode included-audio restore folders.
Credentials, tokens, provider payloads, live SQLite sidecars, temp files, and
untrusted media paths are outside the archive boundary.

## Plan 14 smart features and local analytics boundary

Migration 9 adds only the four logical tables `track_genres`,
`listening_sessions`, `play_history`, and `smart_playlists`. Genres and
validated release dates are derived from local tags and stored with the local
library. `AnalyticsRecorder` observes the existing playback owner, writes
qualified activity in batches, groups sessions using the fixed 30-minute gap,
and emits only typed aggregate/history DTOs. No filesystem path, provider raw
URL, telemetry, or analytics network call crosses the frontend boundary.

`ListeningModeService` holds Private Session and Temporary Mode in memory.
Private activity is never persisted; Temporary Mode saves a durable queue
checkpoint, owns its transient queue mutations, and restores the checkpoint
without autoplay on exit. `PlaybackService` remains the sole queue and
transport owner.

`SmartPlaylistService` validates a bounded typed rule tree and compiles
allowlisted fields/operators into parameter-bound SQL. `SmartShuffleService`
uses a deterministic seeded weighted heuristic over familiarity, variety,
freshness, and discovery signals, with a recent-track and recent-artist
window; it is not an ML recommendation service and does not persist a seed.

The `/analytics` route and Playlists smart-rule surface consume these typed
contracts. Browser preview adapters return empty analytics and reject native
smart operations, so production UI cannot fabricate local history or
recommendations.

## Plan 15 advanced visual exploration boundary

`VisualExplorerService` is the sole native producer for visual exploration
data. It executes one parameter-bound, read-only dataset query over the
existing schema-9 library, collection, source, genre, rating, and play-history
tables, validates a default 2,000/hard 5,000 limit, reports truncation, and
orders ties by stable `TrackId`. The DTO contains bounded metadata, aggregate
listening facts, quality, provider count, and only an existing artwork-cache
path; local media paths, raw provider URLs, credentials, and network results
are outside the contract.

Music Map derives real GENRES -> ARTISTS -> ALBUMS -> TRACKS relationships and
renders a bounded SVG with deterministic order, pan/zoom/reset, filters,
selection, and a 200-item Map Navigator. Library Galaxy derives deterministic
artist/genre clusters with golden-angle placement and hashed TrackId offsets,
renders one-shot 2D Canvas frames, and exposes the same bounded interaction
fallback through Galaxy Navigator. Neither surface uses a continuous
animation loop or synthetic production data.

Visual track actions reuse the existing capability/action policy and
`ContextActionMenu`. The radial menu shows at most eight visible actions and
routes overflow to the linear More menu; the dnd-kit panel has Play Next,
Queue, and Inbox targets plus keyboard buttons. Canceled or invalid drops do
not mutate queue state.

`PreviewService` is intentionally separate from `PlaybackService`. A
TrackId is resolved through the managed local library, online/missing sources
are rejected, playback phases Playing/Seeking/Loading/Recovering return the
stable `Pause playback to preview.` error, and an owned process is capped at
eight seconds with a 35% volume ceiling. Preview state is idle/loading/
playing/failed, is canceled on explicit user loss of context or main playback,
and never writes queue, history, analytics, SMTC, or provider state.

Theme Studio edits a schema-v1 15-token draft, previews it only for the
current session, and persists only after Save & Activate. Import/export uses
the same validated theme shape; clone actions start from Dark, Light, or the
current theme. Dynamic accent is session-only, samples no more than 32x32
client-side artwork pixels, applies contrast checks, and falls back to the
existing accent pair. Layout Workspace reuses the existing persisted layout
profiles.
