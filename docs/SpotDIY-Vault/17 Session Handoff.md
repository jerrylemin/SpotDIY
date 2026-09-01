# Session handoff

The authoritative handoff is the repository-root [session_handoff.md](../../session_handoff.md).

Plans 01–03 provide the Tauri/React shell, typed unified domain, SQLite/WAL
storage through schema version 2, durable settings, persistent local library,
metadata/artwork/fingerprints, watcher recovery, typed library IPC, and safe
reveal. Plan 04 provides the external mpv playback service and transient queue.

The Plan 04 feature commit is `536617d`; review fixes are in `af66127`. Its
release and smoke evidence pass: 117 all-target Rust tests, 26 frontend tests,
three-width Playwright (9 runs), synthetic real-mpv transport, and packaged
playback/restart/process cleanup. The single fresh independent review passed
with no critical, high, or correctness/security medium findings.

Plan 05 was the preceding plan: its approved boundary excluded Source Fusion,
provider playback, and persistent queue work. The completed Plan 06 handoff
below records the separately approved follow-on delivery.

## Plan 05 handoff

Plan 05 is complete through implementation tip `ab6169d`, with the remaining
documentation closure committed separately. Search adapters, SearchService,
strict frontend IPC, Spotify PKCE isolation, browser coverage, native smoke,
live metadata smoke, and packaged search smoke are delivered. The final
verification log records 250 Rust tests, 38 Vitest tests, 45 Playwright runs,
and the successful release/package gates.

At the Plan 05 boundary, Plan 06 was still pending. The completed Plan 06
handoff below records the follow-on delivery; provider search results remain
transient and Spotify remains metadata-only and gated.

## Plan 06 handoff

Plan 06 is complete through implementation tip `afd0149`, with the delivery
documentation commit following final verification. Migration 3, deterministic
fusion normalization/matching, durable merge/split overrides, explicit remote
source acceptance, SourceResolver ranking, resolver-backed playback, typed
availability explanations, and five narrow IPC commands are present.

The final log records 279 Rust unit tests plus real mpv smoke, 40 Vitest tests,
45 Playwright runs, release/package smoke, and v2-to-v3 migration coverage.
Cargo output is external at `C:\CargoTarget\SpotDIY`; repository-local
`src-tauri\target` is absent. Plan 07 completion is recorded below.

## Plan 07 handoff

Plan 07 is complete through implementation tip `6012921`, with documentation
closure following the final verification. The three implementation commits
are `0dbb628` (schema, contracts, repository, settings, and state model),
`22438a0` (FFmpeg/yt-dlp tooling, scheduler, lifecycle, recovery, and safe
finalization), and `6012921` (AppState, typed IPC/events, Downloads UI, search
actions, settings/tool status, and frontend coverage).

Schema version 4 adds only `downloads` and `download_settings`. DownloadService
owns persistent tasks, validates trusted provider/source identity and the
existing downloads directory setting, keeps yt-dlp inside UUID-owned temp
roots, reports machine progress, limits concurrency to 1..4, cancels and
reaps owned children, retries without duplicate rows, requeues interrupted
tasks on restart, and finalizes destination files without overwrite or
cross-volume rename assumptions. Completed history exposes `outputMissing`.

The final verification log records 308 Rust unit tests plus synthetic mpv,
47 Vitest tests, 45 Playwright runs, strict quality gates, Tauri packaging,
real-mpv smoke, packaged playback/restart/cleanup, and five native provider
search smoke checks. Live provider/download smoke was not run. The optional
packaged provider-search harness has a known immediate cancellation race when
yt-dlp is intentionally missing; its cleanup completed and no owned process
remained.

Storage remains external at `C:\CargoTarget\SpotDIY` for build output and
`%LOCALAPPDATA%\SpotDIY\cache\downloads\<DownloadTaskId>` for owned task temp.
Spotify and Local downloads, online playback, automatic fusion/library
mutation, and later plans remain out of scope. Plan 08 completion is recorded
below.

## Plan 08 handoff

