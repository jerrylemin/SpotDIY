pub mod artwork;
pub mod fingerprint;
pub mod folders;
pub mod metadata;
pub mod scanner;
pub mod watcher;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::db::{Database, DatabaseError};
use crate::domain::{
    AlbumId, ArtistId, LibraryFolder, LibraryFolderId, LibraryFolderStatus, LibraryPage,
    LibraryPageRequest, LibrarySort, LibraryStatus, LibraryTrack, LocalFileIndexStatus,
    ScanProgress, ScanSummary, SourceId, TrackId,
};

use self::artwork::{ArtworkCache, ArtworkCacheEntry, ArtworkError};
use self::folders::{
    is_path_within, normalize_file_path, validate_new_folders, FolderPathError,
    NormalizedFolderPath,
};
use self::metadata::ExtractedMetadata;

pub const LIBRARY_PROGRESS_EVENT: &str = "library://scan-progress";
pub const DEFAULT_PAGE_SIZE: u32 = 50;
pub const MAX_PAGE_SIZE: u32 = 100;

pub type ProgressSink = Arc<dyn Fn(ScanProgress) + Send + Sync + 'static>;

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error(transparent)]
    Path(#[from] FolderPathError),
    #[error("database operation failed: {0}")]
    Database(#[from] DatabaseError),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid stored library value for {field}: {value}")]
    InvalidStoredValue { field: &'static str, value: String },
    #[error("library folder {0} was not found")]
    FolderNotFound(LibraryFolderId),
    #[error("library scan for folder {0} is already running")]
    ScanAlreadyRunning(LibraryFolderId),
    #[error("library page size must be between 1 and {MAX_PAGE_SIZE}")]
    InvalidPageSize,
    #[error("could not initialize artwork cache: {0}")]
    ArtworkCache(#[from] ArtworkError),
    #[error("fingerprint failed: {0}")]
    Fingerprint(#[from] fingerprint::FingerprintError),
    #[error("metadata extraction failed: {0}")]
    Metadata(#[from] metadata::MetadataError),
    #[error("watcher failed: {0}")]
    Watcher(String),
}

#[derive(Clone, Debug)]
pub(crate) struct FolderScanInfo {
    pub id: LibraryFolderId,
    pub filesystem_path: PathBuf,
    pub generation: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ExistingLocalFile {
    pub source_id: SourceId,
    pub track_id: TrackId,
    pub provider_item_id: String,
    pub available: bool,
    pub file_size_bytes: Option<u64>,
    pub modified_at: Option<DateTime<Utc>>,
    pub index_status: LocalFileIndexStatus,
}

#[derive(Clone, Debug)]
pub(crate) struct ScannedFile {
    pub folder_id: LibraryFolderId,
    pub generation: u64,
    pub path: PathBuf,
    pub normalized_path_key: String,
    pub file_size_bytes: u64,
    pub modified_at: Option<DateTime<Utc>>,
    pub fingerprint: String,
    pub metadata: ExtractedMetadata,
    pub artwork: Option<ArtworkCacheEntry>,
    pub index_status: LocalFileIndexStatus,
    pub status_detail: Option<String>,
    pub now: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct UpsertOutcome {
    pub is_new: bool,
    pub is_renamed: bool,
}

#[derive(Default)]
struct ScanState {
    active: HashSet<LibraryFolderId>,
    pending: HashMap<LibraryFolderId, PendingScan>,
}

struct PendingScan {
    force: bool,
    sink: Option<ProgressSink>,
}

#[derive(Clone)]
pub struct LibraryService {
    database: Database,
    artwork_cache: ArtworkCache,
    scan_state: Arc<Mutex<ScanState>>,
    watchers: Arc<watcher::WatcherRegistry>,
}

impl LibraryService {
    pub fn new(
        database: Database,
        artwork_cache_root: impl Into<PathBuf>,
    ) -> Result<Self, LibraryError> {
        let service = Self {
            database,
            artwork_cache: ArtworkCache::new(artwork_cache_root)?,
            scan_state: Arc::new(Mutex::new(ScanState::default())),
            watchers: Arc::new(watcher::WatcherRegistry::new()),
        };
        service.recover_interrupted_scans()?;
        Ok(service)
    }

    pub fn list_folders(&self) -> Result<Vec<LibraryFolder>, LibraryError> {
        list_folders(&self.database)
    }

    pub fn add_folders(&self, paths: Vec<PathBuf>) -> Result<Vec<LibraryFolder>, LibraryError> {
        let existing_keys = self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT normalized_path_key FROM library_folders WHERE enabled = 1 ORDER BY rowid",
            )?;
            let values = statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>();
            values
        })?;
        let normalized = validate_new_folders(paths, existing_keys)?;
        insert_folders(&self.database, &normalized)?;
        self.list_folders()
    }

    pub fn add_folders_and_start(
        &self,
        paths: Vec<PathBuf>,
        sink: Option<ProgressSink>,
    ) -> Result<Vec<LibraryFolder>, LibraryError> {
        let existing_ids = self
            .list_folders()?
            .into_iter()
            .map(|folder| folder.id)
            .collect::<HashSet<_>>();
        let folders = self.add_folders(paths)?;
        let added = folders
            .iter()
            .filter(|folder| !existing_ids.contains(&folder.id))
            .cloned()
            .collect::<Vec<_>>();
        self.activate_folders(&added, sink)?;
        Ok(folders)
    }

    pub fn activate_folders(
        &self,
        folders: &[LibraryFolder],
        sink: Option<ProgressSink>,
    ) -> Result<(), LibraryError> {
        for folder in folders.iter().filter(|folder| folder.enabled) {
            if !is_directory(&folder.path) {
                self.mark_folder_unavailable(folder.id, "Library folder is unavailable")?;
                self.set_folder_status(
                    folder.id,
                    LibraryFolderStatus::Failed,
                    Some("Library folder is unavailable".to_owned()),
                )?;
                continue;
            }
            if let Err(error) = self.register_folder_watcher(folder, sink.clone()) {
                self.set_folder_status(
                    folder.id,
                    LibraryFolderStatus::Failed,
                    Some(format!("Could not watch library folder: {error}")),
                )?;
            }
            self.request_scan(folder.id, false, sink.clone())?;
        }
        Ok(())
    }

    pub fn remove_folder(&self, folder_id: LibraryFolderId) -> Result<(), LibraryError> {
        self.watchers.unregister(folder_id);
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        let exists: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM library_folders WHERE id = ?1",
                params![folder_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(LibraryError::FolderNotFound(folder_id));
        }

        transaction.execute(
            "DELETE FROM track_sources WHERE id IN (
                 SELECT source_id FROM local_files WHERE library_folder_id = ?1
             )",
            params![folder_id.to_string()],
        )?;
        transaction.execute(
            "DELETE FROM tracks WHERE id NOT IN (SELECT track_id FROM track_sources)",
            [],
        )?;
        transaction.execute(
            "DELETE FROM library_folders WHERE id = ?1",
            params![folder_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn status(&self) -> Result<LibraryStatus, LibraryError> {
        let folders = self.list_folders()?;
        let (indexed_track_count, available_track_count): (i64, i64) =
            self.database.with_connection(|connection| {
                let indexed: i64 = connection.query_row(
                    "SELECT COUNT(DISTINCT ts.track_id)
                     FROM local_files lf
                     INNER JOIN track_sources ts ON ts.id = lf.source_id
                     WHERE lf.library_folder_id IS NOT NULL
                       AND lf.index_status = 'indexed'",
                    [],
                    |row| row.get(0),
                )?;
                let available: i64 = connection.query_row(
                    "SELECT COUNT(DISTINCT ts.track_id)
                     FROM local_files lf
                     INNER JOIN track_sources ts ON ts.id = lf.source_id
                     WHERE lf.library_folder_id IS NOT NULL
                       AND lf.index_status = 'indexed' AND ts.available = 1",
                    [],
                    |row| row.get(0),
                )?;
                Ok((indexed, available))
            })?;
        Ok(LibraryStatus {
            is_scanning: folders.iter().any(|folder| {
                matches!(
                    folder.status,
                    LibraryFolderStatus::Queued | LibraryFolderStatus::Scanning
                )
            }),
            folders,
            indexed_track_count: non_negative_u64(
                indexed_track_count,
                "library.indexed_track_count",
            )?,
            available_track_count: non_negative_u64(
                available_track_count,
                "library.available_track_count",
            )?,
        })
    }

    pub fn page(&self, request: LibraryPageRequest) -> Result<LibraryPage, LibraryError> {
        let mut page = load_library_page(&self.database, request)?;
        for track in &mut page.items {
            track.artwork_path = track
                .artwork_cache_key
                .as_deref()
                .and_then(|cache_key| self.artwork_path(cache_key))
                .filter(|path| path.is_file());
        }
        Ok(page)
    }

    pub fn start_scan(
        &self,
        folder_id: LibraryFolderId,
        force: bool,
        sink: Option<ProgressSink>,
    ) -> Result<(), LibraryError> {
        self.ensure_folder_enabled(folder_id)?;
        self.claim_scan(folder_id)?;
        if let Err(error) = self.set_folder_status(folder_id, LibraryFolderStatus::Queued, None) {
            self.release_scan(folder_id, None);
            return Err(error);
        }
        emit_progress(
            &sink,
            ScanProgress {
                folder_id,
                status: LibraryFolderStatus::Queued,
                current_file: None,
                processed: 0,
                candidates: 0,
                summary: None,
                started_at: None,
                finished_at: None,
                error: None,
            },
        );

        let service = self.clone();
        let spawn_result = std::thread::Builder::new()
            .name(format!("spotdiy-library-scan-{folder_id}"))
            .spawn(move || {
                let result = service.scan_folder_claimed(folder_id, force, sink.clone(), true);
                if let Err(error) = result {
                    let detail = error.to_string();
                    let _ = service.set_folder_status(
                        folder_id,
                        LibraryFolderStatus::Failed,
                        Some(detail.clone()),
                    );
                    emit_progress(
                        &sink,
                        ScanProgress {
                            folder_id,
                            status: LibraryFolderStatus::Failed,
                            current_file: None,
                            processed: 0,
                            candidates: 0,
                            summary: None,
                            started_at: None,
                            finished_at: Some(Utc::now()),
                            error: Some(detail),
                        },
                    );
                }
                service.release_scan(folder_id, sink);
            });
        match spawn_result {
            Ok(_) => Ok(()),
            Err(error) => {
                self.release_scan(folder_id, None);
                Err(LibraryError::Watcher(format!(
                    "could not start scan thread: {error}"
                )))
            }
        }
    }

    pub fn scan_folder_now(
        &self,
        folder_id: LibraryFolderId,
        force: bool,
        sink: Option<ProgressSink>,
    ) -> Result<ScanSummary, LibraryError> {
        self.ensure_folder_enabled(folder_id)?;
        self.claim_scan(folder_id)?;
        let result = self.scan_folder_claimed(folder_id, force, sink, false);
        self.release_scan(folder_id, None);
        result
    }

    pub fn start_all_scans(&self, sink: Option<ProgressSink>) -> Result<(), LibraryError> {
        self.start_all_scans_with_force(false, sink)
    }

    pub fn rescan_all(&self, sink: Option<ProgressSink>) -> Result<(), LibraryError> {
        self.start_all_scans_with_force(true, sink)
    }

    pub fn rescan_folder(
        &self,
        folder_id: LibraryFolderId,
        sink: Option<ProgressSink>,
    ) -> Result<(), LibraryError> {
        self.ensure_folder_enabled(folder_id)?;
        if !self.watchers.is_registered(folder_id) {
            self.reregister_folder_watcher(folder_id, sink.clone())?;
        }
        self.request_scan(folder_id, true, sink)
    }

    fn start_all_scans_with_force(
        &self,
        force: bool,
        sink: Option<ProgressSink>,
    ) -> Result<(), LibraryError> {
        for folder in self
            .list_folders()?
            .into_iter()
            .filter(|folder| folder.enabled)
        {
            if !is_directory(&folder.path) {
                self.mark_folder_unavailable(folder.id, "Library folder is unavailable")?;
                self.set_folder_status(
                    folder.id,
                    LibraryFolderStatus::Failed,
                    Some("Library folder is unavailable".to_owned()),
                )?;
                continue;
            }
            if force && !self.watchers.is_registered(folder.id) {
                if let Err(error) = self.register_folder_watcher(&folder, sink.clone()) {
                    self.set_folder_status(
                        folder.id,
                        LibraryFolderStatus::Failed,
                        Some(format!("Could not watch library folder: {error}")),
                    )?;
                    continue;
                }
            }
            self.request_scan(folder.id, force, sink.clone())?;
        }
        Ok(())
    }

    pub fn register_watchers(&self, sink: Option<ProgressSink>) -> Result<(), LibraryError> {
        for folder in self
            .list_folders()?
            .into_iter()
            .filter(|folder| folder.enabled)
        {
            if !is_directory(&folder.path) {
                self.mark_folder_unavailable(folder.id, "Library folder is unavailable")?;
                self.set_folder_status(
                    folder.id,
                    LibraryFolderStatus::Failed,
                    Some("Library folder is unavailable".to_owned()),
                )?;
                continue;
            }
            if let Err(error) = self.register_folder_watcher(&folder, sink.clone()) {
                self.set_folder_status(
                    folder.id,
                    LibraryFolderStatus::Failed,
                    Some(format!("Could not watch library folder: {error}")),
                )?;
            }
        }
        Ok(())
    }

    pub fn artwork_path(&self, cache_key: &str) -> Option<PathBuf> {
        if is_safe_cache_key(cache_key) {
            Some(self.artwork_cache.root().join(cache_key))
        } else {
            None
        }
    }

    pub(crate) fn store_artwork(
        &self,
        artwork: &metadata::EmbeddedArtwork,
    ) -> Result<ArtworkCacheEntry, ArtworkError> {
        self.artwork_cache.store(artwork)
    }

    pub fn reveal_path(&self, source_id: SourceId) -> Result<PathBuf, LibraryError> {
        let record: Option<(String, String, String)> =
            self.database.with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT lf.path, lf.normalized_path_key, f.normalized_path_key
                     FROM local_files lf
                     INNER JOIN track_sources ts ON ts.id = lf.source_id
                     INNER JOIN library_folders f ON f.id = lf.library_folder_id
                     WHERE lf.source_id = ?1
                       AND ts.provider_kind = 'local'
                       AND f.enabled = 1",
                        params![source_id.to_string()],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .optional()
            })?;
        let Some((path, stored_path_key, folder_path_key)) = record else {
            return Err(LibraryError::InvalidStoredValue {
                field: "local_files.path",
                value: source_id.to_string(),
            });
        };
        if !is_path_within(&folder_path_key, &stored_path_key) {
            return Err(LibraryError::InvalidStoredValue {
                field: "local_files.normalized_path_key",
                value: stored_path_key,
            });
        }
        let (display_path, actual_path_key) = normalize_file_path(&path)?;
        if !is_path_within(&folder_path_key, &actual_path_key)
            || actual_path_key != stored_path_key
            || !fs::metadata(&display_path).is_ok_and(|metadata| metadata.is_file())
        {
            return Err(LibraryError::InvalidStoredValue {
                field: "local_files.path",
                value: path,
            });
        }
        Ok(display_path)
    }

    pub(crate) fn folder_for_scan(
        &self,
        folder_id: LibraryFolderId,
    ) -> Result<FolderScanInfo, LibraryError> {
        let row: Option<(String, i64)> = self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT path, scan_generation FROM library_folders
                     WHERE id = ?1 AND enabled = 1",
                    params![folder_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
        })?;
        let Some((path, generation)) = row else {
            return Err(LibraryError::FolderNotFound(folder_id));
        };
        Ok(FolderScanInfo {
            id: folder_id,
            filesystem_path: PathBuf::from(path),
            generation: non_negative_u64(generation, "library_folders.scan_generation")?,
        })
    }

    pub(crate) fn find_local_file(
        &self,
        folder_id: LibraryFolderId,
        normalized_path_key: &str,
    ) -> Result<Option<ExistingLocalFile>, LibraryError> {
        let row: Option<RawExistingLocalFile> = self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT lf.source_id, ts.track_id, ts.provider_item_id, ts.available,
                            lf.file_size_bytes, lf.modified_at,
                            lf.index_status
                     FROM local_files lf
                     INNER JOIN track_sources ts ON ts.id = lf.source_id
                     WHERE lf.library_folder_id = ?1 AND lf.normalized_path_key = ?2",
                    params![folder_id.to_string(), normalized_path_key],
                    map_existing_local_file,
                )
                .optional()
        })?;
        row.map(parse_existing_local_file).transpose()
    }

    pub(crate) fn mark_local_file_seen(
        &self,
        folder_id: LibraryFolderId,
        normalized_path_key: &str,
        generation: u64,
        now: DateTime<Utc>,
    ) -> Result<(), LibraryError> {
        let generation = numeric_i64(generation)?;
        self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE local_files
                 SET last_seen_at = ?1,
                     last_seen_generation = ?2,
                     updated_at = ?1
                 WHERE library_folder_id = ?3
                   AND normalized_path_key = ?4
                   AND index_status = 'indexed'",
                params![
                    timestamp(now),
                    generation,
                    folder_id.to_string(),
                    normalized_path_key
                ],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    pub(crate) fn persist_scanned_file(
        &self,
        file: &ScannedFile,
    ) -> Result<UpsertOutcome, LibraryError> {
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        let current =
            query_existing_local_file_tx(&transaction, file.folder_id, &file.normalized_path_key)?;
        let (existing, is_renamed) = if let Some(current) = current {
            (Some(current), false)
        } else {
            let legacy = query_legacy_local_file_tx(&transaction, &file.path)?;
            if legacy.is_some() {
                (legacy, false)
            } else {
                let candidates = query_missing_fingerprint_candidates_tx(
                    &transaction,
                    file.folder_id,
                    &file.fingerprint,
                )?;
                if candidates.len() == 1 {
                    (candidates.into_iter().next(), true)
                } else {
                    (None, false)
                }
            }
        };
        let is_new = existing.is_none();
        let (track_id, source_id) = existing
            .as_ref()
            .map(|existing| (existing.track_id, existing.source_id))
            .unwrap_or_else(|| (TrackId::new(), SourceId::new()));
        let provider_item_id = existing
            .as_ref()
            .map(|existing| existing.provider_item_id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        write_track_aggregate(
            &transaction,
            track_id,
            source_id,
            &provider_item_id,
            file,
            existing.as_ref(),
        )?;
        transaction.commit()?;
        Ok(UpsertOutcome { is_new, is_renamed })
    }

    pub(crate) fn reconcile_missing(
        &self,
        folder_id: LibraryFolderId,
        observed: &HashSet<String>,
        now: DateTime<Utc>,
    ) -> Result<u64, LibraryError> {
        let mut connection = self.database.connection()?;
        let mut statement = connection.prepare(
            "SELECT source_id, normalized_path_key, index_status
             FROM local_files WHERE library_folder_id = ?1",
        )?;
        let rows = statement
            .query_map(params![folder_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);

        let transaction = connection.transaction()?;
        let mut missing = 0_u64;
        for (source_id, normalized_path_key, index_status) in rows {
            let is_observed = normalized_path_key
                .as_deref()
                .is_some_and(|key| observed.contains(key));
            if is_observed || index_status == "missing" {
                continue;
            }
            transaction.execute(
                "UPDATE local_files
                 SET index_status = 'missing',
                     status_detail = 'File was not found during the last scan',
                     last_seen_at = ?1,
                     updated_at = ?1
                 WHERE source_id = ?2",
                params![timestamp(now), source_id],
            )?;
            transaction.execute(
                "UPDATE track_sources
                 SET available = 0,
                     availability_detail = 'Local file was not found during the last scan',
                     updated_at = ?1
                 WHERE id = ?2",
                params![timestamp(now), source_id],
            )?;
            missing += 1;
        }
        transaction.commit()?;
        Ok(missing)
    }

    pub(crate) fn mark_missing_paths_before_scan(
        &self,
        folder_id: LibraryFolderId,
        now: DateTime<Utc>,
    ) -> Result<u64, LibraryError> {
        let rows: Vec<(String, String, String)> = self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT source_id, path, index_status
                 FROM local_files WHERE library_folder_id = ?1",
            )?;
            let values = statement
                .query_map(params![folder_id.to_string()], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<Result<Vec<_>, _>>();
            values
        })?;
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        let mut missing = 0_u64;
        for (source_id, path, index_status) in rows {
            if index_status == "missing" {
                continue;
            }
            let missing_on_disk = match std::fs::metadata(&path) {
                Ok(_) => false,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
                Err(_) => false,
            };
            if !missing_on_disk {
                continue;
            }
            transaction.execute(
                "UPDATE local_files
                 SET index_status = 'missing',
                     status_detail = 'File was not found during the last scan',
                     last_seen_at = ?1,
                     updated_at = ?1
                 WHERE source_id = ?2",
                params![timestamp(now), source_id],
            )?;
            transaction.execute(
                "UPDATE track_sources
                 SET available = 0,
                     availability_detail = 'Local file was not found during the last scan',
                     updated_at = ?1
                 WHERE id = ?2",
                params![timestamp(now), source_id],
            )?;
            missing += 1;
        }
        transaction.commit()?;
        Ok(missing)
    }

    pub(crate) fn set_folder_status(
        &self,
        folder_id: LibraryFolderId,
        status: LibraryFolderStatus,
        error: Option<String>,
    ) -> Result<(), LibraryError> {
        let now = Utc::now();
        self.database.with_connection(|connection| {
            match status {
                LibraryFolderStatus::Queued => connection.execute(
                    "UPDATE library_folders
                     SET scan_status = 'queued', last_scan_error = NULL, updated_at = ?1
                     WHERE id = ?2",
                    params![timestamp(now), folder_id.to_string()],
                ),
                LibraryFolderStatus::Scanning => connection.execute(
                    "UPDATE library_folders
                     SET scan_status = 'scanning', scan_generation = scan_generation + 1,
                         last_scan_started_at = ?1, last_scan_error = NULL, updated_at = ?1
                     WHERE id = ?2",
                    params![timestamp(now), folder_id.to_string()],
                ),
                LibraryFolderStatus::Complete => connection.execute(
                    "UPDATE library_folders
                     SET scan_status = 'complete', last_scan_finished_at = ?1,
                         last_scan_error = ?2, updated_at = ?1
                     WHERE id = ?3",
                    params![timestamp(now), error, folder_id.to_string()],
                ),
                LibraryFolderStatus::Failed => connection.execute(
                    "UPDATE library_folders
                     SET scan_status = 'failed', last_scan_finished_at = ?1,
                         last_scan_error = ?2, updated_at = ?1
                     WHERE id = ?3",
                    params![timestamp(now), error, folder_id.to_string()],
                ),
                LibraryFolderStatus::Idle => connection.execute(
                    "UPDATE library_folders SET scan_status = 'idle', updated_at = ?1 WHERE id = ?2",
                    params![timestamp(now), folder_id.to_string()],
                ),
            }?;
            Ok(())
        })?;
        Ok(())
    }

    pub(crate) fn mark_folder_unavailable(
        &self,
        folder_id: LibraryFolderId,
        detail: &str,
    ) -> Result<(), LibraryError> {
        let now = timestamp(Utc::now());
        self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE track_sources
                 SET available = 0, availability_detail = ?1, updated_at = ?2
                 WHERE id IN (
                     SELECT source_id FROM local_files WHERE library_folder_id = ?3
                 )",
                params![detail, now, folder_id.to_string()],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    fn scan_folder_claimed(
        &self,
        folder_id: LibraryFolderId,
        force: bool,
        sink: Option<ProgressSink>,
        watcher_required: bool,
    ) -> Result<ScanSummary, LibraryError> {
        self.set_folder_status(folder_id, LibraryFolderStatus::Scanning, None)?;
        let result = scanner::scan_folder(self, folder_id, force, sink.clone());
        match &result {
            Ok(summary) => {
                let partial_error = scan_summary_error(summary);
                let watcher_missing = watcher_required && !self.watchers.is_registered(folder_id);
                let watcher_error = if watcher_missing {
                    Some("Library folder watcher is not active".to_owned())
                } else {
                    None
                };
                let error = combine_errors(partial_error, watcher_error);
                let status = if watcher_missing {
                    LibraryFolderStatus::Failed
                } else {
                    LibraryFolderStatus::Complete
                };
                self.set_folder_status(folder_id, status, error.clone())?;
                emit_progress(
                    &sink,
                    ScanProgress {
                        folder_id,
                        status,
                        current_file: None,
                        processed: summary.candidates,
                        candidates: summary.candidates,
                        summary: Some(summary.clone()),
                        started_at: None,
                        finished_at: Some(Utc::now()),
                        error,
                    },
                );
            }
            Err(error) => {
                if matches!(error, LibraryError::Path(_)) {
                    let _ =
                        self.mark_folder_unavailable(folder_id, "Library folder is unavailable");
                }
                self.set_folder_status(
                    folder_id,
                    LibraryFolderStatus::Failed,
                    Some(error.to_string()),
                )?;
            }
        }
        result
    }

    fn ensure_folder_enabled(&self, folder_id: LibraryFolderId) -> Result<(), LibraryError> {
        let enabled: Option<i64> = self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT enabled FROM library_folders WHERE id = ?1",
                    params![folder_id.to_string()],
                    |row| row.get(0),
                )
                .optional()
        })?;
        if enabled == Some(1) {
            Ok(())
        } else {
            Err(LibraryError::FolderNotFound(folder_id))
        }
    }

    pub fn request_scan(
        &self,
        folder_id: LibraryFolderId,
        force: bool,
        sink: Option<ProgressSink>,
    ) -> Result<(), LibraryError> {
        self.ensure_folder_enabled(folder_id)?;
        loop {
            let is_active = self
                .scan_state
                .lock()
                .map_err(|_| {
                    LibraryError::Watcher("library scan state lock is poisoned".to_owned())
                })?
                .active
                .contains(&folder_id);
            if is_active {
                self.queue_pending_scan(folder_id, force, sink);
                return Ok(());
            }
            match self.start_scan(folder_id, force, sink.clone()) {
                Ok(()) => return Ok(()),
                Err(LibraryError::ScanAlreadyRunning(_)) => continue,
                Err(error) => return Err(error),
            }
        }
    }

    fn queue_pending_scan(
        &self,
        folder_id: LibraryFolderId,
        force: bool,
        sink: Option<ProgressSink>,
    ) {
        if let Ok(mut state) = self.scan_state.lock() {
            let pending = state.pending.entry(folder_id).or_insert(PendingScan {
                force: false,
                sink: None,
            });
            pending.force |= force;
            if pending.sink.is_none() {
                pending.sink = sink;
            }
        }
    }

    fn claim_scan(&self, folder_id: LibraryFolderId) -> Result<(), LibraryError> {
        let mut state = self
            .scan_state
            .lock()
            .map_err(|_| LibraryError::Watcher("library scan state lock is poisoned".to_owned()))?;
        if !state.active.insert(folder_id) {
            return Err(LibraryError::ScanAlreadyRunning(folder_id));
        }
        Ok(())
    }

    fn release_scan(&self, folder_id: LibraryFolderId, sink: Option<ProgressSink>) {
        let pending = self.scan_state.lock().ok().and_then(|mut state| {
            state.active.remove(&folder_id);
            state.pending.remove(&folder_id)
        });
        if let Some(pending) = pending {
            let pending_sink = pending.sink.or(sink);
            let _ = self.start_scan(folder_id, pending.force, pending_sink);
        }
    }

    fn register_folder_watcher(
        &self,
        folder: &LibraryFolder,
        sink: Option<ProgressSink>,
    ) -> Result<(), LibraryError> {
        let handler = self.watch_handler(sink);
        self.watchers
            .register(folder.id, &folder.path, handler)
            .map_err(|error| LibraryError::Watcher(error.to_string()))
    }

    fn reregister_folder_watcher(
        &self,
        folder_id: LibraryFolderId,
        sink: Option<ProgressSink>,
    ) -> Result<(), LibraryError> {
        let folder = self
            .list_folders()?
            .into_iter()
            .find(|folder| folder.id == folder_id)
            .ok_or(LibraryError::FolderNotFound(folder_id))?;
        if !folder.enabled || !is_directory(&folder.path) {
            return Err(LibraryError::FolderNotFound(folder_id));
        }
        self.register_folder_watcher(&folder, sink)
    }

    fn watch_handler(&self, sink: Option<ProgressSink>) -> watcher::WatchActionHandler {
        let service = self.clone();
        Arc::new(move |folder_id, actions| {
            if actions.is_empty() {
                return;
            }
            let force = actions.iter().any(|action| {
                matches!(
                    action,
                    watcher::WatchAction::Create(_)
                        | watcher::WatchAction::Modify(_)
                        | watcher::WatchAction::Rename { .. }
                        | watcher::WatchAction::Reconcile
                        | watcher::WatchAction::WatcherFailure
                )
            });
            if actions
                .iter()
                .any(|action| matches!(action, watcher::WatchAction::WatcherFailure))
            {
                if let Err(error) = service.reregister_folder_watcher(folder_id, sink.clone()) {
                    let _ = service.set_folder_status(
                        folder_id,
                        LibraryFolderStatus::Failed,
                        Some(format!("Could not recover library folder watcher: {error}")),
                    );
                }
            }
            let _ = service.request_scan(folder_id, force, sink.clone());
        })
    }

    fn recover_interrupted_scans(&self) -> Result<(), LibraryError> {
        self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE library_folders
                 SET scan_status = 'failed',
                     last_scan_error = 'Scan interrupted before the application restarted',
                     updated_at = ?1
                 WHERE scan_status IN ('queued', 'scanning')",
                params![timestamp(Utc::now())],
            )?;
            Ok(())
        })?;
        Ok(())
    }
}

