# Unified source model

Source Fusion normalizes Unicode, whitespace, case, punctuation, artist separators, featuring syntax, and version qualifiers. Title similarity starts at 0.55, artist similarity at 0.35, and duration similarity at 0.10 with a conservative threshold near 0.88. Live, acoustic, remix, remaster, cover, instrumental, karaoke, slowed, sped-up, nightcore, and similar guarded qualifiers act as merge guards.

Manual merge/split decisions are durable overrides and always outrank automatic matching.

## Plan 05 search boundary

Provider search results are normalized into typed transient DTOs with provider
identity, entity kind, canonical URL, metadata, optional artwork, and optional
local track/source IDs. Local results can invoke the existing managed playback
and file actions; online results are metadata/open-source actions only. The
unified search lenses use Local, YouTube, and SoundCloud. Spotify is excluded
from unified lenses and is available only through its isolated Spotify lens.
Search does not fuse or persist provider records; Source Fusion and resolver
policy remain later-plan work.

## Plan 06 source fusion delivery

Plan 06 keeps provider search results ephemeral and makes fusion explicit. The
normalizer uses Unicode NFKD and deterministic presentation cleanup; the
matcher uses integer Jaro-Winkler basis points with title 55%, artists 35%,
duration 10%, an 8800 threshold, 9000 title/artist hard minima, duration
bands, exact guarded-version equality, and a 300-basis-point ambiguity guard.
Standard and absent guarded qualifiers are equivalent; Studio is not inferred
as a mismatch, and literal titles such as `Live Forever` are not classified as
Live.

Merge and Split overrides are stored in migration-3 `user_track_overrides`.
Only an explicit accepted YouTube or SoundCloud candidate creates a remote
`TrackSource`; evaluation and best-match selection do not write SQLite. Local
identity and Spotify remain excluded from destructive or automatic fusion.
