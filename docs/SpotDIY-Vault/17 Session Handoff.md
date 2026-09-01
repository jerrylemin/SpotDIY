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

## Next atomic task

STOPPED AFTER PLAN 08. Awaiting external ChatGPT GitHub review before Plan 09.
