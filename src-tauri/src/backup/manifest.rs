use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::db::LATEST_SCHEMA_VERSION;
use crate::storage::StorageMode;

pub const SPOTDIY_ARCHIVE_FORMAT_VERSION: u32 = 1;
pub const DATABASE_ARCHIVE_PATH: &str = "database/spotdiy.sqlite3";
pub const MANIFEST_PATH: &str = "manifest.json";
pub const MANIFEST_CHECKSUM_PATH: &str = "manifest.sha256";
pub const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_ARCHIVE_ENTRIES: usize = 250_000;
pub const MAX_DATABASE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpotDiyArchiveEntryKind {
    Database,
    LocalAudio,
    Artwork,
    SidecarLyrics,
}

impl fmt::Display for SpotDiyArchiveEntryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Database => "database",
            Self::LocalAudio => "localAudio",
            Self::Artwork => "artwork",
            Self::SidecarLyrics => "sidecarLyrics",
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpotDiyArchiveEntry {
    pub path: String,
    pub kind: SpotDiyArchiveEntryKind,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpotDiyMediaMapping {
    pub source_id: String,
    pub archive_path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpotDiyManifest {
    pub format_version: u32,
    pub app_version: String,
    pub database_schema_version: u32,
    pub source_storage_mode: StorageMode,
    pub entries: Vec<SpotDiyArchiveEntry>,
    pub media_mappings: Vec<SpotDiyMediaMapping>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpotDiyExportOptions {
    #[serde(default)]
    pub include_local_audio: bool,
    #[serde(default)]
    pub include_artwork_cache: bool,
    #[serde(default)]
    pub include_sidecar_lyrics: bool,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ManifestError {
    #[error("unsupported .spotdiy archive format version {0}")]
    UnsupportedFormat(u32),
    #[error("archive database schema {found} is newer than supported schema {supported}")]
    FutureDatabaseSchema { found: u32, supported: u32 },
    #[error("archive manifest must contain a database entry")]
    MissingDatabase,
    #[error("archive manifest contains a duplicate path: {0}")]
    DuplicatePath(String),
    #[error("archive manifest contains an invalid path: {0}")]
    InvalidPath(String),
    #[error("archive manifest entry {path} has kind {kind}, which does not match its path")]
    KindMismatch { path: String, kind: String },
    #[error("archive manifest entry {path} has an invalid SHA-256 digest")]
    InvalidDigest { path: String },
    #[error("archive media mapping references a non-audio entry: {0}")]
    MediaMappingNotAudio(String),
    #[error("archive media mapping source ID does not match its media path: {source_id} -> {archive_path}")]
    MediaMappingSourceMismatch {
        source_id: String,
        archive_path: String,
    },
    #[error("archive media mapping is duplicated for source {0}")]
    DuplicateMediaMapping(String),
    #[error("archive media mapping references an unknown entry: {0}")]
    UnknownMediaMapping(String),
    #[error("archive local-audio entry is missing a media mapping: {0}")]
    MissingMediaMapping(String),
    #[error("archive sidecar lyrics entry is missing its local-audio mapping: {0}")]
    SidecarWithoutAudio(String),
    #[error("archive manifest contains too many entries")]
    TooManyEntries,
    #[error("archive source ID is invalid: {0}")]
    InvalidSourceId(String),
}

pub fn validate_manifest(manifest: &SpotDiyManifest) -> Result<(), ManifestError> {
    if manifest.format_version != SPOTDIY_ARCHIVE_FORMAT_VERSION {
        return Err(ManifestError::UnsupportedFormat(manifest.format_version));
    }
    if manifest.database_schema_version > LATEST_SCHEMA_VERSION {
        return Err(ManifestError::FutureDatabaseSchema {
            found: manifest.database_schema_version,
            supported: LATEST_SCHEMA_VERSION,
        });
    }
    if manifest.app_version.trim().is_empty() {
        return Err(ManifestError::InvalidPath("appVersion is empty".to_owned()));
    }
    if manifest.entries.len() > MAX_ARCHIVE_ENTRIES {
        return Err(ManifestError::TooManyEntries);
    }

    let mut paths = HashSet::new();
    let mut audio_paths = HashSet::new();
    for entry in &manifest.entries {
        validate_archive_payload_path(&entry.path)?;
        let path_key = entry.path.to_ascii_lowercase();
        if !paths.insert(path_key) {
            return Err(ManifestError::DuplicatePath(entry.path.clone()));
        }
        if !is_digest(&entry.sha256) {
            return Err(ManifestError::InvalidDigest {
                path: entry.path.clone(),
            });
        }
        if entry.path == DATABASE_ARCHIVE_PATH {
            if entry.kind != SpotDiyArchiveEntryKind::Database {
                return Err(ManifestError::KindMismatch {
                    path: entry.path.clone(),
                    kind: entry.kind.to_string(),
                });
            }
        } else {
            match (&entry.kind, entry.path.split('/').next()) {
                (SpotDiyArchiveEntryKind::LocalAudio, Some("media")) => {
                    audio_paths.insert(entry.path.clone());
                }
                (SpotDiyArchiveEntryKind::Artwork, Some("covers")) => {}
                (SpotDiyArchiveEntryKind::SidecarLyrics, Some("lyrics")) => {}
                _ => {
                    return Err(ManifestError::KindMismatch {
                        path: entry.path.clone(),
                        kind: entry.kind.to_string(),
                    });
                }
            }
        }
    }
    if !paths.contains(&DATABASE_ARCHIVE_PATH.to_ascii_lowercase()) {
        return Err(ManifestError::MissingDatabase);
    }

    let mut mapped_sources = HashSet::new();
    for mapping in &manifest.media_mappings {
        if Uuid::parse_str(&mapping.source_id).is_err() {
            return Err(ManifestError::InvalidSourceId(mapping.source_id.clone()));
        }
        if !audio_paths.contains(&mapping.archive_path) {
            if paths.contains(&mapping.archive_path.to_ascii_lowercase()) {
                return Err(ManifestError::MediaMappingNotAudio(
                    mapping.archive_path.clone(),
                ));
            }
            return Err(ManifestError::UnknownMediaMapping(
                mapping.archive_path.clone(),
            ));
        }
        let path_source_id = mapping
            .archive_path
            .split('/')
            .nth(1)
            .ok_or_else(|| ManifestError::InvalidPath(mapping.archive_path.clone()))?;
        if path_source_id != mapping.source_id {
            return Err(ManifestError::MediaMappingSourceMismatch {
                source_id: mapping.source_id.clone(),
                archive_path: mapping.archive_path.clone(),
            });
        }
        if !mapped_sources.insert(mapping.source_id.clone()) {
            return Err(ManifestError::DuplicateMediaMapping(
                mapping.source_id.clone(),
            ));
        }
    }
    for path in audio_paths {
        if !manifest
            .media_mappings
            .iter()
            .any(|mapping| mapping.archive_path == path)
        {
            return Err(ManifestError::MissingMediaMapping(path));
        }
    }
    for entry in manifest
        .entries
        .iter()
        .filter(|entry| entry.kind == SpotDiyArchiveEntryKind::SidecarLyrics)
    {
        let source_id = entry
            .path
            .split('/')
            .nth(1)
            .ok_or_else(|| ManifestError::InvalidPath(entry.path.clone()))?;
        if !manifest
            .media_mappings
            .iter()
            .any(|mapping| mapping.source_id == source_id)
        {
            return Err(ManifestError::SidecarWithoutAudio(entry.path.clone()));
        }
    }
    Ok(())
}

pub fn validate_archive_payload_path(path: &str) -> Result<(), ManifestError> {
    if path.is_empty()
        || path.contains('\\')
        || path.starts_with('/')
        || path.starts_with("//")
        || (path.len() >= 2 && path.as_bytes()[1] == b':')
    {
        return Err(ManifestError::InvalidPath(path.to_owned()));
    }
    let components: Vec<_> = path.split('/').collect();
    if components
        .iter()
        .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err(ManifestError::InvalidPath(path.to_owned()));
    }
    match components.as_slice() {
        ["database", file] if *file == "spotdiy.sqlite3" => Ok(()),
        ["media", source_id, file] if is_source_media_file(source_id, file) => Ok(()),
        ["lyrics", source_id, file] if *file == "media.lrc" && is_uuid(source_id) => Ok(()),
        ["covers", rest @ ..] if !rest.is_empty() => Ok(()),
        _ => Err(ManifestError::InvalidPath(path.to_owned())),
    }
}

fn is_source_media_file(source_id: &str, file: &str) -> bool {
    is_uuid(source_id)
        && file.strip_prefix("media.").is_some_and(|extension| {
            (1..=16).contains(&extension.len())
                && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

fn is_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}

fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(entries: Vec<SpotDiyArchiveEntry>) -> SpotDiyManifest {
        SpotDiyManifest {
            format_version: 1,
            app_version: "0.1.0".to_owned(),
            database_schema_version: 8,
            source_storage_mode: StorageMode::Standard,
            entries,
            media_mappings: Vec::new(),
        }
    }

    fn entry(path: &str, kind: SpotDiyArchiveEntryKind) -> SpotDiyArchiveEntry {
        SpotDiyArchiveEntry {
            path: path.to_owned(),
            kind,
            size_bytes: 1,
            sha256: "a".repeat(64),
        }
    }

    #[test]
    fn manifest_requires_database_and_known_paths() {
        assert!(matches!(
            validate_manifest(&manifest(vec![])),
            Err(ManifestError::MissingDatabase)
        ));
        assert!(validate_manifest(&manifest(vec![entry(
            DATABASE_ARCHIVE_PATH,
            SpotDiyArchiveEntryKind::Database
        )]))
        .is_ok());
        assert!(matches!(
            validate_archive_payload_path("../database/spotdiy.sqlite3"),
            Err(ManifestError::InvalidPath(_))
        ));
    }

    #[test]
    fn audio_mappings_are_typed_and_required() {
        let source_id = Uuid::new_v4().to_string();
        let path = format!("media/{source_id}/media.flac");
        let mut value = manifest(vec![
            entry(DATABASE_ARCHIVE_PATH, SpotDiyArchiveEntryKind::Database),
            entry(&path, SpotDiyArchiveEntryKind::LocalAudio),
        ]);
        assert!(matches!(
            validate_manifest(&value),
            Err(ManifestError::MissingMediaMapping(_))
        ));
        value.media_mappings.push(SpotDiyMediaMapping {
            source_id,
            archive_path: path,
        });
        assert!(validate_manifest(&value).is_ok());
    }

    #[test]
    fn manifest_rejects_unsafe_paths_and_case_duplicates() {
        for path in [
            "",
            "/database/spotdiy.sqlite3",
            r"\\server\share\backup",
            r"C:\backup\file",
            "../database/spotdiy.sqlite3",
            "./database/spotdiy.sqlite3",
            "database//spotdiy.sqlite3",
            "database/../spotdiy.sqlite3",
            "cache/download.tmp",
        ] {
            assert!(
                validate_archive_payload_path(path).is_err(),
                "unsafe archive path was accepted: {path}"
            );
        }

        let duplicate = manifest(vec![
            entry(DATABASE_ARCHIVE_PATH, SpotDiyArchiveEntryKind::Database),
            entry("covers/art.png", SpotDiyArchiveEntryKind::Artwork),
            entry("covers/ART.png", SpotDiyArchiveEntryKind::Artwork),
        ]);
        assert!(matches!(
            validate_manifest(&duplicate),
            Err(ManifestError::DuplicatePath(_))
        ));
    }

    #[test]
    fn manifest_rejects_orphan_sidecars_and_mismatched_media_mappings() {
        let source_id = Uuid::new_v4().to_string();
        let sidecar = manifest(vec![
            entry(DATABASE_ARCHIVE_PATH, SpotDiyArchiveEntryKind::Database),
            entry(
                &format!("lyrics/{source_id}/media.lrc"),
                SpotDiyArchiveEntryKind::SidecarLyrics,
            ),
        ]);
        assert!(matches!(
            validate_manifest(&sidecar),
            Err(ManifestError::SidecarWithoutAudio(_))
        ));

        let other_source_id = Uuid::new_v4().to_string();
        let audio_path = format!("media/{other_source_id}/media.flac");
        let mut mismatched = manifest(vec![
            entry(DATABASE_ARCHIVE_PATH, SpotDiyArchiveEntryKind::Database),
            entry(&audio_path, SpotDiyArchiveEntryKind::LocalAudio),
        ]);
        mismatched.media_mappings.push(SpotDiyMediaMapping {
            source_id,
            archive_path: audio_path,
        });
        assert!(matches!(
            validate_manifest(&mismatched),
            Err(ManifestError::MediaMappingSourceMismatch { .. })
        ));
    }

    #[test]
    fn manifest_rejects_future_database_schema() {
        let mut value = manifest(vec![entry(
            DATABASE_ARCHIVE_PATH,
            SpotDiyArchiveEntryKind::Database,
        )]);
        value.database_schema_version = LATEST_SCHEMA_VERSION + 1;
        assert!(matches!(
            validate_manifest(&value),
            Err(ManifestError::FutureDatabaseSchema { .. })
        ));

        value.database_schema_version = LATEST_SCHEMA_VERSION;
        value.format_version = SPOTDIY_ARCHIVE_FORMAT_VERSION + 1;
        assert!(matches!(
            validate_manifest(&value),
            Err(ManifestError::UnsupportedFormat(_))
        ));
    }

    #[test]
    fn manifest_rejects_entry_count_over_bound() {
        let mut value = manifest(vec![
            entry(
                DATABASE_ARCHIVE_PATH,
                SpotDiyArchiveEntryKind::Database
            );
            MAX_ARCHIVE_ENTRIES + 1
        ]);
        value.entries[1].path = "covers/extra.png".to_owned();
        value.entries[1].kind = SpotDiyArchiveEntryKind::Artwork;
        assert!(matches!(
            validate_manifest(&value),
            Err(ManifestError::TooManyEntries)
        ));
    }
}