fn emit_progress(sink: &Option<ProgressSink>, progress: ScanProgress) {
    if let Some(sink) = sink {
        sink(progress);
    }
}

fn scan_summary_error(summary: &ScanSummary) -> Option<String> {
    let mut issues = Vec::new();
    if summary.missing_files > 0 {
        issues.push(format!("{} missing file(s)", summary.missing_files));
    }
    if summary.metadata_failures > 0 {
        issues.push(format!(
            "{} metadata/I/O failure(s)",
            summary.metadata_failures
        ));
    }
    if summary.artwork_failures > 0 {
        issues.push(format!("{} artwork failure(s)", summary.artwork_failures));
    }
    if summary.database_failures > 0 {
        issues.push(format!("{} database failure(s)", summary.database_failures));
    }
    (!issues.is_empty())
        .then(|| format!("Scan completed with partial errors: {}.", issues.join(", ")))
}

fn combine_errors(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}; {second}")),
        (Some(error), None) | (None, Some(error)) => Some(error),
        (None, None) => None,
    }
}

fn is_directory(path: &std::path::Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

fn list_folders(database: &Database) -> Result<Vec<LibraryFolder>, LibraryError> {
    let connection = database.connection()?;
    let mut statement = connection.prepare(
        "SELECT f.id, f.path, f.normalized_path_key, f.enabled, f.scan_status,
                f.scan_generation, f.last_scan_started_at, f.last_scan_finished_at,
                f.last_scan_error, f.created_at, f.updated_at,
                COUNT(lf.source_id),
                SUM(CASE WHEN lf.index_status = 'indexed' THEN 1 ELSE 0 END)
         FROM library_folders f
         LEFT JOIN local_files lf ON lf.library_folder_id = f.id
         GROUP BY f.id
         ORDER BY f.normalized_path_key COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], map_folder_row)?;
    rows.map(|row| row.map_err(LibraryError::from).and_then(parse_folder_row))
        .collect()
}

