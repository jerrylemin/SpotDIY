# Unified source model

Source Fusion normalizes Unicode, whitespace, case, punctuation, artist separators, featuring syntax, and version qualifiers. Title similarity starts at 0.55, artist similarity at 0.35, and duration similarity at 0.10 with a conservative threshold near 0.88. Live, acoustic, remix, cover, instrumental, karaoke, slowed, sped-up, and similar qualifiers act as merge guards.

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
