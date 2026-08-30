use std::collections::HashSet;
use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::{Database, DatabaseError};
use crate::domain::ProviderKind;

const SETTINGS_SCHEMA_VERSION: i64 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
    System,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    Standard,
    Portable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingClass {
    Ordinary,
    Secret,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretSettingKey {
    SpotifyClientSecret,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub theme: Theme,
    pub downloads_directory: Option<PathBuf>,
    pub source_preference_order: Vec<ProviderKind>,
    pub first_run: bool,
    pub storage_mode: StorageMode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "key", content = "value", rename_all = "camelCase")]
pub enum SettingValue {
    Theme(Theme),
    DownloadsDirectory(Option<PathBuf>),
    SourcePreferenceOrder(Vec<ProviderKind>),
}

impl SettingValue {
    pub const fn class(&self) -> SettingClass {
        SettingClass::Ordinary
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("settings database operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not serialize setting {key}: {source}")]
    Serialization {
        key: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not deserialize setting {key}: {source}")]
    Deserialization {
        key: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("setting {key} has invalid value: {reason}")]
    InvalidValue { key: &'static str, reason: String },
    #[error("setting {key} uses unsupported schema version {version}")]
    UnsupportedSchemaVersion { key: String, version: i64 },
    #[error("setting {key} has unexpected stored type {value_type}")]
    UnexpectedType {
        key: &'static str,
        value_type: String,
    },
    #[error("portable storage mode is not supported by the current standard startup path")]
    UnsupportedStorageMode,
}

pub struct SettingsRepository<'database> {
    database: &'database Database,
}

impl<'database> SettingsRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn get_snapshot(&self) -> Result<SettingsSnapshot, SettingsError> {
        let connection = self.database.connection()?;
        let mut snapshot = SettingsSnapshot::default();