fn insert_folders(
    database: &Database,
    folders: &[NormalizedFolderPath],
) -> Result<(), LibraryError> {
    let mut connection = database.connection()?;
    let transaction = connection.transaction()?;
    let now = timestamp(Utc::now());
    for folder in folders {
        transaction.execute(
            "INSERT INTO library_folders (
                id, path, normalized_path_key, enabled, scan_status, scan_generation,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, 1, 'idle', 0, ?4, ?4)",
            params![
                LibraryFolderId::new().to_string(),
                folder.display_path.to_string_lossy().into_owned(),
                folder.normalized_path_key,
                now,
            ],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

#[derive(Clone, Debug)]
struct RawFolderRow {
    id: String,
    path: String,
    normalized_path_key: String,
    enabled: i64,
    scan_status: String,
    scan_generation: i64,
    last_scan_started_at: Option<String>,
    last_scan_finished_at: Option<String>,
    last_scan_error: Option<String>,
    created_at: String,
    updated_at: String,
    file_count: i64,
    indexed_track_count: Option<i64>,
}

fn map_folder_row(row: &Row<'_>) -> rusqlite::Result<RawFolderRow> {
    Ok(RawFolderRow {
        id: row.get(0)?,
        path: row.get(1)?,
        normalized_path_key: row.get(2)?,
        enabled: row.get(3)?,
        scan_status: row.get(4)?,
        scan_generation: row.get(5)?,
        last_scan_started_at: row.get(6)?,
        last_scan_finished_at: row.get(7)?,
        last_scan_error: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        file_count: row.get(11)?,
        indexed_track_count: row.get(12)?,
    })
}

fn parse_folder_row(row: RawFolderRow) -> Result<LibraryFolder, LibraryError> {
    Ok(LibraryFolder {
        id: parse_id(&row.id, "library_folders.id")?,
        path: PathBuf::from(row.path),
        normalized_path_key: row.normalized_path_key,
        enabled: bool_from_integer(row.enabled, "library_folders.enabled")?,
        status: parse_folder_status(&row.scan_status)?,
        scan_generation: non_negative_u64(row.scan_generation, "library_folders.scan_generation")?,
        last_scan_started_at: row
            .last_scan_started_at
            .map(|value| parse_timestamp(&value, "library_folders.last_scan_started_at"))
            .transpose()?,
        last_scan_finished_at: row
            .last_scan_finished_at
            .map(|value| parse_timestamp(&value, "library_folders.last_scan_finished_at"))
            .transpose()?,
        last_scan_error: row.last_scan_error,
        file_count: non_negative_u64(row.file_count, "library_folders.file_count")?,
        indexed_track_count: non_negative_u64(
            row.indexed_track_count.unwrap_or_default(),
            "library_folders.indexed_track_count",
        )?,
        created_at: parse_timestamp(&row.created_at, "library_folders.created_at")?,
        updated_at: parse_timestamp(&row.updated_at, "library_folders.updated_at")?,
    })
}

fn parse_folder_status(value: &str) -> Result<LibraryFolderStatus, LibraryError> {
    match value {
        "idle" => Ok(LibraryFolderStatus::Idle),
        "queued" => Ok(LibraryFolderStatus::Queued),
        "scanning" => Ok(LibraryFolderStatus::Scanning),
        "complete" => Ok(LibraryFolderStatus::Complete),
        "failed" => Ok(LibraryFolderStatus::Failed),
        value => Err(LibraryError::InvalidStoredValue {
            field: "library_folders.scan_status",
            value: value.to_owned(),
        }),
    }
}

#[derive(Clone, Debug)]
struct RawExistingLocalFile {
    source_id: String,
    track_id: String,
    provider_item_id: String,
    available: i64,
    file_size_bytes: Option<i64>,
    modified_at: Option<String>,
    index_status: String,
}

fn map_existing_local_file(row: &Row<'_>) -> rusqlite::Result<RawExistingLocalFile> {
    Ok(RawExistingLocalFile {
        source_id: row.get(0)?,
        track_id: row.get(1)?,
        provider_item_id: row.get(2)?,
        available: row.get(3)?,
        file_size_bytes: row.get(4)?,
        modified_at: row.get(5)?,
        index_status: row.get(6)?,
    })
}

fn parse_existing_local_file(row: RawExistingLocalFile) -> Result<ExistingLocalFile, LibraryError> {
    Ok(ExistingLocalFile {
        source_id: parse_id(&row.source_id, "local_files.source_id")?,
        track_id: parse_id(&row.track_id, "track_sources.track_id")?,
        provider_item_id: row.provider_item_id,
        available: bool_from_integer(row.available, "track_sources.available")?,
        file_size_bytes: row
            .file_size_bytes
            .map(|value| non_negative_u64(value, "local_files.file_size_bytes"))
            .transpose()?,
        modified_at: row
            .modified_at
            .map(|value| parse_timestamp(&value, "local_files.modified_at"))
            .transpose()?,
        index_status: parse_index_status(&row.index_status)?,
    })
}

fn query_existing_local_file_tx(
    connection: &Connection,
    folder_id: LibraryFolderId,
    normalized_path_key: &str,
) -> Result<Option<ExistingLocalFile>, LibraryError> {
    connection
        .query_row(
            "SELECT lf.source_id, ts.track_id, ts.provider_item_id, ts.available,
                    lf.file_size_bytes, lf.modified_at,
                    lf.index_status
             FROM local_files lf
             INNER JOIN track_sources ts ON ts.id = lf.source_id
             WHERE lf.library_folder_id = ?1 AND lf.normalized_path_key = ?2",
            params![folder_id.to_string(), normalized_path_key],
            map_existing_local_file,
        )
        .optional()?
        .map(parse_existing_local_file)
        .transpose()
}

fn query_missing_fingerprint_candidates_tx(
    connection: &Connection,
    folder_id: LibraryFolderId,
    fingerprint: &str,
) -> Result<Vec<ExistingLocalFile>, LibraryError> {
    let mut statement = connection.prepare(
        "SELECT lf.source_id, ts.track_id, ts.provider_item_id, ts.available,
                lf.file_size_bytes, lf.modified_at,
                lf.index_status
         FROM local_files lf
         INNER JOIN track_sources ts ON ts.id = lf.source_id
         WHERE lf.library_folder_id = ?1
           AND lf.content_fingerprint = ?2
           AND lf.index_status = 'missing'
         ORDER BY lf.normalized_path_key, lf.source_id",
    )?;
    let rows = statement.query_map(
        params![folder_id.to_string(), fingerprint],
        map_existing_local_file,
    )?;
    rows.map(|row| {
        row.map_err(LibraryError::from)
            .and_then(parse_existing_local_file)
    })
    .collect()
}

fn query_legacy_local_file_tx(
    connection: &Connection,
    path: &std::path::Path,
) -> Result<Option<ExistingLocalFile>, LibraryError> {
    let path = path.to_string_lossy();
    connection
        .query_row(
            "SELECT lf.source_id, ts.track_id, ts.provider_item_id, ts.available,
                    lf.file_size_bytes, lf.modified_at,
                    lf.index_status
             FROM local_files lf
             INNER JOIN track_sources ts ON ts.id = lf.source_id
             WHERE lf.library_folder_id IS NULL
               AND ts.provider_kind = 'local'
               AND lf.path = ?1 COLLATE NOCASE",
            params![path.as_ref()],
            map_existing_local_file,
        )
        .optional()?
        .map(parse_existing_local_file)
        .transpose()
}

fn write_track_aggregate(
    transaction: &rusqlite::Transaction<'_>,
    track_id: TrackId,
    source_id: SourceId,
    provider_item_id: &str,
    file: &ScannedFile,
    existing: Option<&ExistingLocalFile>,
) -> Result<(), LibraryError> {
    let now = timestamp(file.now);
    let album_id = if let Some(album_title) = &file.metadata.album {
        let existing_id: Option<String> = transaction
            .query_row(
                "SELECT id FROM albums WHERE title = ?1 COLLATE NOCASE ORDER BY rowid LIMIT 1",
                params![album_title],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(existing_id) = existing_id {
            Some(existing_id)
        } else {
            let album_id = AlbumId::new().to_string();
            transaction.execute(
                "INSERT INTO albums (id, title, release_date, created_at, updated_at)
                 VALUES (?1, ?2, NULL, ?3, ?3)",
                params![album_id, album_title, now],
            )?;
            Some(album_id)
        }
    } else {
        None
    };

    if existing.is_some() {
        transaction.execute(
            "UPDATE tracks SET title = ?1, normalized_title = ?2, album_id = ?3,
                    duration_ms = ?4, updated_at = ?5 WHERE id = ?6",
            params![
                file.metadata.title,
                normalize_title(&file.metadata.title),
                album_id.as_deref(),
                file.metadata.duration_ms.map(numeric_i64).transpose()?,
                now,
                track_id.to_string(),
            ],
        )?;
        transaction.execute(
            "DELETE FROM track_artists WHERE track_id = ?1",
            params![track_id.to_string()],
        )?;
    } else {
        transaction.execute(
            "INSERT INTO tracks (
                id, title, normalized_title, album_id, duration_ms,
                version_qualifiers_json, preferred_source_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, '[\"standard\"]', NULL, ?6, ?6)",
            params![
                track_id.to_string(),
                file.metadata.title,
                normalize_title(&file.metadata.title),
                album_id.as_deref(),
                file.metadata.duration_ms.map(numeric_i64).transpose()?,
                now,
            ],
        )?;
    }

    for (artist_order, artist_name) in file.metadata.artists.iter().enumerate() {
        let artist_id: Option<String> = transaction
            .query_row(
                "SELECT id FROM artists WHERE name = ?1 COLLATE NOCASE ORDER BY rowid LIMIT 1",
                params![artist_name],
                |row| row.get(0),
            )
            .optional()?;
        let artist_id = artist_id.unwrap_or_else(|| ArtistId::new().to_string());
        transaction.execute(
            "INSERT INTO artists (id, name, sort_name, created_at, updated_at)
             VALUES (?1, ?2, NULL, ?3, ?3)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, updated_at = excluded.updated_at",
            params![artist_id, artist_name, now],
        )?;
        transaction.execute(
            "INSERT INTO track_artists (track_id, artist_id, artist_order, role)
             VALUES (?1, ?2, ?3, 'primary')",
            params![track_id.to_string(), artist_id, artist_order as i64],
        )?;
    }

    if existing.is_none() {
        transaction.execute(
            "INSERT INTO track_sources (
                id, track_id, provider_kind, provider_item_id, source_uri, duration_ms,
                version_qualifiers_json, available, availability_detail, can_search,
                can_metadata, can_artwork, can_playback, can_lyrics, can_downloads,
                can_popularity, can_release_date, can_lyrics_metadata, created_at, updated_at
             ) VALUES (?1, ?2, 'local', ?3, NULL, ?4, '[\"standard\"]', ?5, ?6,
                       0, 1, 1, 1, 0, 0, 0, 0, 0, ?7, ?7)",
            params![
                source_id.to_string(),
                track_id.to_string(),
                provider_item_id,
                file.metadata.duration_ms.map(numeric_i64).transpose()?,
                bool_integer(file.index_status == LocalFileIndexStatus::Indexed),
                file.status_detail,
                now,
            ],
        )?;
    } else {
        transaction.execute(
            "UPDATE track_sources
             SET duration_ms = ?1, available = ?2, availability_detail = ?3, updated_at = ?4
             WHERE id = ?5",
            params![
                file.metadata.duration_ms.map(numeric_i64).transpose()?,
                bool_integer(file.index_status == LocalFileIndexStatus::Indexed),
                file.status_detail,
                now,
                source_id.to_string(),
            ],
        )?;
    }

    transaction.execute(
        "INSERT INTO local_files (
            source_id, path, file_size_bytes, modified_at, content_fingerprint,
            codec, bitrate_kbps, sample_rate_hz, bit_depth, created_at, updated_at,
            library_folder_id, normalized_path_key, container, index_status, status_detail,
            last_seen_at, last_indexed_at, last_seen_generation, artwork_cache_key,
            artwork_mime_type
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10,
                   ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
         ON CONFLICT(source_id) DO UPDATE SET
            path = excluded.path,
            file_size_bytes = excluded.file_size_bytes,
            modified_at = excluded.modified_at,
            content_fingerprint = excluded.content_fingerprint,
            codec = excluded.codec,
            bitrate_kbps = excluded.bitrate_kbps,
            sample_rate_hz = excluded.sample_rate_hz,
            bit_depth = excluded.bit_depth,
            updated_at = excluded.updated_at,
            library_folder_id = excluded.library_folder_id,
            normalized_path_key = excluded.normalized_path_key,
            container = excluded.container,
            index_status = excluded.index_status,
            status_detail = excluded.status_detail,
            last_seen_at = excluded.last_seen_at,
            last_indexed_at = excluded.last_indexed_at,
            last_seen_generation = excluded.last_seen_generation,
            artwork_cache_key = excluded.artwork_cache_key,
            artwork_mime_type = excluded.artwork_mime_type",
        params![
            source_id.to_string(),
            file.path.to_string_lossy().into_owned(),
            numeric_i64(file.file_size_bytes)?,
            file.modified_at.map(timestamp),
            file.fingerprint,
            file.metadata.codec,
            file.metadata.bitrate_kbps.map(numeric_i64).transpose()?,
            file.metadata.sample_rate_hz.map(numeric_i64).transpose()?,
            file.metadata
                .bit_depth
                .map(|value| numeric_i64(u64::from(value)))
                .transpose()?,
            now,
            file.folder_id.to_string(),
            file.normalized_path_key,
            file.metadata.container,
            index_status(file.index_status),
            file.status_detail,
            timestamp(file.now),
            (file.index_status == LocalFileIndexStatus::Indexed).then_some(now.clone()),
            numeric_i64(file.generation)?,
            file.artwork.as_ref().map(|value| value.cache_key.clone()),
            file.artwork.as_ref().map(|value| value.mime_type.clone()),
        ],
    )?;
    Ok(())
}

fn load_library_page(
    database: &Database,
    request: LibraryPageRequest,
) -> Result<LibraryPage, LibraryError> {
    if request.page_size == 0 || request.page_size > MAX_PAGE_SIZE {
        return Err(LibraryError::InvalidPageSize);
    }
    let page_size = i64::from(request.page_size);
    let offset = i64::from(request.page)
        .checked_mul(page_size)
        .ok_or(LibraryError::InvalidPageSize)?;
    let connection = database.connection()?;
    let (where_clause, folder_param): (&str, Option<String>) =
        if let Some(folder_id) = request.folder_id {
            ("AND lf.library_folder_id = ?1", Some(folder_id.to_string()))
        } else {
            ("", None)
        };
    let total_sql = format!(
        "SELECT COUNT(*) FROM local_files lf
         INNER JOIN track_sources ts ON ts.id = lf.source_id
         WHERE lf.library_folder_id IS NOT NULL
           AND lf.index_status <> 'pending' {where_clause}"
    );
    let total: i64 = match folder_param.as_deref() {
        Some(folder_id) => {
            connection.query_row(&total_sql, params![folder_id], |row| row.get(0))?
        }
        None => connection.query_row(&total_sql, [], |row| row.get(0))?,
    };

    let sort_column = match request.sort {
        LibrarySort::Title => "t.normalized_title",
        LibrarySort::Artist => {
            "COALESCE((SELECT lower(a.name) FROM artists a
            INNER JOIN track_artists ta ON ta.artist_id = a.id
            WHERE ta.track_id = t.id ORDER BY ta.artist_order LIMIT 1), '')"
        }
        LibrarySort::DateAdded => "lf.created_at",
        LibrarySort::DateModified => "COALESCE(lf.modified_at, '')",
    };
    let direction = if request.descending { "DESC" } else { "ASC" };
    let page_sql = format!(
        "SELECT t.id, ts.id, lf.library_folder_id, t.title,
                COALESCE((SELECT json_group_array(a.name) FROM artists a
                    INNER JOIN track_artists ta ON ta.artist_id = a.id
                    WHERE ta.track_id = t.id ORDER BY ta.artist_order), '[]'),
                al.title, t.duration_ms, lf.path, ts.available, ts.availability_detail,
                lf.index_status, lf.status_detail, lf.file_size_bytes, lf.modified_at,
                lf.codec, lf.container, lf.bitrate_kbps, lf.sample_rate_hz, lf.bit_depth,
                lf.content_fingerprint, lf.artwork_cache_key, lf.artwork_mime_type,
                t.created_at, t.updated_at
         FROM local_files lf
         INNER JOIN track_sources ts ON ts.id = lf.source_id
         INNER JOIN tracks t ON t.id = ts.track_id
         LEFT JOIN albums al ON al.id = t.album_id
         WHERE lf.library_folder_id IS NOT NULL
           AND lf.index_status <> 'pending' {where_clause}
         ORDER BY {sort_column} {direction}, lf.normalized_path_key COLLATE NOCASE ASC, ts.id ASC
         LIMIT ?{limit_index} OFFSET ?{offset_index}",
        limit_index = if folder_param.is_some() { 2 } else { 1 },
        offset_index = if folder_param.is_some() { 3 } else { 2 },
    );
    let mut statement = connection.prepare(&page_sql)?;
    let rows = if let Some(folder_id) = folder_param {
        statement.query_map(params![folder_id, page_size, offset], map_library_track_row)?
    } else {
        statement.query_map(params![page_size, offset], map_library_track_row)?
    };
    let mut items = Vec::new();
    for row in rows {
        items.push(parse_library_track(row?)?);
    }
    let total = non_negative_u64(total, "library_page.total")?;
    Ok(LibraryPage {
        items,
        page: request.page,
        page_size: request.page_size,
        total,
        has_next: offset.saturating_add(i64::from(request.page_size))
            < i64::try_from(total).unwrap_or(i64::MAX),
        sort: request.sort,
        descending: request.descending,
    })
}

