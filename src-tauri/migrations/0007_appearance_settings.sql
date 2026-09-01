-- Plan 11: make Plan 10 appearance settings compatible with shipped schema-6 databases.
CREATE TABLE settings_metadata_v7 (
    setting_key TEXT PRIMARY KEY CHECK (
        setting_key IN (
            'theme',
            'downloads_directory',
            'source_preference_order',
            'first_run',
            'storage_mode',
            'layout_profile',
            'custom_theme'
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
            'custom_theme'
        )
    ),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    updated_at TEXT NOT NULL
);

INSERT INTO settings_metadata_v7 (
    setting_key,
    value_json,
    value_type,
    schema_version,
    updated_at
)
SELECT setting_key, value_json, value_type, schema_version, updated_at
FROM settings_metadata;

DROP TABLE settings_metadata;

ALTER TABLE settings_metadata_v7 RENAME TO settings_metadata;

UPDATE schema_metadata
SET metadata_value = '7',
    updated_at = '1970-01-01T00:00:00Z'
WHERE metadata_key = 'schema_version';
