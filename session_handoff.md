# SpotDIY session handoff

Date: 2026-09-02
Branch: `main`
Origin: `https://github.com/jerrylemin/SpotDIY`
Plan 04 feature commit: `536617d` (`feat: add mpv playback service and queue transport`)
Plan 04 review-fix commit: `af66127` (`fix: harden mpv playback lifecycle and event ordering`)
Plan 05 delivery commits: `58b5adc`, `cbcb43e`, `facab20`, `0db5eac`,
`6c16747`, `51da804`, `905f322`, `0d52f9b`, `9b2a6d8`, and `ab6169d`.
Plan 10 implementation commits: `cc28ba1`, `f2a5995`, `850bc82`, `8c62aed`,
and `6eb231d`. Plan 11 implementation and smoke commits are `e5129a0`,
`f5562e1`, `0012a43`, `0026146`, `dba1f24`, `d631a2a`, `d2199d5`,
`e072fec`, and `15031bf`.

## Completed

- Plans 01–03 remain complete: the Tauri/React shell, typed domain, SQLite
  migrations through schema version 2, durable settings, and managed local
  library are implemented.
- Plan 04 adds `MediaToolManager`, the external Windows mpv backend, bounded
  newline-delimited JSON IPC, typed backend events, the serialized
  `PlaybackService`, transient ID-only queue, local source resolution,
  transport/repeat/shuffle/EOF/previous policy, source switching, recovery,
  retry, and shutdown cleanup.
- Typed Tauri commands and `playback://state` feed strict Zod-validated
  frontend snapshots. PlayerBar, local-library Play Now/Play Next/Add to Queue,
  and Ctrl+K transport actions are functional. The browser adapter is enabled
  only for development E2E runs with `VITE_SPOTDIY_E2E=1` outside Tauri.
- `playwright.config.ts` runs the playback matrix at 1280, 1920, and 2560
  viewport projects. Real synthetic WAV and packaged lifecycle smoke scripts
  are present and do not commit mpv or media artifacts.

## Verification

- Rust all-target tests (117), formatting, clippy with warnings denied,
  frontend typecheck/lint/Vitest (26 tests)/build, and Playwright (9 runs)
  pass locally.
- Real mpv smoke passes load, FileLoaded, position, pause/resume, seek,
  volume/mute, device enumeration, EOF, shutdown, and process exit using the
  local `.tools\mpv\v0.41.0\mpv.exe` development binary.
- Packaged release smoke passes synthetic indexing, Play/Pause/Seek/Resume,
  queue/Next, graceful close, owned-mpv cleanup, restart library persistence,
  and an empty transient queue. The isolated smoke profile and owned process
  are absent after the run; the normal database contains zero harness rows.
- Cargo/Tauri output uses `C:\CargoTarget\SpotDIY`; `src-tauri\target` is
  absent. The release executable and NSIS bundle were built successfully.
- The single fresh read-only reviewer rechecked the fixes with `PASS`; no
  critical, high, or correctness/security medium findings remain. The
  documentation closure is the final Plan 04 record before remote SHA
  verification.

## Plan 05 completion

- Concurrent Local, YouTube, SoundCloud, and isolated Spotify source adapters
  are wired through `SearchService`. Unified lenses exclude Spotify; the
  Spotify lens alone can query Spotify, and only after the explicit PKCE
  development/compliance gate is enabled.
- Search uses typed request/result DTOs, SearchIds, provider-local sorting,
  bounded provider timeouts, partial section updates, exact completion,
  stale-event rejection, and a maximum-100-entry TTL cache. The browser-only
  E2E adapter remains gated by `!isTauriRuntime() && import.meta.env.DEV &&
  VITE_SPOTDIY_E2E === "1"`.
- Verification for this handoff is recorded in
  `docs/execution/verification-log.md`: 250 Rust tests, 38 Vitest tests, 45
  Playwright runs, native synthetic smoke, opt-in YouTube/SoundCloud metadata
  smoke, and isolated packaged search smoke passed. Spotify live smoke was
  skipped because no developer authorization was available.
- Plan 05 adds no migration and stores no provider credentials, tokens, raw
  tool output, or provider payloads in SQLite. The repository-local Cargo
  target remains absent.

## Known limitations

- The queue is intentionally transient. Persistent queue state and queue
  snapshots belong to Plan 08.
- Provider adapters/search, Source Fusion, downloads, lyrics, playlists,
  overlays, global media keys/SMTC, portable mode, analytics, EQ,
  normalization, crossfade, ReplayGain, and later visual/performance work are
  out of Plan 04 scope.