#[derive(Clone, Debug)]
struct RawLibraryTrackRow {
    track_id: String,
    source_id: String,
    folder_id: String,
    title: String,
    artists_json: String,
    album: Option<String>,
    duration_ms: Option<i64>,
    path: String,
    available: i64,
    availability_detail: Option<String>,
    index_status: String,
    status_detail: Option<String>,
    file_size_bytes: Option<i64>,
    modified_at: Option<String>,
    codec: Option<String>,
    container: Option<String>,
    bitrate_kbps: Option<i64>,
    sample_rate_hz: Option<i64>,
    bit_depth: Option<i64>,
    content_fingerprint: Option<String>,
    artwork_cache_key: Option<String>,
    artwork_mime_type: Option<String>,
    created_at: String,
    updated_at: String,
}

fn map_library_track_row(row: &Row<'_>) -> rusqlite::Result<RawLibraryTrackRow> {
    Ok(RawLibraryTrackRow {
        track_id: row.get(0)?,
        source_id: row.get(1)?,
        folder_id: row.get(2)?,
        title: row.get(3)?,
        artists_json: row.get(4)?,
        album: row.get(5)?,
        duration_ms: row.get(6)?,
        path: row.get(7)?,
        available: row.get(8)?,
        availability_detail: row.get(9)?,
        index_status: row.get(10)?,
        status_detail: row.get(11)?,
        file_size_bytes: row.get(12)?,
        modified_at: row.get(13)?,
        codec: row.get(14)?,
        container: row.get(15)?,
        bitrate_kbps: row.get(16)?,
        sample_rate_hz: row.get(17)?,
        bit_depth: row.get(18)?,
        content_fingerprint: row.get(19)?,
        artwork_cache_key: row.get(20)?,
        artwork_mime_type: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
    })
}

