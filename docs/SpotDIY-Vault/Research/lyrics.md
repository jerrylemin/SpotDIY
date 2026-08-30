# Lyrics integration research

Date: 2026-08-30
Scope: practical lyrics integration for a local-first Windows player, with local embedded/sidecar LRC first and LRCLIB as the remote fallback.
Research posture: source and documentation review only; no production code, provider write endpoint, or library file was changed.

## Recommendation

Use this precedence:

1. Embedded synchronized lyrics in the audio container.
2. An exact sibling sidecar, `<audio-stem>.lrc`; optionally `<audio-stem>.txt` for plain lyrics.
3. A validated persistent cache entry from an earlier remote lookup.
4. An opt-in, background `GET /api/get` request to LRCLIB.
5. An explicit user-driven LRCLIB search when the metadata lookup misses or returns an ambiguous match.

Local content must win over remote content and remote lookup must never delay starting or continuing audio playback. Keep the source label, raw source text/response, provider record ID when available, fetch time, and parser version with the normalized lyrics model. Treat lyric text as plain text in the UI.

LRCLIB is the most practical first remote provider found: it has a public machine-oriented API, no API key, documented request identification and rate behavior, and both legacy LRC-compatible fields and a newer `lyricsfile` representation. It should remain a replaceable provider behind a small adapter because the public service and response schema can change.

## Primary sources (URLs)

All sources below were checked on or around 2026-08-30.

