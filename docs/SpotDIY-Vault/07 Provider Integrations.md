# Provider integrations

All providers implement a common adapter contract and report capabilities.
Local is the offline foundation. YouTube and SoundCloud provide bounded
metadata search, MPV-backed playback from validated provider URLs, and managed
downloads through validated yt-dlp tooling. Spotify search uses the local
`spotdl` CLI and can queue audio-only source-matched MP3 downloads through the
existing yt-dlp/FFmpeg boundary. Spotify is not used as a raw audio source or
an in-app playback stream, and no Spotify developer app or PKCE credentials
are required.

See the dated reports in `Research/` for current API constraints and primary-source links.

## Plan 05 provider boundary

- Local queries the indexed SQLite library and returns managed typed IDs for
  local playback/file actions.
- YouTube and SoundCloud use the exact bounded yt-dlp process contract for
  metadata search and managed downloads, with no raw stderr or subprocess
  paths crossing IPC.
- The former Spotify PKCE/catalog boundary is historical. The active boundary
  invokes `spotdl save` for transient metadata/search results. Spotify play
  and download then use public Spotify embed metadata plus bounded `yt-dlp`
  `ytsearch25` matching, accepting only a title/artist match within the
  duration tolerance before the audio-only pipeline runs.
- Search results are transient. Provider payloads, tokens, credentials, and
  raw tool output are not stored in SQLite.

## Plan 06 fusion and playback boundary

Source Fusion may evaluate Local, YouTube, and SoundCloud candidates, but
Spotify is excluded from automatic/manual fusion, overrides, acceptance,
resolver playback, and cross-provider candidate selection. Its search-result
download path is a separate source-matching operation; persisted Spotify
sources still do not become playable or downloadable library sources.

Explicit acceptance persists only a validated YouTube/SoundCloud provider
identity, canonical URL when present, candidate duration, derived guarded
version, availability, and backend-owned metadata capabilities. It does not
persist search results in bulk, create a local-file record, move a track, or
change target metadata. YouTube and SoundCloud remain metadata/search-only for
playback in Plan 06; their resolver explanation is
`ProviderPlaybackNotImplemented`.

## Plan 07 download boundary

Download creation is intentionally narrow: a typed `SearchResult` may queue a
YouTube, SoundCloud, or Spotify track with a validated canonical URL. Spotify
search results are audio-only; `spotdl` resolves the Spotify URL to a validated
YouTube/SoundCloud source and the existing yt-dlp/FFmpeg worker creates MP3
output. A persisted `TrackSource` may still queue only a YouTube or SoundCloud
source. Search-result downloads do not fuse, persist a provider source, create
a `UnifiedTrack`, or alter library metadata. Local and persisted Spotify
requests are rejected before yt-dlp execution.

The persistent `DownloadService` uses schema-v4 task rows, the existing
settings-backed `downloads_directory`, UUID-owned temp roots, structured
`spotdl`/yt-dlp arguments, machine progress records, and FFmpeg for video
merge or Spotify MP3 extraction. `ProviderEncoded` is the honest provenance
label for normal YouTube/SoundCloud output; no lossy source is presented as lossless. Final
names are created by SpotDIY with Windows-safe sanitization and collision
handling, then moved through a destination-side temporary file without
overwriting an existing file.

## Plan 16 provider runtime repair — 2026-09-04

YouTube advertises downloads and canonicalizes a validated `webpage_url`,
falling back only from a strict 11-character video ID. SoundCloud accepts only
validated full URLs from its alternate URL fields; it advertises audio
downloads only. Spotify is ready when `spotdl` and `yt-dlp` are installed,
searches through the bounded local CLI, and advertises audio-only search-result
downloads. The public Spotify embed plus bounded yt-dlp matcher resolves each
result to a validated YouTube URL.
No client ID, market, login, token, or credential is required. Provider
search/download failures cross the native boundary as structured code/detail
values without raw commands or credentials.

The native search path uses a bounded `ytsearch25` query for YouTube and a
faster `scsearch5` query for SoundCloud. A valid result can be played directly
through MPV; playback persists one validated provider source so queue and
restart behavior use the same source-resolution boundary. Download controls
open the native folder picker when no destination is configured and keep
YouTube video mode disabled until FFmpeg is ready.
