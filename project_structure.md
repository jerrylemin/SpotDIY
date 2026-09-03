# SpotDIY project structure

```text
src/                         React/TanStack frontend
  app/                       App shell and cross-cutting composition
  components/                Shared shell, icons, empty states, badges
    library/                 Folder rows, track rows, quality/provenance and collection states
    queue/                   Persistent queue drawer, sections, snapshots, and drag handles
    player/                  Playback controls, progress, volume, audio device menu, source switcher, Mini/Expanded player surfaces
    inspector/               InspectorPanel, persisted Track Inspector, ephemeral search-result inspector
    search/                  Search controls, provider sections, result cards
    downloads/               Persistent task rows, progress, provenance, and actions
    lyrics/                  Synchronized lyrics, source precedence, and manual actions
  hooks/                     TanStack Query, lyrics, playback, and Windows hooks
    useLibrary.ts            Library status/page mutations and scan progress
    usePlayback.ts           Playback snapshot and transport mutations
    useSearch.ts             Debounced provider search lifecycle and stale-ID handling
    useDownloads.ts          Persistent download snapshots, events, and task mutations
    useQueue.ts              Queue workspace/event bridge and native queue mutations
    useLyrics.ts             Local-first lyrics queries, edits, provider actions, and sync state
  useTrackInspector.ts     Read-only persisted Track Inspector query
  useWindowsIntegration.ts Windows settings, native status, overlays, and profiles
  pages/                     Route-level screens
  services/                  Typed native IPC boundary
  stores/                    Zustand interaction state
  styles/                    SpotDIY visual system and native-overlay presentation
  types/                     Shared frontend domain vocabulary
src-tauri/                   Tauri 2 Rust application
  migrations/                Ordered SQLite schema migrations (through 0008)
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
  src/playback/              Typed playback contracts, JSON IPC, mpv backend, queue, controller, and output profiles
  src/windows/               Native overlays, tray, shortcuts, SMTC, and click-through recovery
  crates/spotdiy-windows-smtc/ Isolated Windows SMTC WinRT bridge
  src/search/                Concurrent provider search, cache, cancellation, and timeouts
  src/sources/               Local, YouTube, SoundCloud, Spotify adapters, and SourceResolver
  src/credentials/           Keyring-backed and memory-only credential seam
  src/settings/              Typed durable settings repository
  capabilities/              Narrow desktop and overlay window permissions
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

## Plan 10 frontend structure

The Plan 10 design-system boundary is organized as follows:

```text
src/styles/tokens.css, foundations.css, primitives.css
        |
        +--> src/features/theme/theme-schema.ts
        +--> src/features/theme/theme-presets.ts
        +--> src/features/theme/theme-controller.tsx
        +--> src/features/layout/layout-profiles.ts
        +--> src/components/common/* (Button, Field, Surface, menus, etc.)
        +--> src/components/icons/SpotIcon.tsx, IconGallery.tsx
        `--> src/components/inspector/InspectorPanel.tsx
```

`ThemeController` owns resolved Dark/Light/System/Custom presentation state
and applies `data-theme`/`data-layout` attributes at the document root. The
validated custom-theme definition is shared by the frontend Zod boundary and
the Rust settings DTOs. `SettingsPage` owns the user-facing appearance
controls; native persistence remains behind `SettingsRepository`, while the
browser preview uses its bounded in-memory adapter. The shared primitives own
focus, labels, disabled explanations, keyboard behavior, and reduced-motion
defaults. The InspectorPanel/IconGallery is a development/design surface and
does not add permanent navigation.

## Plan 11 shell structure

`AppShell` owns cross-surface presentation composition and Escape priority:
command palette, inspector, queue drawer, then Expanded Now Playing. Zustand
stores only session UI state (`playerMode` and inspector selection); TanStack
Query owns inspector data and existing service snapshots. `PlayerBar` routes to
the Standard footer, `MiniPlayer`, or `NowPlayingPanel`, all of which consume
the same `usePlayback()` snapshot.

`TrackInspectorService` and `get_track_inspector` expose purpose-built metadata,
collection state, source capabilities, measured local quality, and validated
remote canonical URLs without local paths. `track-actions.ts` is the pure
capability/runtime policy used by search cards and shell menus. The packaged
Plan 11 smoke covers migration 7, appearance persistence, live Home, inspector
privacy, player modes, queue/Lyrics/palette navigation, restart persistence,
and no-autoplay behavior.

## Plan 12 Windows structure

The native Windows boundary is organized as follows:

```text
src-tauri/src/windows/mod.rs
        +--> overlays.rs       lazy native overlay windows and click-through
        +--> shortcuts.rs      global shortcut registry and status reporting
        +--> tray.rs            tray menu/action dispatch
        `--> smtc.rs            typed media controls and isolated WinRT bridge

src-tauri/src/playback/output.rs --> output-device/profile validation and apply
src/components/overlay/*          --> native-window React surfaces
src/components/settings/*         --> Windows Integration settings controls
src/hooks/useWindowsIntegration.ts + overlay-store.ts --> typed UI state
```

`WindowsIntegrationService` is the single native owner. Overlay visibility and
Gaming click-through are session state; settings, bindings, and output profiles
are durable ordinary records. The frontend receives only validated snapshots,
status details, typed overlay kinds, and profile values.

## Plan 13 backup and storage structure

```text
src-tauri/src/storage/mod.rs
        +--> deterministic Standard/Portable layout resolver
        +--> marker, directory, writable-root, and mode-switch ownership
        `--> StorageStatus / StorageModeSwitchResult

src-tauri/src/backup/mod.rs
        +--> archive.rs  online snapshot and deterministic ZIP export
        +--> manifest.rs format-1 manifest, paths, mappings, and bounds
        `--> import.rs   secure staging, preview, pending descriptor, apply/rollback

src-tauri/src/lib.rs
        +--> native save/pick/folder dialogs and typed backup/storage commands
        `--> startup storage selection before Database::open

src/components/backup/* + src/hooks/useBackup.ts
        `--> Settings backup/export/import preview and storage status surface
```

`StorageLayout` is resolved before SQLite opens. The marker wins over every
other mode hint and Portable startup does not fall back to AppData.
`BackupService` owns archive lifecycle and pending-restore state; active
database replacement, media creation, and rollback remain native and
restart-gated. The frontend sees validated DTOs only and never supplies
arbitrary export/import paths.

## Plan 14 smart features and analytics structure

```text
src-tauri/src/analytics/mod.rs  --> recorder, sessions, history, aggregates,
                                   heatmap, timeline, time machine, reopen
src-tauri/src/sessions/mod.rs   --> in-memory Private/Temporary mode state
src-tauri/src/smart/mod.rs      --> typed rules, SQL compiler, CRUD, preview,
                                   deterministic Smart Shuffle
src-tauri/src/playback/mod.rs  --> queue ownership, mode transitions, history hooks
src-tauri/migrations/0009_smart_analytics.sql

src/pages/AnalyticsPage.tsx + src/components/analytics/*
src/components/smart/SmartPlaylistPanel.tsx
src/hooks/useAnalytics.ts + useSmartPlaylists.ts + useListeningModes.ts
```

The native services are local-only and the frontend receives Zod-validated
DTOs. `PlaybackService` remains the only owner allowed to open a smart mix
into the live queue.

## Plan 15 advanced visual exploration structure

```text
src-tauri/src/visual_explorer/mod.rs  --> bounded read-only dataset DTO/query
src-tauri/src/preview/mod.rs          --> local eight-second preview lifecycle
src-tauri/src/lib.rs                  --> visual dataset and preview IPC wiring

src/features/music-map/layout.ts      --> deterministic GENRE/ARTIST/ALBUM/TRACK graph
src/features/library-galaxy/layout.ts --> deterministic clustered Canvas positions
src/pages/MusicMapPage.tsx            --> SVG map, filters, navigator, actions
src/pages/LibraryGalaxyPage.tsx       --> Canvas galaxy, filters, navigator, actions
src/features/visual-exploration/*     --> shared actions and drag/drop targets
src/components/radial-menu/*          --> bounded radial actions and More fallback

src/features/theme/ThemeStudio.tsx
src/features/theme/theme-controller.tsx
src/features/theme/theme-studio/dynamic-accent.ts
src/features/layout/LayoutWorkspace.tsx
                                      --> draft themes, session preview, accent, layout
```

The visual routes consume one typed/Zod-validated dataset and do not invent
production data in browser preview. Native preview resolves only an indexed
managed local source through the existing library/playback policy; it never
joins queue, history, analytics, SMTC, or provider behavior.
