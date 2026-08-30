# SpotDIY shared vocabulary

- `Track Entity`: the durable musical work in the unified library.
- `UnifiedTrack`: normalized track identity with one or more provider/local sources.
- `TrackSource`: a provider-specific representation with capabilities and provenance.
- `ProviderKind`: `LOCAL`, `YOUTUBE`, `SOUNDCLOUD`, or `SPOTIFY`.
- `PlayableSource`: a source that can supply an audio stream/file to `PlaybackService`.
- `Source Resolver`: chooses a playable source for a unified track using user preference and capability state.
- `Source Fusion`: deterministic normalization, matching, merge guards, and user override persistence.
- `Library`: indexed local files and their metadata; it is not a provider cache.
- `Queue`: ordered playback context with `UP NEXT`, `LATER`, and `AUTOPLAY` sections.
- `Listening Session`: a locally grouped playback-history interval with context.
- `Music Inbox`: lightweight unsorted holding area before playlist/tag organization.
- `SpotDIY backup`: ZIP-compatible `.spotdiy` archive with manifest, checksums, schema version, and transactional import.
