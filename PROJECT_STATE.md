# SpotDIY project state

State date: 2026-09-01

## Repository

- Branch: `main`
- Origin: `https://github.com/jerrylemin/SpotDIY`
- Plan 04 feature commit: `536617d` (`feat: add mpv playback service and queue transport`)
- Plan 04 review-fix commit: `af66127` (`fix: harden mpv playback lifecycle and event ordering`)
- Plan 07 implementation commits: `0dbb628`, `22438a0`, and `6012921`.
- Plan 08 implementation commits: `525da8c`, `e5f7161`, `1f31d6a`, and `0a62cad`.
- Delivery status: Plan 08 implementation and final verification are complete; this document is part of the documentation closure boundary.

## Runtime

- Frontend: React 19, TypeScript 6 strict, Vite 8, TanStack Router/Query, Zustand, Zod.
- Native: Tauri 2, Rust stable MSVC, SQLite WAL, typed serialized DTOs, and runtime frontend parsing.
- Library: `LibraryService` owns persistent folder roots, recursive local indexing, metadata/artwork/fingerprint evidence, watcher reconciliation, and managed-source path validation.
- Playback: `PlaybackService` is the sole serialized controller. It owns the persistent ID-only queue, checkpointed position, immutable snapshots, transport, repeat/shuffle/previous/EOF policy, source switching, recovery, and shutdown.
- Playlists: `PlaylistService` owns durable playlists, seeded Inbox, playlist items, one-shot branches, likes, ratings, tags, and bounded collection reads.
- Backend: `MpvBackend` starts one external `mpv.exe` child over one fresh Windows named pipe and keeps JSON protocol/process details behind the backend boundary. Discovery is `SPOTDIY_MPV_PATH`, then PATH.
- Tauri playback surface: `get_playback_snapshot`, `play_track`, `enqueue_track`, `play_track_next`, `toggle_play_pause`, `seek_playback`, `next_track`, `previous_track`, `set_playback_volume`, `set_playback_muted`, `set_repeat_mode`, `set_shuffle_enabled`, `get_audio_devices`, `set_audio_device`, `switch_playback_source`, `retry_playback_backend`, `clear_playback_queue`, playlist playback/queue commands, queue workspace mutations, and queue snapshot commands; state events use `playback://state` and `queue://state`.
- Downloads: `DownloadService` owns schema-v4 task persistence, yt-dlp/FFmpeg execution, bounded progress, scheduling, cancellation, retry, restart recovery, destination-side finalization, and `downloads://state` snapshots. Tasks support YouTube and SoundCloud only; Spotify and Local are rejected.
- Tauri download surface: `get_download_snapshot`, `queue_search_result_download`, `queue_source_download`, `cancel_download`, `retry_download`, `set_download_concurrency`, and `open_download_location`.
- Build cache: Rust/Tauri output is external at `C:\CargoTarget\SpotDIY`; `src-tauri\target` is absent and the path is not committed.

## Decisions in force

- Keep one Tauri application and keep provider-specific logic behind later adapter boundaries.
- Use explicit Rust DTOs plus strict Zod parsing at the IPC boundary; frontend commands carry typed IDs and values, never local paths, pipe names, request IDs, URLs, or raw mpv JSON.
- Permit playback only for managed, indexed, enabled, available local sources resolved by Rust through `LibraryService`.
- Keep `PlaybackService` as the sole queue owner; durable queue state and snapshots use typed IDs and never expose paths, URLs, or raw queue JSON through IPC.
- Use the exact mpv startup arguments in the Plan 04 specification, positive request IDs, bounded 1 MiB frames, six property observations, generation-scoped events, and bounded quit/kill/reap.
- Keep standard data under `%LOCALAPPDATA%\SpotDIY`; `SPOTDIY_PACKAGED_DATA_ROOT` is a smoke-only isolation seam because Windows known-folder resolution does not follow a child `LOCALAPPDATA` override.
- Keep download task temp files under `%LOCALAPPDATA%\SpotDIY\cache\downloads\<DownloadTaskId>`, create final names inside the user-selected `downloads_directory`, and never expose arbitrary paths through IPC.
- Preserve provider-encoded provenance for YouTube/SoundCloud downloads; no lossy-to-FLAC claim, raw provider payload, credential, token, or automatic library mutation is allowed.
- Keep provider playback/search, lyrics, overlays, media keys/SMTC, portable mode, analytics, EQ, normalization, crossfade, ReplayGain, and unrelated refactors outside the Plan 08 boundary.

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

Plan 06 - Source Fusion and Resolver is complete through implementation tip
`afd0149` and the documentation closure that follows. The next slice is Plan
07 only after the external review requested by the delivery workflow.

## Plan 06 delivery snapshot (2026-09-01)

- Deterministic source normalization uses Unicode NFKD, accent/punctuation and
  presentation-noise cleanup, feature-artist extraction, guarded version
  qualifiers including Nightcore, and conservative artist-prefix handling.