- The packaged harness uses the smoke-only `SPOTDIY_PACKAGED_DATA_ROOT` seam
  because Windows known-folder resolution ignores a child `LOCALAPPDATA`
  override; standard startup remains `%LOCALAPPDATA%\SpotDIY`.

## Plan 06 completion

Plan 06 is complete through implementation tip `afd0149`, with documentation
closure committed separately. The delivery adds migration 3 and durable
merge/split overrides, a deterministic conservative matcher, explicit remote
source acceptance, a settings-aware SourceResolver, resolver-backed playback,
source availability explanations, and narrow typed fusion/resolution IPC.

The final verification log records 279 Rust tests, 40 Vitest tests, 45
Playwright runs, frontend/native quality gates, the external-target Tauri
release build, real mpv smoke, packaged playback/restart/process cleanup, and
the v2-to-v3 migration smoke. Spotify remains excluded from Plan 06 fusion,
acceptance, overrides, and playback; its Plan 05 PKCE/gate boundary is
unchanged.

## Plan 07 completion

Plan 07 is complete through implementation tip `6012921`. The implementation
commits are `0dbb628`, `22438a0`, and `6012921`; documentation closure follows
the final verification gates.

Schema version 4 adds only `downloads` and `download_settings`. DownloadService
owns persistent UUID tasks, trusted YouTube/SoundCloud creation paths,
settings-backed destination validation, per-task temp roots, structured
yt-dlp execution, FFmpeg-aware video merge, bounded machine progress,
concurrency, cancel/reap, retry, restart recovery, output-missing history,
and collision-safe destination-side finalization. It does not download Local
or Spotify sources, play online, fuse sources, create library rows, move media,
or persist raw provider payloads, credentials, or tokens.

DownloadsPage and SearchResultCard now expose the narrow native controls,
strict Zod DTOs, revisioned `downloads://state` events, folder selection,
concurrency, progress/provenance/error/output-missing details, and valid
cancel/retry/open actions. Tool health is visible in Downloads and Settings.

Final evidence: 308 Rust unit tests plus synthetic mpv, 47 Vitest tests, 45
Playwright runs, typecheck/lint/build, Rust fmt/clippy, Tauri packaging, real
mpv smoke, packaged playback/restart/cleanup smoke, and five native provider
search smoke checks pass. Live provider/download smoke was not run. The
optional packaged provider-search branch has a known immediate
`start_search`/`cancel_search` race in the missing-yt-dlp profile; cleanup
completed and no owned process remained.

Build output remains at `C:\CargoTarget\SpotDIY`; owned task temp is under
`%LOCALAPPDATA%\SpotDIY\cache\downloads\<DownloadTaskId>` and the repository
`src-tauri\target` path remains absent.

## Plan 08 completion

Plan 08 is complete through implementation commits `525da8c`, `e5f7161`,
`1f31d6a`, and `0a62cad`. It delivers schema-5 durable playlists and
collections, seeded Inbox, one-shot branch diff/selected merge/discard,
likes/ratings/tags, typed playlist/collection IPC, PlaybackService-owned
persistent queue sections, throttled checkpoints, immutable snapshots, restart
restore, and the queue drawer.

Final evidence records 318 Rust unit tests plus synthetic and real mpv smoke, 51
Vitest tests, 48 Playwright runs, strict quality gates, Tauri packaging,
packaged playback/restart/cleanup, explicit Plan 08 persistence/resume, and
v4-to-v5 migration coverage. Build output remains at
`C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` remains absent.

## Plan 09 completion

Plan 09 is complete through implementation commits `1bc7108`, `e4d62d8`,
`c25f954`, and `7b1a097`. Schema 6 persists lyrics records, bookmarks, and
A/B presets while preserving Plan 08 collections/queue/snapshots and Plan 07
downloads. `LyricsService` implements bounded local-first LRC and embedded
metadata handling, manual override/import/delete, synchronized DTOs, and an
explicit HTTPS-only LRCLIB boundary. `BookmarkService` owns durable bookmarks
and presets; `PlaybackService` owns active A/B state, clears it for a new
track, restores it for same-track source/recovery paths, and never autoplay
applies a preset.

