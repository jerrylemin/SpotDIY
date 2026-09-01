-- Plan 09: local-first lyrics, track bookmarks, and A/B loop presets.
CREATE TABLE lyrics (
    id TEXT PRIMARY KEY,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('manual', 'lrclib')),
    plain_text TEXT,
    synced_lrc TEXT,
    instrumental INTEGER NOT NULL DEFAULT 0 CHECK (instrumental IN (0, 1)),
    provider_record_id INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(track_id, source_kind),
    CHECK (
        (source_kind = 'manual' AND provider_record_id IS NULL AND instrumental = 0
            AND (plain_text IS NOT NULL OR synced_lrc IS NOT NULL))
        OR
        (source_kind = 'lrclib' AND provider_record_id IS NOT NULL)
    ),
    CHECK (instrumental = 1 OR plain_text IS NOT NULL OR synced_lrc IS NOT NULL)
);

CREATE TABLE bookmarks (
    id TEXT PRIMARY KEY,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position_ms INTEGER NOT NULL CHECK (position_ms >= 0),
    note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX bookmarks_track_position_idx
    ON bookmarks(track_id, position_ms, id);

CREATE TABLE ab_loop_presets (
    id TEXT PRIMARY KEY,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    a_ms INTEGER NOT NULL CHECK (a_ms >= 0),
    b_ms INTEGER NOT NULL CHECK (b_ms > a_ms),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(track_id, normalized_name)
);

UPDATE schema_metadata
SET metadata_value = '6',
    updated_at = '1970-01-01T00:00:00Z'
WHERE metadata_key = 'schema_version';
