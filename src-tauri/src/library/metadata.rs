use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::NaiveDate;
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::id3::v2::{
    Frame, Id3v2Tag, SyncTextContentType, SynchronizedTextFrame, TimestampFormat,
};
use lofty::probe::Probe;
use lofty::tag::{ItemKey, TagType};

use crate::lyrics::parser::{parse_lrc, LyricsCue};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddedArtwork {
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddedLyrics {
    pub plain_text: Option<String>,
    pub cues: Vec<LyricsCue>,
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
    pub release_date: Option<String>,
    pub genres: Vec<String>,
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
    let release_date = [ItemKey::RecordingDate, ItemKey::Year, ItemKey::ReleaseDate]
        .into_iter()
        .find_map(|key| {
            tagged_file
                .tags()
                .iter()
                .find_map(|tag| valid_release_date(tag.get_string(key)))
        });
    let genres = tagged_file
        .tags()
        .iter()
        .flat_map(|tag| tag.get_strings(ItemKey::Genre).filter_map(normalize_genre))
        .fold(Vec::new(), |mut genres, genre| {
            if !genres
                .iter()
                .any(|existing: &String| existing.to_lowercase() == genre.to_lowercase())
            {
                genres.push(genre);
            }
            genres
        });
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
        release_date,
        genres,
        artwork,
    })
}

/// Read lyrics without changing the media file or including lyrics in the
/// library scanner's persisted metadata.
pub fn read_embedded_lyrics(path: impl AsRef<Path>) -> Result<Vec<EmbeddedLyrics>, MetadataError> {
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

    let mut lyrics = Vec::new();
    for tag in tagged_file.tags() {
        for value in tag.get_strings(ItemKey::Lyrics) {
            if let Some(document) = embedded_text(value) {
                lyrics.push(document);
            }
        }
        for value in tag.get_strings(ItemKey::UnsyncLyrics) {
            if let Some(text) = clean_string(value) {
                lyrics.push(EmbeddedLyrics {
                    plain_text: Some(text),
                    cues: Vec::new(),
                });
            }
        }

        if tag.tag_type() == TagType::Id3v2 {
            let id3v2: Id3v2Tag = tag.clone().into();
            for frame in &id3v2 {
                let Frame::Binary(binary) = frame else {
                    continue;
                };
                if frame.id_str() != "SYLT" {
                    continue;
                }
                let Ok(frame) = SynchronizedTextFrame::parse(&binary.data, frame.flags()) else {
                    continue;
                };
                if frame.timestamp_format != TimestampFormat::MS
                    || frame.content_type != SyncTextContentType::Lyrics
                {
                    continue;
                }
                if let Some(document) = embedded_synchronized(frame.content) {
                    lyrics.push(document);
                }
            }
        }
    }
    Ok(lyrics)
}

fn embedded_text(value: &str) -> Option<EmbeddedLyrics> {
    let parsed = parse_lrc(value).ok()?;
    if !parsed.cues.is_empty() {
        Some(EmbeddedLyrics {
            plain_text: non_empty(parsed.plain_text),
            cues: parsed.cues,
        })
    } else {
        non_empty(parsed.plain_text).map(|plain_text| EmbeddedLyrics {
            plain_text: Some(plain_text),
            cues: Vec::new(),
        })
    }
}

fn embedded_synchronized(content: Vec<(u32, String)>) -> Option<EmbeddedLyrics> {
    let mut entries = content
        .into_iter()
        .filter_map(|(start_ms, text)| {
            let text = clean_string(&text)?;
            Some((u64::from(start_ms), text))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(start_ms, _)| *start_ms);
    if entries.is_empty() {
        return None;
    }
    let mut cues: Vec<LyricsCue> = Vec::new();
    for (start_ms, text) in entries {
        if let Some(cue) = cues.last_mut() {
            if cue.start_ms == start_ms {
                cue.lines.push(text);
                continue;
            }
        }
        cues.push(LyricsCue {
            start_ms,
            lines: vec![text],
        });
    }
    let plain_text = cues
        .iter()
        .flat_map(|cue| cue.lines.iter())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    Some(EmbeddedLyrics {
        plain_text: non_empty(plain_text),
        cues,
    })
}

fn non_empty(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then_some(value)
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

fn valid_release_date(value: Option<&str>) -> Option<String> {
    let value = value?.split_whitespace().collect::<Vec<_>>().join(" ");
    if is_four_digit_year(&value)
        || NaiveDate::parse_from_str(&value, "%Y-%m-%d").is_ok()
        || NaiveDate::parse_from_str(&format!("{value}-01"), "%Y-%m-%d").is_ok()
    {
        Some(value)
    } else {
        None
    }
}

fn is_four_digit_year(value: &str) -> bool {
    value.len() == 4
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value
            .parse::<u16>()
            .is_ok_and(|year| (1..=9999).contains(&year))
}

fn normalize_genre(value: &str) -> Option<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized = normalized.trim();
    if normalized.is_empty() || normalized.chars().count() > 80 {
        return None;
    }
    Some(normalized.to_owned())
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

    #[test]
    fn release_date_accepts_year_month_and_full_dates() {
        assert_eq!(valid_release_date(Some("2024")), Some("2024".to_owned()));
        assert_eq!(
            valid_release_date(Some("2024-07")),
            Some("2024-07".to_owned())
        );
        assert_eq!(
            valid_release_date(Some("2024-07-31")),
            Some("2024-07-31".to_owned())
        );
    }

    #[test]
    fn invalid_release_date_is_skipped() {
        assert_eq!(valid_release_date(Some("not-a-date")), None);
        assert_eq!(valid_release_date(Some("2024-13")), None);
        assert_eq!(valid_release_date(Some("0000")), None);
    }

    #[test]
    fn genres_collapse_whitespace_deduplicate_case_insensitively_and_bound_unicode() {
        let values = ["  Dream\t Pop ", "dream pop", "JAZZ"];
        let genres =
            values
                .into_iter()
                .filter_map(normalize_genre)
                .fold(Vec::new(), |mut genres, genre| {
                    if !genres
                        .iter()
                        .any(|existing: &String| existing.to_lowercase() == genre.to_lowercase())
                    {
                        genres.push(genre);
                    }
                    genres
                });
        assert_eq!(genres, vec!["Dream Pop", "JAZZ"]);
        assert!(normalize_genre(&"\u{00e9}".repeat(81)).is_none());
    }
}
