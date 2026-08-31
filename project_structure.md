# SpotDIY project structure

```text
src/                         React/TanStack frontend
  app/                       App shell and cross-cutting composition
  components/                Shared shell, icons, empty states, badges
    library/                 Folder rows, track rows, quality/provenance states
    player/                  Playback controls, progress, volume, audio device menu
  hooks/                     TanStack Query and playback hooks
    useLibrary.ts            Library status/page mutations and scan progress
    usePlayback.ts           Playback snapshot and transport mutations
  pages/                     Route-level screens
  services/                  Typed native IPC boundary
  stores/                    Zustand interaction state
  styles/                    SpotDIY visual system
  types/                     Shared frontend domain vocabulary
src-tauri/                   Tauri 2 Rust application
  migrations/                Ordered SQLite schema migrations (through 0002)
  src/domain/                Typed unified music domain model
  src/db/                    SQLite initialization and focused repositories
  src/ipc/                   Serialized native DTOs and status commands
  src/library/               Folder ownership, scanner, metadata, fingerprints, artwork, watcher
  src/media_tools/           mpv discovery, validation, version, and health
  src/playback/              Typed playback contracts, JSON IPC, mpv backend, queue, and controller
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
generation-stamped events are filtered at the controller boundary.
`SearchService`, `SourceFusionService`, `SourceResolver`, `DownloadService`,
`LyricsService`, `PlaylistService`, `QueueService`, `BackupService`, and
`AnalyticsService` remain later-plan boundaries and have no empty facade
modules.
