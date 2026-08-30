# SpotDIY project structure

```text
src/                         React/TanStack frontend
  app/                       App shell and cross-cutting composition
  components/                Shared shell, icons, empty states, badges
  hooks/                     TanStack Query hooks
  pages/                     Route-level screens
  services/                  Typed native IPC boundary
  stores/                    Zustand interaction state
  styles/                    SpotDIY visual system
  types/                     Shared frontend domain vocabulary
src-tauri/                   Tauri 2 Rust application
  src/domain/                Provider capability vocabulary
  src/ipc/                   Serialized native DTOs
  icons/                     Generated Windows/app icon assets
tests/                       Frontend behavior tests
docs/superpowers/specs/      Approved design specification
docs/superpowers/plans/      Independent implementation plans
docs/SpotDIY-Vault/          Human-readable project memory and research
docs/execution/              Machine/human execution ledger
.github/workflows/           Windows CI
public/                      Brand source assets
```

The intended Rust service boundaries are `LibraryService`, `SearchService`, `SourceFusionService`, `SourceResolver`, `PlaybackService`, `DownloadService`, `LyricsService`, `PlaylistService`, `QueueService`, `SettingsService`, `BackupService`, `AnalyticsService`, and `MediaToolManager`. Add them only when their slice is implemented; do not create empty façade modules.