- [LRCLIB API documentation](https://lrclib.net/docs) — endpoints, parameters, response fields, client identification, rate limits, errors, and batch guidance.
- [LRCLIB Lyricsfile format](https://lrclib.net/lyricsfile) — current draft 1.0 format, line/word timing model, and format licensing statement.
- [LRCLIB `get` route source](https://raw.githubusercontent.com/tranxuanthang/lrclib/main/server/src/routes/get_lyrics_by_metadata.rs) — current request validation, response shape, and server cache-key implementation.
- [LRCLIB `search` route source](https://raw.githubusercontent.com/tranxuanthang/lrclib/main/server/src/routes/search_lyrics.rs) — search parameters and response shape.
- [LRCLIB error source](https://raw.githubusercontent.com/tranxuanthang/lrclib/main/server/src/errors.rs) — current 400/404/500 error mapping.
- [LRCLIB state/cache source](https://raw.githubusercontent.com/tranxuanthang/lrclib/main/server/src/state.rs) — current provider-internal cache settings; these are observations, not client guarantees.
- [LRCLIB repository README](https://raw.githubusercontent.com/tranxuanthang/lrclib/main/README.md) and [MIT license](https://raw.githubusercontent.com/tranxuanthang/lrclib/main/LICENSE) — project and software-license context.
- [LRCGET README](https://raw.githubusercontent.com/tranxuanthang/lrcget/main/README.md) — a current LRCLIB client that downloads same-directory sidecars.
- [LRCGET LRC parser](https://raw.githubusercontent.com/tranxuanthang/lrcget/master/src-tauri/src/parser/lrc.rs) — practical timestamp parsing, multiple timestamps per line, ID tags, and sorting.
- [LRCGET metadata/sidecar reader](https://raw.githubusercontent.com/tranxuanthang/lrcget/main/src-tauri/src/scanner/metadata.rs) — exact sibling `.lrc`/`.txt` convention and audio metadata reading.
- [LRCGET LRCLIB client](https://raw.githubusercontent.com/tranxuanthang/lrcget/main/src-tauri/src/lrclib/get.rs) — current public-client request headers, timeout precedent, response handling, and status handling.
- [LRCGET export source](https://raw.githubusercontent.com/tranxuanthang/lrcget/main/src-tauri/src/export.rs) — practical MP3 `USLT`/`SYLT` and FLAC `LYRICS`/`UNSYNCEDLYRICS` embedding conventions.
- [ID3v2.4 frame specification](https://id3.org/id3v2.4.0-frames) — `USLT` unsynchronized lyrics and `SYLT` synchronized lyrics frames.
- [ID3v2.4 structure specification](https://id3.org/id3v2.4.0-structure) — text encodings and language-field details needed when reading tags.
- [RFC 9639, FLAC](https://www.rfc-editor.org/rfc/rfc9639.html) and [Xiph Vorbis Comment documentation](https://xiph.org/vorbis/doc/v-comment.html) — UTF-8, free-form metadata fields and the lack of a universal synchronized-lyrics field in Vorbis Comments.
- [`lofty` documentation](https://docs.rs/lofty/latest/lofty/) and [`ItemKey` documentation](https://docs.rs/lofty/latest/lofty/tag/enum.ItemKey.html) — a practical cross-container tag-reader model; its `Lyrics` key may contain LRC or unsynchronized text depending on the container.
- [U.S. Copyright Office: Sound Recordings vs. Musical Works](https://www.copyright.gov/music-modernization/sound-recordings-vs-musical-works.pdf) — lyrics are part of the separately protected musical work, distinct from a sound recording.

## Current API behavior

### Local embedded and sidecar lyrics

- For MP3/ID3, prefer a synchronized `SYLT` frame whose content type is lyrics and whose timestamps are absolute milliseconds. Fall back to `USLT` for unsynchronized text. A tag library should handle ID3v2.3 and v2.4 decoding, encodings, language, and duplicate frames rather than hand-decoding binary frames.
- For FLAC, Ogg, and Opus, Vorbis Comment keys are UTF-8 `name=value` fields with ecosystem-defined names. `LYRICS` commonly carries LRC text and `UNSYNCEDLYRICS` commonly carries plain text, but neither is a universal synchronized-lyrics standard. Attempt to parse a `LYRICS` value as LRC, then fall back to plain text.
- Other containers can expose plain lyric tags without exposing a synchronized standard. Preserve what the tag reader returns, but do not manufacture timing from plain text.
- For a sidecar, use the exact audio stem in the same directory. The practical convention is `track.ext` -> `track.lrc` and, optionally, `track.txt`. On Windows, extension matching can be case-insensitive, but do not add fuzzy filename matching that can attach lyrics to the wrong recording.
- Read local files as untrusted input. Prefer UTF-8 (including a BOM); optionally recognize a UTF-16 BOM. A decode failure should reject that source and allow the next source in the precedence chain to run.

### Timed LRC parsing

LRC is a de facto text format with several dialects, not a single container-level synchronization standard. Use a tolerant, bounded parser and keep the original text for diagnostics or later reparsing.

Minimum compatible grammar:

- Accept line timestamps such as `[00:01.5]`, `[00:03.00]`, and `[00:06.000]`. Interpret one, two, and three fractional digits as tenths, hundredths, and milliseconds; never use floating-point rounding for this conversion.
- Accept minutes with one or more digits and seconds in the range 0–59. Supporting an optional no-fraction form such as `[01:23]` is useful for older files. Supporting both `.` and `:` as the fraction separator is a compatibility extension seen in current provider code.
- Accept multiple leading timestamps on one line and create one cue per timestamp with the same text. Do not deduplicate equal timestamps; repeated or overlapping lines can be intentional.
- Collect common metadata tags such as `[ar:]`, `[ti:]`, `[al:]`, `[by:]`, `[la:]`, and `[au:]` separately from cues. Ignore unknown well-formed metadata tags rather than displaying them as lyric text.
- Treat `[offset:+/-N]` as an optional legacy compatibility tag. Apply a signed millisecond offset at most once, clamp resulting playback times below zero to zero, and retain the raw value. This policy must be covered by fixtures because the current LRCGET parser recognizes ID tags but does not apply an offset.
- Normalize CRLF/CR to LF, trim only the cue text boundary, preserve meaningful empty timed cues when they are used to clear the display, stably sort by timestamp, and use original order as the tie-breaker.
- Skip malformed individual lines with a diagnostic rather than failing the entire document. Reject negative/overflowing times, seconds outside 0–59, impossible fractional values, and inputs over explicit byte/line/timestamp limits.

Example of the intended normalized behavior (synthetic text):

```text
[00:01.5] First cue
[00:04.000][00:08.00] Repeated cue
```

The first cue starts at 1,500 ms. The second source line creates cues at 4,000 ms and 8,000 ms. LRC normally provides starts, not ends; infer a display interval to the next cue or track end. For overlapping cues, retain both and let the presentation layer choose a stacked or otherwise readable layout.

Use a line-level internal model for ordinary LRC. Do not infer word timing from line timestamps. LRCLIB's current `lyricsfile` format can represent line and word timing and explicit ends, so it is a useful optional richer representation, but it should not be required for the first LRC-compatible implementation. If a `lyricsfile` parse fails, fall back to `syncedLyrics` and then `plainLyrics` when present.

### LRCLIB endpoints and response behavior

- `GET https://lrclib.net/api/get` requires URL-encoded `track_name` and `artist_name`. `album_name` is optional but recommended. `duration` is optional and should be sent as a finite number of seconds when known; the current documentation describes duration as important for exact matching and a roughly ±2-second match window.
- Send a descriptive `User-Agent` containing the player name, version, and project URL or contact. Browser-like clients that cannot set `User-Agent` can use the documented `X-User-Agent` or `Lrclib-Client` alternative. No API key or registration is required.
- Current response objects expose `id`, `trackName`, `artistName`, `albumName`, `duration`, `instrumental`, `plainLyrics`, `syncedLyrics`, and `lyricsfile` (also a legacy `name` alias). Current documentation says `lyricsfile` is present on every record, but the client should still tolerate missing or null fields for older responses, proxies, and future schema changes.
- `lyricsfile` is the current richer YAML representation. Its format page calls the format draft 1.0 and describes line/word synchronization and overlap. Continue supporting the legacy plain/LRC fields for compatibility and simpler parsing.
- `GET https://lrclib.net/api/get/{id}` retrieves a known LRCLIB record by ID. Use it only for a previously selected/validated record, not as the primary metadata lookup.
- `GET https://lrclib.net/api/search` accepts `q`, `track_name`, `artist_name`, and `album_name`; at least one search term is required, `q` takes precedence, and the current documentation describes up to 20 results without pagination. Use it for explicit user recovery/selection rather than every track start.
- A missing track is currently a 404. The service notes that a missing record may be picked up by background fetching and become available later, so do not cache a 404 permanently. Malformed request parameters are 400 and should not be retried until metadata or request construction changes.
- The documented rate response is 429 with `Retry-After` in seconds. Honor it exactly, back off when it is absent, and do not continue requests while rate-limited; the documentation warns that ignoring the limit can result in a temporary ban. For library scans, the documentation recommends sequential requests with a 200–500 ms delay. Playback lookups should remain single-flight and should not launch an unbounded scan.
- LRCLIB can take significantly longer for a new lookup because a request may trigger external-source retrieval. Use a finite timeout and run the request in the background. LRCGET 2.1.0 is a practical client precedent with a 10-second request timeout; the exact timeout remains a player policy choice.
- A 200 response with no usable text and not marked instrumental should be treated as unavailable. If synced text is present, use provider plain text when present; otherwise derive a plain display by removing timing tags. If only plain text is present, display it without timed highlighting. If instrumental is true and no text is present, expose an explicit instrumental state.
- The current server source shows internal 24-hour cache TTLs and four-hour idle periods for metadata/search caches. These are implementation details of the public service, not a client retention or freshness promise.

### Request, cache, and playback policy

Use a non-blocking adapter with cancellation on track change:

1. Read and parse local sources immediately. A permission error, locked file, malformed tag, or malformed sidecar should be logged as a source-specific diagnostic and should not stop playback.
2. Check an in-memory entry, then a persistent app-data cache. Key it by provider/base URL plus normalized artist, title, album, and rounded duration; include a response/parser schema version. Title and artist alone are insufficient for remasters, live recordings, and duplicate releases.
3. Validate remote metadata against the local track before displaying it. Require an acceptable artist/title match and, when available, the documented duration tolerance. Treat missing album or materially different duration as lower confidence; offer manual search rather than silently showing a likely wrong lyric.
4. If no valid local/cache result exists and remote lookup is enabled, issue one background `/api/get` request. Do not send audio bytes—only the metadata needed for the lookup. Cancel or ignore stale results after a track change.
5. Retry only transient network errors, timeouts, 5xx responses, and 429 according to bounded exponential backoff and `Retry-After`. Do not retry 400/404 in a tight loop. Fall back to stale cache or an unavailable state without affecting audio.

Suggested client cache starting policy (product policy, not LRCLIB behavior):

| Entry | Key/data | Suggested behavior |
| --- | --- | --- |
| Positive remote result | Canonical metadata key, raw response, normalized model, provider ID, fetched time, parser/schema version | Keep in bounded persistent storage; consider 30 days of freshness and serve stale while offline. |
| Remote 404 | Same canonical key plus request version | Short negative TTL, for example 12 hours; retry after expiry because the provider can gain records later. |
| 429/transient failure | Same key plus retry state | Store backoff/retry-until metadata, not a permanent “no lyrics” result; honor server instructions. |
| Local sidecar/tag parse | Audio path plus file size/mtime or content hash and parser version | Reparse when the source changes; local source always supersedes a remote cache entry. |

Store raw remote data alongside normalized data so parser improvements can be applied without another request. Cap cache size and lyric/response payload sizes, provide a clear-cache action, and do not write fetched lyrics into the music directory unless the user explicitly exports them.

### Attribution, licensing, and privacy

- The LRCLIB MIT license applies to its server software. The Lyricsfile page's CC0 statement applies to the format/specification. Neither is a blanket license for the underlying song lyrics returned by the API.
- The Copyright Office source treats lyrics as part of the musical work, separately from the sound recording. Do not bundle a lyric corpus, silently republish fetched text, or assume that an API response is cleared for redistribution. Confirm the intended distribution/use with counsel or the relevant rightsholder; this note is not legal advice.
- Label local results as `Embedded lyrics` or the sidecar basename. Label remote results as `Lyrics from LRCLIB`, retain the provider record ID and source URL in cache metadata, and expose a link to [lrclib.net](https://lrclib.net) where appropriate. Attribution is good provenance, not a substitute for permission.
- Do not implement LRCLIB `publish` or `flag` calls in the playback path. They require write tokens/proof-of-work and create separate rights, abuse, and moderation obligations.
- Make remote lookup an explicit setting (for example, `Local only` / `Allow LRCLIB`). Explain that track metadata is sent over HTTPS, retain no audio bytes, and make cached remote lyrics removable.

## Rejected alternatives

| Candidate | Why it is not the first integration | Revisit condition |
| --- | --- | --- |
| Apple Music API | Official catalog data exposes a `hasLyrics` attribute, but the reviewed public API documentation does not provide a portable lyric-text/LRC endpoint. It also introduces developer-token and, for library data, music-user-token requirements. | A licensed API contract that returns usable timed text and permits the intended app use. |
| Spotify Web API | The reviewed search endpoint returns catalog metadata, not lyric text or timing, and requires OAuth/account/market context. | A documented, licensed synced-lyrics product/API becomes available for the target player. |
| Genius API or HTML scraping | The official documentation reviewed does not provide a stable timed-lyrics payload. Scraping pages or using undocumented endpoints adds breakage, terms, and rights risk. | A documented API agreement with timed text and clear use/attribution rights. |
| Musixmatch | Not a drop-in public LRC source for this scope without a confirmed commercial/licensed integration and a response contract suitable for a local player. | A commercial agreement and verified timed-lyrics API terms. |
| lyrics.ovh | Useful as a plain-lyrics website, but no dependable synced-LRC contract, attribution/license policy, or rate behavior was established in the reviewed public material. | A documented timed endpoint with clear rights and operational limits. |
| Unofficial provider proxies/scrapers | Unstable schemas, unclear provenance/terms, possible privacy leakage, and no reliable takedown or rate-limit contract. | Only after a separately reviewed provider contract and rights model. |
| Lyricsfile-only implementation | The current format is promising for word timing and explicit ends, but draft-format adoption is narrower than ordinary LRC and is unnecessary for the local-first baseline. | Add as a richer parser/export path after line-level LRC behavior is stable. |

Self-hosted LRCLIB is not rejected: the official repository supports running the service locally and it can be useful for controlled deployments. It brings database, update, and corpus-operation responsibilities, so keep the public base URL configurable and treat self-hosting as an advanced option rather than a first-run dependency.

## Risks

- **Rights and provenance:** lyric text can be copyrighted even when the provider software and file format are permissively licensed. Remote display, persistent retention, export, and redistribution are separate product/legal decisions.
- **Provider availability and limits:** the public service can change, throttle, become unavailable, or take a long time while sourcing a new result. Playback must be independent of it, and the client must honor 429/backoff behavior.
- **False matches:** album variants, live/remastered versions, explicit edits, duplicate titles, punctuation, and missing/incorrect duration can produce plausible but wrong lyrics. Validate metadata and make manual selection visible.
- **Timing drift:** LRC line starts are not ends, recordings can differ from the matched release, and `[offset:]` handling varies. Preserve source timing, apply any compatibility offset once, and let users recover to another source.
- **Format variance:** ID3 has a native `SYLT` frame, while Vorbis Comments are free-form conventions. Duplicate frames, language variants, plain-vs-synced fields, UTF-16 tags, BOMs, and malformed sidecars are normal interoperability cases.
- **Untrusted content:** cap input size and cue counts, parse YAML/data formats safely, render lyrics as plain text, and never let lyric content execute markup or control characters. Use exact local paths and do not derive filenames from remote data.
- **Stale cache:** cached lyrics can become outdated or may have been matched under different metadata. Store fetched/checked times, parser version, and source identity; allow clear/retry and serve stale only with an appropriate offline indication.
- **Privacy:** remote lookup reveals track metadata. Provide a local-only mode and ensure no audio file contents are uploaded.
- **Schema drift:** tolerate absent/null legacy fields and unknown response fields. Keep the provider adapter isolated, retain raw responses, and re-run contract checks before releases.

## Version assumptions

- Research date and current API snapshot: 2026-08-30. LRCLIB documentation and `main` branch source are mutable; re-check the endpoint contract, rate guidance, response fields, and provider terms immediately before implementation/release.
- The current LRCLIB docs/source reviewed include `plainLyrics`, `syncedLyrics`, and `lyricsfile`; the current Lyricsfile page describes draft format version 1.0. The baseline should continue to work if `lyricsfile` is absent.
- LRCGET source reviewed was version 2.1.0. Its source uses a 10-second HTTP timeout, exact same-directory `.lrc`/`.txt` sidecars, and a `User-Agent` identifying the application. These are practical precedents, not requirements imposed on SpotDIY.
- The current LRCGET manifest declares `lofty = "0.24.0"`; current docs.rs resolves a `lofty` 0.25.1 documentation page. If a unified tag reader is selected, pin and verify the actual version in the implementation rather than copying this research assumption.
- ID3v2.4 remains the reference specification for `USLT`/`SYLT`; support for common ID3v2.3 files should be delegated to the selected tag library. FLAC metadata assumptions follow RFC 9639 and Vorbis Comment conventions.
- No committed player code, manifest, tests, or dependency decision was available in the inspected workspace, so this note intentionally remains architecture-agnostic. The first implementation should add fixture tests for embedded `SYLT`/`USLT`, FLAC/Vorbis LRC/plain fields, sidecars, malformed input, offsets, duplicate timestamps, provider 200/404/429/5xx responses, stale cache, and track-change cancellation.
