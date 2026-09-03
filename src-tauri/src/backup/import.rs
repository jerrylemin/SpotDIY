use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;
use zip::{CompressionMethod, ZipArchive};

use crate::db::{Database, DatabaseError, LATEST_SCHEMA_VERSION};
use crate::library::folders::{is_reparse_point, normalize_file_path, normalize_folder_path};
use crate::settings::{SettingValue, SettingsError, SettingsRepository};
use crate::storage::{StorageLayout, StorageMode};

use super::archive::ArchiveError;
use super::manifest::{
    validate_manifest, SpotDiyArchiveEntryKind, SpotDiyManifest, DATABASE_ARCHIVE_PATH,
    MANIFEST_CHECKSUM_PATH, MANIFEST_PATH, MAX_ARCHIVE_ENTRIES, MAX_DATABASE_BYTES,
    MAX_MANIFEST_BYTES, MAX_TOTAL_UNCOMPRESSED_BYTES,
};

const COPY_BUFFER_SIZE: usize = 64 * 1024;
const MAX_MISSING_DETAILS: usize = 500;
pub(crate) const PENDING_RESTORE_FILE_NAME: &str = "pending-restore.json";
pub(crate) const PENDING_RESTORE_VERSION: u32 = 1;
const STAGED_DATABASE_BACKUP_FILE_NAME: &str = ".original.sqlite3";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MissingFileReference {
    pub kind: String,
    pub track_id: Option<String>,
    pub source_id: Option<String>,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MissingFileReport {
    pub total_local_references: u64,
    pub available_local_references: u64,
    pub missing_local_references: u64,
    pub completed_download_references: u64,
    pub missing_download_outputs: u64,
    pub first_missing: Vec<MissingFileReference>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportPreview {
    pub import_id: String,
    pub archive_version: u32,
    pub app_version: String,
    pub database_schema_version: u32,
    pub source_storage_mode: StorageMode,
    pub entry_count: u64,
    pub included_audio_count: u64,
    pub included_artwork_count: u64,
    pub included_sidecar_lyrics_count: u64,
    pub missing: MissingFileReport,
    pub checksum_valid: bool,
    pub restored_audio_planned_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PendingRestoreState {
    Pending,
    Applying,
    Committed,
    RollbackRequired,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PendingRestoreDescriptor {
    pub version: u32,
    pub import_id: String,
    pub state: PendingRestoreState,
    pub staged_root: PathBuf,
    pub staged_database_path: PathBuf,
    pub active_mode: StorageMode,
    pub music_destination: Option<PathBuf>,
    pub manifest: SpotDiyManifest,
    pub preview: ImportPreview,
    pub archive_sha256: String,
    pub staged_database_sha256: String,
    pub rollback_path: Option<PathBuf>,
    pub created_paths: Vec<PathBuf>,
    pub last_error: Option<String>,
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("could not open import archive {path}: {source}")]
    OpenArchive {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("the selected import archive must be a .spotdiy file")]
    InvalidArchiveExtension,
    #[error("could not read import archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("import archive contains an unsafe or unsupported entry path: {0}")]
    UnsafeEntryPath(String),
    #[error("import archive contains duplicate paths: {0}")]
    DuplicatePath(String),
    #[error("import archive contains multiple {0} files")]
    MultipleMetadataFile(&'static str),
    #[error("import archive contains an encrypted entry: {0}")]
    EncryptedEntry(String),
    #[error("import archive contains a symbolic-link entry: {0}")]
    SymlinkEntry(String),
    #[error("import archive uses unsupported compression for {0}")]
    UnsupportedCompression(String),
    #[error("import archive has too many entries")]
    TooManyEntries,
    #[error("import archive exceeds its uncompressed size bound")]
    TotalSizeExceeded,
    #[error("import archive entry is too large: {0}")]
    EntryTooLarge(String),
    #[error("import archive has an abusive compression ratio for {0}")]
    CompressionRatio(String),
    #[error("import archive is missing {0}")]
    MissingMetadata(&'static str),
    #[error("import archive contains an undeclared payload: {0}")]
    UndeclaredPayload(String),
    #[error("import archive is missing declared payload: {0}")]
    MissingPayload(String),
    #[error("import manifest is invalid: {0}")]
    Manifest(#[from] super::manifest::ManifestError),
    #[error("import manifest checksum is invalid")]
    ManifestChecksum,
    #[error("import payload checksum is invalid: {0}")]
    PayloadChecksum(String),
    #[error("import payload size is invalid: {0}")]
    PayloadSize(String),
    #[error("import archive manifest is not valid UTF-8")]
    ManifestEncoding,
    #[error("could not create import staging directory {path}: {source}")]
    CreateStaging {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not write staged import file {path}: {source}")]
    StageFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error("staged database validation failed for {path}: {detail}")]
    InvalidStagedDatabase { path: PathBuf, detail: String },
    #[error("included local audio requires a restore folder in Standard mode")]
    MusicRestoreDirectoryRequired,
    #[error("restore folder is invalid: {0}")]
    InvalidMusicDestination(String),
    #[error("pending restore descriptor is invalid: {0}")]
    InvalidPendingDescriptor(String),
    #[error("pending restore import {0} was not found")]
    ImportNotFound(String),
    #[error("pending restore import {0} has already been committed")]
    AlreadyCommitted(String),
    #[error("restore could not safely create {path}: {source}")]
    RestoreFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("restore failed: {0}")]
    Restore(String),
    #[error("a restore is already pending")]
    RestoreAlreadyPending,
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error("import archive fingerprint is invalid")]
    ArchiveFingerprint,
}

#[derive(Clone, Debug)]
struct ArchiveEntryInfo {
    index: usize,
    name: String,
}

#[derive(Clone, Debug)]
pub(crate) struct StagedImport {
    pub id: Uuid,
    pub root: PathBuf,
    pub staged_database_path: PathBuf,
    pub manifest: SpotDiyManifest,
    pub preview: ImportPreview,
    pub archive_sha256: String,
    pub staged_database_sha256: String,
}

pub(crate) fn stage_archive(
    archive_path: &Path,
    layout: &StorageLayout,
    active_mode: StorageMode,
) -> Result<StagedImport, ImportError> {
    let archive_path = trusted_archive_path(archive_path)?;
    if archive_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("spotdiy"))
    {
        return Err(ImportError::InvalidArchiveExtension);
    }
    let archive_sha256 = hash_file(&archive_path).map_err(|source| ImportError::OpenArchive {
        path: archive_path.clone(),
        source,
    })?;
    let archive_file = File::open(&archive_path).map_err(|source| ImportError::OpenArchive {
        path: archive_path.clone(),
        source,
    })?;
    let mut archive = ZipArchive::new(archive_file)?;
    let entries = inspect_archive(&mut archive)?;
    let manifest_index = entries
        .iter()
        .find(|entry| entry.name == MANIFEST_PATH)
        .map(|entry| entry.index)
        .ok_or(ImportError::MissingMetadata("manifest.json"))?;
    let checksum_index = entries
        .iter()
        .find(|entry| entry.name == MANIFEST_CHECKSUM_PATH)
        .map(|entry| entry.index)
        .ok_or(ImportError::MissingMetadata("manifest.sha256"))?;
    let manifest_bytes = read_zip_bytes(&mut archive, manifest_index, MAX_MANIFEST_BYTES)?;
    let manifest: SpotDiyManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| ImportError::ManifestEncoding)?;
    validate_manifest(&manifest)?;
    let checksum_bytes = read_zip_bytes(&mut archive, checksum_index, 512)?;
    validate_manifest_checksum(&manifest_bytes, &checksum_bytes)?;
    validate_declared_payloads(&entries, &manifest)?;

    let import_id = Uuid::new_v4();
    let root = create_staging_root(layout, import_id)?;
    let staged_database_path =
        root.join(DATABASE_ARCHIVE_PATH.replace('/', std::path::MAIN_SEPARATOR_STR));
    let stage_result = stage_payloads(
        &mut archive,
        &entries,
        &manifest,
        &root,
        &staged_database_path,
    );
    if let Err(error) = stage_result {
        let _ = cleanup_staged_root_for_failure(layout, &root);
        return Err(error);
    }

    let result = finish_staging(
        &root,
        &staged_database_path,
        &manifest,
        &archive_sha256,
        active_mode,
        import_id,
    );
    if result.is_err() {
        let _ = cleanup_staged_root_for_failure(layout, &root);
    }
    result
}

fn staging_error(path: PathBuf, detail: impl Into<String>) -> ImportError {
    ImportError::CreateStaging {
        path,
        source: io::Error::new(io::ErrorKind::InvalidInput, detail.into()),
    }
}

fn trusted_existing_directory(path: &Path) -> Result<PathBuf, ImportError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ImportError::CreateStaging {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(staging_error(
            path.to_path_buf(),
            "staging directory cannot be a symbolic link or reparse point",
        ));
    }
    if !metadata.is_dir() {
        return Err(staging_error(
            path.to_path_buf(),
            "staging path is not a directory",
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|source| ImportError::CreateStaging {
        path: path.to_path_buf(),
        source,
    })?;
    let canonical_metadata =
        fs::symlink_metadata(&canonical).map_err(|source| ImportError::CreateStaging {
            path: canonical.clone(),
            source,
        })?;
    if canonical_metadata.file_type().is_symlink()
        || is_reparse_point(&canonical_metadata)
        || !canonical_metadata.is_dir()
    {
        return Err(staging_error(
            canonical,
            "canonical staging directory is not trusted",
        ));
    }
    Ok(canonical)
}

fn ensure_trusted_directory(root: &Path, directory: &Path) -> Result<PathBuf, ImportError> {
    let canonical_root = trusted_existing_directory(root)?;
    let relative = directory
        .strip_prefix(root)
        .map_err(|_| staging_error(directory.to_path_buf(), "staging path is outside its root"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(staging_error(
                directory.to_path_buf(),
                "staging path contains a non-normal component",
            ));
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || is_reparse_point(&metadata) => {
                return Err(staging_error(
                    current,
                    "staging path component is a symbolic link or reparse point",
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(staging_error(
                    current,
                    "staging path component is not a directory",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|source| ImportError::CreateStaging {
                    path: current.clone(),
                    source,
                })?;
            }
            Err(source) => {
                return Err(ImportError::CreateStaging {
                    path: current,
                    source,
                });
            }
        }

        let metadata =
            fs::symlink_metadata(&current).map_err(|source| ImportError::CreateStaging {
                path: current.clone(),
                source,
            })?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) || !metadata.is_dir() {
            return Err(staging_error(
                current,
                "staging path component is not a trusted directory",
            ));
        }
        let canonical_current =
            fs::canonicalize(&current).map_err(|source| ImportError::CreateStaging {
                path: current.clone(),
                source,
            })?;
        if !canonical_current.starts_with(&canonical_root) {
            return Err(staging_error(
                current,
                "staging path component escapes its trusted root",
            ));
        }
    }

    let canonical = fs::canonicalize(directory).map_err(|source| ImportError::CreateStaging {
        path: directory.to_path_buf(),
        source,
    })?;
    if !canonical.starts_with(&canonical_root) {
        return Err(staging_error(
            directory.to_path_buf(),
            "staging directory escapes its trusted root",
        ));
    }
    Ok(canonical)
}

fn create_staging_root(layout: &StorageLayout, import_id: Uuid) -> Result<PathBuf, ImportError> {
    let canonical_restore_root = trusted_existing_directory(&layout.restore_root)?;
    let imports_root = layout.restore_root.join("imports");
    let canonical_imports_root = ensure_trusted_directory(&layout.restore_root, &imports_root)?;
    let root = imports_root.join(import_id.to_string());
    match fs::symlink_metadata(&root) {
        Ok(_) => return Err(staging_error(root, "generated staging root already exists")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(ImportError::CreateStaging { path: root, source });
        }
    }
    fs::create_dir(&root).map_err(|source| ImportError::CreateStaging {
        path: root.clone(),
        source,
    })?;
    let canonical_root = trusted_existing_directory(&root)?;
    if canonical_root.parent() != Some(canonical_imports_root.as_path())
        || !canonical_root.starts_with(&canonical_restore_root)
    {
        return Err(staging_error(
            root,
            "generated staging root is outside the trusted restore root",
        ));
    }
    Ok(canonical_root)
}

fn trusted_staged_root(layout: &StorageLayout, path: &Path) -> Result<PathBuf, ImportError> {
    let canonical_restore_root = trusted_existing_directory(&layout.restore_root)?;
    let canonical_imports_root = trusted_existing_directory(&layout.restore_root.join("imports"))?;
    let canonical_root = trusted_existing_directory(path)?;
    let name = canonical_root.file_name().and_then(|value| value.to_str());
    if canonical_root.parent() != Some(canonical_imports_root.as_path())
        || !canonical_root.starts_with(&canonical_restore_root)
        || name.is_none_or(|value| Uuid::parse_str(value).is_err())
    {
        return Err(ImportError::InvalidPendingDescriptor(
            "staged root is not an owned UUID directory under restore_root/imports".to_owned(),
        ));
    }
    Ok(canonical_root)
}

pub(crate) fn cleanup_staged_root(layout: &StorageLayout, path: &Path) -> Result<(), ImportError> {
    let trusted = trusted_staged_root(layout, path)?;
    fs::remove_dir_all(&trusted).map_err(|source| ImportError::CreateStaging {
        path: trusted,
        source,
    })
}

pub(crate) fn cleanup_staged_root_for_failure(
    layout: &StorageLayout,
    path: &Path,
) -> Result<(), ImportError> {
    match fs::symlink_metadata(path) {
        Ok(_) => cleanup_staged_root(layout, path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ImportError::CreateStaging {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn cleanup_staging_root_if_present(layout: &StorageLayout, path: &Path) -> Result<(), ImportError> {
    match fs::symlink_metadata(path) {
        Ok(_) => cleanup_staged_root(layout, path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ImportError::CreateStaging {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn inspect_archive<R: Read + io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<Vec<ArchiveEntryInfo>, ImportError> {
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(ImportError::TooManyEntries);
    }
    let mut entries = Vec::with_capacity(archive.len());
    let mut names = HashSet::new();
    let mut total_size = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        let lower_name = name.to_ascii_lowercase();
        if !names.insert(lower_name.clone()) {
            return Err(ImportError::DuplicatePath(name));
        }
        if lower_name == MANIFEST_PATH.to_ascii_lowercase() && name != MANIFEST_PATH {
            return Err(ImportError::MultipleMetadataFile("manifest.json"));
        }
        if lower_name == MANIFEST_CHECKSUM_PATH.to_ascii_lowercase()
            && name != MANIFEST_CHECKSUM_PATH
        {
            return Err(ImportError::MultipleMetadataFile("manifest.sha256"));
        }
        if entry.is_dir() {
            return Err(ImportError::UnsafeEntryPath(name));
        }
        if entry.is_symlink() {
            return Err(ImportError::SymlinkEntry(name));
        }
        if entry.encrypted() {
            return Err(ImportError::EncryptedEntry(name));
        }
        if !matches!(
            entry.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err(ImportError::UnsupportedCompression(name));
        }
        if name != MANIFEST_PATH && name != MANIFEST_CHECKSUM_PATH {
            super::manifest::validate_archive_payload_path(&name)
                .map_err(|_| ImportError::UnsafeEntryPath(name.clone()))?;
        }
        let size = entry.size();
        let compressed_size = entry.compressed_size();
        if name == MANIFEST_PATH && size > MAX_MANIFEST_BYTES {
            return Err(ImportError::EntryTooLarge(name));
        }
        if name == DATABASE_ARCHIVE_PATH && size > MAX_DATABASE_BYTES {
            return Err(ImportError::EntryTooLarge(name));
        }
        if size >= 1024 * 1024
            && (compressed_size == 0 || size > compressed_size.saturating_mul(1000))
        {
            return Err(ImportError::CompressionRatio(name));
        }
        total_size = total_size
            .checked_add(size)
            .ok_or(ImportError::TotalSizeExceeded)?;
        if total_size > MAX_TOTAL_UNCOMPRESSED_BYTES {
            return Err(ImportError::TotalSizeExceeded);
        }
        entries.push(ArchiveEntryInfo { index, name });
    }
    Ok(entries)
}

fn read_zip_bytes<R: Read + io::Seek>(
    archive: &mut ZipArchive<R>,
    index: usize,
    maximum: u64,
) -> Result<Vec<u8>, ImportError> {
    let entry = archive.by_index(index)?;
    let entry_name = entry.name().to_owned();
    if entry.size() > maximum {
        return Err(ImportError::EntryTooLarge(entry_name));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ImportError::Restore(error.to_string()))?;
    if bytes.len() as u64 > maximum {
        return Err(ImportError::EntryTooLarge(entry_name));
    }
    Ok(bytes)
}

fn validate_manifest_checksum(
    manifest_bytes: &[u8],
    checksum_bytes: &[u8],
) -> Result<(), ImportError> {
    let expected = digest_bytes(manifest_bytes);
    let expected_bytes = format!("{expected}\n");
    if checksum_bytes != expected_bytes.as_bytes() {
        return Err(ImportError::ManifestChecksum);
    }
    Ok(())
}

fn validate_declared_payloads(
    archive_entries: &[ArchiveEntryInfo],
    manifest: &SpotDiyManifest,
) -> Result<(), ImportError> {
    let actual = archive_entries
        .iter()
        .filter(|entry| entry.name != MANIFEST_PATH && entry.name != MANIFEST_CHECKSUM_PATH)
        .map(|entry| entry.name.as_str())
        .collect::<HashSet<_>>();
    let declared = manifest
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<HashSet<_>>();
    if let Some(name) = actual.difference(&declared).next() {
        return Err(ImportError::UndeclaredPayload((*name).to_owned()));
    }
    if let Some(name) = declared.difference(&actual).next() {
        return Err(ImportError::MissingPayload((*name).to_owned()));
    }
    Ok(())
}

fn stage_payloads<R: Read + io::Seek>(
    archive: &mut ZipArchive<R>,
    archive_entries: &[ArchiveEntryInfo],
    manifest: &SpotDiyManifest,
    root: &Path,
    staged_database_path: &Path,
) -> Result<(), ImportError> {
    let archive_by_name = archive_entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry))
        .collect::<HashMap<_, _>>();
    for declared in &manifest.entries {
        let archive_entry = archive_by_name
            .get(declared.path.as_str())
            .ok_or_else(|| ImportError::MissingPayload(declared.path.clone()))?;
        let destination = if declared.path == DATABASE_ARCHIVE_PATH {
            staged_database_path.to_path_buf()
        } else {
            staged_payload_path(root, &declared.path)?
        };
        if let Some(parent) = destination.parent() {
            ensure_trusted_directory(root, parent)?;
        }
        let (size, sha256) = copy_zip_entry_to_file(archive, archive_entry.index, &destination)?;
        if size != declared.size_bytes {
            return Err(ImportError::PayloadSize(declared.path.clone()));
        }
        if sha256 != declared.sha256.to_ascii_lowercase() {
            return Err(ImportError::PayloadChecksum(declared.path.clone()));
        }
    }
    Ok(())
}

fn staged_payload_path(root: &Path, archive_path: &str) -> Result<PathBuf, ImportError> {
    super::manifest::validate_archive_payload_path(archive_path)
        .map_err(|_| ImportError::UnsafeEntryPath(archive_path.to_owned()))?;
    let mut path = root.join("payloads");
    for component in archive_path.split('/') {
        path.push(component);
    }
    Ok(path)
}

fn copy_zip_entry_to_file<R: Read + io::Seek>(
    archive: &mut ZipArchive<R>,
    index: usize,
    destination: &Path,
) -> Result<(u64, String), ImportError> {
    let mut source = archive.by_index(index)?;
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|source| ImportError::StageFile {
            path: destination.to_path_buf(),
            source,
        })?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; COPY_BUFFER_SIZE];
    let result = (|| -> Result<(u64, String), ImportError> {
        loop {
            let read = source
                .read(&mut buffer)
                .map_err(|source| ImportError::StageFile {
                    path: destination.to_path_buf(),
                    source,
                })?;
            if read == 0 {
                break;
            }
            target
                .write_all(&buffer[..read])
                .map_err(|source| ImportError::StageFile {
                    path: destination.to_path_buf(),
                    source,
                })?;
            hasher.update(&buffer[..read]);
            total = total
                .checked_add(read as u64)
                .ok_or_else(|| ImportError::PayloadSize(destination.display().to_string()))?;
        }
        target.sync_all().map_err(|source| ImportError::StageFile {
            path: destination.to_path_buf(),
            source,
        })?;
        Ok((total, hex_digest(&hasher.finalize())))
    })();
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

fn finish_staging(
    root: &Path,
    staged_database_path: &Path,
    manifest: &SpotDiyManifest,
    archive_sha256: &str,
    active_mode: StorageMode,
    import_id: Uuid,
) -> Result<StagedImport, ImportError> {
    let database = Database::open(staged_database_path)?;
    if database.schema_version()? != LATEST_SCHEMA_VERSION {
        return Err(ImportError::InvalidStagedDatabase {
            path: staged_database_path.to_path_buf(),
            detail: format!("schema did not migrate to {LATEST_SCHEMA_VERSION}"),
        });
    }
    SettingsRepository::new(&database).set_setting(SettingValue::StorageMode(active_mode))?;
    validate_database(&database, staged_database_path)?;
    let preview = build_preview(&database, manifest, active_mode, import_id)?;
    drop(database);
    remove_database_sidecars(staged_database_path);
    let staged_database_sha256 =
        hash_file(staged_database_path).map_err(|source| ImportError::StageFile {
            path: staged_database_path.to_path_buf(),
            source,
        })?;
    let original_database_path = root.join(STAGED_DATABASE_BACKUP_FILE_NAME);
    let original_database = Database::open(staged_database_path)?;
    original_database.online_backup_to(&original_database_path)?;
    drop(original_database);
    remove_database_sidecars(&original_database_path);
    Ok(StagedImport {
        id: import_id,
        root: root.to_path_buf(),
        staged_database_path: staged_database_path.to_path_buf(),
        manifest: manifest.clone(),
        preview,
        archive_sha256: archive_sha256.to_owned(),
        staged_database_sha256,
    })
}

pub(crate) fn build_preview(
    database: &Database,
    manifest: &SpotDiyManifest,
    active_mode: StorageMode,
    import_id: Uuid,
) -> Result<ImportPreview, ImportError> {
    let missing = missing_file_report(database)?;
    let included_audio_count = manifest
        .entries
        .iter()
        .filter(|entry| entry.kind == SpotDiyArchiveEntryKind::LocalAudio)
        .count() as u64;
    let included_artwork_count = manifest
        .entries
        .iter()
        .filter(|entry| entry.kind == SpotDiyArchiveEntryKind::Artwork)
        .count() as u64;
    let included_sidecar_lyrics_count = manifest
        .entries
        .iter()
        .filter(|entry| entry.kind == SpotDiyArchiveEntryKind::SidecarLyrics)
        .count() as u64;
    Ok(ImportPreview {
        import_id: import_id.to_string(),
        archive_version: manifest.format_version,
        app_version: manifest.app_version.clone(),
        database_schema_version: manifest.database_schema_version,
        source_storage_mode: manifest.source_storage_mode,
        entry_count: manifest.entries.len() as u64,
        included_audio_count,
        included_artwork_count,
        included_sidecar_lyrics_count,
        missing,
        checksum_valid: true,
        restored_audio_planned_count: if active_mode == StorageMode::Portable {
            included_audio_count
        } else {
            0
        },
    })
}

fn missing_file_report(database: &Database) -> Result<MissingFileReport, ImportError> {
    let local_rows = database.with_connection(|connection| {
        let mut statement = connection.prepare(
            "SELECT lf.source_id, ts.track_id, lf.path
             FROM local_files lf
             JOIN track_sources ts ON ts.id = lf.source_id
             WHERE ts.provider_kind = 'local'
             ORDER BY lf.source_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                PathBuf::from(row.get::<_, String>(2)?),
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    })?;
    let download_rows = database.with_connection(|connection| {
        let mut statement = connection.prepare(
            "SELECT id, output_path FROM downloads WHERE state = 'completed' ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    })?;
    let mut report = MissingFileReport {
        total_local_references: local_rows.len() as u64,
        completed_download_references: download_rows.len() as u64,
        ..MissingFileReport::default()
    };
    for (source_id, track_id, path) in local_rows {
        let available = fs::symlink_metadata(&path)
            .map(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
            .unwrap_or(false);
        if available {
            report.available_local_references += 1;
        } else {
            report.missing_local_references += 1;
            push_missing(
                &mut report,
                MissingFileReference {
                    kind: "localAudio".to_owned(),
                    track_id: Some(track_id),
                    source_id: Some(source_id),
                    path,
                },
            );
        }
    }
    for (download_id, output_path) in download_rows {
        let path = output_path.map(PathBuf::from);
        let available = path.as_ref().is_some_and(|path| {
            fs::symlink_metadata(path)
                .map(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
                .unwrap_or(false)
        });
        if !available {
            report.missing_download_outputs += 1;
            push_missing(
                &mut report,
                MissingFileReference {
                    kind: "download".to_owned(),
                    track_id: None,
                    source_id: None,
                    path: path.unwrap_or_else(|| PathBuf::from(format!("download:{download_id}"))),
                },
            );
        }
    }
    Ok(report)
}

fn push_missing(report: &mut MissingFileReport, reference: MissingFileReference) {
    if report.first_missing.len() < MAX_MISSING_DETAILS {
        report.first_missing.push(reference);
    }
}

fn trusted_archive_path(path: &Path) -> Result<PathBuf, ImportError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ImportError::OpenArchive {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ImportError::OpenArchive {
            path: path.to_path_buf(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "archive is not a regular file"),
        });
    }
    normalize_file_path(path)
        .map(|(display, _)| display)
        .map_err(|error| ImportError::Restore(error.to_string()))
}

pub(crate) fn validate_database(database: &Database, path: &Path) -> Result<(), ImportError> {
    if database.schema_version()? != LATEST_SCHEMA_VERSION {
        return Err(ImportError::InvalidStagedDatabase {
            path: path.to_path_buf(),
            detail: format!("schema is not {LATEST_SCHEMA_VERSION}"),
        });
    }
    let user_version: i64 = database.with_connection(|connection| {
        connection.pragma_query_value(None, "user_version", |row| row.get(0))
    })?;
    if user_version > i64::from(LATEST_SCHEMA_VERSION) {
        return Err(ImportError::InvalidStagedDatabase {
            path: path.to_path_buf(),
            detail: format!("user_version {user_version} is newer than {LATEST_SCHEMA_VERSION}"),
        });
    }
    let integrity: String = database.with_connection(|connection| {
        connection.pragma_query_value(None, "integrity_check", |row| row.get(0))
    })?;
    if integrity != "ok" {
        return Err(ImportError::InvalidStagedDatabase {
            path: path.to_path_buf(),
            detail: format!("integrity_check returned {integrity}"),
        });
    }
    let foreign_key_count: i64 = database.with_connection(|connection| {
        connection.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
    })?;
    if foreign_key_count != 0 {
        return Err(ImportError::InvalidStagedDatabase {
            path: path.to_path_buf(),
            detail: format!("foreign_key_check returned {foreign_key_count} row(s)"),
        });
    }
    Ok(())
}

pub(crate) fn pending_path(layout: &StorageLayout) -> PathBuf {
    layout.restore_root.join(PENDING_RESTORE_FILE_NAME)
}

pub(crate) fn write_pending_descriptor(
    layout: &StorageLayout,
    descriptor: &PendingRestoreDescriptor,
) -> Result<(), ImportError> {
    let path = pending_path(layout);
    let temporary = path.with_file_name(format!(
        ".{}.{}.tmp",
        PENDING_RESTORE_FILE_NAME,
        Uuid::new_v4()
    ));
    let bytes = serde_json::to_vec_pretty(descriptor)
        .map_err(|error| ImportError::Restore(error.to_string()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|source| ImportError::StageFile {
            path: temporary.clone(),
            source,
        })?;
    file.write_all(&bytes)
        .map_err(|source| ImportError::StageFile {
            path: temporary.clone(),
            source,
        })?;
    file.sync_all().map_err(|source| ImportError::StageFile {
        path: temporary.clone(),
        source,
    })?;
    drop(file);
    fs::rename(&temporary, &path).map_err(|source| ImportError::StageFile { path, source })
}

pub(crate) fn read_pending_descriptor(
    layout: &StorageLayout,
) -> Result<Option<PendingRestoreDescriptor>, ImportError> {
    let path = pending_path(layout);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(ImportError::StageFile { path, source }),
    };
    if bytes.len() > MAX_MANIFEST_BYTES as usize {
        return Err(ImportError::InvalidPendingDescriptor(
            "descriptor exceeds size bound".to_owned(),
        ));
    }
    let descriptor: PendingRestoreDescriptor = serde_json::from_slice(&bytes)
        .map_err(|error| ImportError::InvalidPendingDescriptor(error.to_string()))?;
    if descriptor.version != PENDING_RESTORE_VERSION {
        return Err(ImportError::InvalidPendingDescriptor(format!(
            "unsupported descriptor version {}",
            descriptor.version
        )));
    }
    Ok(Some(descriptor))
}

pub(crate) fn remove_pending_descriptor(layout: &StorageLayout) -> Result<(), ImportError> {
    let path = pending_path(layout);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ImportError::StageFile {
            path: pending_path(layout),
            source,
        }),
    }
}

pub(crate) fn stage_path_is_trusted(layout: &StorageLayout, path: &Path) -> bool {
    let Ok(root) = trusted_existing_directory(&layout.restore_root) else {
        return false;
    };
    trusted_path_within(&layout.restore_root, path, &root)
}

fn stage_path_is_below_trusted_root(root: &Path, path: &Path) -> bool {
    let Ok(canonical_root) = fs::canonicalize(root) else {
        return false;
    };
    trusted_path_within(root, path, &canonical_root)
}

fn trusted_path_within(root: &Path, path: &Path, canonical_root: &Path) -> bool {
    let (base, relative) = if let Ok(relative) = path.strip_prefix(root) {
        (root, relative)
    } else if let Ok(relative) = path.strip_prefix(canonical_root) {
        (canonical_root, relative)
    } else {
        return false;
    };
    let mut current = base.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return false;
        };
        current.push(name);
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            return false;
        };
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return false;
        }
        let Ok(canonical_current) = fs::canonicalize(&current) else {
            return false;
        };
        if !canonical_current.starts_with(canonical_root) {
            return false;
        }
    }
    true
}

pub(crate) fn revalidate_staged_import(
    descriptor: &PendingRestoreDescriptor,
) -> Result<Database, ImportError> {
    let staged_root = trusted_existing_directory(&descriptor.staged_root)?;
    if !stage_path_is_below_trusted_root(&staged_root, &descriptor.staged_database_path) {
        return Err(ImportError::InvalidPendingDescriptor(
            "staged paths are missing or outside the staging root".to_owned(),
        ));
    }
    validate_manifest(&descriptor.manifest)?;
    let database = Database::open(&descriptor.staged_database_path)?;
    validate_database(&database, &descriptor.staged_database_path)?;
    let actual_database_sha256 =
        hash_file(&descriptor.staged_database_path).map_err(|source| ImportError::StageFile {
            path: descriptor.staged_database_path.clone(),
            source,
        })?;
    if actual_database_sha256 != descriptor.staged_database_sha256 {
        return Err(ImportError::ArchiveFingerprint);
    }
    for entry in descriptor
        .manifest
        .entries
        .iter()
        .filter(|entry| entry.path != DATABASE_ARCHIVE_PATH)
    {
        let path = staged_payload_path(&staged_root, &entry.path)?;
        let metadata = fs::metadata(&path).map_err(|source| ImportError::StageFile {
            path: path.clone(),
            source,
        })?;
        if metadata.len() != entry.size_bytes {
            return Err(ImportError::PayloadSize(entry.path.clone()));
        }
        let sha256 = hash_file(&path).map_err(|source| ImportError::StageFile {
            path: path.clone(),
            source,
        })?;
        if sha256 != entry.sha256.to_ascii_lowercase() {
            return Err(ImportError::PayloadChecksum(entry.path.clone()));
        }
    }
    Ok(database)
}

pub(crate) fn validate_staged_paths(
    layout: &StorageLayout,
    descriptor: &PendingRestoreDescriptor,
) -> Result<(), ImportError> {
    let original_database_path = staged_database_original_path(&descriptor.staged_root);
    if descriptor.active_mode != layout.mode
        || !stage_path_is_trusted(layout, &descriptor.staged_root)
        || !stage_path_is_trusted(layout, &descriptor.staged_database_path)
        || !stage_path_is_trusted(layout, &original_database_path)
    {
        return Err(ImportError::InvalidPendingDescriptor(
            "pending restore does not belong to the active storage root".to_owned(),
        ));
    }
    for path in &descriptor.created_paths {
        if !path_is_under(&layout.music_root, path)
            && !path_is_under(&layout.artwork_cache_root, path)
        {
            return Err(ImportError::InvalidPendingDescriptor(
                "pending restore contains an unsafe created path".to_owned(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn apply_pending_restore(
    layout: &StorageLayout,
) -> Result<RestoreApplyReport, ImportError> {
    let Some(mut descriptor) = read_pending_descriptor(layout)? else {
        return Ok(RestoreApplyReport::default());
    };
    validate_staged_paths(layout, &descriptor)?;
    if descriptor.state == PendingRestoreState::Committed {
        cleanup_descriptor(layout, &descriptor)?;
        return Ok(RestoreApplyReport {
            rollback_path: descriptor.rollback_path,
            ..RestoreApplyReport::default()
        });
    }
    if descriptor.state == PendingRestoreState::RollbackRequired {
        return Ok(RestoreApplyReport {
            rolled_back: true,
            rollback_path: descriptor.rollback_path,
            detail: descriptor.last_error,
            ..RestoreApplyReport::default()
        });
    }

    if descriptor.state == PendingRestoreState::Applying {
        recover_applying_descriptor(layout, &mut descriptor)?;
    }
    descriptor.state = PendingRestoreState::Applying;
    write_pending_descriptor(layout, &descriptor)?;

    let result = apply_descriptor(layout, &mut descriptor);
    match result {
        Ok(()) => {
            let rollback_path = descriptor.rollback_path.clone();
            descriptor.state = PendingRestoreState::Committed;
            descriptor.last_error = None;
            write_pending_descriptor(layout, &descriptor)?;
            cleanup_descriptor(layout, &descriptor)?;
            Ok(RestoreApplyReport {
                applied: true,
                rollback_path,
                ..RestoreApplyReport::default()
            })
        }
        Err(error) => {
            let rollback_path = descriptor.rollback_path.clone();
            let rollback_result = if let Some(path) = rollback_path.as_deref() {
                restore_database_from_rollback(layout, path)
            } else {
                Ok(())
            };
            let rolled_back = rollback_result.is_ok();
            let detail = match &rollback_result {
                Ok(()) => error.to_string(),
                Err(rollback_error) => format!("{error}; rollback also failed: {rollback_error}"),
            };
            cleanup_created_paths(layout, &descriptor.created_paths);
            descriptor.state = PendingRestoreState::RollbackRequired;
            descriptor.last_error = Some(detail);
            write_pending_descriptor(layout, &descriptor)?;
            Ok(RestoreApplyReport {
                rolled_back,
                rollback_path,
                detail: descriptor.last_error,
                ..RestoreApplyReport::default()
            })
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RestoreApplyReport {
    pub applied: bool,
    pub rolled_back: bool,
    pub rollback_path: Option<PathBuf>,
    pub detail: Option<String>,
}

fn apply_descriptor(
    layout: &StorageLayout,
    descriptor: &mut PendingRestoreDescriptor,
) -> Result<(), ImportError> {
    let staged_database = revalidate_staged_import(descriptor)?;
    let music_destination = restore_music_destination(layout, descriptor)?;
    restore_audio(
        layout,
        descriptor,
        &staged_database,
        music_destination.as_deref(),
    )?;
    restore_artwork(layout, descriptor)?;
    drop(staged_database);
    remove_database_sidecars(&descriptor.staged_database_path);
    descriptor.staged_database_sha256 =
        hash_file(&descriptor.staged_database_path).map_err(|source| ImportError::StageFile {
            path: descriptor.staged_database_path.clone(),
            source,
        })?;
    write_pending_descriptor(layout, descriptor)?;
    let staged_database = Database::open(&descriptor.staged_database_path)?;
    validate_database(&staged_database, &descriptor.staged_database_path)?;
    drop(staged_database);

    let rollback_path = create_rollback_database(layout)?;
    descriptor.rollback_path = Some(rollback_path);
    write_pending_descriptor(layout, descriptor)?;
    replace_active_database(layout, descriptor)?;
    retain_latest_rollback(layout, &descriptor.rollback_path.clone().unwrap());
    Ok(())
}

fn restore_music_destination(
    layout: &StorageLayout,
    descriptor: &PendingRestoreDescriptor,
) -> Result<Option<PathBuf>, ImportError> {
    let has_audio = descriptor
        .manifest
        .entries
        .iter()
        .any(|entry| entry.kind == SpotDiyArchiveEntryKind::LocalAudio);
    if !has_audio {
        return Ok(None);
    }
    let destination = if layout.mode == StorageMode::Portable {
        layout.music_root.clone()
    } else {
        descriptor
            .music_destination
            .clone()
            .ok_or(ImportError::MusicRestoreDirectoryRequired)?
    };
    let normalized = normalize_folder_path(&destination)
        .map_err(|error| ImportError::InvalidMusicDestination(error.to_string()))?;
    Ok(Some(normalized.filesystem_path))
}

fn restore_audio(
    layout: &StorageLayout,
    descriptor: &mut PendingRestoreDescriptor,
    database: &Database,
    destination_root: Option<&Path>,
) -> Result<(), ImportError> {
    let Some(destination_root) = destination_root else {
        return Ok(());
    };
    for mapping in descriptor.manifest.media_mappings.clone() {
        let source_id = Uuid::parse_str(&mapping.source_id).map_err(|_| {
            ImportError::Restore(format!("invalid source ID {}", mapping.source_id))
        })?;
        let extension = mapping
            .archive_path
            .strip_prefix(&format!("media/{source_id}/media."))
            .filter(|value| !value.is_empty())
            .unwrap_or("bin");
        let sidecar_archive_path =
            sidecar_entry_for_source(&descriptor.manifest, &mapping.source_id);
        let mut suffix = 0_u32;
        let (audio_destination, sidecar_destination) = loop {
            let stem = if suffix == 0 {
                source_id.to_string()
            } else {
                format!("{source_id}-{suffix}")
            };
            let audio = destination_root.join(format!("{stem}.{extension}"));
            let sidecar = audio.with_extension("lrc");
            if !audio.exists() && (!sidecar.exists() || sidecar_archive_path.is_none()) {
                break (audio, sidecar);
            }
            suffix = suffix.saturating_add(1);
        };
        let staged_audio = staged_payload(&descriptor.staged_root, &mapping.archive_path)?;
        copy_new_file(&staged_audio, &audio_destination)?;
        descriptor.created_paths.push(audio_destination.clone());
        write_pending_descriptor(layout, descriptor)?;

        if let Some(sidecar_archive_path) = sidecar_archive_path {
            let staged_sidecar = staged_payload(&descriptor.staged_root, sidecar_archive_path)?;
            if sidecar_destination.exists() {
                return Err(ImportError::Restore(format!(
                    "sidecar destination already exists: {}",
                    sidecar_destination.display()
                )));
            }
            copy_new_file(&staged_sidecar, &sidecar_destination)?;
            descriptor.created_paths.push(sidecar_destination);
            write_pending_descriptor(layout, descriptor)?;
        }

        let (display_path, normalized_path_key) = normalize_file_path(&audio_destination)
            .map_err(|error| ImportError::InvalidMusicDestination(error.to_string()))?;
        let connection = database.connection()?;
        let changed = connection
            .execute(
                "UPDATE local_files SET path = ?1, normalized_path_key = ?2 WHERE source_id = ?3",
                rusqlite::params![
                    display_path.to_string_lossy().to_string(),
                    normalized_path_key,
                    source_id.to_string()
                ],
            )
            .map_err(|error| ImportError::Database(DatabaseError::Query(error)))?;
        if changed != 1 {
            return Err(ImportError::Restore(format!(
                "staged database has no local file for source {source_id}"
            )));
        }
    }
    Ok(())
}

fn restore_artwork(
    layout: &StorageLayout,
    descriptor: &mut PendingRestoreDescriptor,
) -> Result<(), ImportError> {
    for archive_path in artwork_entries(&descriptor.manifest) {
        let relative = archive_path
            .strip_prefix("covers/")
            .ok_or_else(|| ImportError::Restore(format!("invalid artwork path {archive_path}")))?;
        let mut destination = layout.artwork_cache_root.clone();
        for component in relative.split('/') {
            destination.push(component);
        }
        let staged = staged_payload(&descriptor.staged_root, archive_path)?;
        let destination_metadata = fs::symlink_metadata(&destination).ok();
        if destination_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.file_type().is_symlink() || is_reparse_point(metadata))
        {
            destination = collision_artwork_destination(
                &layout.artwork_cache_root,
                &descriptor.import_id,
                relative,
            );
        } else if destination_metadata.is_some() {
            let expected = descriptor
                .manifest
                .entries
                .iter()
                .find(|entry| entry.path == archive_path)
                .map(|entry| entry.sha256.as_str())
                .unwrap_or_default();
            if hash_file(&destination).ok().as_deref() == Some(expected) {
                continue;
            }
            let file_name = destination
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("artwork");
            destination = collision_artwork_destination(
                &layout.artwork_cache_root,
                &descriptor.import_id,
                file_name,
            );
        }
        if let Some(parent) = destination.parent() {
            ensure_trusted_restore_directory(&layout.artwork_cache_root, parent)?;
        }
        if copy_new_file(&staged, &destination).is_ok() {
            descriptor.created_paths.push(destination);
            write_pending_descriptor(layout, descriptor)?;
        }
    }
    Ok(())
}

fn collision_artwork_destination(root: &Path, import_id: &str, name: &str) -> PathBuf {
    let safe_name = name.replace(['/', '\\'], "-");
    let base = root.join(format!("import-{import_id}-{safe_name}"));
    if path_is_absent(&base) {
        return base;
    }
    let mut suffix = 1_u32;
    loop {
        let candidate = root.join(format!("import-{import_id}-{suffix}-{safe_name}"));
        if path_is_absent(&candidate) {
            return candidate;
        }
        suffix = suffix.saturating_add(1);
    }
}

fn path_is_absent(path: &Path) -> bool {
    fs::symlink_metadata(path).is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
}

fn ensure_trusted_restore_directory(root: &Path, directory: &Path) -> Result<(), ImportError> {
    if !directory.starts_with(root) {
        return Err(ImportError::Restore(format!(
            "restore directory {} is outside the trusted root",
            directory.display()
        )));
    }
    let root_metadata = fs::symlink_metadata(root).map_err(|source| ImportError::RestoreFile {
        path: root.to_path_buf(),
        source,
    })?;
    if root_metadata.file_type().is_symlink()
        || is_reparse_point(&root_metadata)
        || !root_metadata.is_dir()
    {
        return Err(ImportError::Restore(format!(
            "restore root {} is not a trusted directory",
            root.display()
        )));
    }
    let relative = directory.strip_prefix(root).map_err(|_| {
        ImportError::Restore(format!(
            "restore directory {} is outside the trusted root",
            directory.display()
        ))
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || is_reparse_point(&metadata) => {
                return Err(ImportError::Restore(format!(
                    "restore directory {} is a symbolic link or reparse point",
                    current.display()
                )))
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(ImportError::Restore(format!(
                    "restore path {} is not a directory",
                    current.display()
                )))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|source| ImportError::RestoreFile {
                    path: current.clone(),
                    source,
                })?;
            }
            Err(source) => {
                return Err(ImportError::RestoreFile {
                    path: current.clone(),
                    source,
                })
            }
        }
    }
    Ok(())
}

fn copy_new_file(source: &Path, destination: &Path) -> Result<(), ImportError> {
    let mut input = File::open(source).map_err(|error| ImportError::RestoreFile {
        path: source.to_path_buf(),
        source: error,
    })?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|source| ImportError::RestoreFile {
            path: destination.to_path_buf(),
            source,
        })?;
    let mut buffer = [0_u8; COPY_BUFFER_SIZE];
    let result = (|| -> Result<(), ImportError> {
        loop {
            let read = input
                .read(&mut buffer)
                .map_err(|error| ImportError::RestoreFile {
                    path: source.to_path_buf(),
                    source: error,
                })?;
            if read == 0 {
                break;
            }
            output
                .write_all(&buffer[..read])
                .map_err(|source| ImportError::RestoreFile {
                    path: destination.to_path_buf(),
                    source,
                })?;
        }
        output
            .sync_all()
            .map_err(|source| ImportError::RestoreFile {
                path: destination.to_path_buf(),
                source,
            })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

fn create_rollback_database(layout: &StorageLayout) -> Result<PathBuf, ImportError> {
    fs::create_dir_all(&layout.rollback_root).map_err(|source| ImportError::RestoreFile {
        path: layout.rollback_root.clone(),
        source,
    })?;
    let temporary = layout
        .rollback_root
        .join(format!(".spotdiy.sqlite3.rollback-{}.tmp", Uuid::new_v4()));
    let active = Database::open(&layout.database_path)?;
    active.online_backup_to(&temporary)?;
    drop(active);
    let destination = layout.rollback_root.join(format!(
        "spotdiy.sqlite3.rollback-{}.sqlite3",
        Uuid::new_v4()
    ));
    fs::rename(&temporary, &destination).map_err(|source| ImportError::RestoreFile {
        path: destination.clone(),
        source,
    })?;
    remove_database_sidecars(&temporary);
    Ok(destination)
}

fn replace_active_database(
    layout: &StorageLayout,
    descriptor: &PendingRestoreDescriptor,
) -> Result<(), ImportError> {
    let active = &layout.database_path;
    let staged = &descriptor.staged_database_path;
    remove_database_sidecars(staged);
    remove_database_sidecars(active);
    let previous = if active.exists() {
        let previous =
            active.with_file_name(format!(".spotdiy-previous-{}.sqlite3", Uuid::new_v4()));
        fs::rename(active, &previous).map_err(|source| ImportError::RestoreFile {
            path: active.clone(),
            source,
        })?;
        Some(previous)
    } else {
        None
    };
    if let Err(source) = fs::rename(staged, active) {
        if let Some(previous) = previous {
            let _ = fs::rename(&previous, active);
        }
        return Err(ImportError::RestoreFile {
            path: active.clone(),
            source,
        });
    }
    let restored = Database::open(active);
    let validation = match restored {
        Ok(database) => validate_database(&database, active),
        Err(error) => Err(ImportError::Database(error)),
    };
    if let Err(error) = validation {
        remove_database_artifacts(active);
        if let Some(previous) = previous {
            let _ = fs::rename(previous, active);
        }
        return Err(error);
    }
    if let Some(previous) = previous {
        let _ = fs::remove_file(previous);
    }
    Ok(())
}

fn restore_database_from_rollback(
    layout: &StorageLayout,
    rollback_path: &Path,
) -> Result<(), ImportError> {
    if !rollback_path.exists() || !rollback_path.starts_with(&layout.rollback_root) {
        return Err(ImportError::InvalidPendingDescriptor(
            "rollback path is missing or outside the rollback root".to_owned(),
        ));
    }
    let rollback = Database::open(rollback_path)?;
    let temporary = layout
        .restore_root
        .join(format!("rollback-restore-{}.sqlite3", Uuid::new_v4()));
    rollback.online_backup_to(&temporary)?;
    drop(rollback);
    let staged = PendingRestoreDescriptor {
        version: PENDING_RESTORE_VERSION,
        import_id: Uuid::new_v4().to_string(),
        state: PendingRestoreState::Pending,
        staged_root: layout.restore_root.clone(),
        staged_database_path: temporary.clone(),
        active_mode: layout.mode,
        music_destination: None,
        manifest: SpotDiyManifest {
            format_version: 1,
            app_version: "rollback".to_owned(),
            database_schema_version: LATEST_SCHEMA_VERSION,
            source_storage_mode: layout.mode,
            entries: Vec::new(),
            media_mappings: Vec::new(),
        },
        preview: ImportPreview {
            import_id: String::new(),
            archive_version: 1,
            app_version: "rollback".to_owned(),
            database_schema_version: LATEST_SCHEMA_VERSION,
            source_storage_mode: layout.mode,
            entry_count: 0,
            included_audio_count: 0,
            included_artwork_count: 0,
            included_sidecar_lyrics_count: 0,
            missing: MissingFileReport::default(),
            checksum_valid: true,
            restored_audio_planned_count: 0,
        },
        archive_sha256: String::new(),
        staged_database_sha256: hash_file(&temporary).map_err(|source| ImportError::StageFile {
            path: temporary.clone(),
            source,
        })?,
        rollback_path: None,
        created_paths: Vec::new(),
        last_error: None,
    };
    replace_active_database(layout, &staged)?;
    remove_database_artifacts(&temporary);
    Ok(())
}

fn recover_applying_descriptor(
    layout: &StorageLayout,
    descriptor: &mut PendingRestoreDescriptor,
) -> Result<(), ImportError> {
    if let Some(rollback_path) = descriptor.rollback_path.clone() {
        restore_database_from_rollback(layout, &rollback_path)?;
    }
    restore_staged_database_from_original(descriptor)?;
    cleanup_created_paths(layout, &descriptor.created_paths);
    descriptor.created_paths.clear();
    descriptor.rollback_path = None;
    descriptor.state = PendingRestoreState::Pending;
    descriptor.last_error = None;
    write_pending_descriptor(layout, descriptor)
}

fn staged_database_original_path(root: &Path) -> PathBuf {
    root.join(STAGED_DATABASE_BACKUP_FILE_NAME)
}

fn restore_staged_database_from_original(
    descriptor: &PendingRestoreDescriptor,
) -> Result<(), ImportError> {
    let original_path = staged_database_original_path(&descriptor.staged_root);
    let original = Database::open(&original_path)?;
    let temporary = descriptor
        .staged_root
        .join(format!(".restore-original-{}.sqlite3", Uuid::new_v4()));
    original.online_backup_to(&temporary)?;
    drop(original);
    remove_database_sidecars(&temporary);

    let previous = descriptor
        .staged_root
        .join(format!(".modified-staged-{}.sqlite3", Uuid::new_v4()));
    fs::rename(&descriptor.staged_database_path, &previous).map_err(|source| {
        remove_database_artifacts(&temporary);
        ImportError::RestoreFile {
            path: descriptor.staged_database_path.clone(),
            source,
        }
    })?;
    if let Err(source) = fs::rename(&temporary, &descriptor.staged_database_path) {
        let _ = fs::rename(&previous, &descriptor.staged_database_path);
        remove_database_artifacts(&temporary);
        return Err(ImportError::RestoreFile {
            path: descriptor.staged_database_path.clone(),
            source,
        });
    }
    remove_database_artifacts(&previous);
    Ok(())
}

fn cleanup_descriptor(
    layout: &StorageLayout,
    descriptor: &PendingRestoreDescriptor,
) -> Result<(), ImportError> {
    cleanup_staging_root_if_present(layout, &descriptor.staged_root)?;
    remove_pending_descriptor(layout)
}

fn cleanup_created_paths(layout: &StorageLayout, paths: &[PathBuf]) {
    for path in paths.iter().rev() {
        if path_is_under(&layout.music_root, path)
            || path_is_under(&layout.artwork_cache_root, path)
        {
            let _ = fs::remove_file(path);
        }
    }
}

pub(crate) fn cleanup_created_paths_for_cancel(layout: &StorageLayout, paths: &[PathBuf]) {
    cleanup_created_paths(layout, paths);
}

fn path_is_under(root: &Path, child: &Path) -> bool {
    if child == root {
        return false;
    }
    if child.starts_with(root) {
        return true;
    }
    let Ok(canonical_root) = fs::canonicalize(root) else {
        return false;
    };
    let Ok(canonical_child) = fs::canonicalize(child) else {
        return false;
    };
    canonical_child.starts_with(canonical_root)
}

fn retain_latest_rollback(layout: &StorageLayout, latest: &Path) {
    let Ok(entries) = fs::read_dir(&layout.rollback_root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path != latest
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("spotdiy.sqlite3.rollback-"))
        {
            let _ = fs::remove_file(path);
        }
    }
}

pub(crate) fn sidecar_entry_for_source<'manifest>(
    manifest: &'manifest SpotDiyManifest,
    source_id: &str,
) -> Option<&'manifest str> {
    manifest.entries.iter().find_map(|entry| {
        (entry.kind == SpotDiyArchiveEntryKind::SidecarLyrics
            && entry.path.starts_with(&format!("lyrics/{source_id}/")))
        .then_some(entry.path.as_str())
    })
}

pub(crate) fn artwork_entries(manifest: &SpotDiyManifest) -> impl Iterator<Item = &str> {
    manifest
        .entries
        .iter()
        .filter(|entry| entry.kind == SpotDiyArchiveEntryKind::Artwork)
        .map(|entry| entry.path.as_str())
}

pub(crate) fn staged_payload(root: &Path, archive_path: &str) -> Result<PathBuf, ImportError> {
    staged_payload_path(root, archive_path)
}

pub(crate) fn hash_file(path: &Path) -> Result<String, io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; COPY_BUFFER_SIZE];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(&hasher.finalize()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    hex_digest(&Sha256::digest(bytes))
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn remove_database_artifacts(path: &Path) {
    let _ = fs::remove_file(path);
    remove_database_sidecars(path);
}

fn remove_database_sidecars(path: &Path) {
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", path.display(), suffix));
        let _ = fs::remove_file(sidecar);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::*;
    use crate::backup::archive::write_archive;
    use crate::backup::manifest::{SpotDiyArchiveEntry, SpotDiyExportOptions};
    use crate::backup::BackupService;
    use rusqlite::params;
    use zip::write::SimpleFileOptions;

    #[test]
    fn metadata_archive_stages_without_touching_active_database() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        fs::create_dir_all(&exe).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        let database = Database::open(&layout.database_path).unwrap();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO schema_metadata(metadata_key, metadata_value, updated_at)
                     VALUES ('test_marker', 'before', 'now')",
                    [],
                )?;
                let track_id = Uuid::new_v4().to_string();
                let genre_track = connection.execute(
                    "INSERT INTO tracks (id, title, normalized_title,
                     created_at, updated_at)
                     VALUES (?1, 'Fixture', 'fixture', 'now', 'now')",
                    [&track_id],
                )?;
                assert_eq!(genre_track, 1);
                connection.execute(
                    "INSERT INTO track_genres (track_id, genre, normalized_genre)
                     VALUES (?1, 'Rock', 'rock')",
                    [&track_id],
                )?;
                let session_id = Uuid::new_v4().to_string();
                connection.execute(
                    "INSERT INTO listening_sessions
                     (id, started_at, ended_at, created_at, updated_at)
                     VALUES (?1, '2026-01-01T00:00:00Z', '2026-01-01T00:01:00Z',
                             '2026-01-01T00:01:00Z', '2026-01-01T00:01:00Z')",
                    [&session_id],
                )?;
                connection.execute(
                    "INSERT INTO play_history
                     (id, session_id, track_id, title_snapshot, artists_json,
                      started_at, ended_at, local_date, local_hour, local_weekday,
                      listened_ms, outcome, qualified_play, created_at)
                     VALUES (?1, ?2, ?3, 'Fixture', '[\"Artist\"]',
                             '2026-01-01T00:00:00Z', '2026-01-01T00:01:00Z',
                             '2026-01-01', 0, 4, 60000, 'completed', 1,
                             '2026-01-01T00:01:00Z')",
                    params![Uuid::new_v4().to_string(), session_id, track_id],
                )?;
                connection.execute(
                    "INSERT INTO smart_playlists
                     (id, name, normalized_name, rule_json, sort_mode,
                      sort_direction, created_at, updated_at)
                     VALUES (?1, 'Rock', 'rock',
                             '{\"type\":\"predicate\",\"field\":\"genre\",\"operation\":\"equals\",\"value\":\"rock\"}',
                             'title', 'asc', '2026-01-01T00:00:00Z',
                             '2026-01-01T00:00:00Z')",
                    [Uuid::new_v4().to_string()],
                )?;
                Ok(())
            })
            .unwrap();
        let archive_path = root.path().join("backup.spotdiy");
        write_archive(
            &database,
            &layout,
            "0.1.0",
            &SpotDiyExportOptions::default(),
            &archive_path,
        )
        .unwrap();
        let staged = stage_archive(&archive_path, &layout, StorageMode::Standard).unwrap();
        assert_eq!(staged.preview.entry_count, 1);
        let staged_database = Database::open(&staged.staged_database_path).unwrap();
        staged_database
            .with_connection(|connection| {
                for table in [
                    "track_genres",
                    "listening_sessions",
                    "play_history",
                    "smart_playlists",
                ] {
                    let count: i64 = connection.query_row(
                        "SELECT COUNT(*) FROM sqlite_master
                         WHERE type = 'table' AND name = ?1",
                        [table],
                        |row| row.get(0),
                    )?;
                    assert_eq!(count, 1, "missing table {table}");
                }
                let history_count: i64 =
                    connection
                        .query_row("SELECT COUNT(*) FROM play_history", [], |row| row.get(0))?;
                let smart_count: i64 =
                    connection
                        .query_row("SELECT COUNT(*) FROM smart_playlists", [], |row| row.get(0))?;
                assert_eq!(history_count, 1);
                assert_eq!(smart_count, 1);
                Ok(())
            })
            .unwrap();
        assert!(layout.database_path.exists());
        let marker: String = database
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT metadata_value FROM schema_metadata WHERE metadata_key = 'test_marker'",
                    [],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(marker, "before");
    }

    #[test]
    fn schema_eight_archive_is_migrated_to_nine_without_losing_prior_rows() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        fs::create_dir_all(&exe).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();

        let legacy_database_path = root.path().join("legacy.sqlite3");
        let legacy_database = Database::open(&legacy_database_path).unwrap();
        legacy_database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO schema_metadata(metadata_key, metadata_value, updated_at)
                     VALUES ('legacy_marker', 'preserved', '2026-01-01T00:00:00Z')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        drop(legacy_database);

        let connection = rusqlite::Connection::open(&legacy_database_path).unwrap();
        connection
            .execute_batch(
                "DROP TABLE smart_playlists;
                 DROP TABLE play_history;
                 DROP TABLE listening_sessions;
                 DROP TABLE track_genres;
                 UPDATE schema_metadata
                 SET metadata_value = '8'
                 WHERE metadata_key = 'schema_version';
                 PRAGMA user_version = 8;",
            )
            .unwrap();
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        drop(connection);

        let database_bytes = fs::read(&legacy_database_path).unwrap();
        let database_entry = SpotDiyArchiveEntry {
            path: DATABASE_ARCHIVE_PATH.to_owned(),
            kind: SpotDiyArchiveEntryKind::Database,
            size_bytes: database_bytes.len() as u64,
            sha256: digest_bytes(&database_bytes),
        };
        let manifest = SpotDiyManifest {
            format_version: super::super::manifest::SPOTDIY_ARCHIVE_FORMAT_VERSION,
            app_version: "0.1.0".to_owned(),
            database_schema_version: 8,
            source_storage_mode: StorageMode::Standard,
            entries: vec![database_entry],
            media_mappings: Vec::new(),
        };
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        let archive_path = root.path().join("schema-eight.spotdiy");
        let file = File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        writer.start_file(MANIFEST_PATH, options).unwrap();
        writer.write_all(&manifest_bytes).unwrap();
        writer.start_file(MANIFEST_CHECKSUM_PATH, options).unwrap();
        writer
            .write_all(format!("{}\n", digest_bytes(&manifest_bytes)).as_bytes())
            .unwrap();
        writer.start_file(DATABASE_ARCHIVE_PATH, options).unwrap();
        writer.write_all(&database_bytes).unwrap();
        writer.finish().unwrap();

        let staged = stage_archive(&archive_path, &layout, StorageMode::Standard).unwrap();
        let staged_database = Database::open(&staged.staged_database_path).unwrap();
        assert_eq!(
            staged_database.schema_version().unwrap(),
            LATEST_SCHEMA_VERSION
        );
        let marker: String = staged_database
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT metadata_value FROM schema_metadata WHERE metadata_key = 'legacy_marker'",
                    [],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(marker, "preserved");
        staged_database
            .with_connection(|connection| {
                for table in [
                    "track_genres",
                    "listening_sessions",
                    "play_history",
                    "smart_playlists",
                ] {
                    let count: i64 = connection.query_row(
                        "SELECT COUNT(*) FROM sqlite_master
                         WHERE type = 'table' AND name = ?1",
                        [table],
                        |row| row.get(0),
                    )?;
                    assert_eq!(count, 1, "missing table {table}");
                }
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn unsafe_archive_paths_are_rejected_before_staging() {
        let root = tempfile::tempdir().unwrap();
        let file_path = root.path().join("unsafe.spotdiy");
        let file = File::create(&file_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("../escape", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"bad").unwrap();
        writer.finish().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        fs::create_dir_all(&exe).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        assert!(matches!(
            stage_archive(&file_path, &layout, StorageMode::Standard),
            Err(ImportError::UnsafeEntryPath(_))
        ));
    }

    #[test]
    fn staging_rejects_a_non_directory_imports_component() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        fs::create_dir_all(&exe).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        fs::write(layout.restore_root.join("imports"), b"not a directory").unwrap();

        assert!(matches!(
            create_staging_root(&layout, Uuid::new_v4()),
            Err(ImportError::CreateStaging { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn staging_rejects_a_symlink_imports_component() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        let outside = root.path().join("outside");
        fs::create_dir_all(&exe).unwrap();
        fs::create_dir(&outside).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        fs::remove_dir(&layout.restore_root.join("imports")).unwrap();
        symlink(&outside, layout.restore_root.join("imports")).unwrap();

        assert!(matches!(
            create_staging_root(&layout, Uuid::new_v4()),
            Err(ImportError::CreateStaging { .. })
        ));
        assert!(fs::read_dir(&outside).unwrap().next().is_none());
    }

    #[test]
    fn cleanup_refuses_an_unowned_staging_directory() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        fs::create_dir_all(&exe).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        let foreign = layout.restore_root.join("foreign");
        fs::create_dir(&foreign).unwrap();
        let marker = foreign.join("keep");
        fs::write(&marker, b"keep").unwrap();

        assert!(cleanup_staged_root(&layout, &foreign).is_err());
        assert!(marker.exists());

        let owned = create_staging_root(&layout, Uuid::new_v4()).unwrap();
        fs::write(owned.join("owned"), b"owned").unwrap();
        cleanup_staged_root(&layout, &owned).unwrap();
        assert!(!owned.exists());
    }

    #[test]
    fn staging_rejects_a_destination_outside_the_generated_root() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        fs::create_dir_all(&exe).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        let owned = create_staging_root(&layout, Uuid::new_v4()).unwrap();

        assert!(ensure_trusted_directory(&owned, &layout.restore_root).is_err());
        cleanup_staged_root(&layout, &owned).unwrap();
    }

    #[test]
    fn staging_rejects_a_file_payload_parent_without_writing_outside() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        fs::create_dir_all(&exe).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        let owned = create_staging_root(&layout, Uuid::new_v4()).unwrap();
        let payloads = owned.join("payloads");
        fs::create_dir(&payloads).unwrap();
        fs::write(payloads.join("escape"), b"not a directory").unwrap();
        let destination = payloads.join("escape").join("payload.bin");

        assert!(ensure_trusted_directory(&owned, destination.parent().unwrap()).is_err());
        assert!(!layout.restore_root.join("payload.bin").exists());
        cleanup_staged_root(&layout, &owned).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn staging_rejects_a_symlink_payload_parent_without_writing_outside() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        let outside = root.path().join("outside");
        fs::create_dir_all(&exe).unwrap();
        fs::create_dir(&outside).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        let owned = create_staging_root(&layout, Uuid::new_v4()).unwrap();
        let payloads = owned.join("payloads");
        fs::create_dir(&payloads).unwrap();
        symlink(&outside, payloads.join("escape")).unwrap();
        let destination = payloads.join("escape").join("payload.bin");

        assert!(ensure_trusted_directory(&owned, destination.parent().unwrap()).is_err());
        assert!(!outside.join("payload.bin").exists());
        cleanup_staged_root(&layout, &owned).unwrap();
    }

    #[test]
    fn manifest_checksum_requires_the_exact_exported_bytes() {
        let manifest_bytes = br#"{"formatVersion":1}"#;
        let checksum = format!("{}\n", digest_bytes(manifest_bytes));
        assert!(validate_manifest_checksum(manifest_bytes, checksum.as_bytes()).is_ok());
        assert!(
            validate_manifest_checksum(manifest_bytes, format!("{checksum}extra").as_bytes())
                .is_err()
        );
        assert!(
            validate_manifest_checksum(manifest_bytes, checksum.trim_end().as_bytes()).is_err()
        );
    }

    #[test]
    fn declared_payload_validation_rejects_missing_and_undeclared_files() {
        let manifest = SpotDiyManifest {
            format_version: 1,
            app_version: "0.1.0".to_owned(),
            database_schema_version: LATEST_SCHEMA_VERSION,
            source_storage_mode: StorageMode::Standard,
            entries: vec![SpotDiyArchiveEntry {
                path: DATABASE_ARCHIVE_PATH.to_owned(),
                kind: SpotDiyArchiveEntryKind::Database,
                size_bytes: 0,
                sha256: "a".repeat(64),
            }],
            media_mappings: Vec::new(),
        };
        let metadata = vec![
            ArchiveEntryInfo {
                index: 0,
                name: MANIFEST_PATH.to_owned(),
            },
            ArchiveEntryInfo {
                index: 1,
                name: MANIFEST_CHECKSUM_PATH.to_owned(),
            },
            ArchiveEntryInfo {
                index: 2,
                name: DATABASE_ARCHIVE_PATH.to_owned(),
            },
            ArchiveEntryInfo {
                index: 3,
                name: "covers/undeclared.png".to_owned(),
            },
        ];
        assert!(matches!(
            validate_declared_payloads(&metadata, &manifest),
            Err(ImportError::UndeclaredPayload(_))
        ));
        assert!(matches!(
            validate_declared_payloads(
                &metadata[..3],
                &SpotDiyManifest {
                    entries: vec![
                        manifest.entries[0].clone(),
                        SpotDiyArchiveEntry {
                            path: "covers/missing.png".to_owned(),
                            kind: SpotDiyArchiveEntryKind::Artwork,
                            size_bytes: 0,
                            sha256: "b".repeat(64),
                        },
                    ],
                    ..manifest.clone()
                }
            ),
            Err(ImportError::MissingPayload(_))
        ));
    }

    #[test]
    fn archive_inspection_rejects_case_duplicates_symlinks_and_bombs() {
        let root = tempfile::tempdir().unwrap();
        let duplicate_path = root.path().join("duplicate.spotdiy");
        let mut writer = zip::ZipWriter::new(File::create(&duplicate_path).unwrap());
        writer
            .start_file("covers/art.png", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"one").unwrap();
        writer
            .start_file("covers/ART.png", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"two").unwrap();
        writer.finish().unwrap();
        let mut archive = ZipArchive::new(File::open(&duplicate_path).unwrap()).unwrap();
        assert!(matches!(
            inspect_archive(&mut archive),
            Err(ImportError::DuplicatePath(_))
        ));

        let mut archive = ZipArchive::new(std::io::Cursor::new(raw_symlink_zip())).unwrap();
        assert!(matches!(
            inspect_archive(&mut archive),
            Err(ImportError::SymlinkEntry(_))
        ));

        let bomb_path = root.path().join("bomb.spotdiy");
        let mut writer = zip::ZipWriter::new(File::create(&bomb_path).unwrap());
        writer
            .start_file(
                "covers/zeros.bin",
                zip::write::SimpleFileOptions::default()
                    .compression_method(CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(&vec![0_u8; 2 * 1024 * 1024]).unwrap();
        writer.finish().unwrap();
        let mut archive = ZipArchive::new(File::open(&bomb_path).unwrap()).unwrap();
        assert!(matches!(
            inspect_archive(&mut archive),
            Err(ImportError::CompressionRatio(_))
        ));
    }

    fn raw_symlink_zip() -> Vec<u8> {
        let name = b"covers/link.png";
        let data = b"link";
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(data);

        let central_offset = bytes.len() as u32;
        bytes.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
        bytes.extend_from_slice(&0x0314_u16.to_le_bytes());
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&((0o120777_u32) << 16).to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(name);

        let central_size = bytes.len() as u32 - central_offset;
        bytes.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&central_size.to_le_bytes());
        bytes.extend_from_slice(&central_offset.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes
    }

    #[test]
    fn applying_descriptor_recovery_is_restart_safe() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        fs::create_dir_all(&exe).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        let database = Database::open(&layout.database_path).unwrap();
        let archive_path = root.path().join("recovery.spotdiy");
        write_archive(
            &database,
            &layout,
            "0.1.0",
            &SpotDiyExportOptions::default(),
            &archive_path,
        )
        .unwrap();
        let service = BackupService::new(database, layout.clone(), "0.1.0").unwrap();
        let preview = service.stage_import(&archive_path).unwrap();
        service.commit_import(&preview.import_id, None).unwrap();
        drop(service);

        let mut descriptor = read_pending_descriptor(&layout).unwrap().unwrap();
        descriptor.state = PendingRestoreState::Applying;
        write_pending_descriptor(&layout, &descriptor).unwrap();
        let report = BackupService::startup_restore(&layout).unwrap();
        assert!(report.applied, "{report:?}");
        assert!(!pending_path(&layout).exists());
    }

    #[test]
    fn included_audio_and_sidecar_restore_rewrites_only_staged_database() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        let source_root = root.path().join("source");
        let restore_root = root.path().join("restored-music");
        fs::create_dir_all(&exe).unwrap();
        fs::create_dir_all(&source_root).unwrap();
        fs::create_dir_all(&restore_root).unwrap();
        let audio = source_root.join("fixture.flac");
        fs::write(&audio, b"synthetic audio").unwrap();
        fs::write(source_root.join("fixture.lrc"), b"[00:00.00] fixture\n").unwrap();

        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        let database = Database::open(&layout.database_path).unwrap();
        let folder_id = Uuid::new_v4();
        let track_id = Uuid::new_v4();
        let source_id = Uuid::new_v4();
        let existing_restore_path = restore_root.join(format!("{source_id}.flac"));
        fs::write(&existing_restore_path, b"pre-existing media").unwrap();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO library_folders
                     (id, path, normalized_path_key, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'now', 'now')",
                    rusqlite::params![
                        folder_id.to_string(),
                        source_root.to_string_lossy().to_string(),
                        source_root.to_string_lossy().to_string().to_lowercase()
                    ],
                )?;
                connection.execute(
                    "INSERT INTO tracks
                     (id, title, normalized_title, created_at, updated_at)
                     VALUES (?1, 'Fixture', 'fixture', 'now', 'now')",
                    [track_id.to_string()],
                )?;
                connection.execute(
                    "INSERT INTO track_sources
                     (id, track_id, provider_kind, provider_item_id, created_at, updated_at)
                     VALUES (?1, ?2, 'local', 'fixture', 'now', 'now')",
                    rusqlite::params![source_id.to_string(), track_id.to_string()],
                )?;
                connection.execute(
                    "INSERT INTO local_files
                     (source_id, path, library_folder_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'now', 'now')",
                    rusqlite::params![
                        source_id.to_string(),
                        audio.to_string_lossy().to_string(),
                        folder_id.to_string()
                    ],
                )?;
                Ok(())
            })
            .unwrap();

        let archive_path = root.path().join("audio.spotdiy");
        write_archive(
            &database,
            &layout,
            "0.1.0",
            &SpotDiyExportOptions {
                include_local_audio: true,
                include_artwork_cache: false,
                include_sidecar_lyrics: true,
            },
            &archive_path,
        )
        .unwrap();
        let service = BackupService::new(database, layout.clone(), "0.1.0").unwrap();
        let preview = service.stage_import(&archive_path).unwrap();
        assert_eq!(preview.included_audio_count, 1);
        assert_eq!(preview.included_sidecar_lyrics_count, 1);
        service
            .commit_import(&preview.import_id, Some(restore_root.clone()))
            .unwrap();
        drop(service);
        let report = BackupService::startup_restore(&layout).unwrap();
        assert!(report.applied, "{report:?}");
        assert!(report.rollback_path.is_some());

        let restored = Database::open(&layout.database_path).unwrap();
        let restored_path: String = restored
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT path FROM local_files WHERE source_id = ?1",
                    [source_id.to_string()],
                    |row| row.get(0),
                )
            })
            .unwrap();
        let restored_path = PathBuf::from(restored_path);
        assert!(
            fs::canonicalize(&restored_path)
                .unwrap()
                .starts_with(fs::canonicalize(&restore_root).unwrap()),
            "restored path was {}",
            restored_path.display()
        );
        assert_eq!(fs::read(&restored_path).unwrap(), b"synthetic audio");
        assert_eq!(
            fs::read(existing_restore_path).unwrap(),
            b"pre-existing media"
        );
        assert_eq!(
            fs::read(restored_path.with_extension("lrc")).unwrap(),
            b"[00:00.00] fixture\n"
        );
        assert!(!layout.restore_root.join(PENDING_RESTORE_FILE_NAME).exists());
    }

    #[test]
    fn portable_restore_uses_executable_music_root_and_preserves_mode() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("portable-app");
        let local = root.path().join("local");
        let source_root = root.path().join("portable-source");
        fs::create_dir_all(&exe).unwrap();
        fs::create_dir_all(&source_root).unwrap();
        fs::write(
            exe.join(crate::storage::PORTABLE_MARKER_FILE_NAME),
            b"marker",
        )
        .unwrap();
        let source_audio = source_root.join("fixture.ogg");
        fs::write(&source_audio, b"portable fixture").unwrap();
        let layout = StorageLayout::resolve(&exe, &local).unwrap();
        let database = Database::open(&layout.database_path).unwrap();
        let folder_id = Uuid::new_v4();
        let track_id = Uuid::new_v4();
        let source_id = Uuid::new_v4();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO library_folders
                     (id, path, normalized_path_key, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'now', 'now')",
                    rusqlite::params![
                        folder_id.to_string(),
                        source_root.to_string_lossy().to_string(),
                        source_root.to_string_lossy().to_string().to_lowercase()
                    ],
                )?;
                connection.execute(
                    "INSERT INTO tracks
                     (id, title, normalized_title, created_at, updated_at)
                     VALUES (?1, 'Portable Fixture', 'portable fixture', 'now', 'now')",
                    [track_id.to_string()],
                )?;
                connection.execute(
                    "INSERT INTO track_sources
                     (id, track_id, provider_kind, provider_item_id, created_at, updated_at)
                     VALUES (?1, ?2, 'local', 'portable-fixture', 'now', 'now')",
                    rusqlite::params![source_id.to_string(), track_id.to_string()],
                )?;
                connection.execute(
                    "INSERT INTO local_files
                     (source_id, path, library_folder_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'now', 'now')",
                    rusqlite::params![
                        source_id.to_string(),
                        source_audio.to_string_lossy().to_string(),
                        folder_id.to_string()
                    ],
                )?;
                Ok(())
            })
            .unwrap();
        let archive_path = root.path().join("portable.spotdiy");
        write_archive(
            &database,
            &layout,
            "0.1.0",
            &SpotDiyExportOptions {
                include_local_audio: true,
                ..SpotDiyExportOptions::default()
            },
            &archive_path,
        )
        .unwrap();
        let service = BackupService::new(database, layout.clone(), "0.1.0").unwrap();
        let preview = service.stage_import(&archive_path).unwrap();
        service.commit_import(&preview.import_id, None).unwrap();
        drop(service);
        let report = BackupService::startup_restore(&layout).unwrap();
        assert!(report.applied, "{report:?}");
        let restored = Database::open(&layout.database_path).unwrap();
        let restored_path: String = restored
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT path FROM local_files WHERE source_id = ?1",
                    [source_id.to_string()],
                    |row| row.get(0),
                )
            })
            .unwrap();
        let restored_path = PathBuf::from(restored_path);
        assert!(
            fs::canonicalize(&restored_path)
                .unwrap()
                .starts_with(fs::canonicalize(&layout.music_root).unwrap()),
            "restored path {} was not under {}",
            restored_path.display(),
            layout.music_root.display()
        );
        assert_eq!(fs::read(restored_path).unwrap(), b"portable fixture");
        assert_eq!(
            SettingsRepository::new(&restored)
                .get_snapshot()
                .unwrap()
                .storage_mode,
            StorageMode::Portable
        );
    }
}
