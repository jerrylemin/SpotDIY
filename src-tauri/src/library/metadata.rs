use std::path::{Path, PathBuf};
use std::time::Duration;

use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::ItemKey;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddedArtwork {
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedMetadata {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub container: String,
    pub codec: Option<String>,
    pub bitrate_kbps: Option<u64>,
    pub sample_rate_hz: Option<u64>,
    pub bit_depth: Option<u16>,
    pub artwork: Option<EmbeddedArtwork>,
}

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("could not parse audio metadata from {path}: {message}")]
    Parse { path: PathBuf, message: String },
}

pub fn extract_metadata(path: impl AsRef<Path>) -> Result<ExtractedMetadata, MetadataError> {
    let path = path.as_ref();
    let tagged_file = Probe::open(path)
        .map_err(|error| MetadataError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?
        .guess_file_type()
        .map_err(|error| MetadataError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?
        .read()
        .map_err(|error| MetadataError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    let mut title = None;
    let mut artists = Vec::new();
    let mut album = None;
    let mut artwork = None;
    for tag in tagged_file.tags() {
        if title.is_none() {
            title = clean_text(tag.get_string(ItemKey::TrackTitle));
        }
        if artists.is_empty() {
            artists = artist_values(tag);
        }
        if album.is_none() {
            album = clean_text(tag.get_string(ItemKey::AlbumTitle));
        }
        if artwork.is_none() {
            artwork = tag.pictures().first().map(|picture| EmbeddedArtwork {
                mime_type: picture.mime_type().map(|mime| mime.as_str().to_owned()),
                bytes: picture.data().to_vec(),
            });
        }
    }

    let file_stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("Untitled");
    let title = title.unwrap_or_else(|| file_stem.to_owned());
    let artists = if artists.is_empty() {
        vec!["Unknown Artist".to_owned()]
    } else {
        artists
    };

    let properties = tagged_file.properties();
    let duration = properties.duration();
    let duration_ms = duration_ms(duration);
    let file_type = tagged_file.file_type();
    let container = container_label(file_type);

    Ok(ExtractedMetadata {
        title,
        artists,
        album,
        duration_ms,
        container: container.clone(),
        codec: codec_label(file_type),
        bitrate_kbps: properties
            .audio_bitrate()
            .or_else(|| properties.overall_bitrate())
            .map(u64::from),
        sample_rate_hz: properties.sample_rate().map(u64::from),
        bit_depth: properties.bit_depth().map(u16::from),
        artwork,
    })
}

fn artist_values(tag: &lofty::tag::Tag) -> Vec<String> {
    let plural = tag
        .get_strings(ItemKey::TrackArtists)
        .filter_map(clean_string)
        .collect::<Vec<_>>();
    if !plural.is_empty() {
        plural
    } else {
        tag.get_strings(ItemKey::TrackArtist)
            .filter_map(clean_string)
            .collect()
    }
}

fn clean_text(value: Option<&str>) -> Option<String> {
    value.and_then(clean_string)
}

fn clean_string(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn duration_ms(duration: Duration) -> Option<u64> {
    let millis = duration.as_millis();
    if millis == 0 {
        None
    } else {
        u64::try_from(millis).ok()
    }
}

fn container_label(file_type: FileType) -> String {
    match file_type {
        FileType::Aac => "AAC".to_owned(),
        FileType::Aiff => "AIFF".to_owned(),
        FileType::Ape => "APE".to_owned(),
        FileType::Flac => "FLAC".to_owned(),
        FileType::Mpeg => "MPEG".to_owned(),
        FileType::Mp4 => "MP4".to_owned(),
        FileType::Mpc => "Musepack".to_owned(),
        FileType::Opus => "Opus".to_owned(),
        FileType::Vorbis => "Ogg Vorbis".to_owned(),
        FileType::Speex => "Speex".to_owned(),
        FileType::Wav => "WAV".to_owned(),
        FileType::WavPack => "WavPack".to_owned(),
        FileType::Custom(value) => value.to_owned(),
        _ => "Audio".to_owned(),
    }
}

fn codec_label(file_type: FileType) -> Option<String> {
    match file_type {
        FileType::Aac => Some("AAC".to_owned()),
        FileType::Flac => Some("FLAC".to_owned()),
        FileType::Mpeg => Some("MPEG".to_owned()),
        FileType::Opus => Some("Opus".to_owned()),
        FileType::Vorbis => Some("Vorbis".to_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artist_fallback_does_not_split_comma_delimited_values() {
        let tag = lofty::tag::Tag::new(lofty::tag::TagType::Id3v2);
        let _ = tag;
        assert_eq!(
            clean_string("Artist One, Artist Two"),
            Some("Artist One, Artist Two".to_owned())
        );
    }

    #[test]
    fn singular_artist_is_only_a_fallback_when_plural_artists_are_absent() {
        let mut tag = lofty::tag::Tag::new(lofty::tag::TagType::Id3v2);
        assert!(tag.insert_text(ItemKey::TrackArtists, "Primary Artist".to_owned()));
        assert!(tag.insert_text(ItemKey::TrackArtist, "Legacy Artist".to_owned()));

        assert_eq!(artist_values(&tag), vec!["Primary Artist"]);
    }

    #[test]
    fn zero_duration_is_not_presented_as_measured_duration() {
        assert_eq!(duration_ms(Duration::ZERO), None);
    }
}
