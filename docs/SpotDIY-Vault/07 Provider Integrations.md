# Provider integrations

All providers implement a common adapter contract and report capabilities.
Local is the offline foundation. YouTube and SoundCloud provide bounded
metadata search and managed downloads through validated yt-dlp tooling;
provider playback remains outside the current boundary. Spotify is
metadata-only, never a raw audio source, and is isolated to its own search lens
behind the Plan 05 PKCE development and compliance gate.

See the dated reports in `Research/` for current API constraints and primary-source links.

## Plan 05 provider boundary

- Local queries the indexed SQLite library and returns managed typed IDs for
  local playback/file actions.
- YouTube and SoundCloud use the exact bounded yt-dlp process contract for
  metadata search and managed downloads, with no raw stderr or subprocess
  paths crossing IPC.
- Spotify uses loopback Authorization Code with S256 PKCE on `127.0.0.1` and a
  dynamic port. No client secret is accepted. Tokens remain in the keyring or
  process memory, and the explicit gate keeps the provider disabled by default.
- Search results are transient. Provider payloads, tokens, credentials, and
  raw tool output are not stored in SQLite.

## Plan 06 fusion and playback boundary

Source Fusion may evaluate Local, YouTube, and SoundCloud candidates, but
Spotify is excluded from automatic/manual fusion, overrides, acceptance,
resolver playback, and cross-provider candidate selection. Spotify's isolated
Plan 05 search lens and PKCE/compliance gate are unchanged.

Explicit acceptance persists only a validated YouTube/SoundCloud provider
identity, canonical URL when present, candidate duration, derived guarded
version, availability, and backend-owned metadata capabilities. It does not
persist search results in bulk, create a local-file record, move a track, or
change target metadata. YouTube and SoundCloud remain metadata/search-only for
playback in Plan 06; their resolver explanation is
`ProviderPlaybackNotImplemented`.

## Plan 07 download boundary

Download creation is intentionally narrow: a typed `SearchResult` or a
persisted `TrackSource` may queue only a YouTube or SoundCloud track with a
validated canonical URL/source URI. Search-result downloads do not fuse,
persist a provider source, create a `UnifiedTrack`, or alter library metadata.
Spotify and Local requests are rejected before yt-dlp execution.

The persistent `DownloadService` uses schema-v4 task rows, the existing
settings-backed `downloads_directory`, UUID-owned temp roots, structured
yt-dlp arguments, machine progress records, and FFmpeg only when video merge
requires it. `ProviderEncoded` is the honest provenance label for normal
YouTube/SoundCloud output; no lossy source is presented as lossless. Final
names are created by SpotDIY with Windows-safe sanitization and collision
handling, then moved through a destination-side temporary file without
overwriting an existing file.
