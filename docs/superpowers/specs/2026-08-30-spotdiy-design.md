# SpotDIY design specification

Date: 2026-08-30
Status: approved baseline; implementation proceeds in the independently testable plans under `docs/superpowers/plans/`.

## 1. Product intent

SpotDIY is a Windows 11 x64-first local music operating environment. It combines a user’s local music with YouTube, SoundCloud, and Spotify catalog metadata while keeping user data local, useful offline, and free of a SpotDIY account or registration flow. Online providers augment the local application; they do not become the application’s identity system.

The central product idea is the Unified Music Model. A musical work is one `UnifiedTrack` with one or more `TrackSource` records. A source may be a local FLAC/MP3, a YouTube result, a SoundCloud upload, or Spotify catalog metadata. Source Fusion prevents provider results for the same work from becoming permanent duplicates.

## 2. Approved technical shape

The product is one Tauri 2 application. The frontend uses React, strict TypeScript, Vite, Zustand, TanStack Query, TanStack Router, TanStack Virtual, dnd-kit, Motion, and Zod at untrusted boundaries. The native core uses stable Rust, Tokio, Serde, Reqwest, Rusqlite with SQLite WAL, Thiserror, Tracing, UUID, Chrono, URL, keyring, Lofty when compatibility is verified, Notify, Walkdir, Zip, SHA-256, and a maintained string-similarity implementation.

The frontend owns rendering and interaction state. Zustand owns player presentation, queue presentation, overlays, transient selections, layout, and command-palette state. TanStack Query owns backend data such as search results, library pages, downloads, lyrics, analytics, and settings. Rust owns database, filesystem, provider/network, process, media, and Windows boundaries. Initial IPC is explicit Serde DTOs validated with Zod; generated bindings may replace repetition only after compatibility is verified.

Required service boundaries are `LibraryService`, `SearchService`, `SourceFusionService`, `SourceResolver`, `PlaybackService`, `DownloadService`, `LyricsService`, `PlaylistService`, `QueueService`, `SettingsService`, `BackupService`, `AnalyticsService`, and `MediaToolManager`. Provider-specific behavior stays inside adapters; the UI consumes capabilities rather than provider-name conditionals.

## 3. Provider contracts

Every adapter reports `ProviderKind`, capabilities, identifiers, normalized search results, cancellation behavior, timeout/rate-limit mapping, and structured errors. `LOCAL` supports search, playback, metadata, artwork, embedded/sidecar lyrics, inspection, and quality. `YOUTUBE` and `SOUNDCLOUD` support provider-appropriate search, metadata, artwork, metrics, dates, playback resolution, and supported downloads. `SPOTIFY` supports catalog search and metadata only; it never provides downloadable Spotify audio, DRM circumvention, or raw Spotify playback.

When Spotify is configured, Client Credentials are stored locally through Windows Credential Manager. When configuration is absent, Spotify remains visible with a clear setup action. A Spotify catalog result is passed to `SourceResolver`, which searches local, YouTube, and SoundCloud for a playable equivalent.

Default source preference is: local lossless, local playable, official SoundCloud, official YouTube audio, official YouTube video, then other compatible sources. Settings lets users reorder this preference. Source switching preserves queue context, Now Playing state, safe timestamp, lyrics, playlist context, shuffle, and repeat.

## 4. Source Fusion

Before any ML, matching is deterministic. Normalize Unicode, whitespace, case, punctuation, artist separators, and common featuring syntax. Compare title (0.55), primary artists (0.35), duration (0.10), and explicit version qualifiers. Start with a conservative merge threshold near 0.88 and duration tolerance for minor provider variation. Live, acoustic, remix, remaster, cover, instrumental, karaoke, sped-up, slowed, and nightcore qualifiers are merge guards. False merges are more damaging than missed automatic matches.

