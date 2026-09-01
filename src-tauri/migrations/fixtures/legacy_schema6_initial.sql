-- Test fixture for a database shipped before Plan 10 appearance settings.
-- Keep this independent from migrations/0001_initial.sql so its old CHECK
-- constraint remains an explicit compatibility contract.
CREATE TABLE artists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    sort_name TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE albums (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    release_date TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE tracks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    normalized_title TEXT NOT NULL CHECK (length(trim(normalized_title)) > 0),
    album_id TEXT,
    duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
    version_qualifiers_json TEXT NOT NULL DEFAULT '["standard"]' CHECK (json_valid(version_qualifiers_json)),
    preferred_source_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (album_id) REFERENCES albums(id) ON DELETE SET NULL,
    FOREIGN KEY (preferred_source_id) REFERENCES track_sources(id) ON DELETE SET NULL
);

CREATE TABLE track_sources (
    id TEXT PRIMARY KEY,
    track_id TEXT NOT NULL,
    provider_kind TEXT NOT NULL CHECK (provider_kind IN ('local', 'youtube', 'soundcloud', 'spotify')),
    provider_item_id TEXT NOT NULL CHECK (length(trim(provider_item_id)) > 0),
    source_uri TEXT,
    duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
    version_qualifiers_json TEXT NOT NULL DEFAULT '["standard"]' CHECK (json_valid(version_qualifiers_json)),
    available INTEGER NOT NULL DEFAULT 1 CHECK (available IN (0, 1)),
    availability_detail TEXT,
    can_search INTEGER NOT NULL DEFAULT 0 CHECK (can_search IN (0, 1)),
    can_metadata INTEGER NOT NULL DEFAULT 0 CHECK (can_metadata IN (0, 1)),
    can_artwork INTEGER NOT NULL DEFAULT 0 CHECK (can_artwork IN (0, 1)),
    can_playback INTEGER NOT NULL DEFAULT 0 CHECK (can_playback IN (0, 1)),
    can_lyrics INTEGER NOT NULL DEFAULT 0 CHECK (can_lyrics IN (0, 1)),
    can_downloads INTEGER NOT NULL DEFAULT 0 CHECK (can_downloads IN (0, 1)),
    can_popularity INTEGER NOT NULL DEFAULT 0 CHECK (can_popularity IN (0, 1)),
    can_release_date INTEGER NOT NULL DEFAULT 0 CHECK (can_release_date IN (0, 1)),
    can_lyrics_metadata INTEGER NOT NULL DEFAULT 0 CHECK (can_lyrics_metadata IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    UNIQUE (provider_kind, provider_item_id),
    CHECK (
        provider_kind <> 'spotify'
        OR (
            can_playback = 0
            AND can_downloads = 0
            AND can_lyrics = 0
            AND can_lyrics_metadata = 0
        )
    )
);

CREATE TRIGGER tracks_preferred_source_same_track_insert
BEFORE INSERT ON tracks
WHEN NEW.preferred_source_id IS NOT NULL
 AND NOT EXISTS (
     SELECT 1 FROM track_sources
     WHERE id = NEW.preferred_source_id AND track_id = NEW.id
 )
BEGIN
    SELECT RAISE(ABORT, 'preferred source must belong to track');
END;

CREATE TRIGGER tracks_preferred_source_same_track_update
BEFORE UPDATE OF preferred_source_id ON tracks
WHEN NEW.preferred_source_id IS NOT NULL
 AND NOT EXISTS (
     SELECT 1 FROM track_sources
     WHERE id = NEW.preferred_source_id AND track_id = NEW.id
 )
BEGIN
    SELECT RAISE(ABORT, 'preferred source must belong to track');
END;

CREATE TRIGGER track_sources_track_id_preserves_preferred_source
BEFORE UPDATE OF track_id ON track_sources
WHEN EXISTS (
    SELECT 1 FROM tracks
    WHERE preferred_source_id = NEW.id AND id <> NEW.track_id
)
BEGIN
    SELECT RAISE(ABORT, 'source move would invalidate preferred source');
END;

CREATE TABLE track_artists (
    track_id TEXT NOT NULL,
    artist_id TEXT NOT NULL,
    artist_order INTEGER NOT NULL CHECK (artist_order >= 0),
    role TEXT NOT NULL DEFAULT 'primary' CHECK (length(trim(role)) > 0),
    PRIMARY KEY (track_id, artist_id),
    UNIQUE (track_id, artist_order),
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE CASCADE
);

CREATE TABLE local_files (
    source_id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE CHECK (length(trim(path)) > 0),
    file_size_bytes INTEGER CHECK (file_size_bytes IS NULL OR file_size_bytes >= 0),
    modified_at TEXT,
    content_fingerprint TEXT,
    codec TEXT,
    bitrate_kbps INTEGER CHECK (bitrate_kbps IS NULL OR bitrate_kbps >= 0),
    sample_rate_hz INTEGER CHECK (sample_rate_hz IS NULL OR sample_rate_hz >= 0),
    bit_depth INTEGER CHECK (bit_depth IS NULL OR bit_depth >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (source_id) REFERENCES track_sources(id) ON DELETE CASCADE
);

CREATE TABLE settings_metadata (
    setting_key TEXT PRIMARY KEY CHECK (
        setting_key IN ('theme', 'downloads_directory', 'source_preference_order', 'first_run', 'storage_mode')
    ),
    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
    value_type TEXT NOT NULL CHECK (
        value_type IN ('theme', 'downloads_directory', 'source_preference_order', 'boolean', 'storage_mode')
    ),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    updated_at TEXT NOT NULL
);

CREATE TABLE schema_metadata (
    metadata_key TEXT PRIMARY KEY,
    metadata_value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_tracks_normalized_title ON tracks (normalized_title);
CREATE INDEX idx_tracks_updated_at ON tracks (updated_at);
CREATE INDEX idx_track_sources_track_id ON track_sources (track_id);
CREATE INDEX idx_track_sources_provider_identity ON track_sources (provider_kind, provider_item_id);
CREATE INDEX idx_track_artists_artist_id ON track_artists (artist_id);

INSERT INTO settings_metadata (setting_key, value_json, value_type, schema_version, updated_at)
VALUES ('first_run', 'true', 'boolean', 1, '1970-01-01T00:00:00Z');

INSERT INTO settings_metadata (setting_key, value_json, value_type, schema_version, updated_at)
VALUES ('storage_mode', '"standard"', 'storage_mode', 1, '1970-01-01T00:00:00Z');

INSERT INTO schema_metadata (metadata_key, metadata_value, updated_at)
VALUES ('schema_version', '1', '1970-01-01T00:00:00Z');
