# Testing

Frontend tests use Vitest and Testing Library. Rust tests cover migrations,
repositories, domain invariants, local-library path/fingerprint/metadata/artwork
helpers, recursive scanning, transactional reconciliation, identity
preservation, watcher event coalescing/recovery, paging, reveal ownership,
settings, and status IPC. Provider tests use mocks; live integrations are
opt-in. Plan 03 had 53 Rust tests and 18 frontend tests across four files
before Plan 04 added the playback coverage below.

The packaged native smoke uses a temporary synthetic folder and proves launch,
add/initial scan, restart persistence, unchanged and forced rescans, watcher
create/rename/delete/restore behavior, identity stability, reveal validation,
folder removal, and preservation of synthetic media. It does not touch real
music folders. Plan 04 adds a pinned browser project using the typed E2E
adapter; later full visual/design QA remains a follow-up.

## Plan 04 verification

The native Rust suite passes with 117 all-target tests, including protocol
framing/correlation, mpv lifecycle helpers, transient queue traversal,
managed-source resolution, state transitions, source switching, recovery,
stale generations, EOF policy, command races, and bounded shutdown. Frontend
quality checks pass with 26 Vitest tests plus typecheck, lint, and build.

The pinned Playwright matrix runs 9 tests at 1280, 1920, and 2560
viewport projects through a browser-only typed IPC adapter. It covers all
playback phases, queue/repeat/shuffle/previous semantics, volume/mute/device
controls, source labels, local-library actions, stale revisions, long titles,
missing artwork, keyboard focus, source-switch queue identity preservation,
and console errors.

The real Windows smoke uses generated WAV media and the local
`.tools\mpv\v0.41.0\mpv.exe` development binary. The packaged smoke uses the
release executable, indexes two synthetic tracks, exercises transport and
restart behavior, verifies library persistence with an empty transient queue,
and verifies no SpotDIY-owned mpv child remains. Temporary media, profiles,
and test database rows are cleaned after the run.

## Plan 05 verification

The native suite passes 250 unit tests plus one all-target `mpv_smoke`
integration test. Frontend typecheck, lint, build, and 38 Vitest tests pass.
The three-width Playwright matrix runs 45 tests through the development-only
typed browser adapter and covers independent provider loading, partial results,
stale SearchIds, lens mappings, sort controls, Spotify disablement, long-title
overflow, and artwork fallback.

`scripts/provider-search-smoke.ps1` passes five focused native Local/playback
checks. Its opt-in live branch passes YouTube and SoundCloud metadata-only
searches with 25 entries each and skips Spotify without developer authorization.
The isolated packaged search smoke passes Local indexing/result rendering,
missing-provider failure isolation, concurrent cancellation, Spotify gating,
and owned-process cleanup. No provider credentials, tokens, raw output, or
provider payloads are retained by the smoke runs.

## Plan 06 verification

Plan 06 passes 279 Rust unit tests plus one real `mpv_smoke` integration test,
40 Vitest tests, 45 Playwright runs across three viewport projects, strict
frontend typecheck/lint/build, Rust fmt, all-features clippy with warnings
denied, and the external-target Tauri release build. Focused tests cover
normalization, all guarded qualifiers, matcher thresholds/duration/version
guards, merge/split precedence, migration preservation, remote identity
idempotence/conflicts, resolver preference/quality/readiness explanations,
and strict fusion/resolution IPC DTOs.

The explicit real mpv synthetic-WAV smoke and packaged playback/restart/owned
process cleanup smoke pass. The named v2-to-v3 migration smoke passes without
losing existing tracks, sources, or settings. Spotify remains excluded from
Plan 06 fusion and playback, and no media, credentials, tokens, or raw
provider output are retained.

## Plan 07 verification

Plan 07 passes 308 Rust unit tests plus one synthetic mpv integration test,
47 Vitest tests, 45 Playwright runs across the three viewport projects,
frontend typecheck/lint/build, Rust fmt, all-features Clippy with warnings
denied, and the external-target Tauri release build. Focused download tests
cover schema 3-to-4 preservation, repository round trips, state transitions,
queue ordering, concurrency, bounded progress/argv/output, FFmpeg status,
cancellation and reaping, retry, restart recovery, missing outputs,
sanitization, collision naming, cross-volume finalization, provider-encoded
provenance, and cleanup.

