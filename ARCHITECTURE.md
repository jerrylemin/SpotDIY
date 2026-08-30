# SpotDIY architecture

SpotDIY is a single Tauri 2 application with a React frontend and a Rust native core. The frontend owns presentation and interaction state; Rust owns filesystem, database, process, provider, and Windows integration boundaries.

```text
React routes/components
        |
        v
Tauri commands / IPC DTOs
        |
        v
Rust services -- SQLite WAL / local filesystem / managed tools / providers
        |
        +-- playback adapter (mpv JSON IPC)
```

Frontend state uses Zustand for command palette, player presentation, overlay, and layout interaction state. TanStack Query owns asynchronous backend data such as search pages, library pages, downloads, lyrics, settings, and analytics. Authoritative records must not be duplicated across stores.

The current native persistence seam is `Database` plus focused `TrackRepository`, `ArtistRepository`, `SourceRepository`, and `SettingsRepository` modules. Database initialization resolves the caller-provided path, creates the parent directory, enables and verifies WAL/foreign keys, applies ordered migrations, probes FTS5 availability, and exposes typed settings/status commands. The backend will expose additional small service interfaces as later plans implement them.

Provider adapters report capability sets and normalize provider results into shared DTOs. `SourceFusionService` will match sources into `UnifiedTrack` records, while `SourceResolver` will select playable sources according to the user's ordered preferences. Spotify catalog sources remain metadata-only.

Standard storage targets `%LOCALAPPDATA%\SpotDIY\spotdiy.sqlite3`; the current application opens that path through an explicit `Database::open(path)` seam. Portable startup and its beside-executable layout remain a later-plan concern, and persisted portable mode is rejected until that startup path exists. Secrets use Windows Credential Manager, never SQLite or source control.
