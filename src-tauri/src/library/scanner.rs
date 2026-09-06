use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use chrono::Utc;
use walkdir::WalkDir;

use crate::domain::{LibraryFolderStatus, LocalFileIndexStatus, ScanProgress, ScanSummary};

use super::fingerprint::sha256_file;
use super::folders::{is_reparse_point, normalize_file_path};
use super::metadata::{extract_metadata, ExtractedMetadata};
use super::{system_time_to_utc, LibraryError, LibraryService, ProgressSink, ScannedFile};

const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "aac", "ogg", "opus", "wav", "webm"];
const WEBM_PROBE_LIMIT: u64 = 1024 * 1024;

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
            Err(_) if is_webm_path(&path) && is_minimal_webm_file(&path) => (
                fallback_metadata(&path),
                LocalFileIndexStatus::Indexed,
                None,
            ),
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

fn is_webm_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("webm"))
}

fn is_minimal_webm_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() < 12 {
        return false;
    }
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    let mut bytes = Vec::new();
    if file.take(WEBM_PROBE_LIMIT).read_to_end(&mut bytes).is_err() {
        return false;
    }

    let Some((ebml_id, id_width)) = read_ebml_id(&bytes, 0) else {
        return false;
    };
    if ebml_id != 0x1A45DFA3 {
        return false;
    }
    let Some((header_size, size_width, unknown_size)) = read_ebml_vint(&bytes, id_width) else {
        return false;
    };
    if unknown_size {
        return false;
    }
    let header_start = id_width + size_width;
    let Some(header_end) = header_start.checked_add(header_size as usize) else {
        return false;
    };
    if header_end > bytes.len() {
        return false;
    }
    let mut cursor = header_start;
    let mut webm_doctype = false;
    while cursor < header_end {
        let Some((child_id, child_id_width)) = read_ebml_id(&bytes, cursor) else {
            return false;
        };
        cursor += child_id_width;
        let Some((child_size, child_size_width, child_unknown_size)) =
            read_ebml_vint(&bytes, cursor)
        else {
            return false;
        };
        if child_unknown_size {
            return false;
        }
        cursor += child_size_width;
        let Some(child_end) = cursor.checked_add(child_size as usize) else {
            return false;
        };
        if child_end > header_end {
            return false;
        }
        if child_id == 0x4282 && &bytes[cursor..child_end] == b"webm" {
            webm_doctype = true;
        }
        cursor = child_end;
    }
    if !webm_doctype {
        return false;
    }

    let Some((segment_id, segment_id_width)) = read_ebml_id(&bytes, header_end) else {
        return false;
    };
    if segment_id != 0x18538067 {
        return false;
    }
    let Some((segment_size, segment_size_width, segment_unknown_size)) =
        read_ebml_vint(&bytes, header_end + segment_id_width)
    else {
        return false;
    };
    if segment_unknown_size {
        return true;
    }
    let Some(segment_start) = u64::try_from(header_end).ok() else {
        return false;
    };
    let Some(segment_id_width) = u64::try_from(segment_id_width).ok() else {
        return false;
    };
    let Some(segment_size_width) = u64::try_from(segment_size_width).ok() else {
        return false;
    };
    let Some(segment_header_end) = segment_start
        .checked_add(segment_id_width)
        .and_then(|offset| offset.checked_add(segment_size_width))
    else {
        return false;
    };
    segment_header_end
        .checked_add(segment_size)
        .is_some_and(|end| end <= metadata.len())
}

fn read_ebml_id(bytes: &[u8], offset: usize) -> Option<(u32, usize)> {
    let first = *bytes.get(offset)?;
    if first == 0 {
        return None;
    }
    let width = first.leading_zeros() as usize + 1;
    if width > 4 || offset.checked_add(width)? > bytes.len() {
        return None;
    }
    let mut value = 0_u32;
    for byte in &bytes[offset..offset + width] {
        value = (value << 8) | u32::from(*byte);
    }
    Some((value, width))
}

