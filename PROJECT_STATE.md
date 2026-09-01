# SpotDIY project state

State date: 2026-08-31

## Repository

- Branch: `main`
- Origin: `https://github.com/jerrylemin/SpotDIY`
- Plan 04 feature commit: `536617d` (`feat: add mpv playback service and queue transport`)
- Plan 04 review-fix commit: `af66127` (`fix: harden mpv playback lifecycle and event ordering`)
- Delivery status: implementation, independent review, final gates, and documentation closure are recorded on `main`; remote SHA verification is the final delivery check.

## Runtime

- Frontend: React 19, TypeScript 6 strict, Vite 8, TanStack Router/Query, Zustand, Zod.
- Native: Tauri 2, Rust stable MSVC, SQLite WAL, typed serialized DTOs, and runtime frontend parsing.
- Library: `LibraryService` owns persistent folder roots, recursive local indexing, metadata/artwork/fingerprint evidence, watcher reconciliation, and managed-source path validation.
- Playback: `PlaybackService` is the sole serialized controller. It owns the transient ID-only queue, snapshot revisions, transport, repeat/shuffle/previous/EOF policy, source switching, recovery, and shutdown.
- Backend: `MpvBackend` starts one external `mpv.exe` child over one fresh Windows named pipe and keeps JSON protocol/process details behind the backend boundary. Discovery is `SPOTDIY_MPV_PATH`, then PATH.
- Tauri playback surface: `get_playback_snapshot`, `play_track`, `enqueue_track`, `play_track_next`, `toggle_play_pause`, `seek_playback`, `next_track`, `previous_track`, `set_playback_volume`, `set_playback_muted`, `set_repeat_mode`, `set_shuffle_enabled`, `get_audio_devices`, `set_audio_device`, `switch_playback_source`, `retry_playback_backend`, and `clear_playback_queue`; state events use `playback://state`.
- Build cache: Rust/Tauri output is external at `C:\CargoTarget\SpotDIY`; `src-tauri\target` is absent and the path is not committed.

## Decisions in force

- Keep one Tauri application and keep provider-specific logic behind later adapter boundaries.
- Use explicit Rust DTOs plus strict Zod parsing at the IPC boundary; frontend commands carry typed IDs and values, never local paths, pipe names, request IDs, URLs, or raw mpv JSON.
- Permit playback only for managed, indexed, enabled, available local sources resolved by Rust through `LibraryService`.
- Keep the Plan 04 queue transient and non-persistent; persistent queue and queue snapshots belong to Plan 08.
- Use the exact mpv startup arguments in the Plan 04 specification, positive request IDs, bounded 1 MiB frames, six property observations, generation-scoped events, and bounded quit/kill/reap.
- Keep standard data under `%LOCALAPPDATA%\SpotDIY`; `SPOTDIY_PACKAGED_DATA_ROOT` is a smoke-only isolation seam because Windows known-folder resolution does not follow a child `LOCALAPPDATA` override.
- Keep provider playback/search, Source Fusion, downloads, lyrics, playlists, overlays, media keys/SMTC, portable mode, analytics, EQ, normalization, crossfade, ReplayGain, and unrelated refactors out of Plan 04.

## Plan 04 verification snapshot

- Rust: 117 all-target tests pass; formatting and all-features clippy with warnings denied pass; focused playback, protocol, queue, source-resolution, and shutdown behavior is covered.
- Frontend: typecheck, lint, 26 Vitest tests, and production build pass; PlayerBar, local-library actions, and Ctrl+K transport are functional.
- Browser: 9 Playwright runs pass across the 1280, 1920, and 2560 viewport projects with the browser-only typed IPC adapter.
- Native: real synthetic WAV mpv smoke passes load, position, pause/resume, seek, volume/mute, devices, EOF, shutdown, and process exit.
- Packaged: release executable smoke passes local indexing, playback transport, graceful close, owned-mpv cleanup, restart library persistence, and empty transient queue; no temporary profile or owned process remains.
- Review: the single fresh read-only reviewer rechecked the fixes with `PASS`; critical, high, and correctness/security medium findings are zero. One low-priority request for additional hostile-probe regression coverage remains non-blocking.
- Development mpv: local `.tools\mpv\v0.41.0\mpv.exe`, version `v0.41.0-dev-g41f6a6450`, SHA-256 `6145E63F026451A764077D53FD60860EC9F5C2BC76DCD6E62A88967AC375453D`. The documented official Windows x64 asset verification is recorded separately in the execution log.

## Next slice

Plan 05 — Source Adapters and Search. Do not begin Source Fusion or provider playback in that slice until its own boundary is specified.

## Plan 05 delivery snapshot (2026-09-01)

- Delivery status: COMPLETE. Provider adapters, SearchService lifecycle,
  strict search IPC, frontend search surface, Spotify PKCE gate, browser
  matrix, native smoke, live metadata smoke, and packaged smoke are delivered
  through `ab6169d` plus the documentation closure commit.
- Search execution is concurrent and provider-independent. Local, YouTube, and
  SoundCloud participate in unified lenses; Spotify is isolated to the Spotify
  lens and remains disabled without explicit developer authorization.
- Search IDs, cancellation, timeout/error sections, stale-event rejection,
  exact completion, provider-local sorting, and bounded cache behavior are
  covered by native and frontend tests. Early native events are buffered until
  the start response supplies the active SearchId.
- Spotify uses loopback Authorization Code with S256 PKCE, memory/keyring token
  storage, no client secret, and no Spotify data in SQLite.
- Plan 05 adds no database migration. Cargo/Tauri output remains external at
  `C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` remains absent.

## Next slice after Plan 05

Plan 06 - Source Fusion and Resolver is not started; stop here until that
boundary is active.
