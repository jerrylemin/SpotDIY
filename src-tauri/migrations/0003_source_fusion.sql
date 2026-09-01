CREATE TABLE user_track_overrides (
    provider_kind TEXT NOT NULL
        CHECK (provider_kind IN ('local', 'youtube', 'soundcloud')),
    provider_item_id TEXT NOT NULL
        CHECK (length(trim(provider_item_id)) > 0),
    target_track_id TEXT NOT NULL,
    decision TEXT NOT NULL
        CHECK (decision IN ('merge', 'split')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    PRIMARY KEY (
        provider_kind,
        provider_item_id,
        target_track_id
    ),

    FOREIGN KEY (target_track_id)
        REFERENCES tracks(id)
        ON DELETE CASCADE
);

CREATE UNIQUE INDEX ux_user_track_overrides_forced_merge
ON user_track_overrides(provider_kind, provider_item_id)
WHERE decision = 'merge';

CREATE INDEX idx_user_track_overrides_target_track_id
ON user_track_overrides(target_track_id);

UPDATE schema_metadata
SET metadata_value = '3', updated_at = '1970-01-01T00:00:00Z'
WHERE metadata_key = 'schema_version';