The explicit real-mpv and packaged playback/restart/owned-process smokes pass.
The provider-search smoke passes its five native synthetic checks; live
provider/download work is opt-in and was not run. The optional packaged search
branch has a known immediate `start_search`/`cancel_search` race when the
profile intentionally has no yt-dlp; it is recorded as a harness limitation,
not treated as a successful download or search result.

## Plan 08 verification

Plan 08 passes 318 Rust unit tests plus the synthetic and explicit real-mpv
smokes, 51 Vitest tests, 48 Playwright runs across the three viewport projects,
frontend typecheck/lint/build, Rust fmt, all-features Clippy with warnings
denied, and the external-target Tauri release build. Focused coverage includes
schema v4-to-v5 preservation, playlist ordering and duplicates, Inbox
idempotence, one-shot branch diff/merge/discard and conflicts, collection
normalization, queue section policy, movement/pinning/clear, shuffle, snapshots,
restart restore, and saved-position resume.

The regular packaged playback/restart/cleanup smoke and the explicit Plan 08
playlist/collection/queue/snapshot/restart smoke pass. The optional
missing-yt-dlp packaged provider-search race was intentionally not triggered;
live provider/download smoke remains opt-in. CodeGraph and Graphify were each
refreshed once after implementation, and repository-local `src-tauri\target`
remains absent.

## Plan 09 verification

Plan 09 passes 337 Rust unit tests plus one synthetic mpv integration test, 56
Vitest tests, 48 Playwright runs, frontend typecheck/lint/build, Rust fmt and
all-features Clippy with warnings denied, the external-target Tauri release
build, explicit real-mpv smoke, and packaged Plan 08 and Plan 09 persistence
smokes. Focused coverage includes v5-to-v6 migration preservation, LRC variants
and bounds, embedded plain/SYLT metadata, local/manual/provider precedence,
LRCLIB validation and cache behavior, synchronized cue selection, bookmark
validation/persistence, safe loop bounds, recovery/source restoration, and
new-track clearing.

The Plan 09 packaged smoke also proves manual override/delete fallback to the
sidecar, bookmark and preset retention across restart, no automatic A/B preset
application, queue persistence, clean close, and zero owned mpv processes. Live
LRCLIB smoke was optional and skipped; no copyrighted lyrics, media, secrets,
raw provider payloads, or repository-local `src-tauri\target` are retained.

## Plan 10 verification

Plan 10 passes 343 Rust unit tests plus one synthetic mpv integration test, 70
Vitest tests across 18 files, and 51 Playwright tests across the 1280, 1920,
and 2560 viewport projects. Frontend typecheck/lint/build, Rust fmt and
all-features Clippy with warnings denied, Tauri packaging, and `git diff --check`
also pass. Lint has three non-fatal Fast Refresh warnings; build notices for
Browserslist data, Tailwind content discovery, and a large frontend chunk are
non-fatal.

Focused design coverage checks custom-theme schema limits and contrast,
Dark/Light/Custom controls, invalid import recovery, export/reset, all three
layout profiles, visible focus, pointer and keyboard context actions, reduced
motion, icon rendering, long-content overflow, responsive widths, and the
1080-pixel height guard. Screenshots are generated into Playwright output paths
only. The packaged settings smoke proves restart persistence and reset for
theme/layout settings; the packaged Plan 09 playback/lyrics smoke still passes
with clean shutdown and no owned mpv process.

## Plan 11 verification

Plan 11 passes the independent schema-6 old-constraint and Plan-10-shaped
migration fixtures plus fresh schema startup at version 7. Settings survive,
appearance keys write and activate, and foreign-key checks remain clean.
Frontend verification is 73 Vitest tests across 19 files and 63 Playwright
tests across 1280, 1920, and 2560 viewport projects. Coverage includes real
Home data, persisted/ephemeral inspectors, measured quality/provenance,
capability-aware actions, source switching, Standard/Mini/Expanded modes,
command-palette navigation, Escape priority, focus restoration, and existing
search/playback/design regressions.

