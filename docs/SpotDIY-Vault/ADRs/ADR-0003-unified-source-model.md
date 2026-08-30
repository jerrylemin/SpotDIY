# ADR-0003: unified source model

Status: accepted

Provider results representing the same musical work merge into a `UnifiedTrack` with separate source records. Deterministic matching is conservative and user merge/split overrides are durable. Spotify catalog entries never become Spotify audio sources.
