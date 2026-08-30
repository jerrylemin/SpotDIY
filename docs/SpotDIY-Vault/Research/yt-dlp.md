# yt-dlp: YouTube and SoundCloud integration research

## Date

2026-08-30

## Primary sources (URLs)

- [yt-dlp 2026.08.19 stable release](https://github.com/yt-dlp/yt-dlp/releases/tag/2026.08.19)
- [yt-dlp README, pinned to 2026.08.19](https://github.com/yt-dlp/yt-dlp/blob/2026.08.19/README.md)
- [Python API: `YoutubeDL`](https://github.com/yt-dlp/yt-dlp/blob/2026.08.19/yt_dlp/YoutubeDL.py)
- [Extractor result and search contracts](https://github.com/yt-dlp/yt-dlp/blob/2026.08.19/yt_dlp/extractor/common.py)
- [YouTube search extractors](https://github.com/yt-dlp/yt-dlp/blob/2026.08.19/yt_dlp/extractor/youtube/_search.py)
- [YouTube video and stream extraction](https://github.com/yt-dlp/yt-dlp/blob/2026.08.19/yt_dlp/extractor/youtube/_video.py)
- [YouTube clients and PO-token policies](https://github.com/yt-dlp/yt-dlp/blob/2026.08.19/yt_dlp/extractor/youtube/_base.py)
- [SoundCloud extractor, search, metadata, and formats](https://github.com/yt-dlp/yt-dlp/blob/2026.08.19/yt_dlp/extractor/soundcloud.py)
- [yt-dlp CLI option definitions](https://github.com/yt-dlp/yt-dlp/blob/2026.08.19/yt_dlp/options.py)
- [Official EJS setup guide](https://github.com/yt-dlp/yt-dlp/wiki/EJS)
- [Official YouTube PO Token Guide](https://github.com/yt-dlp/yt-dlp/wiki/PO-Token-Guide)
- [FFmpeg documentation: streamcopy and transcoding](https://ffmpeg.org/ffmpeg.html)
- [FFmpeg format documentation: HLS and DASH](https://ffmpeg.org/ffmpeg-formats.html)

This is a source review of the stable tag above, supplemented by the current official yt-dlp wiki pages and FFmpeg documentation. It is not a claim that YouTube or SoundCloud behavior will remain unchanged after the snapshot date.

## Current API behavior

### Search and supported syntax

The shared `SearchInfoExtractor` grammar is:

| Input | Meaning |
| --- | --- |
| `KEY:QUERY` | Return one result. |
| `KEYN:QUERY` | Return `N` results, subject to the extractor's maximum/pagination. |
| `KEYall:QUERY` | Paginate until the extractor's supported result set is exhausted or capped. |

For the two services in scope:

```text
ytsearch:QUERY
ytsearch10:QUERY
ytsearchall:QUERY

scsearch:QUERY
scsearch10:QUERY
scsearchall:QUERY
```

The query is everything after the prefix. Quote the complete input in a shell when it contains spaces, `&`, or other shell metacharacters. Search results are playlist-like results; iterate their `entries` and resolve the selected entry again before playback.

YouTube also has a dedicated search-URL extractor for URLs such as:

```text
https://www.youtube.com/results?search_query=QUERY
https://www.youtube.com/results?q=QUERY
```

The extractor accepts YouTube's `sp` filter/sort parameter. `sp` is an opaque site-generated token; do not hard-code human meanings for it. YouTube Music search URLs are handled separately and can select sections such as songs, albums, artists, playlists, and videos.

SoundCloud's dedicated search extractor uses `scsearch...:` and currently calls the SoundCloud v2 `search/tracks` endpoint with linked pagination. Its search entries are intentionally flat: the source passes `extract_flat=True`, so entries carry partial API metadata but do not yet have resolved media formats. A SoundCloud search-page URL is not the stable integration contract; use the explicit prefix.

`--default-search PREFIX` can apply a prefix to an input that is not recognized as a URL, but `auto` guesses the extractor. For deterministic application behavior, use an explicit `ytsearch` or `scsearch` prefix.

### Metadata and extraction

The Python entry point is `YoutubeDL.extract_info(url, download=False, process=True)`. `download=False` suppresses the file download; it does not mean that extraction is offline. With `process=True` (the API default), yt-dlp resolves playlist/URL references, processes formats, and selects a format. `extract_flat=True` is appropriate for a cheap listing but leaves references unresolved and can omit full item metadata.

Typical application flow:

```python
with yt_dlp.YoutubeDL({"format": "bestaudio/best"}) as ydl:
    info = ydl.extract_info(input_url, download=False, process=True)
    serializable = ydl.sanitize_info(info)
```

The returned object is dictionary-like but is not guaranteed to be a plain JSON-serializable dictionary. Use `sanitize_info` before serializing. Normalized fields commonly include:

- identity and presentation: `id`, `title`, `description`, `webpage_url`, `thumbnail`/`thumbnails`;
- creator and release data: `uploader`, `uploader_id`, `uploader_url`, `channel`, `timestamp`, `upload_date`, `release_timestamp`;
- media and engagement data: `duration`, `view_count`, `like_count`, `comment_count`, `categories`, `tags`, `availability`, and live-status fields;
- audio-oriented fields where supplied: `track`, `artists`, `album`, `genres`, `release`, and `creator`;
- format data: `formats`, plus the selected format fields such as `url`, `ext`, `protocol`, `acodec`, `vcodec`, `abr`, and `http_headers`.

The exact field set is extractor/site dependent. YouTube and SoundCloud can omit fields, return `None`, or expose different counts and release metadata. SoundCloud's extractor explicitly maps track, artist, genre, tag, playback/favorite/comment/repost counts, thumbnails, and media transcodings. A flat search result is not evidence that the full track metadata or a playable URL is available.

For CLI inspection, use `-j/--dump-json`, `-J/--dump-single-json`, or `--print` rather than depending on ordinary human-readable stdout. `--embed-metadata` is a download/post-processing operation and is separate from the metadata returned by `extract_info`.

### Playback URL resolution

Recommended single-stream flow:

1. Search with `ytsearchN:` or `scsearchN:`.
2. Take the selected entry's `webpage_url` or canonical URL/ID.
3. Call `extract_info` again with `download=False`, `process=True`, and a single-stream selector such as `bestaudio/best`.
4. Read the selected `info["url"]`; inspect `protocol`, `ext`, codecs, and `http_headers` before handing it to a player.

After processing, yt-dlp updates the top-level info dictionary with the selected best format for compatibility. For a single-format selector, `info["url"]` is the current media URL. It can be a direct HTTP URL or a playlist/manifest URL such as HLS (`m3u8`) or DASH, not necessarily a permanent standalone file. If a selector requests multiple streams, such as `bv+ba`, inspect `requested_formats` and let yt-dlp/FFmpeg merge them; do not assume the top-level URL is one player-ready stream.

YouTube resolution currently includes player-response format discovery, signature-cipher and `n`-parameter challenge solving, and—in applicable client/request paths—PO-token handling. The YouTube source can expose direct formats as well as HLS/DASH manifests. Full YouTube support requires the `yt-dlp-ejs` challenge scripts and a supported JavaScript runtime; the official guide currently recommends Deno, with Node and QuickJS as alternatives. Some clients and requests require PO tokens, which may be video-bound and can be supplied by a provider/plugin or extractor configuration.

SoundCloud resolution first resolves a track URL through the v2 API, then obtains the current stream URL for one of the track's progressive or HLS transcodings. The extractor dynamically obtains/caches a SoundCloud client ID and may refresh it after authorization failures. Current format identifiers are generated from protocol and codec combinations such as `http_aac`, `hls_opus`, and `http_mp3`; original `download` quality is conditional on the track/account.

The CLI equivalent for inspecting a single resolved URL is `yt-dlp -g -f bestaudio URL`. For an embedded application, the Python API is preferable because it exposes metadata, selected format details, headers, and structured errors together.

Treat resolved URLs as ephemeral. Persist the canonical webpage URL/platform ID and re-resolve close to playback; do not persist raw signed/tokenized URLs or put them in logs. This is an integration recommendation based on yt-dlp's current signature, challenge, client, and tokenized URL flow.

### Progress output

For the Python API, configure `progress_hooks`. Hook dictionaries report a `status` such as `downloading`, `finished`, or `error`; downloading/finished events can include `filename`, byte totals or estimates, `downloaded_bytes`, `elapsed`, `eta`, `speed`, and fragment indexes/counts. Successful downloads guarantee at least one `finished` event. Use `postprocessor_hooks` for post-processing lifecycle events (`started`, `processing`, `finished`). Ignore unknown future statuses.

For CLI integrations, use:

- `--progress-template` with `download`, `download-title`, `postprocess`, or `postprocess-title` templates;
- `--newline` when line-oriented progress is needed;
- `--print` or JSON output for metadata/status fields.

Do not parse the normal human progress bar or ordinary stdout as an API; the official embedding documentation warns that its format may change. Fragmented HLS/DASH downloads may not provide a reliable total byte count, ETA, or speed, so consumers must handle missing or estimated values. `download=False` produces no download-progress lifecycle because no file transfer is performed.

### Download formats and FFmpeg

Format IDs are extractor-specific and must be read from the current extraction (`-F/--list-formats` or `info["formats"]`). Common selectors are:

| Selector | Use |
| --- | --- |
| `b` / `best` | Best single combined format when available. |
| `bv` / `bestvideo` | Video-only format. |
| `ba` / `bestaudio` | Audio-only format. |
| `bv+ba` | Separate best video and audio, then merge. Requires FFmpeg. |
| `bv*+ba/b` | yt-dlp's general best video-containing plus audio fallback. |
| `b[ext=mp4]`, `bv*[height<=720]` | Format filters; combine with `-S` for sorting. |

With no explicit selector, the documented default generally prefers the best video/audio combination and falls back to a single combined format. Without FFmpeg, or when writing to stdout, yt-dlp changes the default toward a single downloadable format. Use `bestaudio/best` when the consumer needs one audio stream rather than a merged video/audio result.

SoundCloud's extractor argument `soundcloud:formats` limits requested protocol/codec combinations. The current default set is `http_aac,hls_aac,http_opus,hls_opus,http_mp3,hls_mp3`; `*` and wildcard codec forms are supported. Formats may be progressive HTTP, HLS, or encrypted HLS, and an original `download` format may be added only when SoundCloud permits it.

yt-dlp uses an FFmpeg binary—not a Python package—for merging and post-processing. FFmpeg streamcopy can remux packets without re-encoding when the target container accepts the codecs; incompatible targets require a different container or transcoding. HLS/DASH manifests are demuxed as segmented/variant media, so a player must support the protocol or the app must use FFmpeg/yt-dlp to download or convert it. `-x/--extract-audio`, audio conversion, and many metadata/container operations require both `ffmpeg` and `ffprobe`.

### Platform limitations

**YouTube**

- YouTube changes clients, player responses, signatures, `n` challenges, PO-token enforcement, and access checks. A stable yt-dlp release can lag a site change.
- Missing `yt-dlp-ejs` or a JavaScript runtime can remove formats or make extraction fail. The official source warns that JavaScript-less YouTube extraction is deprecated.
- PO tokens may be required for Google Video Server, player, or subtitle requests depending on the client. Missing/invalid tokens can yield HTTP 403, missing formats, or blocking.
- Age/login requirements, cookies, geo restrictions, rate limits, CAPTCHA/bot checks, live-stream state, DRM, and SABR/client limitations can prevent a playable format even when metadata is visible.
- Search ordering and filter semantics are controlled by YouTube. `sp` tokens and result availability are not a stable application-level taxonomy.

**SoundCloud**

- The extractor relies on SoundCloud's current v2 API behavior and a dynamically discovered client ID; neither should be treated as a stable public application API.
- The source warns of approximately 600 API requests per 10 minutes. Search pagination and resolving many tracks must be rate-limited and retried conservatively.
- Original downloads, higher-quality/AAC variants, previews, Go+/premium media, geo-restricted tracks, and DRM-protected media are conditional. A search result does not guarantee a downloadable stream.
- Stream availability and metadata can differ by track; HLS/progressive transcodings may be the only options, and preview formats may be limited.

**Both services**

- Metadata is best-effort and can change between extraction and playback. Format IDs, direct URLs, response schemas, and search order are not durable identifiers.
- Multi-stream merging and audio conversion consume temporary disk, CPU, and possibly network bandwidth. Container/codec incompatibility can make an otherwise valid download fail.
- Respect the services' terms, copyright permissions, access controls, and account/cookie boundaries. Never expose cookies, PO tokens, client IDs, or resolved playback URLs in logs or user-visible error messages.

## Rejected alternatives

- **Parse ordinary yt-dlp stdout or the human progress bar.** Rejected because the official embedding guidance says normal stdout is not a stable interface. Use Python hooks, `--progress-template`, `--print`, or JSON.
- **Use a flat search entry as the playback URL.** Rejected because search results—especially SoundCloud results—are unresolved/flat and may have `formats=None`. Resolve the canonical item URL in a second extraction.
- **Call SoundCloud's v2 endpoint directly or hard-code its client ID.** Rejected because the current extractor discovers and refreshes the client ID and handles current transcoding/API details. Use yt-dlp's extractor boundary unless a separately maintained official API contract is adopted.
- **Hard-code YouTube itags, SoundCloud format IDs, or one global format map.** Rejected because format IDs are extractor-specific and current formats/access policies change. Inspect formats at extraction time.
- **Pass a `bv+ba` result directly to a player as one URL.** Rejected because the selection represents multiple streams and normally requires FFmpeg merging. Resolve `bestaudio`/another single stream for playback, or download/mux through yt-dlp and FFmpeg.
- **Use `--default-search auto` as the app's search protocol.** Rejected for deterministic behavior because it guesses an extractor. Use explicit `ytsearch...:` and `scsearch...:` syntax.

## Risks

- **Release/site drift:** pin and test a known yt-dlp version, but plan for rapid updates when either site changes. Recheck EJS, PO-token, client, and extractor behavior before upgrading or after extraction incidents.
- **Ephemeral credentials and URLs:** treat resolved URLs, headers, cookies, and PO-token material as secrets with short lifetimes. Store canonical IDs/URLs instead.
- **Runtime packaging:** a deployment that ships yt-dlp without matching `yt-dlp-ejs`, a supported JavaScript runtime, FFmpeg, and FFprobe will have materially different behavior from a developer machine.
- **Protocol mismatch:** a UI/player that only accepts a direct MP3/MP4 URL will not handle every valid HLS/DASH result. Detect `protocol` and define a fallback policy.
- **Rate and resource limits:** batch search/resolution can hit SoundCloud request limits, YouTube throttling, temporary URLs, disk pressure, or FFmpeg CPU cost. Bound concurrency and retries.
- **Untrusted input:** if an app accepts arbitrary user URLs, constrain accepted hosts and output paths before invoking yt-dlp; its broader extractor set is larger than the two services documented here.
- **Metadata assumptions:** missing fields and unsupported container tags are normal. Treat metadata as nullable and validate the final output rather than assuming `--embed-metadata` can represent every field.

## Version assumptions

- Research date: **2026-08-30**.
- yt-dlp stable baseline: **2026.08.19**, release commit **3a08bea**, verified from the official release page on the research date. Code links in this note are pinned to the `2026.08.19` tag.
- Stable-channel documentation notes that stable releases can be stale relative to external site changes. Nightly/master behavior may differ; this note does not silently substitute those channels for the stable baseline.
- The official EJS and PO-token wiki pages were consulted as current guidance on 2026-08-30. Their operational recommendations can change independently of the pinned source tag.
- FFmpeg links point to current upstream documentation, but no FFmpeg binary/build is pinned here. Validate the exact deployed `ffmpeg`/`ffprobe` versions and codec/container support in the target environment.
- Re-run this research after yt-dlp upgrades, YouTube/SoundCloud extraction failures, major search/API changes, or a change to the playback/player architecture.