Plan 08 is complete through implementation commits `525da8c`, `e5f7161`,
`1f31d6a`, and `0a62cad`. Schema 5 persists playlists/items, seeded Inbox,
one-shot branch base snapshots, likes, ratings, tags, queue entries/state, and
immutable queue snapshots while preserving Plan 07 data. `PlaylistService`
owns collections and branch operations; `PlaybackService` remains the sole
queue owner and restores queue state/position without autoplay.

The final log records 318 Rust unit tests plus synthetic and real mpv smoke, 51
Vitest tests, 48 Playwright runs, strict quality gates, Tauri packaging,
packaged restart/cleanup, explicit Plan 08 persistence/resume, and v4-to-v5
migration coverage. Cargo output remains external at
`C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` is absent.

## Plan 09 handoff

Plan 09 is complete through implementation commits `1bc7108`, `e4d62d8`,
`c25f954`, and `7b1a097`. Schema 6 preserves Plan 07 downloads and Plan 08
collections/queue/snapshots while adding lyrics, bookmarks, and A/B presets.
Local-first precedence is manual, sidecar, embedded timed, embedded plain,
then cached LRCLIB; local reads are read-only and provider lookup is explicit.
`PlaybackService` owns active A/B state, clears it at a new-track boundary,
restores same-track source/recovery state, and never autoplay-applies presets.

Verified evidence: 337 Rust unit tests plus one synthetic mpv integration test,
56 Vitest tests, 48 Playwright runs, strict quality gates, Tauri packaging,
real-mpv smoke, packaged Plan 08/09 persistence smokes, and v5-to-v6 migration
coverage. Cargo output remains external at `C:\CargoTarget\SpotDIY`; live LRCLIB
smoke was optional and skipped; waveform generation is not claimed.

## Plan 10 handoff

Plan 10 is complete through implementation commits `cc28ba1`, `f2a5995`,
`850bc82`, `8c62aed`, and `6eb231d`. The frontend now has a shared semantic
token layer, Dark/Light/System/Custom themes, schema-validated custom theme
import/export/reset, persistent Comfortable/Compact/Dense layout profiles,
reduced motion, accessible primitives, keyboard context actions, custom icons,
and InspectorPanel/IconGallery foundations. Settings APPEARANCE owns the
controls and `LibraryTrackRow` demonstrates context-action adoption.

Verification: 343 Rust unit tests plus one synthetic mpv integration test, 70
Vitest tests, 51 Playwright tests at 1280/1920/2560, frontend/native quality
gates, Tauri packaging, packaged theme/layout persistence across restart,
custom reset, packaged Plan 09 playback/lyrics persistence, and clean shutdown
with no owned mpv process. Storage remains schema 6 with no migration 7;
CodeGraph and Graphify were each refreshed once. Build output is external at
`C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` is absent.

Known limits: browser preview settings are session-scoped in memory. Online
playback and Spotify downloads remain unavailable; Theme Studio, mobile UI,
and Plan 12 Windows overlays remain future work.

## Plan 11 handoff

Plan 11 is complete through `15031bf`. Migration 7 repairs the shipped
schema-6 appearance-settings constraint by rebuilding only
`settings_metadata`, copying all rows, and advancing the database to schema 7.
The independent old-constraint fixture, Plan-10-shaped fixture, and fresh DB
regressions pass.

The shell now uses real Home state and existing service snapshots for persisted
and ephemeral inspectors, source/quality/provenance surfaces, capability-aware
actions, command-palette navigation, Escape priority, and Standard/Mini/
Expanded in-shell players. `PlaybackService` remains the sole playback and
queue owner; no online playback or Plan 12 work was added.

Verified evidence: 347 Rust unit tests plus one real-mpv integration test, 73
Vitest tests, 63 Playwright tests at 1280/1920/2560, frontend/native quality
gates, Tauri packaging, packaged Plan 09 persistence, and packaged Plan 11
migration/shell/restart smoke. Graphify reports 4,131 nodes, 8,235 edges,
and 247 communities. Build output is external at
`C:\CargoTarget\SpotDIY`; repository-local `src-tauri\target` is absent.

## Next atomic task

Plan 12 overlay and Windows integration work remains unstarted.
