use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::db::{Database, DatabaseError, APPLICATION_DATA_DIRECTORY, DATABASE_FILE_NAME};
use crate::library::folders::is_reparse_point;
use crate::settings::{SettingValue, SettingsError, SettingsRepository};

pub const PORTABLE_MARKER_FILE_NAME: &str = "SpotDIY.portable";
pub const PORTABLE_DATA_DIRECTORY: &str = "Data";
pub const PORTABLE_MUSIC_DIRECTORY: &str = "Music";
pub const PORTABLE_COVERS_DIRECTORY: &str = "Covers";
pub const PORTABLE_LYRICS_DIRECTORY: &str = "Lyrics";
pub const PORTABLE_DATABASE_DIRECTORY: &str = "Database";
pub const PORTABLE_CACHE_DIRECTORY: &str = "Cache";
pub const PORTABLE_CONFIG_DIRECTORY: &str = "Config";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    Standard,
    Portable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageLayout {
    pub mode: StorageMode,
    pub executable_dir: PathBuf,
    pub local_data_root: PathBuf,
    pub data_root: PathBuf,
    pub music_root: PathBuf,
    pub covers_root: PathBuf,
    pub lyrics_root: PathBuf,
    pub database_path: PathBuf,
    pub cache_root: PathBuf,
    pub artwork_cache_root: PathBuf,
    pub downloads_cache_root: PathBuf,
    pub restore_root: PathBuf,
    pub rollback_root: PathBuf,
    pub config_root: PathBuf,
    pub portable_marker: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatus {
    pub mode: StorageMode,
    pub data_root: PathBuf,
    pub database_path: PathBuf,
    pub cache_root: PathBuf,
    pub portable_marker_present: bool,
    pub restart_required: bool,
    pub pending_import: bool,
    pub last_rollback_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageModeSwitchResult {
    pub mode: StorageMode,
    pub data_root: PathBuf,
    pub database_path: PathBuf,
    pub cache_root: PathBuf,
    pub restart_required: bool,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("could not inspect portable marker {path}: {source}")]
    MarkerInspect { path: PathBuf, source: io::Error },
    #[error("portable marker {path} must be a regular file")]
    InvalidMarker { path: PathBuf },
    #[error("database path {path} cannot be used: {detail}")]
    InvalidDatabasePath { path: PathBuf, detail: String },
    #[error("could not inspect database path {path}: {source}")]
    DatabasePathInspect { path: PathBuf, source: io::Error },
    #[error("could not create portable storage directory {path}: {source}")]
    CreateDirectory { path: PathBuf, source: io::Error },
    #[error("portable storage directory {path} is not writable: {source}")]
    NotWritable { path: PathBuf, source: io::Error },
    #[error("could not create portable marker {path}: {source}")]
    CreateMarker { path: PathBuf, source: io::Error },
    #[error("could not remove portable marker {path}: {source}")]
    RemoveMarker { path: PathBuf, source: io::Error },
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error("SQLite validation failed for {path}: {detail}")]
    Validation { path: PathBuf, detail: String },
    #[error("portable database already exists at {path}")]
    TargetAlreadyExists { path: PathBuf },
    #[error("could not finalize database {path}: {source}")]
    Finalize { path: PathBuf, source: io::Error },
    #[error("could not restore the previous database at {path}: {source}")]
    RestorePrevious { path: PathBuf, source: io::Error },
}

