use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedFolderPath {
    pub filesystem_path: PathBuf,
    pub display_path: PathBuf,
    pub normalized_path_key: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FolderPathError {
    #[error("library folder path is empty")]
    Empty,
    #[error("library folder path {path} does not exist")]
    Missing { path: PathBuf },
    #[error("library folder path {path} is not a directory")]
    NotDirectory { path: PathBuf },
    #[error("library file path {path} is not a file")]
    NotFile { path: PathBuf },
    #[error("library folder path {path} is not readable: {source}")]
    NotReadable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("library path {path} is a symbolic link or reparse point")]
    ReparsePoint { path: PathBuf },
    #[error("library folder path {path} is not representable as UTF-8")]
    NonUnicode { path: PathBuf },
    #[error("library folder {path} duplicates an existing selected folder")]
    Duplicate { path: PathBuf },
    #[error("library folder {path} overlaps selected folder {other}")]
    Overlap { path: PathBuf, other: PathBuf },
}

pub fn normalize_folder_path(
    input: impl AsRef<Path>,
) -> Result<NormalizedFolderPath, FolderPathError> {
    let input = input.as_ref();
    if input.as_os_str().is_empty() {
        return Err(FolderPathError::Empty);
    }

    let input_metadata = fs::symlink_metadata(input).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            FolderPathError::Missing {
                path: input.to_path_buf(),
            }
        } else {
            FolderPathError::NotReadable {
                path: input.to_path_buf(),
                source,
            }
        }
    })?;
    if input_metadata.file_type().is_symlink() || is_reparse_point(&input_metadata) {
        return Err(FolderPathError::ReparsePoint {
            path: input.to_path_buf(),
        });
    }

    let filesystem_path = fs::canonicalize(input).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            FolderPathError::Missing {
                path: input.to_path_buf(),
            }
        } else {
            FolderPathError::NotReadable {
                path: input.to_path_buf(),
                source: error,
            }
        }
    })?;
    let metadata =
        fs::metadata(&filesystem_path).map_err(|source| FolderPathError::NotReadable {
            path: filesystem_path.clone(),
            source,
        })?;
    if is_reparse_point(&metadata) {
        return Err(FolderPathError::ReparsePoint {
            path: filesystem_path,
        });
    }
    if !metadata.is_dir() {
        return Err(FolderPathError::NotDirectory {
            path: filesystem_path,
        });
    }
    fs::read_dir(&filesystem_path).map_err(|source| FolderPathError::NotReadable {
        path: filesystem_path.clone(),
        source,
    })?;

    let display_path = display_path(&filesystem_path)?;
    let normalized_path_key = normalized_path_key(&display_path)?;
    Ok(NormalizedFolderPath {
        filesystem_path,
        display_path,
        normalized_path_key,
    })
}

pub fn normalize_file_path(input: impl AsRef<Path>) -> Result<(PathBuf, String), FolderPathError> {
    let input = input.as_ref();
    let input_metadata = fs::symlink_metadata(input).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            FolderPathError::Missing {
                path: input.to_path_buf(),
            }
        } else {
            FolderPathError::NotReadable {
                path: input.to_path_buf(),
                source,
            }
        }
    })?;
    if input_metadata.file_type().is_symlink() || is_reparse_point(&input_metadata) {
        return Err(FolderPathError::ReparsePoint {
            path: input.to_path_buf(),
        });
    }
    let filesystem_path = fs::canonicalize(input).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            FolderPathError::Missing {
                path: input.to_path_buf(),
            }
        } else {
            FolderPathError::NotReadable {
                path: input.to_path_buf(),
                source,
            }
        }
    })?;
    let metadata =
        fs::metadata(&filesystem_path).map_err(|source| FolderPathError::NotReadable {
            path: filesystem_path.clone(),
            source,
        })?;
    if is_reparse_point(&metadata) {
        return Err(FolderPathError::ReparsePoint {
            path: filesystem_path,
        });
    }
    if !metadata.is_file() {
        return Err(FolderPathError::NotFile {
            path: filesystem_path,
        });
    }
    let display = display_path(&filesystem_path)?;
    let key = normalized_path_key(&display)?;
    Ok((display, key))
}

