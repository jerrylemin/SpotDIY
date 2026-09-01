# Plan 06 — source fusion

## Goal

Match provider/local results into unified tracks conservatively and resolve playable sources by preference and capability.

## Dependencies

Plans 02 and 05; fusion matrix from the approved spec.

## Exact files

`src-tauri/src/fusion/mod.rs`, `src-tauri/src/fusion/normalize.rs`, `src-tauri/src/fusion/matcher.rs`, `src-tauri/src/fusion/overrides.rs`, `src-tauri/src/sources/resolver.rs`, `src-tauri/src/domain/mod.rs`, and fusion tests.

## Interfaces consumed

Normalized `SourceTrack`, user source preferences, and `user_track_overrides` repository.

## Interfaces produced

`SourceFusionService`, deterministic score/explanation, merge/split override commands, and `SourceResolver`.

## Tests

All required true-match, punctuation, featuring, duration drift, version guard, different-artist/song, and user override cases.

## Acceptance criteria

False-positive version merges are rejected; source switching picks the highest-ranked playable source and explains unavailable sources.

## Delivered implementation (2026-09-01)

- Migration 3 and `user_track_overrides` provide transactional Merge
  replacement, target-specific Split overrides, and Spotify exclusion.
- Deterministic NFKD normalization and integer Jaro-Winkler matching implement
  the approved 55/35/10 weights, 8800 threshold, hard minima, duration bands,
  guarded-version policy, stable tie ordering, and ambiguity guard.
- Explicit YouTube/SoundCloud acceptance persists only remote `TrackSource`
  identities with backend-owned capabilities; evaluation remains read-only.
- `SourceResolver` consumes settings/provider preference, track preference,
  source quality/capabilities/availability, and a narrow readiness probe. It
  keeps production playback local-only and is integrated with normal and exact
  PlaybackService selection.
- Typed strict IPC covers candidate evaluation/acceptance, overrides, and
  source resolution without adding fusion UI or automatic search persistence.

Focused and final evidence: 279 Rust unit tests plus real mpv smoke, 40
Vitest tests, 45 Playwright runs, frontend/native quality gates, Tauri build,
packaged playback/restart cleanup, and v2-to-v3 migration smoke all pass.

## Commit boundary

Implementation: `cf0248f`, `d4f72a7`, `4161810`, `afd0149`.
Documentation closure: `docs: close Plan 06 source fusion delivery`.
