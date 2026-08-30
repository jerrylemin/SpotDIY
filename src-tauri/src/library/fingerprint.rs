use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

const HASH_BUFFER_SIZE: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FingerprintError {
    #[error("could not open {path}: {source}")]
    Open {
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not read {path}: {source}")]
    Read {
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Hashes a file without loading the media into memory.
pub fn sha256_file(path: impl AsRef<Path>) -> Result<String, FingerprintError> {
    let path = path.as_ref();
    let mut file = File::open(path).map_err(|source| FingerprintError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_SIZE];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|source| FingerprintError::Read {
                path: path.to_path_buf(),
                source,
            })?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(to_hex(&hasher.finalize()))
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    to_hex(&Sha256::digest(bytes))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn hashes_streamed_file_content_deterministically() {
        let mut first = NamedTempFile::new().unwrap();
        let mut second = NamedTempFile::new().unwrap();
        first.write_all(b"same content").unwrap();
        second.write_all(b"same content").unwrap();

        assert_eq!(
            sha256_file(first.path()).unwrap(),
            sha256_file(second.path()).unwrap()
        );
        assert_ne!(
            sha256_file(first.path()).unwrap(),
            sha256_bytes(b"different content")
        );
    }
}