pub fn validate_new_folders(
    inputs: impl IntoIterator<Item = PathBuf>,
    existing_keys: impl IntoIterator<Item = String>,
) -> Result<Vec<NormalizedFolderPath>, FolderPathError> {
    let mut normalized = Vec::new();
    let mut keys = existing_keys
        .into_iter()
        .map(|key| key.to_lowercase())
        .collect::<Vec<_>>();
    for input in inputs {
        let candidate = normalize_folder_path(input)?;
        if let Some(existing_key) = keys
            .iter()
            .find(|key| paths_overlap(&candidate.normalized_path_key, key))
        {
            if existing_key == &candidate.normalized_path_key {
                return Err(FolderPathError::Duplicate {
                    path: candidate.display_path,
                });
            }
            return Err(FolderPathError::Overlap {
                path: candidate.display_path,
                other: PathBuf::from(existing_key),
            });
        }
        if let Some(other) = normalized.iter().find(|other: &&NormalizedFolderPath| {
            paths_overlap(&candidate.normalized_path_key, &other.normalized_path_key)
        }) {
            return Err(FolderPathError::Overlap {
                path: candidate.display_path,
                other: other.display_path.clone(),
            });
        }
        keys.push(candidate.normalized_path_key.clone());
        normalized.push(candidate);
    }
    Ok(normalized)
}

pub fn paths_overlap(first: &str, second: &str) -> bool {
    first == second || is_descendant(first, second) || is_descendant(second, first)
}

pub fn is_path_within(root: &str, child: &str) -> bool {
    is_descendant(&child.to_lowercase(), &root.to_lowercase())
}

fn is_descendant(child: &str, parent: &str) -> bool {
    child.strip_prefix(parent).is_some_and(|remainder| {
        if parent.ends_with('\\') {
            !remainder.is_empty()
        } else {
            remainder.starts_with('\\')
        }
    })
}

pub(crate) fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn display_path(path: &Path) -> Result<PathBuf, FolderPathError> {
    let raw = path.to_str().ok_or_else(|| FolderPathError::NonUnicode {
        path: path.to_path_buf(),
    })?;
    let display = if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        raw.to_owned()
    };
    Ok(PathBuf::from(display))
}

fn normalized_path_key(path: &Path) -> Result<String, FolderPathError> {
    let raw = path.to_str().ok_or_else(|| FolderPathError::NonUnicode {
        path: path.to_path_buf(),
    })?;
    let mut normalized = raw.replace('/', "\\");
    let is_drive_root = normalized.len() == 3
        && normalized.as_bytes().get(1) == Some(&b':')
        && normalized.ends_with('\\');
    if !is_drive_root {
        while normalized.ends_with('\\') {
            normalized.pop();
        }
    }
    Ok(normalized.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_and_nested_roots() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("nested");
        std::fs::create_dir(&nested).unwrap();

        let first = normalize_folder_path(root.path()).unwrap();
        let duplicate = validate_new_folders(
            [root.path().to_path_buf()],
            [first.normalized_path_key.clone()],
        );
        assert!(matches!(duplicate, Err(FolderPathError::Duplicate { .. })));

        let overlap = validate_new_folders([nested], [first.normalized_path_key]);
        assert!(matches!(overlap, Err(FolderPathError::Overlap { .. })));
    }

    #[test]
    fn path_overlap_requires_a_separator_boundary() {
        assert!(paths_overlap(r"c:\music", r"c:\music\albums"));
        assert!(!paths_overlap(r"c:\music", r"c:\music2"));
        assert!(paths_overlap("c:\\", r"c:\music"));
        assert!(is_path_within("c:\\", r"c:\music\song.flac"));
        assert!(!is_path_within(r"c:\music", r"c:\music2\song.flac"));
    }

    #[test]
    fn rejects_missing_and_non_directory_folder_inputs() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("track.flac");
        std::fs::write(&file, b"fixture").unwrap();
        let missing = root.path().join("missing");

        assert!(matches!(
            normalize_folder_path(&missing),
            Err(FolderPathError::Missing { .. })
        ));
        assert!(matches!(
            normalize_folder_path(&file),
            Err(FolderPathError::NotDirectory { .. })
        ));
        assert!(matches!(
            normalize_file_path(root.path()),
            Err(FolderPathError::NotFile { .. })
        ));
    }

    #[test]
    fn case_only_root_variants_are_duplicates() {
        let root = tempfile::tempdir().unwrap();
        let first = normalize_folder_path(root.path()).unwrap();
        let upper = PathBuf::from(first.display_path.to_string_lossy().to_uppercase());

        let result = validate_new_folders([upper], [first.normalized_path_key]);
        assert!(matches!(result, Err(FolderPathError::Duplicate { .. })));
    }
}
