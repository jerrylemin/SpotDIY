CREATE TABLE library_folders (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    path TEXT NOT NULL CHECK (length(trim(path)) > 0),
    normalized_path_key TEXT NOT NULL COLLATE NOCASE CHECK (length(trim(normalized_path_key)) > 0),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    scan_status TEXT NOT NULL DEFAULT 'idle' CHECK (scan_status IN ('idle', 'queued', 'scanning', 'complete', 'failed')),
    scan_generation INTEGER NOT NULL DEFAULT 0 CHECK (scan_generation >= 0),
    last_scan_started_at TEXT,
    last_scan_finished_at TEXT,
    last_scan_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (normalized_path_key)
);

ALTER TABLE local_files ADD COLUMN library_folder_id TEXT REFERENCES library_folders(id) ON DELETE SET NULL;
ALTER TABLE local_files ADD COLUMN normalized_path_key TEXT COLLATE NOCASE;
ALTER TABLE local_files ADD COLUMN container TEXT;
ALTER TABLE local_files ADD COLUMN index_status TEXT NOT NULL DEFAULT 'indexed'
    CHECK (index_status IN ('pending', 'indexed', 'missing', 'error'));
ALTER TABLE local_files ADD COLUMN status_detail TEXT;
ALTER TABLE local_files ADD COLUMN last_seen_at TEXT;
ALTER TABLE local_files ADD COLUMN last_indexed_at TEXT;
ALTER TABLE local_files ADD COLUMN last_seen_generation INTEGER NOT NULL DEFAULT 0
    CHECK (last_seen_generation >= 0);
ALTER TABLE local_files ADD COLUMN artwork_cache_key TEXT;
ALTER TABLE local_files ADD COLUMN artwork_mime_type TEXT;

UPDATE track_sources
SET provider_item_id = 'legacy-local-' || id
WHERE provider_kind = 'local'
  AND EXISTS (
      SELECT 1
      FROM local_files
      WHERE local_files.source_id = track_sources.id
        AND local_files.path = track_sources.provider_item_id
  );

CREATE UNIQUE INDEX ux_local_files_normalized_path_key
    ON local_files (normalized_path_key)
    WHERE normalized_path_key IS NOT NULL;

CREATE INDEX idx_local_files_content_fingerprint
    ON local_files (content_fingerprint)
    WHERE content_fingerprint IS NOT NULL;

CREATE INDEX idx_local_files_folder_generation
    ON local_files (library_folder_id, last_seen_generation);

CREATE INDEX idx_local_files_folder_page
    ON local_files (library_folder_id, index_status, normalized_path_key, source_id);

CREATE INDEX idx_local_files_page
    ON local_files (index_status, normalized_path_key, source_id);

INSERT INTO schema_metadata (metadata_key, metadata_value, updated_at)
VALUES ('library_schema_version', '1', '1970-01-01T00:00:00Z')
ON CONFLICT(metadata_key) DO UPDATE SET
    metadata_value = excluded.metadata_value,
    updated_at = excluded.updated_at;

UPDATE schema_metadata
SET metadata_value = '2', updated_at = '1970-01-01T00:00:00Z'
WHERE metadata_key = 'schema_version';
