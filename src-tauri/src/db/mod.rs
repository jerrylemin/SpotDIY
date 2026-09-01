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

pub const LATEST_SCHEMA_VERSION: u32 = 4;
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
    use rusqlite::params;

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
        assert_eq!(schema_version, 4);
    }

    #[test]
    fn schema_two_fixture_upgrades_to_four_without_losing_tracks_sources_or_settings() {
        let path = TempDatabasePath::new("migration-four-fixture");
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
        assert_eq!(schema_version, 4);
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
