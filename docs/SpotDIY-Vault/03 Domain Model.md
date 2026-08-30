# Domain model

## Plan 02 foundation

The core identity is `UnifiedTrack`. It owns a normalized title, ordered artists, an optional album, track-level duration/version metadata, zero or more `TrackSource` records, an optional preferred source, and timestamps.

Rust uses UUID-backed `TrackId`, `ArtistId`, `AlbumId`, and `SourceId` newtypes. `ProviderKind` is an enum for `Local`, `Youtube`, `Soundcloud`, and `Spotify`; provider item IDs remain opaque strings and are never used as SpotDIY primary keys.

`TrackSource` carries source URI, duration, `VersionInfo`, availability detail, `SourceCapabilities`, and optional `LocalFileSource` metadata. Version qualifiers include standard, studio, live, acoustic, remix, remaster, cover, instrumental, karaoke, sped-up, slowed, and unknown. Multiple artists are relationally ordered and retain a role field in SQLite.

Capabilities are explicit data rather than UI provider-name logic. Spotify catalog sources are metadata-only: playback, download, lyrics, and lyrics-metadata capabilities are rejected by Rust and SQLite. Preferred-source ownership is checked by the repository and database triggers.

See [ADR-0003](ADRs/ADR-0003-unified-source-model.md) and [ADR-0006](ADRs/ADR-0006-provider-source-identity.md). Source Fusion scoring and provider adapters remain later-plan work.

## Plan 03 local library

Plan 03 adds UUID-backed `LibraryFolderId` and `ArtworkId`, plus explicit
`LibraryFolderStatus`, `LocalFileIndexStatus`, `ScanProgress`, `ScanSummary`,
`LibraryStatus`, `LibraryPageRequest`, `LibraryPage`, and `LibraryTrack` DTOs.
`LibraryTrack` exposes the original path, folder ownership, availability and
error detail, measured container/codec/bitrate/sample-rate/bit-depth values,
content fingerprint evidence, and an app-cache artwork path when available.

`LibraryService` keeps local source identity opaque: a discovered file receives
an independent `TrackId`, `SourceId`, and local provider item identity. A path
is only ownership evidence. An unambiguous missing-fingerprint rename and a
same-path restoration reuse the existing identity; ambiguous duplicate bytes
remain separate entries. Plan 02 local rows remain preserved and are excluded
from managed reads until a matching discovered path promotes them.

The service treats `available` and `index_status` separately so an unavailable
root can retain metadata while its sources are not reported playable. Partial
metadata/artwork/database failures remain visible in scan summaries and folder
status. Playback is intentionally not implemented in this boundary; Plan 04
owns the playback service and source selection behavior.

See [ADR-0008](ADRs/ADR-0008-local-library-identity-and-reconciliation.md).
