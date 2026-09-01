CREATE TABLE downloads (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    provider_kind TEXT NOT NULL
        CHECK (provider_kind IN ('youtube', 'soundcloud')),
    provider_item_id TEXT NOT NULL CHECK (length(trim(provider_item_id)) > 0),
    canonical_url TEXT NOT NULL CHECK (length(trim(canonical_url)) > 0),
    target_track_id TEXT,
    target_source_id TEXT,
    title TEXT NOT NULL,
    artists_json TEXT NOT NULL CHECK (json_valid(artists_json)),
    artwork_url TEXT,
    mode TEXT NOT NULL CHECK (mode IN ('audio', 'video')),
    state TEXT NOT NULL CHECK (
        state IN ('queued', 'resolving', 'downloading', 'postprocessing', 'completed', 'failed', 'cancelled')
    ),
    destination_directory TEXT NOT NULL CHECK (length(trim(destination_directory)) > 0),
    output_path TEXT,
    output_extension TEXT,
    output_codec TEXT,
    source_quality_provenance TEXT NOT NULL CHECK (
        source_quality_provenance IN ('provider_encoded', 'unknown')
    ),
    transcoded INTEGER NOT NULL DEFAULT 0 CHECK (transcoded IN (0, 1)),
    expected_bytes INTEGER CHECK (expected_bytes IS NULL OR expected_bytes >= 0),
    downloaded_bytes INTEGER NOT NULL DEFAULT 0 CHECK (downloaded_bytes >= 0),
    progress_permille INTEGER NOT NULL DEFAULT 0 CHECK (progress_permille BETWEEN 0 AND 1000),
    speed_bytes_per_second INTEGER CHECK (speed_bytes_per_second IS NULL OR speed_bytes_per_second >= 0),
    eta_seconds INTEGER CHECK (eta_seconds IS NULL OR eta_seconds >= 0),
    retry_count INTEGER NOT NULL DEFAULT 0 CHECK (retry_count >= 0),
    error_code TEXT,
    error_detail TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    FOREIGN KEY (target_track_id) REFERENCES tracks(id) ON DELETE SET NULL,
    FOREIGN KEY (target_source_id) REFERENCES track_sources(id) ON DELETE SET NULL
);

CREATE INDEX idx_downloads_state_created_at
    ON downloads (state, created_at, id);

CREATE INDEX idx_downloads_target_track_id
    ON downloads (target_track_id);

CREATE TABLE download_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    max_concurrent INTEGER NOT NULL DEFAULT 2 CHECK (max_concurrent BETWEEN 1 AND 4)
);

INSERT INTO download_settings (id, max_concurrent)
VALUES (1, 2);

UPDATE track_sources
SET can_downloads = 1
WHERE provider_kind IN ('youtube', 'soundcloud');

UPDATE schema_metadata
SET metadata_value = '4', updated_at = '1970-01-01T00:00:00Z'
WHERE metadata_key = 'schema_version';