Final evidence records 337 Rust unit tests plus one synthetic mpv integration
test, 56 Vitest tests, 48 Playwright runs, strict frontend/native gates, Tauri
packaging, real-mpv smoke, packaged Plan 08 and Plan 09 persistence smokes, and
the named v5-to-v6 migration smoke. CodeGraph and Graphify were each refreshed
once after implementation. Live LRCLIB smoke was optional and skipped; waveform
generation is not claimed. Cargo output remains external at
`C:\CargoTarget\SpotDIY` and repository-local `src-tauri\target` is absent.

## Plan 10 completion

Plan 10 is complete through `cc28ba1`, `f2a5995`, `850bc82`, `8c62aed`, and
`6eb231d`. It delivers the 15-token semantic theme contract, validated custom
theme import/export/reset, Dark/Light/System/Custom resolution, persistent
Comfortable/Compact/Dense layout profiles, reduced motion, shared accessible
primitives, keyboard context actions, InspectorPanel/IconGallery foundations,
and Settings APPEARANCE integration with a representative LibraryTrackRow.

Verification is green: 343 Rust unit tests plus one synthetic mpv integration
test, 70 Vitest tests, 51 Playwright tests at 1280/1920/2560, frontend quality
gates, Rust fmt/Clippy/tests, Tauri packaging, packaged settings persistence and
reset smoke, packaged Plan 09 playback/lyrics smoke, and clean shutdown with
no owned mpv process. CodeGraph and Graphify were each refreshed once; the
current stats are recorded in the verification log. Build output is external at
`C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` is absent.

## Plan 11 completion

Plan 11 is complete through `15031bf`. Migration 7 is the only Plan 11
database change: it rebuilds `settings_metadata`, copies all schema-6 rows,
and adds the Plan 10 appearance keys without rewriting the historical 0001
contract. Legacy old-constraint, Plan-10-shaped, and fresh database fixtures
all pass; latest schema is 7.

The main shell now presents real Home dashboard data, persisted and ephemeral
inspectors, measured quality/provenance, capability-aware search/library/
playlist/download actions, source switching, command-palette navigation,
overlay Escape priority, and Standard/Mini/Expanded in-shell player modes.
Existing service ownership remains authoritative: PlaybackService owns queue
and transport, while SearchService, DownloadService, LyricsService, and
PlaylistService retain their existing boundaries. Online playback, Spotify
  download; Plan 12 now owns the optional native Windows integration boundary.

Verification: 347 Rust unit tests plus one real-mpv integration test, 73
Vitest tests, 63 Playwright tests across 1280/1920/2560, frontend/native
quality gates, Tauri packaging, packaged Plan 09 persistence smoke, and
packaged Plan 11 migration/shell/restart smoke pass. Lint retains three
pre-existing Fast Refresh warnings; Graphify reports 4,131 nodes, 8,235
edges, and 247 communities. Build output is external at
`C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` is absent.

## Next atomic task

## Plan 12 completion

Plan 12 is complete through implementation commits `95eb41b`, `b7daac6`,
`d9b58c3`, `e4793b6`, and `3d39e1d`. Schema 8 persists Windows integration
settings, nine global shortcut bindings, and normalized output profiles while
preserving schema-7 rows. Native Rust owns lazy Mini/Edge/Lyrics/Gaming
overlays, tray actions, shortcut status, SMTC, click-through recovery, and
output-profile apply/rollback; the frontend uses strict typed IPC and a bounded
browser-preview adapter.

Verified evidence: 365 Rust unit tests plus real-mpv integration, 78 Vitest
tests, 69 Playwright tests at 1280/1920/2560, typecheck/lint/build, Rust
fmt/Clippy, schema 7-to-8 migration coverage, Tauri packaging, regular and
Plan 11 packaged smokes, and the dedicated Plan 12 packaged smoke. The live
Windows run reported `SMTC READY`, registered the controlled shortcut, proved
overlay reuse/topmost/click-through recovery, applied and restored an output
profile without changing playback context, and left no owned mpv process.
CodeGraph reports 166 files, 5,349 nodes, and 19,394 edges; Graphify reports
4,467 nodes, 8,952 edges, and 262 communities. Build output is external at
`C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` is absent.

Known limits: browser preview cannot provide native windows or SMTC; Gaming
Overlay is best for windowed/borderless games because exclusive fullscreen can
cover desktop overlays; and unsupported Windows media-control environments
remain explicit unavailable states.

## Next atomic task

Plan 13 import/export and portable mode remains unstarted.

## Plan 13 completion

Plan 13 is complete through `7579312`, `d287f65`, `6c2b026`, `bdf04f0`, and
`5e70fdf`.
The Plan 12 shortcut repair is `3ca57a4`. Schema 8 remains the latest schema;
startup resolves Standard or Portable from the executable-adjacent marker
before opening SQLite, and the settings storage-mode value is only a mirror.

