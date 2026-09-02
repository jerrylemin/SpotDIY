use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::db::{Database, DatabaseError, LATEST_SCHEMA_VERSION};
use crate::domain::ProviderKind;
use crate::library::folders::{
    is_path_within, is_reparse_point, normalize_file_path, normalize_folder_path,
};
use crate::storage::{StorageLayout, StorageMode};

use super::manifest::{
    validate_archive_payload_path, validate_manifest, SpotDiyArchiveEntry, SpotDiyArchiveEntryKind,
    SpotDiyExportOptions, SpotDiyManifest, SpotDiyMediaMapping, DATABASE_ARCHIVE_PATH,
    MANIFEST_CHECKSUM_PATH, MANIFEST_PATH, MAX_ARCHIVE_ENTRIES, MAX_DATABASE_BYTES,
    MAX_TOTAL_UNCOMPRESSED_BYTES, SPOTDIY_ARCHIVE_FORMAT_VERSION,
};

const ZIP_BUFFER_SIZE: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportResult {
    pub destination: PathBuf,
    pub manifest: SpotDiyManifest,
    pub manifest_sha256: String,
}

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("archive filesystem operation {operation} failed for {path}: {source}")]
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("archive serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("archive ZIP operation failed: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("archive I/O operation failed: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Manifest(#[from] super::manifest::ManifestError),
    #[error("archive path is not valid UTF-8: {0}")]
    NonUnicodePath(PathBuf),
    #[error("archive source path is not inside the trusted library folder: {0}")]
    UnmanagedLocalFile(PathBuf),
    #[error("archive source ID is invalid: {0}")]
    InvalidSourceId(String),
    #[error("archive contains too many entries")]
    TooManyEntries,
    #[error("archive payload is larger than the supported limit: {path}")]
    PayloadTooLarge { path: String },
    #[error("archive payload total exceeds the supported limit")]
    TotalPayloadTooLarge,
    #[error("archive destination already exists: {0}")]
    DestinationExists(PathBuf),
    #[error("sidecar lyrics can only be included with local audio")]
    SidecarRequiresAudio,
    #[error("database snapshot validation failed for {path}: {detail}")]
    InvalidDatabase { path: PathBuf, detail: String },
}

#[derive(Clone, Debug)]
struct Payload {
    archive_path: String,
    source_path: PathBuf,
    kind: SpotDiyArchiveEntryKind,
    size_bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug)]
struct LocalAudioReference {
    source_id: String,
    path: PathBuf,
    library_folder_id: Option<String>,
}

pub fn write_archive(
    database: &Database,
    layout: &StorageLayout,
    app_version: &str,
    options: &SpotDiyExportOptions,
    destination: impl AsRef<Path>,
) -> Result<ExportResult, ArchiveError> {
    let destination = destination.as_ref().to_path_buf();
    if options.include_sidecar_lyrics && !options.include_local_audio {
        return Err(ArchiveError::SidecarRequiresAudio);
    }
    if destination.exists() {
        return Err(ArchiveError::DestinationExists(destination));
    }
    fs::create_dir_all(&layout.restore_root).map_err(|source| ArchiveError::Filesystem {
        operation: "create archive workspace",
        path: layout.restore_root.clone(),
        source,
    })?;

    let snapshot_path = layout
        .restore_root
        .join(format!("export-snapshot-{}.sqlite3", Uuid::new_v4()));
    let result = write_archive_with_snapshot(
        database,
        layout,
        app_version,
        options,
        &destination,
        &snapshot_path,
    );
    remove_database_artifacts(&snapshot_path);
    result
}

