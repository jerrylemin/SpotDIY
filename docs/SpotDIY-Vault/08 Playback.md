# Playback

## Plan 04 boundary

`PlaybackService` is the sole authoritative playback controller. It consumes
typed user commands and typed backend events through one serialized loop and
publishes a revisioned `PlaybackSnapshot` through `tokio::watch` and the
`playback://state` Tauri event. The frontend sends only `TrackId`, optional
`SourceId`, and transport values; it never receives or supplies a filesystem
path, URL, mpv pipe name, request ID, executable path, or raw JSON.

The service resolves only managed, indexed, enabled, available local sources
through `LibraryService`. An explicit source must belong to the requested
track, advertise playback capability, have a managed `LocalFileSource`, and
still be a regular file. Without an explicit source, the valid playable local
preferred source is selected first, followed by deterministic repository
order. A missing file returns `LocalFileMissing`; PlaybackService does not
mutate library rows.

## Backend

`MpvBackend` starts one persistent external `mpv.exe` child over one fresh
random Windows named pipe. Its exact startup arguments are:

```text
--no-config --idle=yes --terminal=no --input-terminal=no --audio-display=no --input-ipc-server=<fresh pipe>
```

The backend owns newline-delimited JSON framing, a 1 MiB frame limit, positive
request IDs, reply correlation, interleaved event/reply handling, child-exit
monitoring, and bounded quit/kill/reap. It observes pause, time-pos, duration,
volume, mute, and seeking at approximately 250 ms and normalizes mpv events to
product-level events. Lifecycle queues are bounded; critical events await
capacity and position samples may be coalesced at approximately 4 Hz. The
`--no-config --version` probe has a finite process/output budget and cleans up
only its own child. mpv health is discovered through `SPOTDIY_MPV_PATH`, then
PATH; missing or broken mpv leaves the library usable.

## State and queue policy

Playback phases are `Idle`, `Loading`, `Playing`, `Paused`, `Seeking`, `Ended`,
`Recovering`, `Failed`, and `ShuttingDown`. Queue entries contain only an
opaque `QueueEntryId`, `TrackId`, and optional requested `SourceId`.

Play Now replaces the transient queue with one entry and loads it. Add to
Queue appends without autoplay; Play Next inserts after the current canonical
entry; Clear Queue stops playback, clears state, and leaves mpv idle. EOF is
the only automatic advancement trigger. Repeat Off ends at the final entry,
Repeat One reloads the current entry from zero, and Repeat All wraps. Manual
Next ignores Repeat One and wraps only for Repeat All. Previous restarts when
position is greater than 3000 ms; otherwise it moves to the previous entry or
restarts the first entry.

Canonical queue order and active play order are separate. Shuffle uses
Fisher-Yates, preserves the current item and consumed history, and restores
canonical ordering when disabled. The queue is transient and non-persistent;
persistent queue state and queue snapshots belong to Plan 08.

Source switching preserves queue context, timestamp, pause state, volume,
mute, repeat, shuffle, and selected device where valid. If the replacement
load fails, rollback restores the prior identity/source and queue entry; a
rollback backend failure enters normal recovery without advancing the queue.
Backend disconnect or crash recovery retries at 250/750/1500 ms, rejects
stale-generation events, and exposes manual retry after exhaustion. Shutdown publishes
`ShuttingDown`, sends bounded quit/kill/reap, and rejects new commands.

See [ADR-0009](ADRs/ADR-0009-external-mpv-json-ipc.md) and the repository
`ARCHITECTURE.md` for the implementation boundary.

## Plan 06 source resolution

`SourceResolver` now owns source ranking for normal playback and exact-source
switches. A currently playable `preferredSourceId` wins; otherwise the
validated settings provider order is used. Local candidates are playable only
when available, playback-capable, and successfully resolved through
`LibraryService`. Known lossless local codecs (`flac`, `alac`, `pcm_*`,
`wavpack`, `ape`) rank ahead of lossy sources, followed by bit depth, sample
rate, bitrate, and stable `SourceId`.

YouTube and SoundCloud return `ProviderPlaybackNotImplemented`; Spotify
returns `MetadataOnly`. No online URL is sent to mpv and yt-dlp is not invoked
for playback. `PlaybackSourceOption` carries `availabilityDetail` so the
existing source selector/player surface can explain unavailable sources
without a UI redesign.
