pub mod repository;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};
use thiserror::Error;

const INITIAL_MIGRATION_SQL: &str = include_str!("../../migrations/0001_initial.sql");
const LOCAL_LIBRARY_MIGRATION_SQL: &str = include_str!("../../migrations/0002_local_library.sql");
const SOURCE_FUSION_MIGRATION_SQL: &str = include_str!("../../migrations/0003_source_fusion.sql");
const DOWNLOADS_MIGRATION_SQL: &str = include_str!("../../migrations/0004_downloads.sql");
const COLLECTIONS_AND_QUEUE_MIGRATION_SQL: &str =
    include_str!("../../migrations/0005_collections_and_queue.sql");
const LYRICS_BOOKMARKS_MIGRATION_SQL: &str =
    include_str!("../../migrations/0006_lyrics_bookmarks.sql");
const APPEARANCE_SETTINGS_MIGRATION_SQL: &str =
    include_str!("../../migrations/0007_appearance_settings.sql");
const WINDOWS_INTEGRATION_SETTINGS_MIGRATION_SQL: &str =
    include_str!("../../migrations/0008_windows_integration_settings.sql");
const SMART_ANALYTICS_MIGRATION_SQL: &str =
    include_str!("../../migrations/0009_smart_analytics.sql");

pub const LATEST_SCHEMA_VERSION: u32 = 9;
pub const DATABASE_FILE_NAME: &str = "spotdiy.sqlite3";
pub const APPLICATION_DATA_DIRECTORY: &str = "SpotDIY";

pub fn standard_database_path(local_data_root: impl AsRef<Path>) -> PathBuf {
    local_data_root
        .as_ref()
        .join(APPLICATION_DATA_DIRECTORY)
        .join(DATABASE_FILE_NAME)
}

#[derive(Clone, Copy, Debug)]
struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
    destructive: bool,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "0001_initial",
        sql: INITIAL_MIGRATION_SQL,
        destructive: false,
    },
    Migration {
        version: 2,
        name: "0002_local_library",
        sql: LOCAL_LIBRARY_MIGRATION_SQL,
        destructive: false,
    },
    Migration {
        version: 3,
        name: "0003_source_fusion",
        sql: SOURCE_FUSION_MIGRATION_SQL,
        destructive: false,
    },
    Migration {
        version: 4,
        name: "0004_downloads",
        sql: DOWNLOADS_MIGRATION_SQL,
        destructive: false,
    },
    Migration {
        version: 5,
        name: "0005_collections_and_queue",
        sql: COLLECTIONS_AND_QUEUE_MIGRATION_SQL,
        destructive: false,
    },
    Migration {
        version: 6,
        name: "0006_lyrics_bookmarks",
        sql: LYRICS_BOOKMARKS_MIGRATION_SQL,
        destructive: false,
    },
    Migration {
        version: 7,
        name: "0007_appearance_settings",
        sql: APPEARANCE_SETTINGS_MIGRATION_SQL,
        destructive: true,
    },
    Migration {
        version: 8,
        name: "0008_windows_integration_settings",
        sql: WINDOWS_INTEGRATION_SETTINGS_MIGRATION_SQL,
        destructive: true,
    },
    Migration {
        version: 9,
        name: "0009_smart_analytics",
        sql: SMART_ANALYTICS_MIGRATION_SQL,
        destructive: false,
    },
];

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("could not create database directory {path}: {source}")]
    CreateParent {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not open database {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("could not configure SQLite pragma {pragma}: {source}")]
    Configure {
        pragma: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("could not query SQLite state {operation}: {source}")]
    StateQuery {
        operation: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("database connection lock is poisoned")]
    ConnectionLock,
    #[error("database migration {version} ({name}) failed: {source}")]
    Migration {
        version: u32,
        name: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("database schema version {found} is newer than supported version {supported}")]
    FutureSchema { found: u32, supported: u32 },
    #[error("database migrations are not strictly ordered at version {version}")]
    InvalidMigrationOrder { version: u32 },
    #[error("database foreign-key check failed after migration")]
    ForeignKeyCheck,
    #[error("could not checkpoint database WAL before migration: {source}")]
    Checkpoint {
        #[source]
        source: rusqlite::Error,
    },
    #[error("could not checkpoint database WAL before migration because readers are active (busy status {busy})")]
    CheckpointBusy { busy: i64 },
    #[error("could not create migration backup {path}: {source}")]
    Backup {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("database query failed: {0}")]
    Query(#[from] rusqlite::Error),
}

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
    path: PathBuf,
    fts5_available: bool,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| DatabaseError::CreateParent {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let mut connection = Connection::open(&path).map_err(|source| DatabaseError::Open {
            path: path.clone(),
            source,
        })?;
        configure_connection(&connection)?;
        run_migrations(&mut connection, Some(&path), MIGRATIONS)?;
        let fts5_available = probe_fts5(&connection);

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            path,
            fts5_available,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn fts5_available(&self) -> bool {
        self.fts5_available
    }

    pub fn schema_version(&self) -> Result<u32, DatabaseError> {
        let connection = self.connection()?;
        current_schema_version(&connection)
    }

    pub fn wal_enabled(&self) -> Result<bool, DatabaseError> {
        let connection = self.connection()?;
        journal_mode(&connection).map(|mode| mode.eq_ignore_ascii_case("wal"))
    }

    pub fn foreign_keys_enabled(&self) -> Result<bool, DatabaseError> {
        let connection = self.connection()?;
        foreign_keys_enabled(&connection)
    }

    pub fn online_backup_to(&self, destination: impl AsRef<Path>) -> Result<(), DatabaseError> {
        let destination = destination.as_ref().to_path_buf();
        if let Some(parent) = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| DatabaseError::CreateParent {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let source = self.connection()?;
        let mut target = Connection::open(&destination).map_err(|source| DatabaseError::Open {
            path: destination.clone(),
            source,
        })?;
        let backup =
            rusqlite::backup::Backup::new(&source, &mut target).map_err(DatabaseError::Query)?;
        backup
            .run_to_completion(128, Duration::from_millis(5), None)
            .map_err(DatabaseError::Query)
    }

    pub fn with_connection<T>(
        &self,
        action: impl FnOnce(&Connection) -> Result<T, rusqlite::Error>,
    ) -> Result<T, DatabaseError> {
        let connection = self.connection()?;
        action(&connection).map_err(DatabaseError::Query)
    }

    pub(crate) fn connection(&self) -> Result<MutexGuard<'_, Connection>, DatabaseError> {
        self.connection
            .lock()
            .map_err(|_| DatabaseError::ConnectionLock)
    }
}

fn configure_connection(connection: &Connection) -> Result<(), DatabaseError> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|source| DatabaseError::Configure {
            pragma: "busy_timeout",
            source,
        })?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|source| DatabaseError::Configure {
            pragma: "foreign_keys",
            source,
        })?;
    connection
        .execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL;")
        .map_err(|source| DatabaseError::Configure {
            pragma: "journal_mode/synchronous",
            source,
        })?;

    if !foreign_keys_enabled(connection)? {
        return Err(DatabaseError::Configure {
            pragma: "foreign_keys",
            source: rusqlite::Error::InvalidQuery,
        });
    }
    if !journal_mode(connection)?.eq_ignore_ascii_case("wal") {
        return Err(DatabaseError::Configure {
            pragma: "journal_mode",
            source: rusqlite::Error::InvalidQuery,
        });
    }
    Ok(())
}