Manual merge/split decisions are persisted in `user_track_overrides` and always outrank automatic matching. The test matrix includes official cross-provider matches, punctuation, feat/featuring, duration drift, all listed version guards, same-title/different-artist, and same-artist/different-song cases.

## 5. Search

Global search dispatches local, YouTube, SoundCloud, and configured Spotify queries independently after shared initialization. Faster providers render partial results without waiting for slower providers. Results show artwork/thumbnail, title, artist/channel, duration, compact badge (`LOCAL`, `YT`, `SC`, `SP`), provider metric/date, downloaded state, local quality, version labels, and capability-valid context actions.

Lenses are ALL, TRACKS, ARTISTS, ALBUMS, PLAYLISTS, LOCAL, YOUTUBE, SOUNDCLOUD, and SPOTIFY. Sorts are relevance, popularity, newest, oldest, duration, date added, downloaded, and audio quality with meaningful direction. Combined popularity uses normalized 0.0–1.0 scores but displays original provider metrics; provider metrics are never presented as numerically equivalent.

Ctrl+K opens a typed command registry with play/pause/next/previous, search, play artist/album/playlist, queue, download, open Downloads/Lyrics/Queue/Library/Settings, clear queue, shuffle liked songs, open storage, open mini player, and toggle overlay. Disabled commands explain why they are unavailable.

## 6. Library, storage, and database

Local library supports multiple recursive folders, incremental scans, filesystem watching, added/removed/renamed files where practical, embedded tags/artwork, sidecar lyrics, duration, codec, bitrate, sample rate, bit depth, file size, modified time, and content-fingerprint metadata. Unchanged files are not rescanned. Users can rescan and open a file’s location.

Standard mode stores application data under `%LOCALAPPDATA%\SpotDIY`; selected music stays at user paths. Portable mode selection is deterministic at startup and stores `Data`, `Music`, `Covers`, `Lyrics`, `Database`, `Cache`, and `Config` beside `SpotDIY.exe`. SQLite uses WAL and migrations with backups before destructive changes. Logical tables include tracks, artists, track_artists, albums, track_sources, local_files, playlists, playlist_items, queue_state, queue_snapshots, likes, ratings, tags, track_tags, bookmarks, lyrics, downloads, play_history, listening_sessions, smart_playlists, source_preferences, provider_cache, user_track_overrides, settings_metadata, and schema_metadata. FTS5 is used when the selected SQLite build supports it.

## 7. Playback and downloads

`PlaybackService` talks only to a `PlaybackBackend`; mpv JSON IPC is the primary planned backend. Playback states are idle, loading, playing, paused, seeking, ended, and failed. Queue operations cover next, previous, repeat track/queue, shuffle, source switch, missing-source fallback, crash recovery, output device, and volume. `MediaToolManager` locates/validates mpv, FFmpeg, and yt-dlp, reports versions/health, and verifies managed downloads.

Downloads persist queued, resolving, downloading, post-processing, completed, failed, and cancelled states. Each task records provider, URL/ID, track identity, destination, format, codec, expected/downloaded bytes, progress, speed, ETA, retry count, errors, and timestamps. The queue survives restart, supports conservative configurable concurrency, pause scheduling where feasible, cancel, retry, filename sanitization, audio-only/video choice, and metadata/artwork. A lossy source converted to FLAC remains lossy-origin and is labeled honestly.

## 8. Playlists, queue, lyrics, and history

Playlists support create, rename, delete, duplicate, reorder, multi-select, drag/drop, add/remove, play, play-next, queue, download, offline keeping, and export. Branches support create branch, keep separate, merge selected changes, and discard branch without becoming a full version-control engine. Smart playlists use SQL-friendly AND/OR rules across artist, album, genre, year, dates, play/skip count, rating, liked, downloaded, provider, audio quality, duration, and tags.