fn parse_library_track(row: RawLibraryTrackRow) -> Result<LibraryTrack, LibraryError> {
    let artists: Vec<String> =
        serde_json::from_str(&row.artists_json).map_err(|_| LibraryError::InvalidStoredValue {
            field: "library_page.artists",
            value: row.artists_json.clone(),
        })?;
    Ok(LibraryTrack {
        track_id: parse_id(&row.track_id, "tracks.id")?,
        source_id: parse_id(&row.source_id, "track_sources.id")?,
        folder_id: parse_id(&row.folder_id, "local_files.library_folder_id")?,
        title: row.title,
        artists,
        album: row.album,
        duration_ms: row
            .duration_ms
            .map(|value| non_negative_u64(value, "tracks.duration_ms"))
            .transpose()?,
        path: PathBuf::from(row.path),
        available: bool_from_integer(row.available, "track_sources.available")?,
        availability_detail: row.availability_detail,
        index_status: parse_index_status(&row.index_status)?,
        status_detail: row.status_detail,
        file_size_bytes: row
            .file_size_bytes
            .map(|value| non_negative_u64(value, "local_files.file_size_bytes"))
            .transpose()?,
        modified_at: row
            .modified_at
            .map(|value| parse_timestamp(&value, "local_files.modified_at"))
            .transpose()?,
        codec: row.codec,
        container: row.container,
        bitrate_kbps: row
            .bitrate_kbps
            .map(|value| non_negative_u64(value, "local_files.bitrate_kbps"))
            .transpose()?,
        sample_rate_hz: row
            .sample_rate_hz
            .map(|value| non_negative_u64(value, "local_files.sample_rate_hz"))
            .transpose()?,
        bit_depth: row
            .bit_depth
            .map(|value| {
                u16::try_from(value).map_err(|_| LibraryError::InvalidStoredValue {
                    field: "local_files.bit_depth",
                    value: value.to_string(),
                })
            })
            .transpose()?,
        content_fingerprint: row.content_fingerprint,
        artwork_cache_key: row.artwork_cache_key,
        artwork_mime_type: row.artwork_mime_type,
        artwork_path: None,
        created_at: parse_timestamp(&row.created_at, "tracks.created_at")?,
        updated_at: parse_timestamp(&row.updated_at, "tracks.updated_at")?,
    })
}

