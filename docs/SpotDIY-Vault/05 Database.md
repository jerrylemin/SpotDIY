# Database

SQLite with WAL mode and migrations is the runtime source of truth for user data. Planned logical tables cover tracks, artists, albums, sources, local files, playlists, queue state/snapshots, likes, ratings, tags, bookmarks, lyrics, downloads, history, sessions, smart playlists, preferences, caches, overrides, and schema metadata.

Secrets never belong in SQLite. Destructive migrations create a backup first; `.spotdiy` imports validate manifests/checksums and roll back transactionally.