        if let Some((value_json, value_type, schema_version)) = read_setting(&connection, "theme")?
        {
            ensure_record("theme", &value_type, "theme", schema_version)?;
            snapshot.theme = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "theme",
                    source,
                }
            })?;
        }
        if let Some((value_json, value_type, schema_version)) =
            read_setting(&connection, "downloads_directory")?
        {
            ensure_record(
                "downloads_directory",
                &value_type,
                "downloads_directory",
                schema_version,
            )?;
            snapshot.downloads_directory = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "downloads_directory",
                    source,
                }
            })?;
        }
        if let Some((value_json, value_type, schema_version)) =
            read_setting(&connection, "source_preference_order")?
        {
            ensure_record(
                "source_preference_order",
                &value_type,
                "source_preference_order",
                schema_version,
            )?;
            let order: Vec<ProviderKind> = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "source_preference_order",
                    source,
                }
            })?;
            validate_source_preference_order(&order)?;
            snapshot.source_preference_order = order;
        }
        if let Some((value_json, value_type, schema_version)) =
            read_setting(&connection, "first_run")?
        {
            ensure_record("first_run", &value_type, "boolean", schema_version)?;
            snapshot.first_run = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "first_run",
                    source,
                }
            })?;
        }
        if let Some((value_json, value_type, schema_version)) =
            read_setting(&connection, "storage_mode")?
        {
            ensure_record("storage_mode", &value_type, "storage_mode", schema_version)?;
            snapshot.storage_mode = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "storage_mode",
                    source,
                }
            })?;
        }

        if snapshot.storage_mode == StorageMode::Portable {
            return Err(SettingsError::UnsupportedStorageMode);
        }

        Ok(snapshot)
    }

    pub fn get_theme(&self) -> Result<Theme, SettingsError> {
        Ok(self.get_snapshot()?.theme)
    }

    pub fn get_downloads_directory(&self) -> Result<Option<PathBuf>, SettingsError> {
        Ok(self.get_snapshot()?.downloads_directory)
    }

    pub fn get_source_preference_order(&self) -> Result<Vec<ProviderKind>, SettingsError> {
        Ok(self.get_snapshot()?.source_preference_order)
    }

    pub fn set_setting(&self, setting: SettingValue) -> Result<SettingsSnapshot, SettingsError> {
        let (key, value_type, value_json) = encode_setting(&setting)?;
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO settings_metadata (setting_key, value_json, value_type, schema_version, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(setting_key) DO UPDATE SET
                 value_json = excluded.value_json,
                 value_type = excluded.value_type,
                 schema_version = excluded.schema_version,
                 updated_at = excluded.updated_at",
            params![
                key,
                value_json,
                value_type,
                SETTINGS_SCHEMA_VERSION,
                Utc::now().to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_snapshot()
    }

    pub fn mark_initialized(&self) -> Result<(), SettingsError> {
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO settings_metadata (setting_key, value_json, value_type, schema_version, updated_at)
             VALUES ('first_run', 'false', 'boolean', ?1, ?2)
             ON CONFLICT(setting_key) DO UPDATE SET
                 value_json = excluded.value_json,
                 value_type = excluded.value_type,
                 schema_version = excluded.schema_version,
                 updated_at = excluded.updated_at",
            params![SETTINGS_SCHEMA_VERSION, Utc::now().to_rfc3339()],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

impl Default for SettingsSnapshot {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            downloads_directory: None,
            source_preference_order: vec![
                ProviderKind::Local,
                ProviderKind::Soundcloud,
                ProviderKind::Youtube,
                ProviderKind::Spotify,
            ],
            first_run: true,
            storage_mode: StorageMode::Standard,
        }
    }
}

fn encode_setting(
    setting: &SettingValue,
) -> Result<(&'static str, &'static str, String), SettingsError> {
    match setting {
        SettingValue::Theme(value) => Ok((
            "theme",
            "theme",
            serde_json::to_string(value).map_err(|source| SettingsError::Serialization {
                key: "theme",
                source,
            })?,
        )),
        SettingValue::DownloadsDirectory(value) => Ok((
            "downloads_directory",
            "downloads_directory",
            serde_json::to_string(value).map_err(|source| SettingsError::Serialization {
                key: "downloads_directory",
                source,
            })?,
        )),
        SettingValue::SourcePreferenceOrder(value) => {
            validate_source_preference_order(value)?;
            Ok((
                "source_preference_order",
                "source_preference_order",
                serde_json::to_string(value).map_err(|source| SettingsError::Serialization {
                    key: "source_preference_order",
                    source,
                })?,
            ))
        }
    }
}

fn read_setting(
    connection: &Connection,
    key: &str,
) -> Result<Option<(String, String, i64)>, SettingsError> {
    connection
        .query_row(
            "SELECT value_json, value_type, schema_version FROM settings_metadata WHERE setting_key = ?1",
            params![key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(SettingsError::from)
}

fn ensure_record(
    key: &'static str,
    actual_type: &str,
    expected_type: &'static str,
    schema_version: i64,
) -> Result<(), SettingsError> {
    if actual_type != expected_type {
        return Err(SettingsError::UnexpectedType {
            key,
            value_type: actual_type.to_owned(),
        });
    }
    if schema_version != SETTINGS_SCHEMA_VERSION {
        return Err(SettingsError::UnsupportedSchemaVersion {
            key: key.to_owned(),
            version: schema_version,
        });
    }
    Ok(())
}

fn validate_source_preference_order(order: &[ProviderKind]) -> Result<(), SettingsError> {
    let unique: HashSet<_> = order.iter().copied().collect();
    let expected: HashSet<_> = ProviderKind::all().iter().copied().collect();
    if unique != expected || order.len() != expected.len() {
        return Err(SettingsError::InvalidValue {
            key: "source_preference_order",
            reason: "the order must contain each supported provider exactly once".to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, TempDatabasePath};

    #[test]
    fn defaults_are_available_without_speculative_rows() {
        let path = TempDatabasePath::new("settings-defaults");
        let database = Database::open(path.path()).unwrap();
        let repository = SettingsRepository::new(&database);

        let snapshot = repository.get_snapshot().unwrap();

        assert_eq!(snapshot.theme, Theme::Dark);
        assert_eq!(snapshot.downloads_directory, None);
        assert!(snapshot.first_run);
        assert_eq!(snapshot.storage_mode, StorageMode::Standard);
        assert_eq!(snapshot.source_preference_order.len(), 4);
    }

    #[test]
    fn setting_updates_persist_across_reopen() {
        let path = TempDatabasePath::new("settings-persist");
        {
            let database = Database::open(path.path()).unwrap();
            let repository = SettingsRepository::new(&database);
            repository
                .set_setting(SettingValue::Theme(Theme::Light))
                .unwrap();
            repository
                .set_setting(SettingValue::DownloadsDirectory(Some(PathBuf::from(
                    "C:\\SpotDIY\\Downloads",
                ))))
                .unwrap();
            repository.mark_initialized().unwrap();
        }

        let database = Database::open(path.path()).unwrap();
        let snapshot = SettingsRepository::new(&database).get_snapshot().unwrap();

        assert_eq!(snapshot.theme, Theme::Light);
        assert_eq!(
            snapshot.downloads_directory,
            Some(PathBuf::from("C:\\SpotDIY\\Downloads"))
        );
        assert!(!snapshot.first_run);
    }

    #[test]
    fn setting_overwrite_replaces_the_previous_typed_value() {
        let path = TempDatabasePath::new("settings-overwrite");
        let database = Database::open(path.path()).unwrap();
        let repository = SettingsRepository::new(&database);

        repository
            .set_setting(SettingValue::Theme(Theme::Light))
            .unwrap();
        repository
            .set_setting(SettingValue::Theme(Theme::System))
            .unwrap();

        assert_eq!(repository.get_theme().unwrap(), Theme::System);
    }

    #[test]
    fn invalid_serialized_setting_fails_safely() {
        let path = TempDatabasePath::new("settings-invalid");
        let database = Database::open(path.path()).unwrap();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO settings_metadata (setting_key, value_json, value_type, schema_version, updated_at)
                     VALUES ('theme', '\"neon\"', 'theme', 1, '2026-01-01T00:00:00Z')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        let result = SettingsRepository::new(&database).get_snapshot();

        assert!(matches!(
            result,
            Err(SettingsError::Deserialization { key: "theme", .. })
        ));
    }

    #[test]
    fn invalid_preference_order_is_rejected_before_persistence() {
        let path = TempDatabasePath::new("settings-invalid-order");
        let database = Database::open(path.path()).unwrap();
        let repository = SettingsRepository::new(&database);

        let result = repository.set_setting(SettingValue::SourcePreferenceOrder(vec![
            ProviderKind::Local,
            ProviderKind::Local,
        ]));

        assert!(matches!(result, Err(SettingsError::InvalidValue { .. })));
    }

    #[test]
    fn portable_mode_is_rejected_until_portable_startup_is_implemented() {
        let path = TempDatabasePath::new("settings-portable");
        let database = Database::open(path.path()).unwrap();
        database
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE settings_metadata SET value_json = '\"portable\"' WHERE setting_key = 'storage_mode'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        let result = SettingsRepository::new(&database).get_snapshot();

        assert!(matches!(result, Err(SettingsError::UnsupportedStorageMode)));
    }
}
