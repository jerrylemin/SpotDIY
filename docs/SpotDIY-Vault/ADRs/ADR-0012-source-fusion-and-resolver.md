# ADR-0012: conservative Source Fusion and SourceResolver

- Status: Accepted
- Date: 2026-09-01

## Context

Plan 05 produces independent, transient Local/YouTube/SoundCloud search
results and keeps Spotify behind an isolated metadata-only boundary. Plan 06
needs a deterministic way to explain candidate matches, persist only explicit
source acceptance, and choose a playable source without allowing online URLs
or provider playback to bypass the local playback boundary.

## Decision

Use a conservative `SourceFusionService` with deterministic Unicode NFKD
normalization and integer Jaro-Winkler basis-point scores. Automatic matching
uses title 55%, artists 35%, duration 10%, an 8800 threshold, 9000 title and
artist hard minima, the approved duration bands/guard, and exact equality of
guarded non-standard version qualifiers. Standard and absent guarded
qualifiers are equivalent; Studio is not a guarded mismatch.

Persist user intent separately in migration-3 `user_track_overrides`. A Merge
override forces one target per provider identity and replaces the previous
forced target transactionally. A Split override excludes one identity from one
target and may coexist with splits for other targets. Overrides outrank
automatic scoring, but Spotify, invalid targets, and persisted identity
conflicts remain excluded. Evaluation and candidate selection are read-only;
only explicit acceptance and override operations write.

Accept only YouTube and SoundCloud remote candidates into a new
`TrackSource`, using validated canonical URLs and backend-owned capabilities.
Do not move existing Local sources, create remote `local_files` rows, rewrite
track metadata, or set a preferred source. Spotify never participates in Plan
06 fusion, overrides, acceptance, or playback.

Use `SourceResolver` as the single source-selection policy for normal playback
and exact source switching. A currently playable per-track preference wins;
otherwise validated settings/provider order applies. Local readiness requires
availability, playback capability, and a successful managed
`LibraryService.resolve_playback_path` check. Local quality ranks known
lossless codecs (`flac`, `alac`, `pcm_*`, `wavpack`, `ape`), then bit depth,
sample rate, bitrate, and stable `SourceId`. YouTube/SoundCloud report
`ProviderPlaybackNotImplemented`, and Spotify reports `MetadataOnly`.

`SourceResolver` exposes a narrow readiness probe seam for future provider
playback tests without enabling online playback now. The existing
`PlaybackService`, mpv backend, queue, recovery, and source-switch rollback
architecture remains intact; no arbitrary provider URL reaches mpv.

## Consequences

False-positive merges are favored over missed automatic matches, and every
candidate/source choice has a typed reason suitable for future UI. Search
remains independent and ephemeral, while explicit remote acceptance is
auditable in `track_sources`. Later plans can add provider readiness through
the probe boundary without rewriting source ranking or the playback state
machine.

## Verification

The delivery passes the Plan 06 Rust, frontend, browser, real mpv, packaged
playback, release-build, and v2-to-v3 migration gates recorded in
`docs/execution/verification-log.md`.
