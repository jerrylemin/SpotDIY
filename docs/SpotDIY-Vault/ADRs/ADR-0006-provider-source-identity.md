# ADR-0006: Provider source identity and unified track integrity

- Status: Accepted
- Date: 2026-08-30
- Scope: Plan 02 unified music model

## Context

SpotDIY represents one musical work as a `UnifiedTrack` with multiple `TrackSource` records. SpotDIY identities must remain distinct from opaque provider IDs, and source fusion needs stable version, capability, and provenance fields before provider adapters exist.

## Decision

Rust uses UUID-backed `TrackId`, `ArtistId`, `AlbumId`, and `SourceId` newtypes. `ProviderKind` is an enum for local, YouTube, SoundCloud, and Spotify; provider item IDs remain opaque strings on `TrackSource`. SQLite enforces global uniqueness for the exact `(provider_kind, provider_item_id)` pair, and local file paths are unique. A track may have many ordered artists, an optional album, many sources, and an optional preferred source.

Preferred-source integrity is enforced in repository validation and in SQLite triggers: a preferred source must belong to the same track, and moving a source cannot invalidate an existing preferred-source reference. Provider capabilities are explicit data. Spotify sources are catalog metadata only and cannot advertise playback, download, lyrics, or lyrics-metadata capability at the domain, repository, or schema boundary.

## Consequences

Source fusion can preserve provider provenance without title-based primary keys or comma-separated artist fields. The database rejects obvious identity and relationship corruption even when a future importer bypasses the repositories. Provider adapters and the resolver can consume capabilities without UI provider-name conditionals.