fn index_status(value: LocalFileIndexStatus) -> &'static str {
    match value {
        LocalFileIndexStatus::Pending => "pending",
        LocalFileIndexStatus::Indexed => "indexed",
        LocalFileIndexStatus::Missing => "missing",
        LocalFileIndexStatus::Error => "error",
    }
}

fn parse_index_status(value: &str) -> Result<LocalFileIndexStatus, LibraryError> {
    match value {
        "pending" => Ok(LocalFileIndexStatus::Pending),
        "indexed" => Ok(LocalFileIndexStatus::Indexed),
        "missing" => Ok(LocalFileIndexStatus::Missing),
        "error" => Ok(LocalFileIndexStatus::Error),
        value => Err(LibraryError::InvalidStoredValue {
            field: "local_files.index_status",
            value: value.to_owned(),
        }),
    }
}

fn normalize_title(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn is_safe_cache_key(value: &str) -> bool {
    let Some((hash, extension)) = value.rsplit_once('.') else {
        return false;
    };
    hash.len() == 64
        && hash.chars().all(|character| character.is_ascii_hexdigit())
        && matches!(extension, "jpg" | "png" | "gif" | "bmp" | "tif")
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

fn parse_timestamp(value: &str, field: &'static str) -> Result<DateTime<Utc>, LibraryError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| LibraryError::InvalidStoredValue {
            field,
            value: value.to_owned(),
        })
}

fn numeric_i64(value: u64) -> Result<i64, LibraryError> {
    i64::try_from(value).map_err(|_| LibraryError::InvalidStoredValue {
        field: "library.numeric",
        value: value.to_string(),
    })
}

fn non_negative_u64(value: i64, field: &'static str) -> Result<u64, LibraryError> {
    u64::try_from(value).map_err(|_| LibraryError::InvalidStoredValue {
        field,
        value: value.to_string(),
    })
}

fn bool_integer(value: bool) -> i64 {
    i64::from(value)
}

fn bool_from_integer(value: i64, field: &'static str) -> Result<bool, LibraryError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(LibraryError::InvalidStoredValue {
            field,
            value: value.to_string(),
        }),
    }
}