impl StorageLayout {
    pub fn resolve(
        executable_dir: impl AsRef<Path>,
        local_data_root: impl AsRef<Path>,
    ) -> Result<Self, StorageError> {
        let executable_dir = executable_dir.as_ref().to_path_buf();
        let local_data_root = local_data_root.as_ref().to_path_buf();
        let portable_marker = executable_dir.join(PORTABLE_MARKER_FILE_NAME);
        let portable = match fs::symlink_metadata(&portable_marker) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    return Err(StorageError::InvalidMarker {
                        path: portable_marker,
                    });
                }
                true
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => false,
            Err(source) => {
                return Err(StorageError::MarkerInspect {
                    path: portable_marker,
                    source,
                });
            }
        };
        let mode = if portable {
            StorageMode::Portable
        } else {
            StorageMode::Standard
        };
        let layout = Self::for_mode(executable_dir, local_data_root, mode);
        if mode == StorageMode::Portable {
            layout.ensure_portable_directories()?;
        }
        validate_database_path(&layout.database_path)?;
        Ok(layout)
    }

    pub fn for_mode(
        executable_dir: impl Into<PathBuf>,
        local_data_root: impl Into<PathBuf>,
        mode: StorageMode,
    ) -> Self {
        let executable_dir = executable_dir.into();
        let local_data_root = local_data_root.into();
        let portable_marker = executable_dir.join(PORTABLE_MARKER_FILE_NAME);
        match mode {
            StorageMode::Standard => {
                let data_root = local_data_root.join(APPLICATION_DATA_DIRECTORY);
                let cache_root = data_root.join("cache");
                Self {
                    mode,
                    executable_dir,
                    local_data_root,
                    data_root: data_root.clone(),
                    music_root: data_root.join(PORTABLE_MUSIC_DIRECTORY),
                    covers_root: data_root.join(PORTABLE_COVERS_DIRECTORY),
                    lyrics_root: data_root.join(PORTABLE_LYRICS_DIRECTORY),
                    database_path: data_root.join(DATABASE_FILE_NAME),
                    artwork_cache_root: cache_root.join("artwork"),
                    downloads_cache_root: cache_root.join("downloads"),
                    cache_root,
                    restore_root: data_root.join("restore"),
                    rollback_root: data_root.join("restore").join("rollback"),
                    config_root: data_root.join(PORTABLE_CONFIG_DIRECTORY),
                    portable_marker,
                }
            }
            StorageMode::Portable => {
                let data_root = executable_dir.join(PORTABLE_DATA_DIRECTORY);
                let cache_root = executable_dir.join(PORTABLE_CACHE_DIRECTORY);
                Self {
                    mode,
                    executable_dir: executable_dir.clone(),
                    local_data_root,
                    data_root: data_root.clone(),
                    music_root: executable_dir.join(PORTABLE_MUSIC_DIRECTORY),
                    covers_root: executable_dir.join(PORTABLE_COVERS_DIRECTORY),
                    lyrics_root: executable_dir.join(PORTABLE_LYRICS_DIRECTORY),
                    database_path: executable_dir
                        .join(PORTABLE_DATABASE_DIRECTORY)
                        .join(DATABASE_FILE_NAME),
                    artwork_cache_root: cache_root.join("artwork"),
                    downloads_cache_root: cache_root.join("downloads"),
                    cache_root,
                    restore_root: data_root.join("restore"),
                    rollback_root: data_root.join("restore").join("rollback"),
                    config_root: executable_dir.join(PORTABLE_CONFIG_DIRECTORY),
                    portable_marker,
                }
            }
        }
    }

    pub fn ensure_runtime_directories(&self) -> Result<(), StorageError> {
        match self.mode {
            StorageMode::Standard => {
                for path in [
                    self.data_root.clone(),
                    self.cache_root.clone(),
                    self.artwork_cache_root.clone(),
                    self.downloads_cache_root.clone(),
                    self.restore_root.clone(),
                    self.rollback_root.clone(),
                    self.config_root.clone(),
                ] {
                    create_directory(&path)?;
                }
            }
            StorageMode::Portable => self.ensure_portable_directories()?,
        }
        Ok(())
    }

    pub fn ensure_portable_directories(&self) -> Result<(), StorageError> {
        if self.mode != StorageMode::Portable {
            return Ok(());
        }
        for path in [
            self.data_root.clone(),
            self.music_root.clone(),
            self.covers_root.clone(),
            self.lyrics_root.clone(),
            self.database_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| self.database_path.clone()),
            self.cache_root.clone(),
            self.artwork_cache_root.clone(),
            self.downloads_cache_root.clone(),
            self.restore_root.clone(),
            self.rollback_root.clone(),
            self.config_root.clone(),
        ] {
            create_directory(&path)?;
        }
        ensure_writable_directory(&self.executable_dir)?;
        Ok(())
    }

    pub fn status(
        &self,
        pending_import: bool,
        last_rollback_path: Option<PathBuf>,
    ) -> StorageStatus {
        let portable_marker_present = fs::symlink_metadata(&self.portable_marker)
            .map(|metadata| metadata.is_file())
            .unwrap_or(false);
        StorageStatus {
            mode: self.mode,
            data_root: self.data_root.clone(),
            database_path: self.database_path.clone(),
            cache_root: self.cache_root.clone(),
            portable_marker_present,
            restart_required: portable_marker_present != (self.mode == StorageMode::Portable),
            pending_import,
            last_rollback_path,
        }
    }

    pub fn prepare_mode_switch(
        &self,
        database: &Database,
        target_mode: StorageMode,
    ) -> Result<StorageModeSwitchResult, StorageError> {
        if target_mode == self.mode {
            return Ok(StorageModeSwitchResult {
                mode: target_mode,
                data_root: self.data_root.clone(),
                database_path: self.database_path.clone(),
                cache_root: self.cache_root.clone(),
                restart_required: false,
            });
        }

        let target = Self::for_mode(
            self.executable_dir.clone(),
            self.local_data_root.clone(),
            target_mode,
        );
        target.ensure_runtime_directories()?;
        validate_database_path(&target.database_path)?;
        if target_mode == StorageMode::Portable {
            self.prepare_standard_to_portable(database, &target)?;
        } else {
            self.prepare_portable_to_standard(database, &target)?;
        }

        Ok(StorageModeSwitchResult {
            mode: target_mode,
            data_root: target.data_root,
            database_path: target.database_path,
            cache_root: target.cache_root,
            restart_required: true,
        })
    }

    fn prepare_standard_to_portable(
        &self,
        database: &Database,
        target: &StorageLayout,
    ) -> Result<(), StorageError> {
        if target.database_path.exists() {
            return Err(StorageError::TargetAlreadyExists {
                path: target.database_path.clone(),
            });
        }
        let temporary = sibling_temp_path(&target.database_path, "portable-switch");
        if let Err(error) = database.online_backup_to(&temporary) {
            remove_database_files(&temporary);
            return Err(error.into());
        }
        if let Err(error) = prepare_database_copy(&temporary, target.mode) {
            remove_database_files(&temporary);
            return Err(error);
        }
        if let Err(error) = finalize_new_database(&temporary, &target.database_path) {
            remove_database_files(&temporary);
            return Err(error);
        }
        if let Err(error) = create_marker(&target.portable_marker) {
            remove_database_files(&target.database_path);
            return Err(error);
        }
        Ok(())
    }

    fn prepare_portable_to_standard(
        &self,
        database: &Database,
        target: &StorageLayout,
    ) -> Result<(), StorageError> {
        let mut existing_backup = None;
        if target.database_path.exists() {
            let backup = sibling_temp_path(&target.database_path, "standard-switch-backup");
            let existing = Database::open(&target.database_path)?;
            if let Err(error) = existing.online_backup_to(&backup) {
                remove_database_files(&backup);
                return Err(error.into());
            }
            existing_backup = Some(backup);
        }

        let temporary = sibling_temp_path(&target.database_path, "standard-switch");
        if let Err(error) = database.online_backup_to(&temporary) {
            remove_database_files(&temporary);
            if let Some(backup) = existing_backup {
                remove_database_files(&backup);
            }
            return Err(error.into());
        }
        if let Err(error) = prepare_database_copy(&temporary, target.mode) {
            remove_database_files(&temporary);
            if let Some(backup) = existing_backup {
                remove_database_files(&backup);
            }
            return Err(error);
        }
        if let Err(error) = replace_database(&temporary, &target.database_path) {
            remove_database_files(&temporary);
            if let Some(backup) = existing_backup {
                remove_database_files(&backup);
            }
            return Err(error);
        }
        if let Err(error) = remove_marker(&self.portable_marker) {
            if let Some(backup) = existing_backup {
                let _ = fs::remove_file(&target.database_path);
                let _ = fs::rename(backup, &target.database_path);
            }
            return Err(error);
        }
        if let Some(backup) = existing_backup {
            remove_database_files(&backup);
        }
        Ok(())
    }
}