fn write_archive_with_snapshot(
    database: &Database,
    layout: &StorageLayout,
    app_version: &str,
    options: &SpotDiyExportOptions,
    destination: &Path,
    snapshot_path: &Path,
) -> Result<ExportResult, ArchiveError> {
    database.online_backup_to(snapshot_path)?;
    let snapshot = Database::open(snapshot_path)?;
    validate_database_snapshot(&snapshot, snapshot_path)?;

    let database_size = file_size(snapshot_path)?;
    if database_size > MAX_DATABASE_BYTES {
        return Err(ArchiveError::PayloadTooLarge {
            path: DATABASE_ARCHIVE_PATH.to_owned(),
        });
    }

    let mut payloads = vec![Payload {
        archive_path: DATABASE_ARCHIVE_PATH.to_owned(),
        source_path: snapshot_path.to_path_buf(),
        kind: SpotDiyArchiveEntryKind::Database,
        size_bytes: database_size,
        sha256: hash_file(snapshot_path)?,
    }];
    let mut media_mappings = Vec::new();

    if options.include_local_audio {
        let local_audio = local_audio_references(database)?;
        for reference in local_audio {
            let Some((source_path, _normalized_key)) = trusted_local_path(database, &reference)?
            else {
                continue;
            };
            let source_uuid = Uuid::parse_str(&reference.source_id)
                .map_err(|_| ArchiveError::InvalidSourceId(reference.source_id.clone()))?;
            let extension = safe_extension(&source_path).unwrap_or_else(|| "bin".to_owned());
            let archive_path = format!("media/{source_uuid}/media.{extension}");
            let payload = payload_from_file(
                archive_path.clone(),
                source_path.clone(),
                SpotDiyArchiveEntryKind::LocalAudio,
            )?;
            payloads.push(payload);
            media_mappings.push(SpotDiyMediaMapping {
                source_id: source_uuid.to_string(),
                archive_path: archive_path.clone(),
            });

            if options.include_sidecar_lyrics {
                let sidecar = source_path.with_extension("lrc");
                if let Some((sidecar_path, _)) =
                    trusted_local_sidecar(database, &reference, &sidecar)?
                {
                    payloads.push(payload_from_file(
                        format!("lyrics/{source_uuid}/media.lrc"),
                        sidecar_path,
                        SpotDiyArchiveEntryKind::SidecarLyrics,
                    )?);
                }
            }
        }
    }

    if options.include_artwork_cache {
        payloads.extend(artwork_payloads(layout)?);
    }

    payloads.sort_by(|left, right| left.archive_path.cmp(&right.archive_path));
    if payloads.len() > MAX_ARCHIVE_ENTRIES {
        return Err(ArchiveError::TooManyEntries);
    }
    let total_bytes = payloads.iter().try_fold(0_u64, |total, payload| {
        total
            .checked_add(payload.size_bytes)
            .ok_or(ArchiveError::TotalPayloadTooLarge)
    })?;
    if total_bytes > MAX_TOTAL_UNCOMPRESSED_BYTES {
        return Err(ArchiveError::TotalPayloadTooLarge);
    }

    media_mappings.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    let manifest = SpotDiyManifest {
        format_version: SPOTDIY_ARCHIVE_FORMAT_VERSION,
        app_version: app_version.to_owned(),
        database_schema_version: snapshot.schema_version()?,
        source_storage_mode: storage_mode(&snapshot)?,
        entries: payloads
            .iter()
            .map(|payload| SpotDiyArchiveEntry {
                path: payload.archive_path.clone(),
                kind: payload.kind,
                size_bytes: payload.size_bytes,
                sha256: payload.sha256.clone(),
            })
            .collect(),
        media_mappings,
    };
    validate_manifest(&manifest)?;
    let manifest_bytes = serde_json::to_vec(&manifest)?;
    if manifest_bytes.len() as u64 > super::manifest::MAX_MANIFEST_BYTES {
        return Err(ArchiveError::PayloadTooLarge {
            path: MANIFEST_PATH.to_owned(),
        });
    }
    let manifest_sha256 = digest_bytes(&manifest_bytes);
    let manifest_checksum_bytes = format!("{manifest_sha256}\n").into_bytes();

    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent).map_err(|source| ArchiveError::Filesystem {
            operation: "create archive destination directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let archive_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|source| ArchiveError::Filesystem {
            operation: "create archive",
            path: destination.to_path_buf(),
            source,
        })?;
    let mut writer = ZipWriter::new(archive_file);
    let options = fixed_zip_options()?;
    writer.start_file(MANIFEST_PATH, options)?;
    writer.write_all(&manifest_bytes)?;
    writer.start_file(MANIFEST_CHECKSUM_PATH, options)?;
    writer.write_all(&manifest_checksum_bytes)?;
    for payload in payloads {
        writer.start_file(&payload.archive_path, options)?;
        copy_file_to_zip(&payload.source_path, &mut writer)?;
    }
    let archive_file = writer.finish()?;
    archive_file
        .sync_all()
        .map_err(|source| ArchiveError::Filesystem {
            operation: "sync archive",
            path: destination.to_path_buf(),
            source,
        })?;

    Ok(ExportResult {
        destination: destination.to_path_buf(),
        manifest,
        manifest_sha256,
    })
}