fn read_ebml_vint(bytes: &[u8], offset: usize) -> Option<(u64, usize, bool)> {
    let first = *bytes.get(offset)?;
    if first == 0 {
        return None;
    }
    let width = first.leading_zeros() as usize + 1;
    if width > 8 || offset.checked_add(width)? > bytes.len() {
        return None;
    }
    let marker = 1_u8 << (8 - width);
    let mut value = u64::from(first & (marker - 1));
    for byte in &bytes[offset + 1..offset + width] {
        value = (value << 8) | u64::from(*byte);
    }
    let unknown = value == (1_u64 << (7 * width)) - 1;
    Some((value, width, unknown))
}

fn fallback_metadata(path: &Path) -> ExtractedMetadata {
    let title = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("Untitled")
        .to_owned();
    let is_webm = is_webm_path(path);
    let container = if is_webm {
        "WebM".to_owned()
    } else {
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_uppercase())
            .unwrap_or_else(|| "Audio".to_owned())
    };
    ExtractedMetadata {
        title,
        artists: vec!["Unknown Artist".to_owned()],
        album: None,
        duration_ms: None,
        container: container.clone(),
        codec: (!is_webm).then_some(container),
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

    #[test]
    fn webm_extension_and_fallback_metadata_are_truthful() {
        assert!(is_supported_audio_path(Path::new("song.webm")));
        assert!(is_supported_audio_path(Path::new("song.WEBM")));
        assert!(is_supported_audio_path(Path::new("song.WebM")));

        let metadata = fallback_metadata(Path::new("Voice Note.WEBM"));
        assert_eq!(metadata.title, "Voice Note");
        assert_eq!(metadata.artists, ["Unknown Artist"]);
        assert_eq!(metadata.album, None);
        assert_eq!(metadata.container, "WebM");
        assert_eq!(metadata.codec, None);
        assert_eq!(metadata.duration_ms, None);
        assert_eq!(metadata.bitrate_kbps, None);
        assert_eq!(metadata.sample_rate_hz, None);
        assert_eq!(metadata.bit_depth, None);
        assert_eq!(metadata.artwork, None);
    }

    #[test]
    fn minimal_webm_container_probe_accepts_valid_fixture_only() {
        let directory = tempfile::tempdir().unwrap();
        let valid = directory.path().join("valid.webm");
        let invalid = directory.path().join("invalid.webm");
        std::fs::write(
            &valid,
            [
                0x1A, 0x45, 0xDF, 0xA3, 0x8B, 0x42, 0x86, 0x81, 0x01, 0x42, 0x82, 0x84, b'w', b'e',
                b'b', b'm', 0x18, 0x53, 0x80, 0x67, 0x80,
            ],
        )
        .unwrap();
        std::fs::write(&invalid, b"not a webm container").unwrap();

        assert!(is_minimal_webm_file(&valid));
        assert!(!is_minimal_webm_file(&invalid));
    }

    #[test]
    fn minimal_webm_container_probe_accepts_finite_segments_larger_than_probe_window() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("large-finite-segment.webm");
        let total_len = WEBM_PROBE_LIMIT as usize + 1024;
        let segment_payload_offset = 28_usize;
        let segment_size = u64::try_from(total_len - segment_payload_offset).unwrap();
        let mut bytes = vec![0_u8; total_len];
        bytes[..16].copy_from_slice(&[
            0x1A, 0x45, 0xDF, 0xA3, 0x8B, 0x42, 0x86, 0x81, 0x01, 0x42, 0x82, 0x84, b'w', b'e',
            b'b', b'm',
        ]);
        bytes[16..20].copy_from_slice(&[0x18, 0x53, 0x80, 0x67]);
        let mut encoded_size = [0_u8; 8];
        for index in (1..8).rev() {
            encoded_size[index] = (segment_size >> (8 * (7 - index))) as u8;
        }
        encoded_size[0] = 0x01;
        bytes[20..28].copy_from_slice(&encoded_size);
        std::fs::write(&path, bytes).unwrap();

        assert!(is_minimal_webm_file(&path));
    }
}
