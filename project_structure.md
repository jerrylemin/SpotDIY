# SpotDIY project structure

```text
src/                         React/TanStack frontend
  app/                       App shell and cross-cutting composition
  components/                Shared shell, icons, empty states, badges
    library/                 Folder rows, track rows, quality/provenance states
    player/                  Playback controls, progress, volume, audio device menu
    search/                  Search controls, provider sections, result cards
  hooks/                     TanStack Query and playback hooks
    useLibrary.ts            Library status/page mutations and scan progress
    usePlayback.ts           Playback snapshot and transport mutations
    useSearch.ts             Debounced provider search lifecycle and stale-ID handling
  pages/                     Route-level screens
  services/                  Typed native IPC boundary
  stores/                    Zustand interaction state
  styles/                    SpotDIY visual system
  types/                     Shared frontend domain vocabulary
src-tauri/                   Tauri 2 Rust application
  migrations/                Ordered SQLite schema migrations (through 0003)
  src/domain/                Typed unified music domain model
  src/db/                    SQLite initialization and focused repositories
  src/fusion/                Deterministic normalization, matching, and overrides
  src/ipc/                   Serialized native DTOs and status commands
  src/library/               Folder ownership, scanner, metadata, fingerprints, artwork, watcher
  src/media_tools/           mpv discovery, validation, version, and health
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
`PlaybackService`, and `MediaToolManager`, plus the existing domain, database,
IPC, and settings modules. `PlaybackService` owns the serialized playback
controller and transient queue; `MpvBackend` owns the external process and
JSON IPC; `LibraryService` remains the only source-path ownership boundary.
The Plan 04 delivery tip is `af66127`; backend commands are enqueue-only and
generation-stamped events are filtered at the controller boundary. Plan 05
adds `SearchService`, the provider adapter boundary, and the credentials seam.
Plan 06 adds `SourceFusionService` and `SourceResolver`: search results remain
ephemeral until explicit YouTube/SoundCloud acceptance, local playback remains
owned by `LibraryService`, and provider capability truth remains backend-owned.
`DownloadService`, `LyricsService`, `PlaylistService`, `QueueService`,
`BackupService`, and `AnalyticsService` remain later-plan boundaries and have
no empty facade modules.