fn fixed_zip_options() -> Result<SimpleFileOptions, ArchiveError> {
    let timestamp = DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0).map_err(|error| {
        ArchiveError::Zip(zip::result::ZipError::Io(io::Error::other(
            error.to_string(),
        )))
    })?;
    Ok(SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(timestamp)
        .unix_permissions(0o644))
}

fn copy_file_to_zip(path: &Path, writer: &mut ZipWriter<File>) -> Result<(), ArchiveError> {
    let mut source = File::open(path).map_err(|source| ArchiveError::Filesystem {
        operation: "open archive payload",
        path: path.to_path_buf(),
        source,
    })?;
    let mut buffer = [0_u8; ZIP_BUFFER_SIZE];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|source| ArchiveError::Filesystem {
                operation: "read archive payload",
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
    }
    Ok(())
}

fn payload_from_file(
    archive_path: String,
    source_path: PathBuf,
    kind: SpotDiyArchiveEntryKind,
) -> Result<Payload, ArchiveError> {
    validate_archive_payload_path(&archive_path)?;
    let size_bytes = file_size(&source_path)?;
    let maximum = if kind == SpotDiyArchiveEntryKind::Database {
        MAX_DATABASE_BYTES
    } else {
        MAX_TOTAL_UNCOMPRESSED_BYTES
    };
    if size_bytes > maximum {
        return Err(ArchiveError::PayloadTooLarge { path: archive_path });
    }
    Ok(Payload {
        archive_path,
        source_path: source_path.clone(),
        kind,
        size_bytes,
        sha256: hash_file(&source_path)?,
    })
}

fn local_audio_references(database: &Database) -> Result<Vec<LocalAudioReference>, ArchiveError> {
    database
        .with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT lf.source_id, lf.path, lf.library_folder_id
                 FROM local_files lf
                 JOIN track_sources ts ON ts.id = lf.source_id
                 WHERE ts.provider_kind = ?1
                 ORDER BY lf.source_id",
            )?;
            let rows = statement.query_map([ProviderKind::Local.to_string()], |row| {
                Ok(LocalAudioReference {
                    source_id: row.get(0)?,
                    path: PathBuf::from(row.get::<_, String>(1)?),
                    library_folder_id: row.get(2)?,
                })
            })?;
            rows.collect()
        })
        .map_err(ArchiveError::from)
}

fn trusted_local_path(
    database: &Database,
    reference: &LocalAudioReference,
) -> Result<Option<(PathBuf, String)>, ArchiveError> {
    let Some(folder_id) = reference.library_folder_id.as_deref() else {
        return Ok(None);
    };
    let Some(folder) = library_folder(database, folder_id)? else {
        return Ok(None);
    };
    let normalized_folder = match normalize_folder_path(folder) {
        Ok(folder) => folder,
        Err(_) => return Ok(None),
    };
    let (display_path, normalized_path_key) = match normalize_file_path(&reference.path) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    if !is_path_within(&normalized_folder.normalized_path_key, &normalized_path_key) {
        return Err(ArchiveError::UnmanagedLocalFile(display_path));
    }
    Ok(Some((display_path, normalized_path_key)))
}

