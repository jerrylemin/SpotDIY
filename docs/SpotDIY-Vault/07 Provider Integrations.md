# Provider integrations

All providers implement a common adapter contract and report capabilities.
Local is the offline foundation. YouTube and SoundCloud provide bounded
metadata search through managed yt-dlp tooling; provider playback and downloads
remain later-plan behavior. Spotify is metadata-only, never a raw audio source,
and is isolated to its own search lens behind the Plan 05 PKCE development and
compliance gate.

See the dated reports in `Research/` for current API constraints and primary-source links.

## Plan 05 provider boundary

- Local queries the indexed SQLite library and returns managed typed IDs for
  local playback/file actions.
- YouTube and SoundCloud use the exact bounded metadata-only yt-dlp process
  contract, with no downloads, raw stderr, or subprocess paths crossing IPC.
- Spotify uses loopback Authorization Code with S256 PKCE on `127.0.0.1` and a
  dynamic port. No client secret is accepted. Tokens remain in the keyring or
  process memory, and the explicit gate keeps the provider disabled by default.
- Search results are transient. Provider payloads, tokens, credentials, and
  raw tool output are not stored in SQLite.
