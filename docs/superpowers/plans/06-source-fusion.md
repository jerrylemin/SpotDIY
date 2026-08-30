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

## Commit boundary

`feat: add conservative source fusion and resolver`
