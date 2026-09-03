use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Instant;

use chrono::Utc;
use walkdir::WalkDir;

use crate::domain::{LibraryFolderStatus, LocalFileIndexStatus, ScanProgress, ScanSummary};

use super::fingerprint::sha256_file;
use super::folders::{is_reparse_point, normalize_file_path};
use super::metadata::{extract_metadata, ExtractedMetadata};
use super::{system_time_to_utc, LibraryError, LibraryService, ProgressSink, ScannedFile};

const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "aac", "ogg", "opus", "wav"];

pub(crate) fn scan_folder(
    service: &LibraryService,
    folder_id: crate::domain::LibraryFolderId,
    force: bool,
    sink: Option<ProgressSink>,
) -> Result<ScanSummary, LibraryError> {
    let started = Instant::now();
    let context = service.folder_for_scan(folder_id)?;
    let root_metadata = fs::metadata(&context.filesystem_path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            LibraryError::Path(super::folders::FolderPathError::Missing {
                path: context.filesystem_path.clone(),
            })
        } else {
            LibraryError::Path(super::folders::FolderPathError::NotReadable {
                path: context.filesystem_path.clone(),
                source,
            })
        }
    })?;
    if !root_metadata.is_dir() {
        return Err(LibraryError::Path(
            super::folders::FolderPathError::NotDirectory {
                path: context.filesystem_path,
            },
        ));
    }

    let mut summary = ScanSummary::default();
    let mut scan_complete = true;
    let missing_before_scan = service.mark_missing_paths_before_scan(folder_id, Utc::now())?;
    let mut observed = HashSet::new();
    let entries = WalkDir::new(&context.filesystem_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.file_type().is_symlink() {
                return false;
            }
            fs::symlink_metadata(entry.path())
                .map(|metadata| !is_reparse_point(&metadata))
                .unwrap_or(true)
        });
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                summary.metadata_failures += 1;
                scan_complete = false;
                continue;
            }
        };
        let entry_metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(_) => {
                summary.metadata_failures += 1;
                scan_complete = false;
                continue;
            }
        };
        if is_reparse_point(&entry_metadata) || entry_metadata.file_type().is_symlink() {
            continue;
        }
        if entry_metadata.is_dir() {
            summary.directories_visited += 1;
            continue;
        }
        if !entry_metadata.is_file() {
            continue;
        }
        if !is_supported_audio_path(entry.path()) {
            summary.unsupported_skipped += 1;
            continue;
        }
        summary.candidates += 1;
        let path = entry.path().to_path_buf();
        let (display_path, normalized_path_key) = match normalize_file_path(&path) {
            Ok(value) => value,
            Err(_) => {
                summary.metadata_failures += 1;
                scan_complete = false;
                continue;
            }
        };
        let file_size_bytes = entry_metadata.len();
        let modified_at = entry_metadata.modified().ok().and_then(system_time_to_utc);
        observed.insert(normalized_path_key.clone());
        let existing = service.find_local_file(context.id, &normalized_path_key)?;
        let unchanged = !force
            && existing.as_ref().is_some_and(|existing| {
                existing.available
                    && existing.file_size_bytes == Some(file_size_bytes)
                    && existing.modified_at == modified_at
                    && existing.index_status == LocalFileIndexStatus::Indexed
            });
        if unchanged {
            if service
                .mark_local_file_seen(
                    folder_id,
                    &normalized_path_key,
                    context.generation,
                    Utc::now(),
                )
                .is_err()
            {
                summary.database_failures += 1;
            }
            summary.unchanged_skipped += 1;
            emit_progress(&sink, folder_id, &display_path, &summary);
            continue;
        }

        let fingerprint = match sha256_file(&path) {
            Ok(fingerprint) => fingerprint,
            Err(_) => {
                summary.metadata_failures += 1;
                continue;
            }
        };
        let (metadata, index_status, status_detail) = match extract_metadata(&path) {
            Ok(metadata) => (metadata, LocalFileIndexStatus::Indexed, None),
            Err(error) => {
                summary.metadata_failures += 1;
                (
                    fallback_metadata(&path),
                    LocalFileIndexStatus::Error,
                    Some(error.to_string()),
                )
            }
        };
        let artwork = match metadata.artwork.as_ref() {
            Some(artwork) => match service.store_artwork(artwork) {
                Ok(entry) => Some(entry),
                Err(_) => {
                    summary.artwork_failures += 1;
                    None
                }
            },
            None => None,
        };
        let scanned_file = ScannedFile {
            folder_id,
            generation: context.generation,
            path: display_path.clone(),
            normalized_path_key,
            file_size_bytes,
            modified_at,
            fingerprint,
            metadata,
            artwork,
            index_status,
            status_detail,
            now: Utc::now(),
        };
        let outcome = match service.persist_scanned_file(&scanned_file) {
            Ok(outcome) => outcome,
            Err(_) => {
                summary.database_failures += 1;
                emit_progress(&sink, folder_id, &display_path, &summary);
                continue;
            }
        };
        if outcome.is_new {
            summary.new_files += 1;
        } else if outcome.is_renamed {
            summary.renamed_files += 1;
        } else {
            summary.changed_files += 1;
        }
        emit_progress(&sink, folder_id, &display_path, &summary);
    }

    let missing_after_scan = if scan_complete {
        service.reconcile_missing(folder_id, &observed, Utc::now())?
    } else {
        0
    };
    summary.missing_files = missing_before_scan + missing_after_scan;
    summary.elapsed_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    Ok(summary)
}

fn emit_progress(
    sink: &Option<ProgressSink>,
    folder_id: crate::domain::LibraryFolderId,
    path: &Path,
    summary: &ScanSummary,
) {
    if let Some(sink) = sink {
        sink(ScanProgress {
            folder_id,
            status: LibraryFolderStatus::Scanning,
            current_file: Some(path.to_path_buf()),
            processed: summary.unchanged_skipped
                + summary.new_files
                + summary.changed_files
                + summary.renamed_files
                + summary.metadata_failures,
            candidates: summary.candidates,
            summary: Some(summary.clone()),
            started_at: None,
            finished_at: None,
            error: None,
        });
    }
}

fn is_supported_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

fn fallback_metadata(path: &Path) -> ExtractedMetadata {
    let title = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("Untitled")
        .to_owned();
    let container = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_uppercase())
        .unwrap_or_else(|| "Audio".to_owned());
    ExtractedMetadata {
        title,
        artists: vec!["Unknown Artist".to_owned()],
        album: None,
        duration_ms: None,
        container: container.clone(),
        codec: Some(container),
        bitrate_kbps: None,
        sample_rate_hz: None,
        bit_depth: None,
        release_date: None,
        genres: Vec::new(),
        artwork: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_filter_is_case_insensitive_and_excludes_video_containers() {
        assert!(is_supported_audio_path(Path::new("song.FLAC")));
        assert!(is_supported_audio_path(Path::new("song.OpUs")));
        assert!(!is_supported_audio_path(Path::new("video.mp4")));
        assert!(!is_supported_audio_path(Path::new("song.txt")));
    }
}