fn parse_id<T>(value: &str, field: &'static str) -> Result<T, LibraryError>
where
    T: LibraryId,
{
    T::parse(value).map_err(|_| LibraryError::InvalidStoredValue {
        field,
        value: value.to_owned(),
    })
}

trait LibraryId: Sized {
    fn parse(value: &str) -> Result<Self, uuid::Error>;
}

impl LibraryId for TrackId {
    fn parse(value: &str) -> Result<Self, uuid::Error> {
        TrackId::parse_str(value)
    }
}

impl LibraryId for SourceId {
    fn parse(value: &str) -> Result<Self, uuid::Error> {
        SourceId::parse_str(value)
    }
}

impl LibraryId for LibraryFolderId {
    fn parse(value: &str) -> Result<Self, uuid::Error> {
        LibraryFolderId::parse_str(value)
    }
}

impl LibraryId for AlbumId {
    fn parse(value: &str) -> Result<Self, uuid::Error> {
        AlbumId::parse_str(value)
    }
}

impl LibraryId for ArtistId {
    fn parse(value: &str) -> Result<Self, uuid::Error> {
        ArtistId::parse_str(value)
    }
}

pub(crate) fn system_time_to_utc(value: SystemTime) -> Option<DateTime<Utc>> {
    Some(DateTime::<Utc>::from(value))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::db::{Database, TempDatabasePath};

    #[test]
    fn cache_keys_reject_path_traversal() {
        assert!(is_safe_cache_key(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.jpg"
        ));
        assert!(!is_safe_cache_key("..\\secret.jpg"));
        assert!(!is_safe_cache_key("C:\\secret.jpg"));
    }

    #[test]
    fn service_persists_and_lists_a_valid_folder() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(TempDatabasePath::new("library-folder").path()).unwrap();
        let service = LibraryService::new(database, directory.path().join("artwork")).unwrap();

        let folders = service
            .add_folders(vec![PathBuf::from(directory.path())])
            .unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].file_count, 0);
        assert_eq!(service.list_folders().unwrap()[0].id, folders[0].id);
    }

    #[test]
    fn legacy_plan_two_local_rows_are_excluded_from_managed_library_reads() {
        let database_path = TempDatabasePath::new("library-legacy");
        let database = Database::open(database_path.path()).unwrap();
        let track_id = TrackId::new();
        let source_id = SourceId::new();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO tracks (
                        id, title, normalized_title, created_at, updated_at
                     ) VALUES (?1, 'Legacy', 'legacy', ?2, ?2)",
                    params![track_id.to_string(), "2026-01-01T00:00:00Z"],
                )?;
                connection.execute(
                    "INSERT INTO track_sources (
                        id, track_id, provider_kind, provider_item_id, created_at, updated_at
                     ) VALUES (?1, ?2, 'local', ?3, ?4, ?4)",
                    params![
                        source_id.to_string(),
                        track_id.to_string(),
                        r"C:\Music\legacy.flac",
                        "2026-01-01T00:00:00Z"
                    ],
                )?;
                connection.execute(
                    "INSERT INTO local_files (source_id, path, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?3)",
                    params![
                        source_id.to_string(),
                        r"C:\Music\legacy.flac",
                        "2026-01-01T00:00:00Z"
                    ],
                )?;
                Ok(())
            })
            .unwrap();

        let artwork = tempfile::tempdir().unwrap();
        let service = LibraryService::new(database, artwork.path()).unwrap();
        let status = service.status().unwrap();
        assert_eq!(status.indexed_track_count, 0);
        assert_eq!(status.available_track_count, 0);
        let page = service.page(LibraryPageRequest::default()).unwrap();
        assert_eq!(page.total, 0);
        assert!(page.items.is_empty());
    }

    #[test]
    fn scan_promotes_a_matching_legacy_local_row_without_changing_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy.wav");
        fs::write(&path, minimal_wav()).unwrap();
        let display_path = folders::normalize_file_path(&path).unwrap().0;

        let database =
            Database::open(TempDatabasePath::new("library-legacy-promotion").path()).unwrap();
        let track_id = TrackId::new();
        let source_id = SourceId::new();
        let provider_item_id = "legacy-local-stable-id";
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO tracks (
                        id, title, normalized_title, created_at, updated_at
                     ) VALUES (?1, 'Legacy', 'legacy', ?2, ?2)",
                    params![track_id.to_string(), "2026-01-01T00:00:00Z"],
                )?;
                connection.execute(
                    "INSERT INTO track_sources (
                        id, track_id, provider_kind, provider_item_id, created_at, updated_at
                     ) VALUES (?1, ?2, 'local', ?3, ?4, ?4)",
                    params![
                        source_id.to_string(),
                        track_id.to_string(),
                        provider_item_id,
                        "2026-01-01T00:00:00Z"
                    ],
                )?;
                connection.execute(
                    "INSERT INTO local_files (source_id, path, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?3)",
                    params![
                        source_id.to_string(),
                        display_path.to_string_lossy().into_owned(),
                        "2026-01-01T00:00:00Z"
                    ],
                )?;
                Ok(())
            })
            .unwrap();

        let artwork = tempfile::tempdir().unwrap();
        let service = LibraryService::new(database.clone(), artwork.path()).unwrap();
        let folder = service
            .add_folders(vec![directory.path().to_path_buf()])
            .unwrap()
            .remove(0);
        service.scan_folder_now(folder.id, false, None).unwrap();

        let page = service.page(LibraryPageRequest::default()).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].track_id, track_id);
        assert_eq!(page.items[0].source_id, source_id);
        assert_eq!(page.items[0].folder_id, folder.id);
        let persisted_provider_item_id: String = database
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT provider_item_id FROM track_sources WHERE id = ?1",
                    params![source_id.to_string()],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(persisted_provider_item_id, provider_item_id);
    }

    #[test]
    fn scan_persists_tracks_skips_unchanged_and_reconciles_rename_missing_restore() {
        let directory = tempfile::tempdir().unwrap();
        let first_path = directory.path().join("first.wav");
        let second_path = directory.path().join("second.wav");
        fs::write(&first_path, minimal_wav()).unwrap();
        fs::write(&second_path, minimal_wav()).unwrap();
        let first_display_path = folders::normalize_file_path(&first_path).unwrap().0;
        let second_display_path = folders::normalize_file_path(&second_path).unwrap().0;

        let database_path = TempDatabasePath::new("library-scan");
        let database = Database::open(database_path.path()).unwrap();
        let service =
            LibraryService::new(database.clone(), directory.path().join("artwork")).unwrap();
        let folder = service
            .add_folders(vec![directory.path().to_path_buf()])
            .unwrap()
            .remove(0);

        let first_scan = service.scan_folder_now(folder.id, false, None).unwrap();
        assert_eq!(first_scan.candidates, 2);
        assert_eq!(first_scan.new_files, 2);
        assert_eq!(first_scan.unchanged_skipped, 0);

        let initial_page = service.page(LibraryPageRequest::default()).unwrap();
        assert_eq!(initial_page.total, 2);
        assert!(initial_page.items.iter().all(|item| item.available));
        let original = initial_page
            .items
            .iter()
            .find(|item| item.path == first_display_path)
            .unwrap()
            .clone();
        let provider_item_id: String = database
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT provider_item_id FROM track_sources WHERE id = ?1",
                    rusqlite::params![original.source_id.to_string()],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_ne!(provider_item_id, first_path.to_string_lossy());

        let second_scan = service.scan_folder_now(folder.id, false, None).unwrap();
        assert_eq!(second_scan.unchanged_skipped, 2);
        let seen_generation: i64 = service
            .database
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT last_seen_generation FROM local_files WHERE library_folder_id = ?1 LIMIT 1",
                    params![folder.id.to_string()],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert!(seen_generation >= 2);

        let renamed_path = directory.path().join("renamed.wav");
        fs::rename(&first_path, &renamed_path).unwrap();
        let renamed_display_path = folders::normalize_file_path(&renamed_path).unwrap().0;
        let rename_scan = service.scan_folder_now(folder.id, false, None).unwrap();
        assert_eq!(rename_scan.renamed_files, 1);
        let renamed_page = service.page(LibraryPageRequest::default()).unwrap();
        let renamed = renamed_page
            .items
            .iter()
            .find(|item| item.path == renamed_display_path)
            .unwrap();
        assert_eq!(renamed.source_id, original.source_id);
        assert_eq!(renamed.track_id, original.track_id);

        fs::remove_file(&second_path).unwrap();
        let missing_scan = service.scan_folder_now(folder.id, false, None).unwrap();
        assert_eq!(missing_scan.missing_files, 1);
        let missing_page = service.page(LibraryPageRequest::default()).unwrap();
        let missing = missing_page
            .items
            .iter()
            .find(|item| item.path == second_display_path)
            .unwrap();
        assert!(!missing.available);
        assert_eq!(missing.index_status, LocalFileIndexStatus::Missing);

        fs::write(&second_path, minimal_wav()).unwrap();
        let restore_scan = service.scan_folder_now(folder.id, false, None).unwrap();
        assert_eq!(restore_scan.changed_files, 1);
        let restored = service
            .page(LibraryPageRequest::default())
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.path == second_display_path)
            .unwrap();
        assert!(restored.available);
        assert_eq!(restored.index_status, LocalFileIndexStatus::Indexed);
    }

    #[test]
    fn scan_isolates_corrupt_supported_files_and_preserves_measured_quality() {
        let directory = tempfile::tempdir().unwrap();
        let valid_path = directory.path().join("Good.WAV");
        let corrupt_path = directory.path().join("broken.flac");
        fs::write(&valid_path, minimal_wav()).unwrap();
        fs::write(&corrupt_path, b"not a valid audio file").unwrap();
        fs::write(directory.path().join("notes.txt"), b"ignore me").unwrap();

        let database_path = TempDatabasePath::new("library-corrupt");
        let database = Database::open(database_path.path()).unwrap();
        let artwork = tempfile::tempdir().unwrap();
        let service = LibraryService::new(database, artwork.path()).unwrap();
        let folder = service
            .add_folders(vec![directory.path().to_path_buf()])
            .unwrap()
            .remove(0);

        let summary = service.scan_folder_now(folder.id, false, None).unwrap();
        assert_eq!(summary.candidates, 2);
        assert_eq!(summary.new_files, 2);
        assert_eq!(summary.metadata_failures, 1);
        assert_eq!(summary.unsupported_skipped, 1);

        let page = service.page(LibraryPageRequest::default()).unwrap();
        assert_eq!(page.total, 2);
        let valid = page.items.iter().find(|item| item.title == "Good").unwrap();
        assert!(valid.available);
        assert_eq!(valid.index_status, LocalFileIndexStatus::Indexed);
        assert_eq!(valid.container.as_deref(), Some("WAV"));
        assert_eq!(valid.sample_rate_hz, Some(8_000));
        assert_eq!(valid.bit_depth, Some(16));

        let corrupt = page
            .items
            .iter()
            .find(|item| item.path.ends_with("broken.flac"))
            .unwrap();
        assert!(!corrupt.available);
        assert_eq!(corrupt.index_status, LocalFileIndexStatus::Error);
        assert!(corrupt.status_detail.is_some());
        let folder_after_scan = service
            .list_folders()
            .unwrap()
            .into_iter()
            .find(|item| item.id == folder.id)
            .unwrap();
        assert_eq!(folder_after_scan.status, LibraryFolderStatus::Complete);
        assert!(folder_after_scan
            .last_scan_error
            .as_deref()
            .is_some_and(|error| error.contains("metadata/I/O failure")));
    }

    #[test]
    fn forced_rescan_refreshes_same_size_content() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("forced.wav");
        fs::write(&path, minimal_wav_with_sample(0)).unwrap();

        let database_path = TempDatabasePath::new("library-forced");
        let database = Database::open(database_path.path()).unwrap();
        let artwork = tempfile::tempdir().unwrap();
        let service = LibraryService::new(database, artwork.path()).unwrap();
        let folder = service
            .add_folders(vec![directory.path().to_path_buf()])
            .unwrap()
            .remove(0);
        service.scan_folder_now(folder.id, false, None).unwrap();
        let before = service.page(LibraryPageRequest::default()).unwrap();
        let before_fingerprint = before.items[0].content_fingerprint.clone();

        fs::write(&path, minimal_wav_with_sample(1)).unwrap();
        let summary = service.scan_folder_now(folder.id, true, None).unwrap();
        assert_eq!(summary.changed_files, 1);
        assert_eq!(summary.unchanged_skipped, 0);
        let after = service.page(LibraryPageRequest::default()).unwrap();
        assert_ne!(after.items[0].content_fingerprint, before_fingerprint);
        assert_eq!(after.items[0].index_status, LocalFileIndexStatus::Indexed);
    }

    #[test]
    fn ambiguous_fingerprint_rename_creates_a_new_identity() {
        let directory = tempfile::tempdir().unwrap();
        let first_path = directory.path().join("first.wav");
        let second_path = directory.path().join("second.wav");
        fs::write(&first_path, minimal_wav()).unwrap();
        fs::write(&second_path, minimal_wav()).unwrap();

        let database_path = TempDatabasePath::new("library-ambiguous-rename");
        let database = Database::open(database_path.path()).unwrap();
        let artwork = tempfile::tempdir().unwrap();
        let service = LibraryService::new(database, artwork.path()).unwrap();
        let folder = service
            .add_folders(vec![directory.path().to_path_buf()])
            .unwrap()
            .remove(0);
        service.scan_folder_now(folder.id, false, None).unwrap();
        let original = service.page(LibraryPageRequest::default()).unwrap();
        let original_ids = original
            .items
            .iter()
            .map(|item| item.source_id)
            .collect::<HashSet<_>>();

        fs::remove_file(&first_path).unwrap();
        fs::remove_file(&second_path).unwrap();
        let replacement = directory.path().join("replacement.wav");
        fs::write(&replacement, minimal_wav()).unwrap();
        let summary = service.scan_folder_now(folder.id, false, None).unwrap();

        assert_eq!(summary.new_files, 1);
        assert_eq!(summary.renamed_files, 0);
        assert_eq!(summary.missing_files, 2);
        let page = service.page(LibraryPageRequest::default()).unwrap();
        assert_eq!(page.total, 3);
        let replacement = page
            .items
            .iter()
            .find(|item| item.path.ends_with("replacement.wav"))
            .unwrap();
        assert!(!original_ids.contains(&replacement.source_id));
        assert_eq!(
            page.items
                .iter()
                .filter(|item| item.index_status == LocalFileIndexStatus::Missing)
                .count(),
            2
        );
    }

    #[test]
    fn library_page_reads_bounded_deterministic_pages() {
        let directory = tempfile::tempdir().unwrap();
        for index in 0..120 {
            fs::write(
                directory.path().join(format!("track-{index:03}.wav")),
                minimal_wav_with_sample(index as i16),
            )
            .unwrap();
        }

        let database_path = TempDatabasePath::new("library-page");
        let database = Database::open(database_path.path()).unwrap();
        let artwork = tempfile::tempdir().unwrap();
        let service = LibraryService::new(database, artwork.path()).unwrap();
        let folder = service
            .add_folders(vec![directory.path().to_path_buf()])
            .unwrap()
            .remove(0);
        service.scan_folder_now(folder.id, false, None).unwrap();

        let first = service
            .page(LibraryPageRequest {
                page: 0,
                page_size: 50,
                ..LibraryPageRequest::default()
            })
            .unwrap();
        let third = service
            .page(LibraryPageRequest {
                page: 2,
                page_size: 50,
                ..LibraryPageRequest::default()
            })
            .unwrap();
        assert_eq!(first.items.len(), 50);
        assert_eq!(first.total, 120);
        assert!(first.has_next);
        assert_eq!(third.items.len(), 20);
        assert!(!third.has_next);
        assert!(first.items[0].title < first.items[1].title);
        assert_eq!(first.items[0].folder_id, folder.id);
    }

    #[test]
    fn missing_persisted_roots_are_failed_without_aborting_watcher_registration() {
        let parent = tempfile::tempdir().unwrap();
        let missing_path = parent.path().join("removable-music");
        fs::create_dir(&missing_path).unwrap();
        fs::remove_dir(&missing_path).unwrap();

        let database_path = TempDatabasePath::new("library-missing-root");
        let database = Database::open(database_path.path()).unwrap();
        let folder_id = LibraryFolderId::new();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO library_folders (
                        id, path, normalized_path_key, enabled, scan_status,
                        scan_generation, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 1, 'scanning', 3, ?4, ?4)",
                    params![
                        folder_id.to_string(),
                        missing_path.to_string_lossy().into_owned(),
                        missing_path.to_string_lossy().to_lowercase(),
                        "2026-01-01T00:00:00Z"
                    ],
                )?;
                Ok(())
            })
            .unwrap();

        let artwork = tempfile::tempdir().unwrap();
        let service = LibraryService::new(database, artwork.path()).unwrap();
        service.register_watchers(None).unwrap();

        let folder = service.list_folders().unwrap().remove(0);
        assert_eq!(folder.status, LibraryFolderStatus::Failed);
        assert!(folder.last_scan_error.is_some());

        fs::create_dir(&missing_path).unwrap();
        service.reregister_folder_watcher(folder_id, None).unwrap();
        service.scan_folder_now(folder_id, false, None).unwrap();
        let recovered = service.list_folders().unwrap().remove(0);
        assert_eq!(recovered.status, LibraryFolderStatus::Complete);
    }

    #[test]
    fn reveal_rejects_a_local_file_outside_its_managed_folder() {
        let managed = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("outside.wav");
        fs::write(&outside_file, minimal_wav()).unwrap();
        let (outside_display, outside_key) = folders::normalize_file_path(&outside_file).unwrap();

        let database_path = TempDatabasePath::new("library-reveal");
        let database = Database::open(database_path.path()).unwrap();
        let artwork = tempfile::tempdir().unwrap();
        let service = LibraryService::new(database.clone(), artwork.path()).unwrap();
        let folder = service
            .add_folders(vec![managed.path().to_path_buf()])
            .unwrap()
            .remove(0);
        let track_id = TrackId::new();
        let source_id = SourceId::new();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO tracks (
                        id, title, normalized_title, created_at, updated_at
                     ) VALUES (?1, 'Outside', 'outside', ?2, ?2)",
                    params![track_id.to_string(), "2026-01-01T00:00:00Z"],
                )?;
                connection.execute(
                    "INSERT INTO track_sources (
                        id, track_id, provider_kind, provider_item_id, created_at, updated_at
                     ) VALUES (?1, ?2, 'local', ?3, ?4, ?4)",
                    params![
                        source_id.to_string(),
                        track_id.to_string(),
                        "opaque-outside",
                        "2026-01-01T00:00:00Z"
                    ],
                )?;
                connection.execute(
                    "INSERT INTO local_files (
                        source_id, path, created_at, updated_at,
                        library_folder_id, normalized_path_key, index_status
                     ) VALUES (?1, ?2, ?3, ?3, ?4, ?5, 'indexed')",
                    params![
                        source_id.to_string(),
                        outside_display.to_string_lossy().into_owned(),
                        "2026-01-01T00:00:00Z",
                        folder.id.to_string(),
                        outside_key
                    ],
                )?;
                Ok(())
            })
            .unwrap();

        assert!(service.reveal_path(source_id).is_err());
    }

    fn minimal_wav() -> Vec<u8> {
        minimal_wav_with_sample(0)
    }

    fn minimal_wav_with_sample(sample: i16) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(46);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&38_u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&8_000_u32.to_le_bytes());
        bytes.extend_from_slice(&16_000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&sample.to_le_bytes());
        bytes
    }
}