fn current_schema_version(connection: &Connection) -> Result<u32, DatabaseError> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|source| DatabaseError::StateQuery {
            operation: "user_version",
            source,
        })?;
    u32::try_from(version).map_err(|_| DatabaseError::StateQuery {
        operation: "user_version range",
        source: rusqlite::Error::IntegralValueOutOfRange(0, version),
    })
}

fn journal_mode(connection: &Connection) -> Result<String, DatabaseError> {
    connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(|source| DatabaseError::StateQuery {
            operation: "journal_mode",
            source,
        })
}

fn foreign_keys_enabled(connection: &Connection) -> Result<bool, DatabaseError> {
    let enabled: i64 = connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(|source| DatabaseError::StateQuery {
            operation: "foreign_keys",
            source,
        })?;
    Ok(enabled == 1)
}

fn run_migrations(
    connection: &mut Connection,
    database_path: Option<&Path>,
    migrations: &[Migration],
) -> Result<u32, DatabaseError> {
    let latest = validate_migrations(migrations)?;
    let current = current_schema_version(connection)?;
    if current > latest {
        return Err(DatabaseError::FutureSchema {
            found: current,
            supported: latest,
        });
    }

    for migration in migrations {
        if migration.version <= current {
            continue;
        }

        if migration.destructive {
            if let Some(database_path) = database_path {
                create_wal_safe_backup(connection, database_path)?;
            }
        }

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| DatabaseError::Migration {
                version: migration.version,
                name: migration.name,
                source,
            })?;
        transaction
            .execute_batch(migration.sql)
            .map_err(|source| DatabaseError::Migration {
                version: migration.version,
                name: migration.name,
                source,
            })?;
        transaction
            .pragma_update(None, "user_version", migration.version)
            .map_err(|source| DatabaseError::Migration {
                version: migration.version,
                name: migration.name,
                source,
            })?;
        let foreign_keys_clean = foreign_key_check_is_clean(&transaction).map_err(|source| {
            DatabaseError::Migration {
                version: migration.version,
                name: migration.name,
                source,
            }
        })?;
        if !foreign_keys_clean {
            return Err(DatabaseError::Migration {
                version: migration.version,
                name: migration.name,
                source: rusqlite::Error::InvalidQuery,
            });
        }
        transaction
            .commit()
            .map_err(|source| DatabaseError::Migration {
                version: migration.version,
                name: migration.name,
                source,
            })?;
    }

    verify_foreign_keys(connection)?;
    current_schema_version(connection)
}

fn validate_migrations(migrations: &[Migration]) -> Result<u32, DatabaseError> {
    let mut previous_version = 0;
    for migration in migrations {
        if migration.version <= previous_version {
            return Err(DatabaseError::InvalidMigrationOrder {
                version: migration.version,
            });
        }
        previous_version = migration.version;
    }
    Ok(previous_version)
}

fn verify_foreign_keys(connection: &Connection) -> Result<(), DatabaseError> {
    let is_clean =
        foreign_key_check_is_clean(connection).map_err(|source| DatabaseError::StateQuery {
            operation: "foreign_key_check",
            source,
        })?;
    if !is_clean {
        return Err(DatabaseError::ForeignKeyCheck);
    }
    Ok(())
}

fn foreign_key_check_is_clean(connection: &Connection) -> Result<bool, rusqlite::Error> {
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    let mut rows = statement.query([])?;
    Ok(rows.next()?.is_none())
}

fn probe_fts5(connection: &Connection) -> bool {
    connection
        .execute_batch(
            "CREATE VIRTUAL TABLE temp.spotdiy_fts5_probe USING fts5(content); \
             DROP TABLE temp.spotdiy_fts5_probe;",
        )
        .is_ok()
}

fn create_wal_safe_backup(
    connection: &Connection,
    database_path: &Path,
) -> Result<PathBuf, DatabaseError> {
    let (busy, _log_frames, _checkpointed_frames): (i64, i64, i64) = connection
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|source| DatabaseError::Checkpoint { source })?;
    if busy != 0 {
        return Err(DatabaseError::CheckpointBusy { busy });
    }

    let file_name = database_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("spotdiy.sqlite3");
    let mut backup_path = database_path.with_file_name(format!("{file_name}.pre-migration"));
    let mut suffix = 1_u32;
    while backup_path.exists() {
        backup_path = database_path.with_file_name(format!("{file_name}.pre-migration.{suffix}"));
        suffix = suffix.saturating_add(1);
    }

    fs::copy(database_path, &backup_path).map_err(|source| DatabaseError::Backup {
        path: backup_path.clone(),
        source,
    })?;
    Ok(backup_path)
}

#[cfg(test)]
pub(crate) struct TempDatabasePath {
    path: PathBuf,
}

