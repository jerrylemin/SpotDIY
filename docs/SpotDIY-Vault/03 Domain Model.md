# Domain model

## Plan 02 foundation

The core identity is `UnifiedTrack`. It owns a normalized title, ordered artists, an optional album, track-level duration/version metadata, zero or more `TrackSource` records, an optional preferred source, and timestamps.

Rust uses UUID-backed `TrackId`, `ArtistId`, `AlbumId`, and `SourceId` newtypes. `ProviderKind` is an enum for `Local`, `Youtube`, `Soundcloud`, and `Spotify`; provider item IDs remain opaque strings and are never used as SpotDIY primary keys.

`TrackSource` carries source URI, duration, `VersionInfo`, availability detail, `SourceCapabilities`, and optional `LocalFileSource` metadata. Version qualifiers include standard, studio, live, acoustic, remix, remaster, cover, instrumental, karaoke, sped-up, slowed, and unknown. Multiple artists are relationally ordered and retain a role field in SQLite.

Capabilities are explicit data rather than UI provider-name logic. Spotify catalog sources are metadata-only: playback, download, lyrics, and lyrics-metadata capabilities are rejected by Rust and SQLite. Preferred-source ownership is checked by the repository and database triggers.

See [ADR-0003](ADRs/ADR-0003-unified-source-model.md) and [ADR-0006](ADRs/ADR-0006-provider-source-identity.md). Source Fusion scoring and provider adapters remain later-plan work.
