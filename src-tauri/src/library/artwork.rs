use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::fingerprint::sha256_bytes;
use super::metadata::EmbeddedArtwork;

pub const MAX_ARTWORK_BYTES: usize = 10 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtworkCacheEntry {
    pub cache_key: String,
    pub mime_type: String,
    pub byte_size: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ArtworkError {
    #[error("embedded artwork is larger than the {max_bytes} byte limit")]
    TooLarge { max_bytes: usize },
    #[error("embedded artwork is not a supported image format")]
    UnsupportedFormat,
    #[error("could not create artwork cache directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not write artwork cache file {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Clone, Debug)]
pub struct ArtworkCache {
    root: PathBuf,
}

impl ArtworkCache {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, ArtworkError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|source| ArtworkError::CreateDirectory {
            path: root.clone(),
            source,
        })?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn store(&self, artwork: &EmbeddedArtwork) -> Result<ArtworkCacheEntry, ArtworkError> {
        if artwork.bytes.len() > MAX_ARTWORK_BYTES {
            return Err(ArtworkError::TooLarge {
                max_bytes: MAX_ARTWORK_BYTES,
            });
        }

        let (mime_type, extension) =
            detect_image_format(&artwork.bytes).ok_or(ArtworkError::UnsupportedFormat)?;
        let hash = sha256_bytes(&artwork.bytes);
        let cache_key = format!("{hash}.{extension}");
        let destination = self.root.join(&cache_key);

        if !destination.exists() {
            let temporary = self.root.join(format!(".{hash}.{}.tmp", Uuid::new_v4()));
            let write_result = write_atomically(&temporary, &destination, &artwork.bytes);
            if let Err(error) = write_result {
                let _ = fs::remove_file(&temporary);
                return Err(error);
            }
        }

        Ok(ArtworkCacheEntry {
            cache_key,
            mime_type: mime_type.to_owned(),
            byte_size: artwork.bytes.len() as u64,
        })
    }
}

fn write_atomically(
    temporary: &Path,
    destination: &Path,
    bytes: &[u8],
) -> Result<(), ArtworkError> {
    let mut file = File::create_new(temporary).map_err(|source| ArtworkError::Write {
        path: temporary.to_path_buf(),
        source,
    })?;
    file.write_all(bytes)
        .map_err(|source| ArtworkError::Write {
            path: temporary.to_path_buf(),
            source,
        })?;
    file.sync_all().map_err(|source| ArtworkError::Write {
        path: temporary.to_path_buf(),
        source,
    })?;
    drop(file);

    if destination.exists() {
        fs::remove_file(temporary).map_err(|source| ArtworkError::Write {
            path: temporary.to_path_buf(),
            source,
        })?;
    } else {
        match fs::rename(temporary, destination) {
            Ok(()) => {}
            Err(_source) if destination.exists() => {
                fs::remove_file(temporary).map_err(|remove_source| ArtworkError::Write {
                    path: temporary.to_path_buf(),
                    source: remove_source,
                })?;
            }
            Err(source) => {
                return Err(ArtworkError::Write {
                    path: destination.to_path_buf(),
                    source,
                });
            }
        }
    }
    Ok(())
}

fn detect_image_format(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("image/jpeg", "jpg"))
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(("image/gif", "gif"))
    } else if bytes.starts_with(b"BM") {
        Some(("image/bmp", "bmp"))
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        Some(("image/tiff", "tif"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_is_content_addressed_and_does_not_rewrite_existing_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let cache = ArtworkCache::new(directory.path()).unwrap();
        let artwork = EmbeddedArtwork {
            mime_type: Some("image/png".to_owned()),
            bytes: b"\x89PNG\r\n\x1a\nfixture".to_vec(),
        };

        let first = cache.store(&artwork).unwrap();
        let second = cache.store(&artwork).unwrap();

        assert_eq!(first, second);
        assert_eq!(
            std::fs::read(directory.path().join(first.cache_key)).unwrap(),
            artwork.bytes
        );
    }

    #[test]
    fn unsupported_artwork_is_reported_separately() {
        let directory = tempfile::tempdir().unwrap();
        let cache = ArtworkCache::new(directory.path()).unwrap();
        let artwork = EmbeddedArtwork {
            mime_type: None,
            bytes: b"not an image".to_vec(),
        };

        assert!(matches!(
            cache.store(&artwork),
            Err(ArtworkError::UnsupportedFormat)
        ));
    }
}