fn create_directory(path: &Path) -> Result<(), StorageError> {
    let mut pending = Vec::new();
    let mut current = path.to_path_buf();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
                    return Err(StorageError::CreateDirectory {
                        path: current,
                        source: io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "storage directory cannot be a symbolic link or reparse point",
                        ),
                    });
                }
                if !metadata.is_dir() {
                    return Err(StorageError::CreateDirectory {
                        path: current,
                        source: io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "storage path is not a directory",
                        ),
                    });
                }
                break;
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                pending.push(current.clone());
                let Some(parent) = current.parent() else {
                    return Err(StorageError::CreateDirectory {
                        path: current,
                        source,
                    });
                };
                current = parent.to_path_buf();
            }
            Err(source) => {
                return Err(StorageError::CreateDirectory {
                    path: current,
                    source,
                });
            }
        }
    }
    for directory in pending.into_iter().rev() {
        fs::create_dir(&directory).map_err(|source| StorageError::CreateDirectory {
            path: directory.clone(),
            source,
        })?;
        let metadata =
            fs::symlink_metadata(&directory).map_err(|source| StorageError::CreateDirectory {
                path: directory.clone(),
                source,
            })?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) || !metadata.is_dir() {
            return Err(StorageError::CreateDirectory {
                path: directory,
                source: io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "storage directory is not a regular directory",
                ),
            });
        }
    }
    Ok(())
}

