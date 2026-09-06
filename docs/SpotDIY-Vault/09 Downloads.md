# Downloads

Plan 07 delivers a persistent, native-only download manager. Rust owns task
lifecycle, scheduling, process boundaries, filesystem validation, persistence,
and state events; React renders the authoritative snapshots through TanStack
Query.

## Supported creation paths

Only these narrow commands create tasks:

- `queue_search_result_download(SearchResult, DownloadMode)` for a YouTube,
  SoundCloud, or Spotify track with a validated canonical URL and non-empty
  provider ID. Spotify is audio-only and is source-matched through public
  Spotify embed metadata plus bounded yt-dlp matching.
- `queue_source_download(TrackId, SourceId, DownloadMode)` for a persisted
  YouTube/SoundCloud source belonging to the requested track and carrying a
  validated source URI.

Local and persisted Spotify-source downloads are rejected before yt-dlp
execution. Downloading does not fuse sources, persist an ephemeral search
result, create a local `TrackSource`, create a `UnifiedTrack`, move library
media, or enable online playback.

## Persistence and lifecycle

Schema version 4 adds only `downloads` and the singleton `download_settings`
table. Tasks use UUID IDs and persist provider identity, canonical URL,
normalized title/artists/artwork, mode, state, destination, output format and
codec when known, provider-encoded provenance, progress, speed, ETA, retry
count, errors, and timestamps. Valid states are `queued`, `resolving`,
`downloading`, `postprocessing`, `completed`, `failed`, and `cancelled`.

The default concurrency is 2, configurable from 1 through 4. Only queued
tasks start. Active cancellation kills and reaps only the task-owned yt-dlp
child, cleans only its owned temp root, and records `cancelled`. Retry reuses
the trusted persisted provider identity without making a duplicate row.
Startup requeues interrupted active tasks after owned-temp cleanup; completed
history remains visible with `outputMissing` when its recorded file is gone.

## Tools and provenance

Spotify search-result tasks first read public Spotify embed metadata and use
bounded yt-dlp matching to resolve the result to a validated YouTube source.
yt-dlp then runs through separate structured arguments with `--no-config`, `--no-playlist`, `--newline`,
`--no-warnings`, and a machine progress template. Normal audio uses the best
provider audio; Spotify audio is explicitly extracted as MP3 with FFmpeg and
embeds the provider metadata when FFmpeg is available.
Video uses best video plus best audio and requires FFmpeg for merge/remux;
missing FFmpeg fails truthfully rather than silently selecting a lower-quality
format.
Normal YouTube/SoundCloud output is labeled `ProviderEncoded`, never
`Lossless`, unless future hard evidence changes that contract.

## Storage and finalization

The download directory comes only from `SettingsSnapshot.downloads_directory`
and is chosen through the native folder dialog. Each task owns
`%LOCALAPPDATA%\SpotDIY\cache\downloads\<DownloadTaskId>`. yt-dlp writes only
`media.%(ext)s` inside that directory. SpotDIY validates a regular output,
creates a Windows-safe `Artist - Title` name, handles bounded
collisions as `(2)`, `(3)`, and so on, copies through a destination-side
temporary file, flushes and renames without overwrite, persists the final
path, and only then removes the owned temp directory. Cross-volume moves are
therefore supported without trusting provider filenames.

## Native interface and UI

The state stream is `downloads://state` with monotonically increasing
snapshot revisions. The narrow commands are `get_download_snapshot`,
`queue_search_result_download`, `queue_source_download`, `cancel_download`,
`retry_download`, `set_download_concurrency`, and `open_download_location`.
The last command accepts only a `DownloadTaskId`; Rust resolves the trusted
destination directory. DownloadsPage shows task facts, provider/tool health,
folder selection, concurrency, filtering, progress, provenance, output
missing, and valid cancel/retry/open actions. Search cards expose Audio/Video
download controls only for supported provider tracks.

## Plan 16 runtime download usability — 2026-09-04

Action readiness now reflects the native track URL, provider capability,
`spotdl`/yt-dlp health, destination-folder state, and FFmpeg requirement for
video or Spotify MP3 extraction. YouTube exposes Audio and Video; SoundCloud
exposes Audio only; Spotify search results expose Audio only; Local exposes no
download action. Native queue failures return structured error codes and safe
details rather than raw command lines or filesystem secrets. If the
destination is missing or invalid, the search card and inspector open the
native folder picker and persist the selected directory before queueing. Live
download execution remains skipped without an approved legal fixture.
