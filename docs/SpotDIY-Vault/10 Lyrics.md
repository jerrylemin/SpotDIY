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

LRCLIB is opt-in only: HTTPS-only, bounded, rate-gated, metadata-safe, and
cached only after explicit lookup/search/select actions. No full copyrighted
lyrics are committed to fixtures or logs, no automatic lookup runs, and no raw
provider response or credentials are retained. Waveform generation is outside
this delivery.
