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
