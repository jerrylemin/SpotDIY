-- Plan 16: persist explicit Spotify catalog opt-in and native MPV selection.
CREATE TABLE settings_metadata_v10 (
    setting_key TEXT PRIMARY KEY CHECK (
        setting_key IN (
            'theme',
            'downloads_directory',
            'source_preference_order',
            'first_run',
            'storage_mode',
            'layout_profile',
            'custom_theme',
            'windows_integration',
            'global_shortcuts',
            'output_profiles',
            'spotify_catalog_enabled',
            'mpv_path'
        )
    ),
    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
    value_type TEXT NOT NULL CHECK (
        value_type IN (
            'theme',
            'downloads_directory',
            'source_preference_order',
            'boolean',
            'storage_mode',
            'layout_profile',
            'custom_theme',
            'windows_integration',
            'global_shortcuts',
            'output_profiles',
            'mpv_path'
        )
    ),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO settings_metadata_v10 (
    setting_key,
    value_json,
    value_type,
    schema_version,
    updated_at
)
SELECT setting_key, value_json, value_type, schema_version, updated_at
FROM settings_metadata;

INSERT OR IGNORE INTO settings_metadata_v10 (
    setting_key,
    value_json,
    value_type,
    schema_version,
    updated_at
)
VALUES ('spotify_catalog_enabled', 'false', 'boolean', 1, '1970-01-01T00:00:00Z');

DROP TABLE settings_metadata;

ALTER TABLE settings_metadata_v10 RENAME TO settings_metadata;

UPDATE schema_metadata
SET metadata_value = '10',
    updated_at = '1970-01-01T00:00:00Z'
WHERE metadata_key = 'schema_version';