fn trusted_local_sidecar(
    database: &Database,
    reference: &LocalAudioReference,
    sidecar: &Path,
) -> Result<Option<(PathBuf, String)>, ArchiveError> {
    let sidecar_reference = LocalAudioReference {
        source_id: reference.source_id.clone(),
        path: sidecar.to_path_buf(),
        library_folder_id: reference.library_folder_id.clone(),
    };
    match trusted_local_path(database, &sidecar_reference) {
        Ok(value) => Ok(value),
        Err(ArchiveError::UnmanagedLocalFile(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn library_folder(database: &Database, folder_id: &str) -> Result<Option<PathBuf>, ArchiveError> {
    database
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT path FROM library_folders WHERE id = ?1 AND enabled = 1",
                    [folder_id],
                    |row| row.get::<_, String>(0).map(PathBuf::from),
                )
                .optional()
        })
        .map_err(ArchiveError::from)
}

fn artwork_payloads(layout: &StorageLayout) -> Result<Vec<Payload>, ArchiveError> {
    let cache_metadata = match fs::symlink_metadata(&layout.artwork_cache_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(ArchiveError::Filesystem {
                operation: "inspect artwork cache",
                path: layout.artwork_cache_root.clone(),
                source,
            })
        }
    };
    if cache_metadata.file_type().is_symlink()
        || is_reparse_point(&cache_metadata)
        || !cache_metadata.is_dir()
    {
        return Err(ArchiveError::UnmanagedLocalFile(
            layout.artwork_cache_root.clone(),
        ));
    }
    let root = normalize_folder_path(&layout.artwork_cache_root)
        .map_err(|error| ArchiveError::Filesystem {
            operation: "resolve artwork cache",
            path: layout.artwork_cache_root.clone(),
            source: io::Error::other(error.to_string()),
        })?
        .display_path;
    let mut payloads = Vec::new();
    for entry in WalkDir::new(&root).follow_links(false) {
        let entry = entry.map_err(|source| ArchiveError::Filesystem {
            operation: "enumerate artwork cache",
            path: root.clone(),
            source: io::Error::other(source.to_string()),
        })?;
        if entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        let (display_path, _) = normalize_file_path(path)
            .map_err(|_| ArchiveError::UnmanagedLocalFile(path.to_path_buf()))?;
        if !display_path.starts_with(&root) {
            return Err(ArchiveError::UnmanagedLocalFile(display_path));
        }
        let relative = display_path
            .strip_prefix(&root)
            .map_err(|_| ArchiveError::UnmanagedLocalFile(display_path.clone()))?;
        let relative = relative
            .to_str()
            .ok_or_else(|| ArchiveError::NonUnicodePath(relative.to_path_buf()))?
            .replace('\\', "/");
        let archive_path = format!("covers/{relative}");
        payloads.push(payload_from_file(
            archive_path,
            display_path,
            SpotDiyArchiveEntryKind::Artwork,
        )?);
    }
    Ok(payloads)
}

fn storage_mode(database: &Database) -> Result<StorageMode, ArchiveError> {
    let value = crate::settings::SettingsRepository::new(database)
        .get_snapshot()
        .map_err(|error| ArchiveError::InvalidDatabase {
            path: database.path().to_path_buf(),
            detail: error.to_string(),
        })?
        .storage_mode;
    Ok(value)
}