Native verification is 347 Rust unit tests plus one passing real-mpv integration
test, fmt, strict all-features Clippy, frontend typecheck/lint/build, and the
external-target Tauri release build. The explicit real-mpv smoke, packaged
Plan 09 persistence smoke, and new packaged Plan 11 migration/shell/restart
smoke all pass with clean owned-process shutdown. Live provider, LRCLIB, and
download smoke remain optional and were not run.

## Plan 12 verification

Plan 12 passes 365 Rust unit tests plus one real-mpv integration test, 78
Vitest tests across 21 files, and 69 Playwright tests across the 1280, 1920,
and 2560 viewport projects. Frontend typecheck/lint/build, Rust fmt, strict
all-features Clippy, `git diff --check`, the external-target Tauri release
build, and the named schema 7-to-8 migration test all pass.

Focused native coverage includes schema-8 settings-row preservation, strict
shortcut syntax and duplicate identity, frozen overlay labels/dimensions and
edge placement, tray action mapping, typed SMTC command mapping, output-profile
validation and rollback, and truthful click-through errors. Frontend coverage
includes browser-preview isolation, settings controls, overlay state, output
profile editing, shortcut reset, and native-only command-palette gating.

The regular packaged playback smoke, packaged Plan 11 migration/shell/restart
smoke, and dedicated Plan 12 Windows smoke all pass with clean shutdown. Plan
12's live run reports `SMTC READY`, a registered controlled shortcut, overlay
reuse and topmost state, click-through recovery, output-profile apply/restore
without playback-context mutation, schema version 8, and zero owned mpv
processes. Browser preview remains native-free; live provider, LRCLIB, and
download smoke remain optional.

## Plan 13 verification

Plan 13 passes 393 Rust unit tests plus one real-mpv integration test, 81
Vitest tests across 22 files, and 69 Playwright tests across the 1280, 1920,
and 2560 viewport projects. Frontend typecheck/lint/build, Rust fmt, strict
all-features Clippy, all-target tests, `git diff --check`, and the external
target Tauri release/NSIS build pass. The same three pre-existing Fast Refresh
lint warnings and documented frontend build notices remain non-fatal.

Focused native coverage includes deterministic ZIP bytes and fixed timestamps,
manifest/hash exactness, safe path and case collision checks, symlink and
compression-bomb rejection, payload declaration/size/hash checks, staged
database migration/integrity/foreign-key validation, missing-file previews,
audio/sidecar restoration, artwork cache allowlisting, crash recovery,
database rollback, no-active-mutation staging, marker authority, exact
Portable paths, and failure-preserving mode switches.

The regular packaged playback smoke, packaged Plan 11 shell smoke, packaged
Plan 12 Windows smoke, and the isolated packaged Plan 13 Standard/Portable
restart smoke all pass with clean shutdown. The Plan 13 packaged harness does
not automate OS-native save/open/folder dialogs; those production dialog
boundaries remain covered by native command wiring and the archive/restore
unit suite. Live provider, LRCLIB, and download smoke remain optional.

## Plan 14 verification

Plan 14 passes 420 Rust unit tests plus one real-mpv synthetic WAV integration
test, 83 Vitest tests across 23 files, 69 Playwright tests across the 1280,
1920, and 2560 viewport projects, frontend typecheck/lint/build, Rust fmt,
strict all-target/all-feature Clippy, the external-target Tauri release/NSIS
build, and the regular/Plan 11/Plan 12/Plan 13/Plan 14 packaged smokes.

The isolated schema-9 `.spotdiy` export/import/restart check passes with
format-version 1, preserved genres/sessions/history/smart playlists, and
fresh Private/Temporary mode state disabled. Plan 13 staging symlink/reparse,
path-ownership, and cleanup security regressions remain green. Lint retains
the three pre-existing Fast Refresh warnings and the build retains documented
non-fatal notices. Graphify reports 281 files, 5,329 nodes, 12,057 edges, and
260 communities; CodeGraph is unavailable because no command/index is present.
Build output remains external at `C:\CargoTarget\SpotDIY`, and repository-local
`src-tauri\target` remains absent.
