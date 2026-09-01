# ADR-0013: Persistent download manager

- Status: Accepted
- Date: 2026-09-01
- Scope: Plan 07

## Context

SpotDIY needs provider downloads that remain observable across restarts without
turning the React frontend into a process or filesystem owner. YouTube and
SoundCloud provide provider-encoded media through yt-dlp; Spotify is
metadata-only and Local playback files are already owned by the library
boundary. Download output must not trust provider filenames, overwrite an
unrelated user file, or mislabel lossy provider audio as lossless.

## Decision

Implement one Rust `DownloadService` backed by schema version 4. It persists
only normalized task metadata and lifecycle facts in `downloads`, plus the
1..4 concurrency setting in the singleton `download_settings` row. The service
accepts only two typed creation paths: a validated YouTube/SoundCloud search
result or a validated persisted source belonging to the requested track.

Every task receives a UUID and an owned temp root under
`%LOCALAPPDATA%\SpotDIY\cache\downloads\<DownloadTaskId>`. yt-dlp is launched
directly with separate arguments, `--no-config`, `--no-playlist`, `--newline`,
`--no-warnings`, a machine progress template, and `media.%(ext)s` under that
root. Output and diagnostics are bounded; only the owned child can be
cancelled and it is always waited/reaped.

Audio preserves the best available provider encoding and records
`ProviderEncoded`; it never performs a lossy-to-FLAC conversion. Video selects
best video plus best audio and requires the validated FFmpeg tool for merge or
remux. Tool absence is surfaced as a truthful failure.

Finalization validates a regular output inside the exact task root, creates a
SpotDIY-owned Windows-safe filename, chooses a bounded collision-free name,
copies through a destination-side temporary file, flushes and renames without
overwrite, persists the trusted final path, and only then removes the owned
temp root. The procedure works across volumes and never deletes outside the
owned root.

State transitions, progress persistence throttling, cancellation, retry,
restart recovery, output-missing history, and shutdown cleanup remain in Rust.
The frontend consumes revisioned `downloads://state` snapshots and uses only
narrow typed commands. `open_download_location` accepts a `DownloadTaskId`
and resolves the trusted destination on the backend; no arbitrary-path opener
is exposed.

## Consequences

Downloads are durable and honest: interrupted tasks can be requeued safely,
completed history remains visible when output is missing, and users can see
provider provenance and tool health. Downloaded files outside configured
library roots remain valid completed downloads without automatic library
creation or movement. Spotify and Local requests fail before yt-dlp execution,
and online playback, source fusion, playlists, persistent playback queue, and
other later-plan behavior remain outside this boundary.

The native runtime requires a configured destination and available yt-dlp;
video merge additionally requires FFmpeg. Live provider/download smoke remains
opt-in, while deterministic fake-runner and process-boundary tests cover the
service lifecycle without committing media or credentials.
