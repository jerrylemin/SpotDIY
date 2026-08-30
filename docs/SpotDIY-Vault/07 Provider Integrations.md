# Provider integrations

All providers implement a common adapter contract and report capabilities. YouTube and SoundCloud are search/metadata/playback/download candidates through managed tooling. Spotify is catalog metadata only and requires local Client Credentials when configured; it is never a raw audio source. Local is the offline foundation.

See the dated reports in `Research/` for current API constraints and primary-source links.
