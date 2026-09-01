# SpotDIY project structure

```text
src/                         React/TanStack frontend
  app/                       App shell and cross-cutting composition
  components/                Shared shell, icons, empty states, badges
    library/                 Folder rows, track rows, quality/provenance and collection states
    queue/                   Persistent queue drawer, sections, snapshots, and drag handles
    player/                  Playback controls, progress, volume, audio device menu
    search/                  Search controls, provider sections, result cards
    downloads/               Persistent task rows, progress, provenance, and actions
    lyrics/                  Synchronized lyrics, source precedence, and manual actions
  hooks/                     TanStack Query, lyrics, and playback hooks
    useLibrary.ts            Library status/page mutations and scan progress
    usePlayback.ts           Playback snapshot and transport mutations
    useSearch.ts             Debounced provider search lifecycle and stale-ID handling
    useDownloads.ts          Persistent download snapshots, events, and task mutations
    useQueue.ts              Queue workspace/event bridge and native queue mutations
    useLyrics.ts             Local-first lyrics queries, edits, provider actions, and sync state
  pages/                     Route-level screens
  services/                  Typed native IPC boundary
  stores/                    Zustand interaction state
  styles/                    SpotDIY visual system
  types/                     Shared frontend domain vocabulary
src-tauri/                   Tauri 2 Rust application
  migrations/                Ordered SQLite schema migrations (through 0006)
  src/domain/                Typed unified music domain model
  src/db/                    SQLite initialization and focused repositories
  src/fusion/                Deterministic normalization, matching, and overrides
  src/ipc/                   Serialized native DTOs and status commands
  src/library/               Folder ownership, scanner, metadata, fingerprints, artwork, watcher
  src/media_tools/           mpv, yt-dlp, and FFmpeg discovery, validation, version, and health
  src/downloads/              Persistent tasks, scheduler, bounded runner lifecycle, and finalization
  src/bookmarks/              Durable bookmarks and A/B loop presets
  src/lyrics/                 Local-first lyrics service, parser, metadata, and LRCLIB boundary
  src/playlists/              Durable playlists, Inbox, branches, likes, ratings, tags, and collection state
  src/queue/                  Typed persistent queue model, repository, sections, and snapshots
  src/playback/              Typed playback contracts, JSON IPC, mpv backend, queue, and controller
  src/search/                Concurrent provider search, cache, cancellation, and timeouts
  src/sources/               Local, YouTube, SoundCloud, Spotify adapters, and SourceResolver
  src/credentials/           Keyring-backed and memory-only credential seam
  src/settings/              Typed durable settings repository
  capabilities/              Narrow dialog/opener permissions for the desktop window
  icons/                     Generated Windows/app icon assets
tests/                       Frontend behavior and browser tests
docs/superpowers/specs/      Approved design specification
docs/superpowers/plans/      Independent implementation plans
docs/SpotDIY-Vault/          Human-readable project memory and research
docs/execution/              Machine/human execution ledger
.github/workflows/           Windows CI
public/                      Brand source assets
```

The implemented Rust service boundaries are `LibraryService`,
`PlaylistService`, `PlaybackService`, `DownloadService`, `LyricsService`,
`BookmarkService`, and `MediaToolManager`, plus the existing
domain, database, IPC, and settings modules. `PlaybackService` owns the
serialized playback controller and persistent queue; `PlaylistService` owns
durable playlist and collection records; `DownloadService` owns
persistent task lifecycle, bounded yt-dlp children, progress, cancellation,
retry, recovery, and destination-side finalization. `MpvBackend` owns the
external playback process and JSON IPC; `LibraryService` remains the only
library source-path ownership boundary.
The Plan 04 delivery tip is `af66127`; backend commands are enqueue-only and
generation-stamped events are filtered at the controller boundary. Plan 05
adds `SearchService`, the provider adapter boundary, and the credentials seam.
Plan 06 adds `SourceFusionService` and `SourceResolver`: search results remain
ephemeral until explicit YouTube/SoundCloud acceptance, local playback remains
owned by `LibraryService`, and provider capability truth remains backend-owned.
`LyricsService` owns local-first source resolution and bounded synchronized
documents; `BookmarkService` owns durable bookmarks and presets. Plan 08
deliberately keeps queue ownership inside `PlaybackService` instead of adding a
separate `QueueService`; `BackupService` and `AnalyticsService` remain
later-plan boundaries with no empty facade modules. Plan 07 downloads do not
create library tracks, move media, or inject paths into playback.
