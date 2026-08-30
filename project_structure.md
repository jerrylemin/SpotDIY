# SpotDIY project structure

```text
src/                         React/TanStack frontend
  app/                       App shell and cross-cutting composition
  components/                Shared shell, icons, empty states, badges
    library/                 Folder rows, track rows, quality/provenance states
  hooks/                     TanStack Query hooks
    useLibrary.ts            Library status/page mutations and scan progress
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
  src/settings/              Typed durable settings repository
  capabilities/              Narrow dialog/opener permissions for the desktop window
  icons/                     Generated Windows/app icon assets
tests/                       Frontend behavior tests
docs/superpowers/specs/      Approved design specification
docs/superpowers/plans/      Independent implementation plans
docs/SpotDIY-Vault/          Human-readable project memory and research
docs/execution/              Machine/human execution ledger
.github/workflows/           Windows CI
public/                      Brand source assets
```

The intended Rust service boundaries are `LibraryService`, `SearchService`, `SourceFusionService`, `SourceResolver`, `PlaybackService`, `DownloadService`, `LyricsService`, `PlaylistService`, `QueueService`, `SettingsService`, `BackupService`, `AnalyticsService`, and `MediaToolManager`. Add them only when their slice is implemented; do not create empty facade modules.