fn validate_database_path(path: &Path) -> Result<(), StorageError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || is_reparse_point(&metadata) => {
            Err(StorageError::InvalidDatabasePath {
                path: path.to_path_buf(),
                detail: "database cannot be a symbolic link or reparse point".to_owned(),
            })
        }
        Ok(metadata) if !metadata.is_file() => Err(StorageError::InvalidDatabasePath {
            path: path.to_path_buf(),
            detail: "database path is not a regular file".to_owned(),
        }),
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(StorageError::DatabasePathInspect {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn ensure_writable_directory(path: &Path) -> Result<(), StorageError> {
    create_directory(path)?;
    let probe = path.join(format!(".spotdiy-write-probe-{}", Uuid::new_v4()));
    let result = OpenOptions::new().write(true).create_new(true).open(&probe);
    match result {
        Ok(file) => {
            drop(file);
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(source) => Err(StorageError::NotWritable {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn create_marker(path: &Path) -> Result<(), StorageError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| StorageError::CreateMarker {
            path: path.to_path_buf(),
            source,
        })?;
    std::io::Write::write_all(&mut file, b"SpotDIY portable mode\n").map_err(|source| {
        StorageError::CreateMarker {
            path: path.to_path_buf(),
            source,
        }
    })?;
    file.sync_all()
        .map_err(|source| StorageError::CreateMarker {
            path: path.to_path_buf(),
            source,
        })
}

fn remove_marker(path: &Path) -> Result<(), StorageError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(StorageError::RemoveMarker {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn sibling_temp_path(path: &Path, label: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(DATABASE_FILE_NAME);
    path.with_file_name(format!(".{file_name}.{label}-{}.tmp", Uuid::new_v4()))
}

fn prepare_database_copy(path: &Path, mode: StorageMode) -> Result<(), StorageError> {
    let database = Database::open(path)?;
    SettingsRepository::new(&database).set_setting(SettingValue::StorageMode(mode))?;
    validate_database(&database, path)?;
    drop(database);
    remove_sidecars(path);
    Ok(())
}

fn validate_database(database: &Database, path: &Path) -> Result<(), StorageError> {
    let integrity: String = database.with_connection(|connection| {
        connection.pragma_query_value(None, "integrity_check", |row| row.get(0))
    })?;
    if integrity != "ok" {
        return Err(StorageError::Validation {
            path: path.to_path_buf(),
            detail: format!("PRAGMA integrity_check returned {integrity}"),
        });
    }
    let foreign_key_count: i64 = database.with_connection(|connection| {
        connection.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
    })?;
    if foreign_key_count != 0 {
        return Err(StorageError::Validation {
            path: path.to_path_buf(),
            detail: format!("PRAGMA foreign_key_check returned {foreign_key_count} row(s)"),
        });
    }
    Ok(())
}

fn finalize_new_database(temporary: &Path, destination: &Path) -> Result<(), StorageError> {
    fs::rename(temporary, destination).map_err(|source| StorageError::Finalize {
        path: destination.to_path_buf(),
        source,
    })
}

fn replace_database(temporary: &Path, destination: &Path) -> Result<(), StorageError> {
    let previous = if destination.exists() {
        let previous = sibling_temp_path(destination, "previous");
        fs::rename(destination, &previous).map_err(|source| StorageError::Finalize {
            path: destination.to_path_buf(),
            source,
        })?;
        Some(previous)
    } else {
        None
    };

    if let Err(source) = fs::rename(temporary, destination) {
        if let Some(previous) = previous {
            let _ = fs::rename(&previous, destination);
        }
        return Err(StorageError::Finalize {
            path: destination.to_path_buf(),
            source,
        });
    }
    if let Some(previous) = previous {
        let _ = fs::remove_file(previous);
    }
    Ok(())
}

fn remove_database_files(path: &Path) {
    let _ = fs::remove_file(path);
    remove_sidecars(path);
}

fn remove_sidecars(path: &Path) {
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", path.display(), suffix));
        let _ = fs::remove_file(sidecar);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(label: &str) -> (tempfile::TempDir, tempfile::TempDir) {
        (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap_or_else(|_| panic!("could not create {label} temp root")),
        )
    }

    #[test]
    fn no_marker_resolves_standard_compatible_path() {
        let (exe, local) = roots("standard");
        let layout = StorageLayout::resolve(exe.path(), local.path()).unwrap();
        assert_eq!(layout.mode, StorageMode::Standard);
        assert_eq!(
            layout.database_path,
            local.path().join("SpotDIY").join("spotdiy.sqlite3")
        );
        assert!(!layout.portable_marker.exists());
    }

    #[test]
    fn marker_resolves_portable_and_creates_exact_layout() {
        let (exe, local) = roots("portable");
        fs::write(exe.path().join(PORTABLE_MARKER_FILE_NAME), b"marker").unwrap();
        let layout = StorageLayout::resolve(exe.path(), local.path()).unwrap();
        assert_eq!(layout.mode, StorageMode::Portable);
        for path in [
            exe.path().join("Data"),
            exe.path().join("Music"),
            exe.path().join("Covers"),
            exe.path().join("Lyrics"),
            exe.path().join("Database"),
            exe.path().join("Cache"),
            exe.path().join("Cache").join("artwork"),
            exe.path().join("Cache").join("downloads"),
            exe.path().join("Data").join("restore"),
            exe.path().join("Config"),
        ] {
            assert!(path.is_dir(), "missing portable path {}", path.display());
        }
        assert_eq!(
            layout.database_path,
            exe.path().join("Database").join("spotdiy.sqlite3")
        );
        assert!(!local.path().join("SpotDIY").exists());
    }

    #[test]
    fn portable_mode_does_not_fallback_when_root_is_a_file() {
        let (exe, local) = roots("unwritable");
        fs::write(exe.path().join(PORTABLE_MARKER_FILE_NAME), b"marker").unwrap();
        fs::write(exe.path().join("Database"), b"not a directory").unwrap();
        let result = StorageLayout::resolve(exe.path(), local.path());
        assert!(matches!(result, Err(StorageError::CreateDirectory { .. })));
        assert!(!local.path().join("SpotDIY").exists());
    }

    #[test]
    fn status_requires_restart_when_marker_changes_after_startup() {
        let (exe, local) = roots("status");
        let layout = StorageLayout::resolve(exe.path(), local.path()).unwrap();
        fs::write(exe.path().join(PORTABLE_MARKER_FILE_NAME), b"marker").unwrap();
        let status = layout.status(false, None);
        assert!(status.portable_marker_present);
        assert!(status.restart_required);
    }

    #[test]
    fn mode_switch_copies_database_and_changes_marker_last() {
        let (exe, local) = roots("mode-switch");
        let standard = StorageLayout::resolve(exe.path(), local.path()).unwrap();
        standard.ensure_runtime_directories().unwrap();
        let standard_database = Database::open(&standard.database_path).unwrap();
        SettingsRepository::new(&standard_database)
            .set_setting(SettingValue::Theme(crate::settings::Theme::Light))
            .unwrap();
        standard_database
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE settings_metadata
                     SET value_json = '\"light\"'
                     WHERE setting_key = 'theme'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        let portable_result = standard
            .prepare_mode_switch(&standard_database, StorageMode::Portable)
            .unwrap();
        assert!(portable_result.restart_required);
        assert!(standard.database_path.exists());
        assert!(standard.portable_marker.exists());
        let portable = StorageLayout::resolve(exe.path(), local.path()).unwrap();
        let portable_database = Database::open(&portable.database_path).unwrap();
        assert_eq!(
            SettingsRepository::new(&portable_database)
                .get_snapshot()
                .unwrap()
                .storage_mode,
            StorageMode::Portable
        );
        let theme: String = portable_database
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT value_json FROM settings_metadata WHERE setting_key = 'theme'",
                    [],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(theme, "\"light\"");
        drop(portable_database);
        drop(standard_database);

        let standard_again =
            StorageLayout::for_mode(exe.path(), local.path(), StorageMode::Standard);
        let portable_database = Database::open(&portable.database_path).unwrap();
        portable
            .prepare_mode_switch(&portable_database, StorageMode::Standard)
            .unwrap();
        assert!(!standard_again.portable_marker.exists());
        drop(portable_database);
        let restored = Database::open(&standard_again.database_path).unwrap();
        assert_eq!(
            SettingsRepository::new(&restored)
                .get_snapshot()
                .unwrap()
                .storage_mode,
            StorageMode::Standard
        );
    }

    #[test]
    fn mode_switch_failure_before_selector_change_preserves_current_mode() {
        let (exe, local) = roots("mode-switch-failure");
        let standard = StorageLayout::resolve(exe.path(), local.path()).unwrap();
        standard.ensure_runtime_directories().unwrap();
        let standard_database = Database::open(&standard.database_path).unwrap();
        let portable = StorageLayout::for_mode(exe.path(), local.path(), StorageMode::Portable);
        portable.ensure_runtime_directories().unwrap();
        let portable_database = Database::open(&portable.database_path).unwrap();
        assert!(matches!(
            standard.prepare_mode_switch(&standard_database, StorageMode::Portable),
            Err(StorageError::TargetAlreadyExists { .. })
        ));
        assert!(!standard.portable_marker.exists());
        assert!(standard.database_path.exists());
        drop(portable_database);
        drop(standard_database);

        let (exe, local) = roots("mode-switch-portable-failure");
        fs::write(exe.path().join(PORTABLE_MARKER_FILE_NAME), b"marker").unwrap();
        let portable = StorageLayout::resolve(exe.path(), local.path()).unwrap();
        let portable_database = Database::open(&portable.database_path).unwrap();
        let standard_target =
            StorageLayout::for_mode(exe.path(), local.path(), StorageMode::Standard);
        fs::create_dir_all(&standard_target.data_root).unwrap();
        fs::create_dir(&standard_target.database_path).unwrap();
        assert!(matches!(
            portable.prepare_mode_switch(&portable_database, StorageMode::Standard),
            Err(StorageError::InvalidDatabasePath { .. })
        ));
        assert!(portable.portable_marker.exists());
        assert!(portable.database_path.exists());
    }
}