fn validate_database_snapshot(database: &Database, path: &Path) -> Result<(), ArchiveError> {
    let schema = database.schema_version()?;
    if schema > LATEST_SCHEMA_VERSION {
        return Err(ArchiveError::InvalidDatabase {
            path: path.to_path_buf(),
            detail: format!("schema {schema} is newer than {LATEST_SCHEMA_VERSION}"),
        });
    }
    let user_version: i64 = database.with_connection(|connection| {
        connection.pragma_query_value(None, "user_version", |row| row.get(0))
    })?;
    if user_version > i64::from(LATEST_SCHEMA_VERSION) {
        return Err(ArchiveError::InvalidDatabase {
            path: path.to_path_buf(),
            detail: format!("user_version {user_version} is newer than {LATEST_SCHEMA_VERSION}"),
        });
    }
    let integrity: String = database.with_connection(|connection| {
        connection.pragma_query_value(None, "integrity_check", |row| row.get(0))
    })?;
    if integrity != "ok" {
        return Err(ArchiveError::InvalidDatabase {
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
        return Err(ArchiveError::InvalidDatabase {
            path: path.to_path_buf(),
            detail: format!("foreign_key_check returned {foreign_key_count} row(s)"),
        });
    }
    Ok(())
}

fn file_size(path: &Path) -> Result<u64, ArchiveError> {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|source| ArchiveError::Filesystem {
            operation: "read archive payload metadata",
            path: path.to_path_buf(),
            source,
        })
}

fn hash_file(path: &Path) -> Result<String, ArchiveError> {
    let mut file = File::open(path).map_err(|source| ArchiveError::Filesystem {
        operation: "open file for hashing",
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; ZIP_BUFFER_SIZE];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| ArchiveError::Filesystem {
                operation: "read file for hashing",
                path: path.to_path_buf(),
                source,
            })?;
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

fn safe_extension(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if (1..=16).contains(&extension.len())
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        Some(extension)
    } else {
        None
    }
}

fn remove_database_artifacts(path: &Path) {
    let _ = fs::remove_file(path);
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", path.display(), suffix));
        let _ = fs::remove_file(sidecar);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::storage::StorageLayout;

    #[test]
    fn metadata_export_is_readable_and_has_fixed_entries() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        fs::create_dir_all(&exe).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        let database = Database::open(&layout.database_path).unwrap();
        let destination = root.path().join("backup.spotdiy");
        let result = write_archive(
            &database,
            &layout,
            "0.1.0",
            &SpotDiyExportOptions::default(),
            &destination,
        )
        .unwrap();
        assert_eq!(result.manifest.entries.len(), 1);
        assert_eq!(result.manifest.entries[0].path, DATABASE_ARCHIVE_PATH);
        let mut archive = zip::ZipArchive::new(File::open(destination).unwrap()).unwrap();
        let names = archive.file_names().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                MANIFEST_PATH.to_owned(),
                MANIFEST_CHECKSUM_PATH.to_owned(),
                DATABASE_ARCHIVE_PATH.to_owned()
            ]
        );
        let database_entry = archive.by_name(DATABASE_ARCHIVE_PATH).unwrap();
        assert_eq!(database_entry.last_modified().unwrap().year(), 1980);
    }

    #[test]
    fn artwork_export_includes_only_the_trusted_cache() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        fs::create_dir_all(&exe).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        let artwork = layout.artwork_cache_root.join("nested").join("cover.png");
        fs::create_dir_all(artwork.parent().unwrap()).unwrap();
        fs::write(&artwork, b"artwork").unwrap();
        fs::write(
            layout.downloads_cache_root.join("ignored.tmp"),
            b"temporary",
        )
        .unwrap();
        let database = Database::open(&layout.database_path).unwrap();
        let destination = root.path().join("artwork.spotdiy");
        let result = write_archive(
            &database,
            &layout,
            "0.1.0",
            &SpotDiyExportOptions {
                include_artwork_cache: true,
                ..SpotDiyExportOptions::default()
            },
            &destination,
        )
        .unwrap();
        assert!(result
            .manifest
            .entries
            .iter()
            .any(|entry| entry.path == "covers/nested/cover.png"));
        assert!(!result
            .manifest
            .entries
            .iter()
            .any(|entry| entry.path.contains("ignored")));
    }

    #[test]
    fn archive_hash_is_stable_for_same_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("exe");
        let local = root.path().join("local");
        fs::create_dir_all(&exe).unwrap();
        let layout = StorageLayout::for_mode(&exe, &local, StorageMode::Standard);
        layout.ensure_runtime_directories().unwrap();
        let database = Database::open(&layout.database_path).unwrap();
        let first = root.path().join("first.spotdiy");
        let second = root.path().join("second.spotdiy");
        write_archive(
            &database,
            &layout,
            "0.1.0",
            &SpotDiyExportOptions::default(),
            &first,
        )
        .unwrap();
        write_archive(
            &database,
            &layout,
            "0.1.0",
            &SpotDiyExportOptions::default(),
            &second,
        )
        .unwrap();
        assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
    }
}
