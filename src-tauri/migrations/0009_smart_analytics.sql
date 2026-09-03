CREATE TABLE track_genres (
    track_id TEXT NOT NULL,
    genre TEXT NOT NULL CHECK (length(trim(genre)) > 0),
    normalized_genre TEXT NOT NULL COLLATE NOCASE CHECK (length(trim(normalized_genre)) > 0),
    PRIMARY KEY (track_id, normalized_genre),
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

CREATE INDEX idx_track_genres_genre
    ON track_genres(normalized_genre, track_id);

CREATE TABLE listening_sessions (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    label TEXT CHECK (label IS NULL OR length(trim(label)) BETWEEN 1 AND 80),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_listening_sessions_started_at
    ON listening_sessions(started_at DESC, id);

CREATE TABLE play_history (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    track_id TEXT,
    source_id TEXT,
    title_snapshot TEXT NOT NULL,
    artists_json TEXT NOT NULL CHECK (json_valid(artists_json)),
    album_snapshot TEXT,
    provider_kind TEXT,
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    local_date TEXT NOT NULL,
    local_hour INTEGER NOT NULL CHECK (local_hour BETWEEN 0 AND 23),
    local_weekday INTEGER NOT NULL CHECK (local_weekday BETWEEN 0 AND 6),
    listened_ms INTEGER NOT NULL CHECK (listened_ms >= 0),
    duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
    outcome TEXT NOT NULL CHECK (outcome IN ('completed', 'skipped', 'stopped', 'interrupted')),
    qualified_play INTEGER NOT NULL CHECK (qualified_play IN (0, 1)),
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES listening_sessions(id) ON DELETE SET NULL,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE SET NULL,
    FOREIGN KEY (source_id) REFERENCES track_sources(id) ON DELETE SET NULL
);

CREATE INDEX idx_play_history_started_at
    ON play_history(started_at, id);
CREATE INDEX idx_play_history_local_date
    ON play_history(local_date, started_at, id);
CREATE INDEX idx_play_history_track_id
    ON play_history(track_id, started_at, id);
CREATE INDEX idx_play_history_session_id
    ON play_history(session_id, started_at, id);
CREATE INDEX idx_play_history_qualified_play
    ON play_history(qualified_play, started_at, id);
CREATE INDEX idx_play_history_outcome
    ON play_history(outcome, started_at, id);

CREATE TABLE smart_playlists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
    normalized_name TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(normalized_name)) BETWEEN 1 AND 120),
    rule_json TEXT NOT NULL CHECK (json_valid(rule_json)),
    sort_mode TEXT NOT NULL,
    sort_direction TEXT NOT NULL,
    limit_count INTEGER CHECK (limit_count IS NULL OR limit_count BETWEEN 1 AND 5000),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_smart_playlists_updated_at
    ON smart_playlists(updated_at DESC, id);

UPDATE schema_metadata
SET metadata_value = '9', updated_at = '1970-01-01T00:00:00Z'
WHERE metadata_key = 'schema_version';
