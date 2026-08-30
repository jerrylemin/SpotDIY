# SpotDIY architecture

SpotDIY is a single Tauri 2 application with a React frontend and a Rust native core. The frontend owns presentation and interaction state; Rust owns filesystem, database, process, provider, and Windows integration boundaries.

```text
React routes/components
        │ typed invoke + Zod validation
        ▼
Tauri commands / IPC DTOs
        ▼
Rust services ── SQLite WAL / local filesystem / managed tools / providers
        │
        └── playback adapter (mpv JSON IPC)
```

Frontend state uses Zustand for command palette, player presentation, overlay, and layout interaction state. TanStack Query owns asynchronous backend data such as search pages, library pages, downloads, lyrics, settings, and analytics. Authoritative records must not be duplicated across stores.

The backend will expose small service interfaces. Provider adapters report capability sets and normalize provider results into shared DTOs. `SourceFusionService` matches sources into `UnifiedTrack` records, while `SourceResolver` selects playable sources according to the user’s ordered preferences.

Standard storage targets `%LOCALAPPDATA%\SpotDIY`; portable mode is deterministic at startup and keeps its data beside the executable. Secrets use Windows Credential Manager, never SQLite or source control.
