use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use thiserror::Error;

use crate::db::Database;
use crate::library::folders::normalize_folder_path;
use crate::storage::{StorageLayout, StorageMode, StorageModeSwitchResult, StorageStatus};

pub mod archive;
pub mod import;
pub mod manifest;

pub use import::{ImportPreview, MissingFileReference, MissingFileReport};
pub use manifest::{
    SpotDiyArchiveEntry, SpotDiyArchiveEntryKind, SpotDiyExportOptions, SpotDiyManifest,
    SpotDiyMediaMapping,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCommitResult {
    pub import_id: String,
    pub restart_required: bool,
    pub preview: ImportPreview,
}

#[derive(Debug, Error)]
pub enum BackupError {
    #[error(transparent)]
    Archive(#[from] archive::ArchiveError),
    #[error(transparent)]
    Import(#[from] import::ImportError),
    #[error(transparent)]
    Storage(#[from] crate::storage::StorageError),
    #[error("backup service lock is poisoned")]
    Lock,
    #[error("import {0} was not found")]
    ImportNotFound(String),
    #[error("a different import is already pending")]
    PendingImport,
    #[error("included local audio requires a valid restore folder")]
    RestoreFolderRequired,
}

#[derive(Clone)]
pub struct BackupService {
    database: Database,
    layout: StorageLayout,
    app_version: String,
    imports: Arc<Mutex<HashMap<String, import::StagedImport>>>,
    last_rollback_path: Arc<Mutex<Option<PathBuf>>>,
}

impl BackupService {
    pub fn new(
        database: Database,
        layout: StorageLayout,
        app_version: impl Into<String>,
    ) -> Result<Self, BackupError> {
        layout.ensure_runtime_directories()?;
        let last_rollback_path = latest_rollback_path(&layout);
        Ok(Self {
            database,
            layout,
            app_version: app_version.into(),
            imports: Arc::new(Mutex::new(HashMap::new())),
            last_rollback_path: Arc::new(Mutex::new(last_rollback_path)),
        })
    }

    pub fn export(
        &self,
        options: SpotDiyExportOptions,
        destination: impl AsRef<Path>,
    ) -> Result<archive::ExportResult, BackupError> {
        Ok(archive::write_archive(
            &self.database,
            &self.layout,
            &self.app_version,
            &options,
            destination,
        )?)
    }

    pub fn stage_import(&self, archive_path: &Path) -> Result<ImportPreview, BackupError> {
        let has_staged_import = !self
            .imports
            .lock()
            .map_err(|_| BackupError::Lock)?
            .is_empty();
        if has_staged_import || import::read_pending_descriptor(&self.layout)?.is_some() {
            return Err(BackupError::PendingImport);
        }
        let staged = import::stage_archive(archive_path, &self.layout, self.layout.mode)?;
        let preview = staged.preview.clone();
        self.imports
            .lock()
            .map_err(|_| BackupError::Lock)?
            .insert(staged.id.to_string(), staged);
        Ok(preview)
    }

    pub fn pending_preview(&self) -> Result<Option<ImportPreview>, BackupError> {
        if let Some(preview) = self
            .imports
            .lock()
            .map_err(|_| BackupError::Lock)?
            .values()
            .next()
            .map(|staged| staged.preview.clone())
        {
            return Ok(Some(preview));
        }
        Ok(import::read_pending_descriptor(&self.layout)?.map(|descriptor| descriptor.preview))
    }

    pub fn commit_import(
        &self,
        import_id: &str,
        music_destination: Option<PathBuf>,
    ) -> Result<ImportCommitResult, BackupError> {
        let (has_audio, restored_audio_count) = {
            let imports = self.imports.lock().map_err(|_| BackupError::Lock)?;
            let staged = imports
                .get(import_id)
                .ok_or_else(|| BackupError::ImportNotFound(import_id.to_owned()))?;
            let audio_count = staged
                .manifest
                .entries
                .iter()
                .filter(|entry| entry.kind == manifest::SpotDiyArchiveEntryKind::LocalAudio)
                .count() as u64;
            (audio_count != 0, audio_count)
        };
        let music_destination = if has_audio && self.layout.mode == StorageMode::Standard {
            let destination = music_destination.ok_or(BackupError::RestoreFolderRequired)?;
            Some(
                normalize_folder_path(&destination)
                    .map_err(|error| {
                        BackupError::Import(import::ImportError::InvalidMusicDestination(
                            error.to_string(),
                        ))
                    })?
                    .filesystem_path,
            )
        } else {
            None
        };
        let staged = self
            .imports
            .lock()
            .map_err(|_| BackupError::Lock)?
            .remove(import_id)
            .ok_or_else(|| BackupError::ImportNotFound(import_id.to_owned()))?;
        let mut preview = staged.preview.clone();
        preview.restored_audio_planned_count = restored_audio_count;
        let descriptor = import::PendingRestoreDescriptor {
            version: import::PENDING_RESTORE_VERSION,
            import_id: import_id.to_owned(),
            state: import::PendingRestoreState::Pending,
            staged_root: staged.root,
            staged_database_path: staged.staged_database_path,
            active_mode: self.layout.mode,
            music_destination,
            manifest: staged.manifest,
            preview: preview.clone(),
            archive_sha256: staged.archive_sha256,
            staged_database_sha256: staged.staged_database_sha256,
            rollback_path: None,
            created_paths: Vec::new(),
            last_error: None,
        };
        if let Err(error) = import::write_pending_descriptor(&self.layout, &descriptor) {
            let _ = fs::remove_dir_all(&descriptor.staged_root);
            return Err(error.into());
        }
        Ok(ImportCommitResult {
            import_id: import_id.to_owned(),
            restart_required: true,
            preview,
        })
    }

    pub fn cancel_import(&self, import_id: &str) -> Result<(), BackupError> {
        if let Some(staged) = self
            .imports
            .lock()
            .map_err(|_| BackupError::Lock)?
            .remove(import_id)
        {
            let root = staged.root.clone();
            fs::remove_dir_all(&root).map_err(|error| {
                BackupError::Import(import::ImportError::CreateStaging {
                    path: root,
                    source: error,
                })
            })?;
            return Ok(());
        }
        if let Some(descriptor) = import::read_pending_descriptor(&self.layout)? {
            if descriptor.import_id != import_id {
                return Err(BackupError::ImportNotFound(import_id.to_owned()));
            }
            import::validate_staged_paths(&self.layout, &descriptor)?;
            import::cleanup_created_paths_for_cancel(&self.layout, &descriptor.created_paths);
            let _ = fs::remove_dir_all(&descriptor.staged_root);
            import::remove_pending_descriptor(&self.layout)?;
            return Ok(());
        }
        Err(BackupError::ImportNotFound(import_id.to_owned()))
    }

    pub fn storage_status(&self) -> Result<StorageStatus, BackupError> {
        let pending_import = self.pending_preview()?.is_some();
        let last_rollback_path = self
            .last_rollback_path
            .lock()
            .map_err(|_| BackupError::Lock)?
            .clone();
        Ok(self.layout.status(pending_import, last_rollback_path))
    }

    pub fn prepare_storage_mode_switch(
        &self,
        target_mode: StorageMode,
    ) -> Result<StorageModeSwitchResult, BackupError> {
        Ok(self
            .layout
            .prepare_mode_switch(&self.database, target_mode)?)
    }

    pub(crate) fn startup_restore(
        layout: &StorageLayout,
    ) -> Result<import::RestoreApplyReport, BackupError> {
        Ok(import::apply_pending_restore(layout)?)
    }

    pub(crate) fn record_startup_restore(
        &self,
        report: &import::RestoreApplyReport,
    ) -> Result<(), BackupError> {
        if let Some(path) = report.rollback_path.clone() {
            *self
                .last_rollback_path
                .lock()
                .map_err(|_| BackupError::Lock)? = Some(path);
        }
        Ok(())
    }

    pub fn layout(&self) -> &StorageLayout {
        &self.layout
    }
}

fn latest_rollback_path(layout: &StorageLayout) -> Option<PathBuf> {
    let mut paths = fs::read_dir(&layout.rollback_root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("spotdiy.sqlite3.rollback-"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.pop()
}
