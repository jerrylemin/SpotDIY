# Domain model

The core identity is `UnifiedTrack`. It owns normalized title/artist/album metadata and one or more `TrackSource` records. `ProviderKind` is LOCAL, YOUTUBE, SOUNDCLOUD, or SPOTIFY. A source can be playable, metadata-only, downloadable, or unavailable depending on capabilities and runtime health.

Core identifiers are typed at the Rust boundary: TrackId, ArtistId, AlbumId, PlaylistId, SourceId. QueueEntry, DownloadTask, LyricsDocument, TrackBookmark, and ListeningSession build on the same identity.
