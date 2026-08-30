# Playback

`PlaybackService` owns queue-aware state and talks to an isolated `PlaybackBackend`. mpv JSON IPC is the planned backend. Source switching preserves queue context, timestamp, lyrics, playlist context, shuffle, and repeat while clamping seeks safely for duration differences.
