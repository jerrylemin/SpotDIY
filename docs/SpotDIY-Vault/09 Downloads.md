# Downloads

Plan 07 delivers a persistent, native-only download manager. Rust owns task
lifecycle, scheduling, process boundaries, filesystem validation, persistence,
and state events; React renders the authoritative snapshots through TanStack
Query.

## Supported creation paths

Only these narrow commands create tasks:

- `queue_search_result_download(SearchResult, DownloadMode)` for a YouTube or
  SoundCloud track with a validated canonical URL and non-empty provider ID.
- `queue_source_download(TrackId, SourceId, DownloadMode)` for a persisted
  YouTube/SoundCloud source belonging to the requested track and carrying a
  validated source URI.

Local and Spotify downloads are rejected before yt-dlp execution. Downloading
does not fuse sources, persist an ephemeral search result, create a local
`TrackSource`, create a `UnifiedTrack`, move library media, or enable online
playback.

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

yt-dlp runs through separate structured arguments with `--no-config`,
`--no-playlist`, `--newline`, `--no-warnings`, and a machine progress template.
Audio uses the best provider audio without lossy-to-FLAC conversion. Video
uses best video plus best audio and requires FFmpeg for merge/remux; missing
FFmpeg fails truthfully rather than silently selecting a lower-quality format.
Normal YouTube/SoundCloud output is labeled `ProviderEncoded`, never
`Lossless`, unless future hard evidence changes that contract.

## Storage and finalization

The download directory comes only from `SettingsSnapshot.downloads_directory`
and is chosen through the native folder dialog. Each task owns
`%LOCALAPPDATA%\SpotDIY\cache\downloads\<DownloadTaskId>`. yt-dlp writes only
`media.%(ext)s` inside that directory. SpotDIY validates a regular output,
creates a Windows-safe `Artist - Title [provider-id]` name, handles bounded
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
