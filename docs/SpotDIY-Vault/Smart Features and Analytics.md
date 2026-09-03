# Smart Features and Analytics

Plan 14 keeps history and recommendations local to the authoritative SpotDIY
database and playback services.

## History and sessions

`AnalyticsRecorder` measures monotonic playing time and qualifies a play after
30 seconds, or halfway through a validated track shorter than 60 seconds.
Paused time and recovery gaps do not count. A 30-minute inactivity gap starts
a new `listening_session`; outcomes distinguish completed, skipped, stopped,
and interrupted activity. History DTOs omit filesystem paths and provider raw
URLs.

## Privacy boundaries

Private Session is in-memory and writes no history or session rows. Temporary
Mode forces Private Session, checkpoints the durable queue, allows transient
queue changes, discards temporary activity, and restores the durable queue
without autoplay. Neither mode is persisted as a durable setting.

## Smart playlists and shuffle

`SmartPlaylistService` persists bounded typed rule trees and compiles only
allowlisted fields/operators with bound values. It supports CRUD and live
preview without accepting raw SQL or untrusted paths. Smart Shuffle is a
seeded deterministic non-ML heuristic over familiarity, variety, freshness,
and discovery signals, with recent-track and recent-artist anti-repetition
windows; the seed is not persisted.

## UI and verification

`/analytics` renders overview, top tracks/artists, weekly heatmap, Taste
Timeline, sessions, and Time Machine/reopen controls from typed local DTOs.
Playlists owns the smart-rule editor and Play Smart Mix action. Browser
preview returns empty analytics and rejects native smart actions. Frontend
gates pass; native/package/browser verification awaits the available Rust/SDK
and Chromium runtimes.