Queue has `UP NEXT`, `LATER`, and `AUTOPLAY`; entries can be dragged, pinned, removed, played next, added later, cleared, saved, and restored. Queue snapshots persist ordered entries, current track/source/time, shuffle/repeat state, section positions, name, and creation time. Listening sessions group playback history with timestamps, tracks, durations, context, playlist origin, and optional label. Inbox is a lightweight unsorted holding area.

Lyrics load embedded tags or sidecar LRC first, then optional LRCLIB/provider lookup. Timed lines synchronize in the player and can render dual-language and Lyrics Focus views. Timestamp bookmarks and notes render on the waveform; A/B loop stores A, B, and optional per-track presets. Per-track start/end offsets, gain, and preferred source never modify original audio.

## 9. UI and power surfaces

The default visual direction is premium minimal plus power-user depth. Home, Search, Library, Playlists, and Player remain clean; inspectors, expandable panels, command palette, context menus, and Settings reveal advanced depth. Use strong hierarchy, intentional whitespace, selective glass/blur, and Motion only to explain panel/track/source/queue/lyrics state. Respect reduced motion and stop background decoration.

The custom identity uses rounded geometric SVG icons with consistent optical weight and a shared waveform notch motif: play, queue, download, library, source, lyrics, local, online, fusion, history, and shuffle. The SpotDIY mark shows multiple source paths converging into one waveform/play form and works at app-icon, sidebar, monochrome, light, and dark sizes. Provider colors remain restrained and secondary.

Main player exposes artwork, metadata, transport, seek/waveform, volume, source switcher, lyrics, queue, bookmarks, A/B controls, quality/provenance, and inspector access. Mini Player, Micro Mode, Lyrics Overlay, Gaming Overlay, Edge Player, Dynamic-Island-style overlay, and always-on-top behavior are capability-tested Windows surfaces. Gaming click-through is implemented only if the selected Tauri/Win32 route is reliable.

Advanced workspaces include Track Inspector, Playback Inspector, metadata editor, duplicate detector, audio-upgrade detector, Library Health, listening analytics, heatmap, Taste Timeline, Time Machine, Private Session, Temporary Mode, Smart Shuffle, Discovery controls, Theme Studio, layout profiles, Music Map, Library Galaxy, radial context menu, drag actions, hover/audio preview, and dynamic theme. They must have real paths and tests before acceptance; no static mock cards count as implementation.

## 10. Import, export, privacy, security

`.spotdiy` is ZIP-compatible with manifest, schema version, database snapshot, settings excluding secure secrets unless explicitly encrypted, playlists, likes, ratings, tags, bookmarks, history, queue snapshots, lyrics, themes, layouts, and selected provider cache. Optional audio/covers/lyrics/history inclusion is explicit. Imports validate version, checksums, archive paths, and file references, create rollback backups, run transactionally, report missing files, and cannot destroy the current library on failure.

Secrets never enter Git, SQLite settings, logs, screenshots, or reports. Raw provider URLs and user inputs are validated; process arguments are structured; sidecars are checksum-verified. No automatic media deletion exists. Telemetry is off by default and analytics stay local.

## 11. Acceptance and verification

The staged acceptance path is: launch packaged Windows build; choose optional local folder; index/play local music; search local and configured online providers with independent sections; match and switch sources; use queue/playlists/likes; download supported sources with persisted progress; display lyrics; use mini player/overlay; import/export; restart and confirm persistence; exercise portable mode. Advanced acceptance additionally requires every named advanced workspace to have a real implemented path and relevant tests.

Rust tests cover migrations, repositories, fusion, queue/shuffle, backup, provider parsers, and media process parsing. Frontend tests cover state, interactions, routing, and accessibility. CI mocks online providers and excludes live integrations. Visual QA uses mocked-IPC Playwright states at 1280×720, 1920×1080, 2560×1440, and an ultrawide viewport. Release verification records evidence in `docs/SpotDIY-Vault/Sessions/final-verification.md` and performance results in `docs/SpotDIY-Vault/Research/performance-baseline.md`.