- Automatic matching uses Jaro-Winkler integer basis points with title 55%,
  artists 35%, duration 10%, an 8800 threshold, 9000 title/artist hard
  minimums, explicit duration bands, exact guarded-version equality, stable
  ambiguity handling, and typed explanations.
- Migration 3 adds only `user_track_overrides`. Merge overrides have one
  forced target per provider identity; split overrides are target-specific;
  Spotify is rejected; search evaluation remains read-only; explicit accepted
  YouTube/SoundCloud sources are persisted without local-file rows or metadata
  moves.
- `SourceResolver` ranks the preferred playable source first, then validated
  settings/provider order and local quality. Local readiness requires the
  managed library path; YouTube/SoundCloud remain not implemented for
  playback; Spotify remains metadata-only. Playback and source switching use
  the resolver, and unavailable sources carry typed explanations.
- Final evidence: 279 Rust tests, 40 Vitest tests, 45 Playwright runs, strict
  typecheck/lint/fmt/clippy, frontend and Tauri builds, real mpv smoke,
  packaged playback/restart/cleanup smoke, and an explicit v2-to-v3 migration
  smoke all pass. Cargo output remains at `C:\CargoTarget\SpotDIY`; the
  repository-local `src-tauri\target` is absent.

## Prior Plan 06 handoff

Plan 06 was the predecessor boundary; its external review gate was completed before Plan 07 implementation began.

## Plan 07 delivery snapshot (2026-09-01)

- Delivery status: COMPLETE through implementation tip `6012921`; documentation closure is the final Plan 07 commit.
- Schema version 4 adds only `downloads` and the singleton `download_settings` table. Existing schema-3 data is preserved, YT/SC sources gain downloads capability, and playback capability remains unchanged.
- `DownloadService` persists UUID tasks, owns bounded yt-dlp children, parses machine progress, schedules up to four tasks, cancels/reaps owned processes, retries without duplication, recovers active tasks after restart, retains missing completed outputs, and finalizes across volumes without overwriting existing files.
- Downloads UI exposes task history, progress, bytes, speed, ETA, state, retry count, provider-encoded provenance, output format, errors, output-missing state, folder selection, concurrency, cancel/retry, and trusted folder opening. Search offers Audio/Video only for YouTube/SoundCloud tracks; Spotify and Local remain unavailable.
- Final evidence: 308 Rust unit tests plus the synthetic mpv integration test, 47 Vitest tests, 45 Playwright runs, typecheck/lint/build, fmt/clippy, Tauri release packaging, explicit real-mpv smoke, packaged playback/restart/cleanup smoke, and five native provider-search smoke checks pass. Optional live provider/download smoke was not run; the existing optional packaged-search harness still has an immediate start/cancel race when yt-dlp is intentionally missing.
- Storage remains external at `C:\CargoTarget\SpotDIY` for build output and `%LOCALAPPDATA%\SpotDIY\cache\downloads\<DownloadTaskId>` for owned task temp files. No media, credentials, tokens, or raw provider payloads are committed.

## Historical next slice after Plan 07

Plan 08 is the completed follow-on delivery recorded below.

## Plan 08 delivery snapshot (2026-09-01)

- Schema version 5 adds durable `playlists`, `playlist_items`, branch-base
  snapshots, `likes`, `ratings`, `tags`, `track_tags`, `queue_entries`,
  `queue_state`, `queue_snapshots`, and immutable `queue_snapshot_entries`.
  Existing Plan 07 tracks, sources, overrides, downloads, and settings survive
  the v4-to-v5 migration.
- `PlaylistService` provides normal playlist CRUD, duplicate/remove/reorder,
  deterministic seeded Inbox, one-level one-shot branches with base snapshots,
  selected merge/discard and revision conflicts, likes, ratings 1..5, normalized
  tags, and bounded batch collection state.
- `PlaybackService` remains the sole queue owner. Up Next and Later are ordered
  sections, Autoplay is structurally empty, shuffle affects Later only, position
  checkpoints are throttled, startup restores state without autoplay, first Play
  resumes the saved current item/position, and snapshots restore fresh live IDs.
- Typed Rust/Zod IPC and `queue://state` bridge the native owner to the
  presentation-only queue drawer. Playlists and library rows expose the scoped
  collection actions; browser preview remains deterministic and native-free.
- Final evidence: 318 Rust unit tests plus synthetic and explicit real-mpv
  integration smoke, 51 Vitest tests, 48 Playwright runs, typecheck/lint/build,
  fmt/clippy, Tauri release packaging, packaged playback/restart smoke, explicit
  Plan 08 playlist/collection/queue/snapshot/restart smoke, and the v4-to-v5
  migration smoke all pass.
- CodeGraph and Graphify were refreshed once after implementation. Their final
  counts are recorded in `docs/execution/verification-log.md`.

## Next slice after Plan 08

STOPPED AFTER PLAN 08. Awaiting external ChatGPT GitHub review before Plan 09.