Native code owns deterministic format-1 `.spotdiy` export, online WAL-safe
database snapshots, trusted media/artwork/sidecar inclusion, secure bounded ZIP
validation, staged preview, restart-gated commit, crash recovery, media-aware
rollback, and exact Standard/Portable mode switches. Settings now exposes
typed backup actions and status. The packaged Plan 13 smoke runs the release
executable from an isolated copy and proves Standard -> Portable -> Standard
restart selection, exact portable directories, and retained databases.

Verified evidence: 393 Rust unit tests plus real-mpv integration, 81 Vitest
tests across 22 files, 69 Playwright tests, frontend/native quality gates,
Tauri release/NSIS packaging, regular/Plan 11/Plan 12 packaged smokes, and the
Plan 13 packaged storage smoke. CodeGraph reports 176 files, 5,781 nodes, and
21,456 edges; Graphify reports 4,723 nodes, 9,470 edges, and 277 communities.
Build output is external at
`C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` is absent.
The requested Gmail completion message `Plan 13 finished` was sent to
`jerryle.minh.3@gmail.com`.

## Next atomic task

STOPPED AFTER PLAN 13.
Awaiting external ChatGPT GitHub review before Plan 14.

## Plan 14 completion handoff — 2026-09-03

The current checkout is based on the expected Plan 13 baseline
`ef6d7e9c…` and contains local Plan 14 phase commits, but the worktree is not
clean: `.gitignore` and two old Plan 05 report files have unrelated
pre-existing changes. Preserve those changes; do not reset, clean, or bundle
them into Plan 14 commits.

Implemented boundaries: schema 9's four tables; local-only qualified history
and 30-minute sessions; in-memory Private/Temporary modes; PlaybackService-
owned queue restoration; parameter-bound smart rule CRUD/preview; deterministic
non-ML Smart Shuffle; Analytics and Smart Playlist UI; and trusted Plan 13
restore staging with symlink/reparse rejection and cleanup ownership proof.

Final evidence: 420 Rust unit tests plus real mpv, 83 Vitest tests across 23
files, 69 Playwright tests, frontend/native quality gates, Tauri release/NSIS
packaging, regular/Plan 11/Plan 12/Plan 13/Plan 14 packaged smokes, Plan 13
staging security coverage, and an isolated format-1/schema-9 export/import/
restart roundtrip preserving analytics and smart data. Fresh Private/Temporary
mode state is not persisted. Graphify reports 281 files, 5,329 nodes, 12,057
edges, and 260 communities; CodeGraph is unavailable. Build output remains at
`C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` is absent.

## Next atomic task

STOPPED AFTER PLAN 14. Awaiting external ChatGPT GitHub review. Do not start
Plan 15. This historical handoff predates the completion record below.

## Plan 15 completion handoff — 2026-09-03

Plan 15 is complete through `1403955` (Plan-14 private transition repair),
`d73e755` (visual dataset), `e442767` (Music Map/Galaxy and shared visual
surfaces), `977176e` (bounded local preview), `aee1dad` (Theme Studio and
dynamic accent), and `2612804` (interaction/package coverage). Preserve the
unrelated `.gitignore` modification and the two deleted historical Plan 05
report files; they remain unstaged and were not bundled.

Implemented boundaries: schema 9 remains the latest schema; visual data is one
bounded read-only SQL query path with no local media/provider-secret leakage;
Music Map is SVG and Galaxy is deterministic 2D Canvas with 200-item DOM
navigators; actions support Play Now/Next/Queue/Inbox/Inspect/Lyrics/Reveal,
radial More, keyboard drag alternatives, and capability gating; PreviewService
is separate from playback and limited to explicit eight-second local samples;
Theme Studio has schema v1/15 tokens, draft preview, persistence, import/export,
contrast-safe dynamic accent, and existing layout profiles.

Final evidence: 430 Rust unit tests plus one real-mpv integration test, 88
Vitest tests, 70 Playwright tests, frontend/native quality gates, Tauri
release/NSIS packaging, and the packaged Plan 15 visual/restart smoke. Graphify
reports 5,517 nodes, 12,505 edges, and 268 communities; CodeGraph is
unavailable. Build output is external at `C:\CargoTarget\SpotDIY`, and
repository-local `src-tauri\target` is absent.

## Next atomic task

STOPPED AFTER PLAN 15. Do not start Plan 16 in this task.