#[cfg(test)]
impl TempDatabasePath {
    pub(crate) fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("spotdiy-{label}-{}.sqlite3", uuid::Uuid::new_v4()));
        Self { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
impl Drop for TempDatabasePath {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let target = if suffix.is_empty() {
                self.path.clone()
            } else {
                PathBuf::from(format!("{}{}", self.path.display(), suffix))
            };
            let _ = fs::remove_file(target);
        }
        let prefix = format!("{}.pre-migration", self.path.display());
        if let Some(parent) = self.path.parent() {
            if let Ok(entries) = fs::read_dir(parent) {
                for entry in entries.flatten() {
                    let candidate = entry.path().display().to_string();
                    if candidate.starts_with(&prefix) {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{
        LayoutProfile, SettingValue, SettingsRepository, SpotThemeDefinition, SpotThemeTokens,
        Theme, ThemeBaseMode,
    };
    use rusqlite::params;

    const LEGACY_SCHEMA_6_INITIAL_SQL: &str =
        include_str!("../../migrations/fixtures/legacy_schema6_initial.sql");

    fn open_legacy_schema_six_fixture(label: &str) -> (TempDatabasePath, Connection) {
        let path = TempDatabasePath::new(label);
        let mut connection = Connection::open(path.path()).unwrap();
        configure_connection(&connection).unwrap();
        connection
            .execute_batch(LEGACY_SCHEMA_6_INITIAL_SQL)
            .unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        run_migrations(&mut connection, None, &MIGRATIONS[1..6]).unwrap();
        assert_eq!(current_schema_version(&connection).unwrap(), 6);
        (path, connection)
    }

    fn replace_settings_with_plan10_shape(connection: &Connection) {
        connection
            .execute_batch(
                "CREATE TABLE settings_metadata_plan10 (
                    setting_key TEXT PRIMARY KEY CHECK (
                        setting_key IN ('theme', 'downloads_directory', 'source_preference_order', 'first_run', 'storage_mode', 'layout_profile', 'custom_theme')
                    ),
                    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
                    value_type TEXT NOT NULL CHECK (
                        value_type IN ('theme', 'downloads_directory', 'source_preference_order', 'boolean', 'storage_mode', 'layout_profile', 'custom_theme')
                    ),
                    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
                    updated_at TEXT NOT NULL
                );
                INSERT INTO settings_metadata_plan10
                    (setting_key, value_json, value_type, schema_version, updated_at)
                SELECT setting_key, value_json, value_type, schema_version, updated_at
                FROM settings_metadata;
                DROP TABLE settings_metadata;
                ALTER TABLE settings_metadata_plan10 RENAME TO settings_metadata;",
            )
            .unwrap();
    }

    fn fixture_theme() -> SpotThemeDefinition {
        SpotThemeDefinition {
            schema_version: 1,
            name: "Migration theme".to_owned(),
            base_mode: ThemeBaseMode::Dark,
            tokens: SpotThemeTokens {
                background: "#101113".to_owned(),
                surface: "#17181D".to_owned(),
                surface_raised: "#1D1E24".to_owned(),
                surface_soft: "#22232A".to_owned(),
                text: "#F3F1EC".to_owned(),
                text_muted: "#A8A7AE".to_owned(),
                text_subtle: "#807F87".to_owned(),
                border: "#2E2F36".to_owned(),
                border_strong: "#4B4C55".to_owned(),
                accent: "#D7FF60".to_owned(),
                accent_contrast: "#151617".to_owned(),
                success: "#81E2D0".to_owned(),
                warning: "#FFB570".to_owned(),
                danger: "#FF806F".to_owned(),
                info: "#8E7BFF".to_owned(),
            },
        }
    }

    #[test]
    fn online_backup_copies_an_open_wal_database() {
        let source_path = TempDatabasePath::new("online-backup-source");
        let target_path = TempDatabasePath::new("online-backup-target");
        let source = Database::open(source_path.path()).unwrap();
        SettingsRepository::new(&source)
            .set_setting(SettingValue::Theme(Theme::Light))
            .unwrap();
        source
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE settings_metadata
                     SET value_json = '\"backup-marker\"'
                     WHERE setting_key = 'theme'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        source.online_backup_to(target_path.path()).unwrap();

        let target = Database::open(target_path.path()).unwrap();
        let theme: String = target
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT value_json FROM settings_metadata WHERE setting_key = 'theme'",
                    [],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(theme, "\"backup-marker\"");
        assert_eq!(target.schema_version().unwrap(), LATEST_SCHEMA_VERSION);
    }

    #[test]
    fn clean_database_reaches_latest_schema_with_core_tables() {
        let path = TempDatabasePath::new("clean");
        let database = Database::open(path.path()).expect("database should initialize");

        assert_eq!(database.schema_version().unwrap(), LATEST_SCHEMA_VERSION);
        let metadata_version: String = database
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT metadata_value FROM schema_metadata WHERE metadata_key = 'schema_version'",
                    [],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(metadata_version, LATEST_SCHEMA_VERSION.to_string());
        for table in [
            "tracks",
            "artists",
            "track_artists",
            "albums",
            "track_sources",
            "local_files",
            "settings_metadata",
            "schema_metadata",
            "user_track_overrides",
            "downloads",
            "download_settings",
            "playlists",
            "playlist_items",
            "playlist_branch_base_items",
            "likes",
            "ratings",
            "tags",
            "track_tags",
            "queue_state",
            "queue_entries",
            "queue_snapshots",
            "queue_snapshot_entries",
            "lyrics",
            "bookmarks",
            "ab_loop_presets",
            "track_genres",
            "listening_sessions",
            "play_history",
            "smart_playlists",
        ] {
            let exists: i64 = database
                .with_connection(|connection| {
                    connection.query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                        params![table],
                        |row| row.get(0),
                    )
                })
                .unwrap();
            assert_eq!(exists, 1, "missing table {table}");
        }

        let columns: Vec<String> = database
            .with_connection(|connection| {
                let mut statement = connection.prepare("PRAGMA table_info(library_folders)")?;
                let values = statement
                    .query_map([], |row| row.get(1))?
                    .collect::<Result<Vec<_>, _>>();
                values
            })
            .unwrap();
        assert!(columns.contains(&"scan_generation".to_owned()));
        assert!(columns.contains(&"last_scan_finished_at".to_owned()));
        assert!(columns.contains(&"last_scan_error".to_owned()));

        let rejected = database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO library_folders (
                    id, path, normalized_path_key, created_at, updated_at
                 ) VALUES (NULL, 'C:\\\\Music', 'c:\\\\music', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
        });
        assert!(rejected.is_err(), "folder IDs must not accept NULL");
    }

    #[test]
    fn legacy_schema_six_old_settings_constraint_migrates_to_seven_without_loss() {
        let (path, mut connection) = open_legacy_schema_six_fixture("migration-six-old-settings");
        let now = "2026-01-01T00:00:00Z";

        connection
            .execute(
                "UPDATE settings_metadata
                 SET value_json = CASE setting_key
                     WHEN 'first_run' THEN 'false'
                     WHEN 'storage_mode' THEN '\"standard\"'
                     ELSE value_json
                 END,
                 updated_at = ?1",
                params![now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO settings_metadata
                    (setting_key, value_json, value_type, schema_version, updated_at)
                 VALUES ('theme', '\"light\"', 'theme', 1, ?1)
                 ON CONFLICT(setting_key) DO UPDATE SET
                    value_json = excluded.value_json,
                    updated_at = excluded.updated_at",
                params![now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO settings_metadata
                    (setting_key, value_json, value_type, schema_version, updated_at)
                 VALUES ('downloads_directory', '\"C:\\\\Downloads\"', 'downloads_directory', 1, ?1)",
                params![now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO settings_metadata
                    (setting_key, value_json, value_type, schema_version, updated_at)
                 VALUES ('source_preference_order',
                    '[\"youtube\",\"soundcloud\",\"local\",\"spotify\"]',
                    'source_preference_order', 1, ?1)",
                params![now],
            )
            .unwrap();

        let rejected = connection.execute(
            "INSERT INTO settings_metadata
                (setting_key, value_json, value_type, schema_version, updated_at)
             VALUES ('layout_profile', '\"dense\"', 'layout_profile', 1, ?1)",
            params![now],
        );
        assert!(
            rejected.is_err(),
            "the legacy schema must reject Plan 10 keys"
        );

        run_migrations(&mut connection, None, MIGRATIONS).unwrap();
        assert_eq!(
            current_schema_version(&connection).unwrap(),
            LATEST_SCHEMA_VERSION
        );
        let old_values: Vec<(String, String, String)> = {
            let mut statement = connection
                .prepare(
                    "SELECT setting_key, value_json, value_type
                     FROM settings_metadata
                     WHERE setting_key IN ('theme', 'downloads_directory', 'source_preference_order', 'first_run', 'storage_mode')
                     ORDER BY setting_key",
                )
                .unwrap();
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(old_values.len(), 5);
        assert_eq!(
            old_values[0],
            (
                "downloads_directory".to_owned(),
                "\"C:\\\\Downloads\"".to_owned(),
                "downloads_directory".to_owned()
            )
        );
        assert_eq!(
            old_values[1],
            (
                "first_run".to_owned(),
                "false".to_owned(),
                "boolean".to_owned()
            )
        );
        assert_eq!(
            old_values[2],
            (
                "source_preference_order".to_owned(),
                "[\"youtube\",\"soundcloud\",\"local\",\"spotify\"]".to_owned(),
                "source_preference_order".to_owned()
            )
        );
        assert_eq!(
            old_values[3],
            (
                "storage_mode".to_owned(),
                "\"standard\"".to_owned(),
                "storage_mode".to_owned()
            )
        );
        assert_eq!(
            old_values[4],
            (
                "theme".to_owned(),
                "\"light\"".to_owned(),
                "theme".to_owned()
            )
        );
        drop(connection);

        let database = Database::open(path.path()).unwrap();
        let repository = SettingsRepository::new(&database);
        let snapshot = repository.get_snapshot().unwrap();
        assert_eq!(snapshot.theme, Theme::Light);
        assert_eq!(
            snapshot.downloads_directory,
            Some(PathBuf::from(r"C:\Downloads"))
        );
        assert!(!snapshot.first_run);
        assert_eq!(
            snapshot.storage_mode,
            crate::settings::StorageMode::Standard
        );
        assert_eq!(snapshot.source_preference_order.len(), 4);

        repository
            .set_setting(SettingValue::LayoutProfile(LayoutProfile::Dense))
            .unwrap();
        repository
            .set_setting(SettingValue::CustomTheme(Box::new(Some(fixture_theme()))))
            .unwrap();
        let active = repository
            .set_setting(SettingValue::Theme(Theme::Custom))
            .unwrap();
        assert_eq!(active.layout_profile, LayoutProfile::Dense);
        assert_eq!(active.theme, Theme::Custom);
        assert!(active.custom_theme.is_some());
        assert_eq!(database.schema_version().unwrap(), LATEST_SCHEMA_VERSION);
        let foreign_key_rows: i64 = database
            .with_connection(|connection| {
                connection.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get(0)
                })
            })
            .unwrap();
        assert_eq!(foreign_key_rows, 0);
    }

    #[test]
    fn plan10_schema_six_settings_shape_migrates_to_seven_and_preserves_appearance() {
        let (path, mut connection) =
            open_legacy_schema_six_fixture("migration-six-plan10-settings");
        replace_settings_with_plan10_shape(&connection);
        let theme_json = serde_json::to_string(&fixture_theme()).unwrap();
        connection
            .execute(
                "INSERT INTO settings_metadata
                    (setting_key, value_json, value_type, schema_version, updated_at)
                 VALUES ('layout_profile', '\"compact\"', 'layout_profile', 1, '2026-01-01T00:00:00Z'),
                        ('custom_theme', ?1, 'custom_theme', 1, '2026-01-01T00:00:00Z')",
                params![theme_json],
            )
            .unwrap();

        run_migrations(&mut connection, None, MIGRATIONS).unwrap();
        assert_eq!(
            current_schema_version(&connection).unwrap(),
            LATEST_SCHEMA_VERSION
        );
        let (layout, custom_theme): (String, String) = connection
            .query_row(
                "SELECT
                    (SELECT value_json FROM settings_metadata WHERE setting_key = 'layout_profile'),
                    (SELECT value_json FROM settings_metadata WHERE setting_key = 'custom_theme')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(layout, "\"compact\"");
        assert_eq!(custom_theme, theme_json);
        drop(connection);

        let database = Database::open(path.path()).unwrap();
        let repository = SettingsRepository::new(&database);
        let snapshot = repository.get_snapshot().unwrap();
        assert_eq!(snapshot.layout_profile, LayoutProfile::Compact);
        assert_eq!(snapshot.custom_theme, Some(fixture_theme()));
        repository
            .set_setting(SettingValue::Theme(Theme::Custom))
            .unwrap();
        assert_eq!(repository.get_theme().unwrap(), Theme::Custom);
        let foreign_key_rows: i64 = database
            .with_connection(|connection| {
                connection.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get(0)
                })
            })
            .unwrap();
        assert_eq!(foreign_key_rows, 0);
    }

    #[test]
    fn schema_seven_settings_rows_are_copied_unchanged_through_schema_nine() {
        let (_path, mut connection) =
            open_legacy_schema_six_fixture("migration-seven-to-eight-settings");
        replace_settings_with_plan10_shape(&connection);
        let theme_json = serde_json::to_string(&fixture_theme()).unwrap();
        connection
            .execute(
                "INSERT INTO settings_metadata
                    (setting_key, value_json, value_type, schema_version, updated_at)
                 VALUES ('layout_profile', '  \"dense\"  ', 'layout_profile', 1, '2026-02-01T00:00:00Z'),
                        ('custom_theme', ?1, 'custom_theme', 1, '2026-02-01T00:00:00Z')",
                params![theme_json],
            )
            .unwrap();
        run_migrations(&mut connection, None, &MIGRATIONS[6..7]).unwrap();
        assert_eq!(current_schema_version(&connection).unwrap(), 7);

        let before: Vec<(String, String, String, i64, String)> = {
            let mut statement = connection
                .prepare(
                    "SELECT setting_key, value_json, value_type, schema_version, updated_at
                     FROM settings_metadata ORDER BY setting_key",
                )
                .unwrap();
            statement
                .query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert!(connection
            .execute(
                "INSERT INTO settings_metadata
                    (setting_key, value_json, value_type, schema_version, updated_at)
                 VALUES ('windows_integration', '{}', 'windows_integration', 1, '2026-02-01T00:00:00Z')",
                [],
            )
            .is_err());

        run_migrations(&mut connection, None, &MIGRATIONS[7..]).unwrap();
        assert_eq!(current_schema_version(&connection).unwrap(), 9);
        let after: Vec<(String, String, String, i64, String)> = {
            let mut statement = connection
                .prepare(
                    "SELECT setting_key, value_json, value_type, schema_version, updated_at
                     FROM settings_metadata WHERE setting_key IN
                       ('theme', 'downloads_directory', 'source_preference_order', 'first_run',
                        'storage_mode', 'layout_profile', 'custom_theme')
                     ORDER BY setting_key",
                )
                .unwrap();
            statement
                .query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(after, before);
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get::<_, i64>(0)
                },)
                .unwrap(),
            0
        );
    }

    #[test]
    fn schema_eight_database_migrates_to_nine_with_smart_analytics_tables() {
        let (_path, mut connection) =
            open_legacy_schema_six_fixture("migration-eight-to-nine-smart-analytics");
        replace_settings_with_plan10_shape(&connection);

        run_migrations(&mut connection, None, &MIGRATIONS[6..8]).unwrap();
        assert_eq!(current_schema_version(&connection).unwrap(), 8);

        for table in [
            "track_genres",
            "listening_sessions",
            "play_history",
            "smart_playlists",
        ] {
            let exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 0, "schema eight unexpectedly contains {table}");
        }

        run_migrations(&mut connection, None, &MIGRATIONS[8..]).unwrap();
        assert_eq!(current_schema_version(&connection).unwrap(), 9);
        for table in [
            "track_genres",
            "listening_sessions",
            "play_history",
            "smart_playlists",
        ] {
            let exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "migration nine is missing {table}");
        }
    }

    #[test]
    fn migration_two_preserves_plan_two_rows_and_rewrites_path_identity() {
        let path = TempDatabasePath::new("migration-two-legacy");
        let mut connection = Connection::open(path.path()).unwrap();
        configure_connection(&connection).unwrap();
        run_migrations(&mut connection, None, &MIGRATIONS[..1]).unwrap();

        let track_id = uuid::Uuid::new_v4().to_string();
        let source_id = uuid::Uuid::new_v4().to_string();
        let legacy_path = r"C:\Music\legacy.flac";
        connection
            .execute(
                "INSERT INTO tracks (id, title, normalized_title, created_at, updated_at)
                 VALUES (?1, 'Legacy', 'legacy', ?2, ?2)",
                params![track_id, "2026-01-01T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO track_sources (
                    id, track_id, provider_kind, provider_item_id, created_at, updated_at
                 ) VALUES (?1, ?2, 'local', ?3, ?4, ?4)",
                params![source_id, track_id, legacy_path, "2026-01-01T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO local_files (source_id, path, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)",
                params![source_id, legacy_path, "2026-01-01T00:00:00Z"],
            )
            .unwrap();

        run_migrations(&mut connection, None, MIGRATIONS).unwrap();

        let provider_item_id: String = connection
            .query_row(
                "SELECT provider_item_id FROM track_sources WHERE id = ?1",
                params![source_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(provider_item_id, legacy_path);
        assert!(provider_item_id.starts_with("legacy-local-"));

        let local_file_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM local_files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(local_file_count, 1);
        let schema_version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(schema_version, LATEST_SCHEMA_VERSION as i64);
    }

    #[test]
    fn schema_two_fixture_upgrades_to_seven_without_losing_tracks_sources_or_settings() {
        let path = TempDatabasePath::new("migration-five-fixture");
        let mut connection = Connection::open(path.path()).unwrap();
        configure_connection(&connection).unwrap();
        run_migrations(&mut connection, None, &MIGRATIONS[..2]).unwrap();

        let track_id = uuid::Uuid::new_v4().to_string();
        let source_id = uuid::Uuid::new_v4().to_string();
        connection
            .execute(
                "INSERT INTO tracks (
                    id, title, normalized_title, duration_ms, version_qualifiers_json,
                    created_at, updated_at
                 ) VALUES (?1, 'Existing Track', 'existing track', 180000, '[\"standard\"]', ?2, ?2)",
                params![track_id, "2026-01-01T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO track_sources (
                    id, track_id, provider_kind, provider_item_id, duration_ms,
                    version_qualifiers_json, created_at, updated_at
                 ) VALUES (?1, ?2, 'soundcloud', 'existing-source', 180000,
                           '[\"standard\"]', ?3, ?3)",
                params![source_id, track_id, "2026-01-01T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO settings_metadata (
                    setting_key, value_json, value_type, schema_version, updated_at
                 ) VALUES ('theme', '\"light\"', 'theme', 1, '2026-01-01T00:00:00Z')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO settings_metadata (
                    setting_key, value_json, value_type, schema_version, updated_at
                 ) VALUES (
                    'source_preference_order',
                    '[\"soundcloud\",\"youtube\",\"local\",\"spotify\"]',
                    'source_preference_order', 1, '2026-01-01T00:00:00Z'
                 )",
                [],
            )
            .unwrap();

        run_migrations(&mut connection, None, MIGRATIONS).unwrap();

        let schema_version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(schema_version, LATEST_SCHEMA_VERSION as i64);
        let track_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM tracks WHERE id = ?1",
                params![track_id],
                |row| row.get(0),
            )
            .unwrap();
        let source_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM track_sources WHERE id = ?1 AND provider_item_id = 'existing-source'",
                params![source_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(track_count, 1);
        assert_eq!(source_count, 1);
        let theme: String = connection
            .query_row(
                "SELECT value_json FROM settings_metadata WHERE setting_key = 'theme'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let preference_order: String = connection
            .query_row(
                "SELECT value_json FROM settings_metadata WHERE setting_key = 'source_preference_order'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(theme, "\"light\"");
        assert_eq!(
            preference_order,
            "[\"soundcloud\",\"youtube\",\"local\",\"spotify\"]"
        );
        let (can_downloads, can_playback): (i64, i64) = connection
            .query_row(
                "SELECT can_downloads, can_playback FROM track_sources WHERE id = ?1",
                params![source_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(can_downloads, 1);
        assert_eq!(can_playback, 0);
        let foreign_key_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(foreign_key_rows, 0);
    }

    #[test]
    fn schema_four_fixture_upgrades_to_seven_preserving_plan_seven_data() {
        let path = TempDatabasePath::new("migration-four-to-five");
        let mut connection = Connection::open(path.path()).unwrap();
        configure_connection(&connection).unwrap();
        run_migrations(&mut connection, None, &MIGRATIONS[..4]).unwrap();

        let track_id = uuid::Uuid::new_v4().to_string();
        let source_id = uuid::Uuid::new_v4().to_string();
        let folder_id = uuid::Uuid::new_v4().to_string();
        connection
            .execute(
                "INSERT INTO tracks (
                    id, title, normalized_title, version_qualifiers_json,
                    created_at, updated_at
                 ) VALUES (?1, 'Migration Track', 'migration track', '[\"standard\"]', ?2, ?2)",
                params![track_id, "2026-01-01T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO track_sources (
                    id, track_id, provider_kind, provider_item_id, can_downloads,
                    created_at, updated_at
                 ) VALUES (?1, ?2, 'youtube', 'migration-source', 1, ?3, ?3)",
                params![source_id, track_id, "2026-01-01T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO user_track_overrides (
                    provider_kind, provider_item_id, target_track_id, decision,
                    created_at, updated_at
                 ) VALUES ('youtube', 'migration-source', ?1, 'merge', ?2, ?2)",
                params![track_id, "2026-01-01T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO downloads (
                    id, provider_kind, provider_item_id, canonical_url,
                    target_track_id, target_source_id, title, artists_json, mode,
                    state, destination_directory, source_quality_provenance,
                    created_at, updated_at
                 ) VALUES ('migration-completed', 'youtube', 'migration-source',
                           'https://youtube.com/watch?v=migration', ?1, ?2,
                           'Migration Track', '[]', 'audio', 'completed',
                           'C:\\Downloads', 'provider_encoded', ?3, ?3)",
                params![track_id, source_id, "2026-01-01T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO downloads (
                    id, provider_kind, provider_item_id, canonical_url,
                    title, artists_json, mode, state, destination_directory,
                    source_quality_provenance, created_at, updated_at
                 ) VALUES ('migration-failed', 'soundcloud', 'failed-source',
                           'https://soundcloud.com/migration/failed',
                           'Failed Track', '[]', 'audio', 'failed',
                           'C:\\Downloads', 'unknown', ?1, ?1)",
                params!["2026-01-01T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO library_folders (
                    id, path, normalized_path_key, created_at, updated_at
                 ) VALUES (?1, 'C:\\Music', 'c:\\music', ?2, ?2)",
                params![folder_id, "2026-01-01T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO settings_metadata (
                    setting_key, value_json, value_type, schema_version, updated_at
                 ) VALUES ('theme', '\"light\"', 'theme', 1, '2026-01-01T00:00:00Z')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE download_settings SET max_concurrent = 3 WHERE id = 1",
                [],
            )
            .unwrap();

        run_migrations(&mut connection, None, MIGRATIONS).unwrap();

        let schema_version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(schema_version, LATEST_SCHEMA_VERSION as i64);
        let track_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM tracks WHERE id = ?1",
                params![track_id],
                |row| row.get(0),
            )
            .unwrap();
        let override_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM user_track_overrides WHERE target_track_id = ?1",
                params![track_id],
                |row| row.get(0),
            )
            .unwrap();
        let (completed_count, failed_count): (i64, i64) = connection
            .query_row(
                "SELECT
                    SUM(state = 'completed'), SUM(state = 'failed')
                 FROM downloads",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let (max_concurrent, theme): (i64, String) = connection
            .query_row(
                "SELECT
                    (SELECT max_concurrent FROM download_settings WHERE id = 1),
                    (SELECT value_json FROM settings_metadata WHERE setting_key = 'theme')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let (inbox_count, queue_state_count, folder_count): (i64, i64, i64) = connection
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM playlists WHERE kind = 'inbox'),
                    (SELECT COUNT(*) FROM queue_state WHERE id = 1),
                    (SELECT COUNT(*) FROM library_folders WHERE id = ?1)",
                params![folder_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let foreign_key_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(track_count, 1);
        assert_eq!(override_count, 1);
        assert_eq!(completed_count, 1);
        assert_eq!(failed_count, 1);
        assert_eq!(max_concurrent, 3);
        assert_eq!(theme, "\"light\"");
        assert_eq!(inbox_count, 1);
        assert_eq!(queue_state_count, 1);
        assert_eq!(folder_count, 1);
        assert_eq!(foreign_key_rows, 0);
    }

    #[test]
    fn schema_five_fixture_upgrades_to_seven_preserving_plan_eight_collections_and_queue() {
        let path = TempDatabasePath::new("migration-five-to-six");
        let mut connection = Connection::open(path.path()).unwrap();
        configure_connection(&connection).unwrap();
        run_migrations(&mut connection, None, &MIGRATIONS[..5]).unwrap();

        let now = "2026-01-01T00:00:00Z";
        let track_id = uuid::Uuid::new_v4().to_string();
        let source_id = uuid::Uuid::new_v4().to_string();
        let playlist_id = uuid::Uuid::new_v4().to_string();
        let branch_id = uuid::Uuid::new_v4().to_string();
        let playlist_item_id = uuid::Uuid::new_v4().to_string();
        let queue_entry_id = uuid::Uuid::new_v4().to_string();
        let snapshot_id = uuid::Uuid::new_v4().to_string();
        let snapshot_entry_id = uuid::Uuid::new_v4().to_string();

        connection
            .execute(
                "INSERT INTO tracks (id, title, normalized_title, version_qualifiers_json, created_at, updated_at)
                 VALUES (?1, 'Migration Track', 'migration track', '[\"standard\"]', ?2, ?2)",
                params![track_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO track_sources (id, track_id, provider_kind, provider_item_id, created_at, updated_at)
                 VALUES (?1, ?2, 'local', 'migration-local', ?3, ?3)",
                params![source_id, track_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO playlists (id, name, kind, revision, created_at, updated_at)
                 VALUES (?1, 'Migration Playlist', 'normal', 2, ?2, ?2)",
                params![playlist_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO playlists (id, name, kind, parent_playlist_id, base_parent_revision, branch_status, revision, created_at, updated_at)
                 VALUES (?1, 'Migration Branch', 'branch', ?2, 2, 'open', 0, ?3, ?3)",
                params![branch_id, playlist_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO playlist_items (id, playlist_id, track_id, requested_source_id, position, added_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
                params![playlist_item_id, playlist_id, track_id, source_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO playlist_branch_base_items (branch_playlist_id, base_item_id, track_id, requested_source_id, position)
                 VALUES (?1, ?2, ?3, ?4, 0)",
                params![branch_id, playlist_item_id, track_id, source_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO likes (track_id, liked_at) VALUES (?1, ?2)",
                params![track_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO ratings (track_id, rating, updated_at) VALUES (?1, 5, ?2)",
                params![track_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
                 VALUES ('migration-tag', 'Migration Tag', 'migration tag', ?1, ?1)",
                params![now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO track_tags (track_id, tag_id, created_at) VALUES (?1, 'migration-tag', ?2)",
                params![track_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO queue_entries (id, track_id, requested_source_id, section, position, pinned, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'up_next', 0, 1, ?4, ?4)",
                params![queue_entry_id, track_id, source_id, now],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE queue_state SET current_entry_id = ?1, current_position_ms = 321, revision = 4, updated_at = ?2 WHERE id = 1",
                params![queue_entry_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO queue_snapshots (id, name, current_track_id, current_source_id, current_position_ms, repeat_mode, shuffle_enabled, history_order_json, shuffle_order_json, created_at)
                 VALUES (?1, 'Migration Snapshot', ?2, ?3, 321, 'off', 0, '[]', '[]', ?4)",
                params![snapshot_id, track_id, source_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO queue_snapshot_entries (id, snapshot_id, track_id, requested_source_id, section, position, pinned, traversal_position)
                 VALUES (?1, ?2, ?3, ?4, 'up_next', 0, 1, 0)",
                params![snapshot_entry_id, snapshot_id, track_id, source_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE queue_snapshots SET current_snapshot_entry_id = ?1 WHERE id = ?2",
                params![snapshot_entry_id, snapshot_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO user_track_overrides (provider_kind, provider_item_id, target_track_id, decision, created_at, updated_at)
                 VALUES ('local', 'migration-local', ?1, 'merge', ?2, ?2)",
                params![track_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO downloads (id, provider_kind, provider_item_id, canonical_url, title, artists_json, mode, state, destination_directory, source_quality_provenance, created_at, updated_at)
                 VALUES ('migration-download', 'youtube', 'migration-video', 'https://youtube.example/migration', 'Migration Track', '[]', 'audio', 'queued', 'C:\\Downloads', 'unknown', ?1, ?1)",
                params![now],
            )
            .unwrap();

        run_migrations(&mut connection, None, &MIGRATIONS[5..]).unwrap();
        let schema_version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(schema_version, LATEST_SCHEMA_VERSION as i64);
        for (table, expected) in [
            ("playlists", 3_i64),
            ("playlist_branch_base_items", 1),
            ("likes", 1),
            ("ratings", 1),
            ("tags", 1),
            ("track_tags", 1),
            ("queue_entries", 1),
            ("queue_snapshots", 1),
            ("queue_snapshot_entries", 1),
            ("downloads", 1),
            ("user_track_overrides", 1),
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, expected, "rows lost from {table}");
        }
        let foreign_key_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(foreign_key_rows, 0);
    }

    #[test]
    fn schema_three_override_constraints_reject_spotify_and_duplicate_forced_merge() {
        let path = TempDatabasePath::new("migration-three-constraints");
        let database = Database::open(path.path()).unwrap();
        let first_track = uuid::Uuid::new_v4().to_string();
        let second_track = uuid::Uuid::new_v4().to_string();
        database
            .with_connection(|connection| {
                for track_id in [&first_track, &second_track] {
                    connection.execute(
                        "INSERT INTO tracks (
                            id, title, normalized_title, version_qualifiers_json,
                            created_at, updated_at
                         ) VALUES (?1, ?2, ?2, '[\"standard\"]', ?3, ?3)",
                        params![track_id, track_id, "2026-01-01T00:00:00Z"],
                    )?;
                }
                connection.execute(
                    "INSERT INTO user_track_overrides (
                        provider_kind, provider_item_id, target_track_id, decision,
                        created_at, updated_at
                     ) VALUES ('youtube', 'duplicate-id', ?1, 'merge', ?2, ?2)",
                    params![first_track, "2026-01-01T00:00:00Z"],
                )?;
                let duplicate = connection.execute(
                    "INSERT INTO user_track_overrides (
                        provider_kind, provider_item_id, target_track_id, decision,
                        created_at, updated_at
                     ) VALUES ('youtube', 'duplicate-id', ?1, 'merge', ?2, ?2)",
                    params![second_track, "2026-01-01T00:00:00Z"],
                );
                assert!(duplicate.is_err());
                let spotify = connection.execute(
                    "INSERT INTO user_track_overrides (
                        provider_kind, provider_item_id, target_track_id, decision,
                        created_at, updated_at
                     ) VALUES ('spotify', 'spotify-id', ?1, 'merge', ?2, ?2)",
                    params![first_track, "2026-01-01T00:00:00Z"],
                );
                assert!(spotify.is_err());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn standard_database_path_is_resolved_from_the_caller_root() {
        let root = PathBuf::from(r"C:\Users\Example\AppData\Local");

        assert_eq!(
            standard_database_path(root),
            PathBuf::from(r"C:\Users\Example\AppData\Local\SpotDIY\spotdiy.sqlite3")
        );
    }

    #[test]
    fn wal_and_foreign_keys_are_enabled_and_fts5_is_probed() {
        let path = TempDatabasePath::new("pragmas");
        let database = Database::open(path.path()).expect("database should initialize");

        assert!(database.wal_enabled().unwrap());
        assert!(database.foreign_keys_enabled().unwrap());
        assert!(database.fts5_available());
    }

    #[test]
    fn migration_initialization_is_idempotent_and_reopen_preserves_state() {
        let path = TempDatabasePath::new("reopen");
        {
            let database = Database::open(path.path()).expect("database should initialize");
            database
                .with_connection(|connection| {
                    connection.execute(
                        "INSERT INTO artists (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                        params!["artist-1", "Test Artist", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"],
                    )?;
                    Ok(())
                })
                .unwrap();
        }

        let database = Database::open(path.path()).expect("database should reopen");
        assert_eq!(database.schema_version().unwrap(), LATEST_SCHEMA_VERSION);
        let count: i64 = database
            .with_connection(|connection| {
                connection.query_row("SELECT COUNT(*) FROM artists", [], |row| row.get(0))
            })
            .unwrap();
        assert_eq!(count, 1);
        assert!(database.wal_enabled().unwrap());
    }

    #[test]
    fn failed_migration_rolls_back_schema_and_version() {
        let path = TempDatabasePath::new("rollback");
        let mut connection = Connection::open(path.path()).unwrap();
        configure_connection(&connection).unwrap();
        connection
            .execute("CREATE TABLE existing (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        let migration = Migration {
            version: 1,
            name: "broken",
            sql: "CREATE TABLE created (id INTEGER); CREATE TABLE existing (id INTEGER);",
            destructive: false,
        };

        let result = run_migrations(&mut connection, Some(path.path()), &[migration]);
        assert!(matches!(result, Err(DatabaseError::Migration { .. })));
        assert_eq!(current_schema_version(&connection).unwrap(), 0);
        let created_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'created'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(created_count, 0);
    }

    #[test]
    fn migration_order_is_validated_before_any_migration_runs() {
        let path = TempDatabasePath::new("migration-order");
        let mut connection = Connection::open(path.path()).unwrap();
        configure_connection(&connection).unwrap();
        let migrations = [
            Migration {
                version: 2,
                name: "second",
                sql: "CREATE TABLE second (id INTEGER);",
                destructive: false,
            },
            Migration {
                version: 1,
                name: "first",
                sql: "CREATE TABLE first (id INTEGER);",
                destructive: false,
            },
        ];

        let result = run_migrations(&mut connection, Some(path.path()), &migrations);

        assert!(matches!(
            result,
            Err(DatabaseError::InvalidMigrationOrder { version: 1 })
        ));
        assert_eq!(current_schema_version(&connection).unwrap(), 0);
        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('first', 'second')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 0);
    }

    #[test]
    fn wal_backup_refuses_to_copy_when_a_reader_keeps_wal_frames_active() {
        let path = TempDatabasePath::new("backup-busy");
        let writer = Connection::open(path.path()).unwrap();
        configure_connection(&writer).unwrap();
        writer
            .execute("CREATE TABLE existing (value TEXT NOT NULL)", [])
            .unwrap();
        writer
            .execute("INSERT INTO existing (value) VALUES ('before')", [])
            .unwrap();

        let reader = Connection::open(path.path()).unwrap();
        configure_connection(&reader).unwrap();
        reader.busy_timeout(Duration::from_millis(100)).unwrap();
        let mut statement = reader.prepare("SELECT value FROM existing").unwrap();
        let mut rows = statement.query([]).unwrap();
        assert!(rows.next().unwrap().is_some());

        writer
            .execute("INSERT INTO existing (value) VALUES ('after')", [])
            .unwrap();
        writer.busy_timeout(Duration::from_millis(100)).unwrap();
        let result = create_wal_safe_backup(&writer, path.path());

        assert!(matches!(
            result,
            Err(DatabaseError::CheckpointBusy { busy }) if busy != 0
        ));
        let backup = PathBuf::from(format!("{}.pre-migration", path.path().display()));
        assert!(!backup.exists());
    }

    #[test]
    fn destructive_migration_creates_wal_safe_backup_before_apply() {
        let path = TempDatabasePath::new("backup");
        let mut connection = Connection::open(path.path()).unwrap();
        configure_connection(&connection).unwrap();
        connection
            .execute("CREATE TABLE existing (value TEXT NOT NULL)", [])
            .unwrap();
        connection
            .execute("INSERT INTO existing (value) VALUES ('before')", [])
            .unwrap();
        let migration = Migration {
            version: 1,
            name: "destructive-fixture",
            sql: "CREATE TABLE added (id INTEGER PRIMARY KEY);",
            destructive: true,
        };

        run_migrations(&mut connection, Some(path.path()), &[migration]).unwrap();
        let backup = PathBuf::from(format!("{}.pre-migration", path.path().display()));
        assert!(backup.exists());
        let backup_connection = Connection::open(backup).unwrap();
        let value: String = backup_connection
            .query_row("SELECT value FROM existing", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "before");
    }
}
