# Lyrics

Plan 09 delivers local-first lyrics with deterministic precedence: manual
override, exact `.lrc` sidecar, embedded timed text, embedded plain text, then
cached LRCLIB. Sidecar and embedded reads are bounded and read-only through the
managed `LibraryService` path boundary; manual import uses the native picker and
never accepts an arbitrary frontend path.

LRC parsing supports integer and 1/2/3-digit fractions, multiple timestamps,
metadata, signed offsets, inline timestamps, stable ordering, and malformed-line
plain-text fallback. Embedded ID3 plain and SYLT text are exposed as typed
documents. The `/lyrics` surface follows playback position, shows source and
attribution state, and keeps edit/delete/import actions explicit.

When local lyrics are absent, `useLyrics` automatically looks up LRCLIB once
per track/source during the app session. Explicit retry/search/select remain
available. Requests are HTTPS-only, bounded and rate-gated. Search uses
`track_name` and `artist_name` without `q`, because LRCLIB ignores structured
filters when `q` is present. Common video annotations are removed from search
titles; live/remix/version qualifiers are retained. Existing title, artist and
duration validation still applies before caching. No raw provider response or
credentials are retained or logged.

The lyrics page uses a large type, green stage with smooth line transitions,
viewport-only scrolling and a manual-scroll follow toggle. Clicking a timestamped
line seeks with the user's offset applied. Reduced motion disables transitions.
Plain lyrics remain readable text; timings are not fabricated.

Spotify's public Web API does not expose a lyrics endpoint. Spotify metadata
already present on a unified track can inform lookup, but this implementation
does not access Spotify's private lyrics service. Coverage depends on LRCLIB
and local lyrics; near-universal coverage has not been established.
Sources: https://developer.spotify.com/documentation/web-api/reference and
https://lrclib.net/docs (checked 2026-09-07).
