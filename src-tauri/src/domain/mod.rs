use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("{entity} name cannot be empty")]
    EmptyName { entity: &'static str },
    #[error("track title cannot be empty")]
    EmptyTitle,
    #[error("provider item ID cannot be empty")]
    EmptyProviderItemId,
    #[error("version qualifier {qualifier} appears more than once")]
    DuplicateVersionQualifier { qualifier: String },
    #[error("invalid provider kind {value}")]
    InvalidProviderKind { value: String },
    #[error("Spotify sources can advertise metadata only")]
    SpotifyAudioCapability,
    #[error("source {source_id} belongs to a different track")]
    SourceBelongsToDifferentTrack { source_id: SourceId },
    #[error("source ID {source_id} appears more than once")]
    DuplicateSourceId { source_id: SourceId },
    #[error("artist ID {artist_id} appears more than once")]
    DuplicateArtistId { artist_id: ArtistId },
}

macro_rules! typed_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            pub fn parse_str(value: &str) -> Result<Self, uuid::Error> {
                value.parse().map(Self)
            }

            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self::from_uuid(value)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse_str(value)
            }
        }
    };
}

typed_id!(TrackId);
typed_id!(ArtistId);
typed_id!(AlbumId);
typed_id!(SourceId);
typed_id!(LibraryFolderId);
typed_id!(ArtworkId);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Local,
    Youtube,
    Soundcloud,
    Spotify,
}

impl ProviderKind {
    pub const ALL: [Self; 4] = [Self::Local, Self::Youtube, Self::Soundcloud, Self::Spotify];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Youtube => "youtube",
            Self::Soundcloud => "soundcloud",
            Self::Spotify => "spotify",
        }
    }

    pub const fn all() -> &'static [Self; 4] {
        &Self::ALL
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ProviderKind {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "local" => Ok(Self::Local),
            "youtube" => Ok(Self::Youtube),
            "soundcloud" => Ok(Self::Soundcloud),
            "spotify" => Ok(Self::Spotify),
            value => Err(DomainError::InvalidProviderKind {
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCapabilities {
    pub search: bool,
    pub playback: bool,
    pub metadata: bool,
    pub artwork: bool,
    pub lyrics: bool,
    pub downloads: bool,
    pub popularity: bool,
    pub release_date: bool,
    pub lyrics_metadata: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VersionQualifier {
    Standard,
    Studio,
    Live,
    Acoustic,
    Remix,
    Remaster,
    Cover,
    Instrumental,
    Karaoke,
    SpedUp,
    Slowed,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub qualifiers: Vec<VersionQualifier>,
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self::standard()
    }
}

impl VersionInfo {
    pub fn new(qualifiers: Vec<VersionQualifier>) -> Result<Self, DomainError> {
        let mut seen = HashSet::new();
        for qualifier in &qualifiers {
            if !seen.insert(*qualifier) {
                return Err(DomainError::DuplicateVersionQualifier {
                    qualifier: format!("{qualifier:?}"),
                });
            }
        }
        Ok(Self { qualifiers })
    }

    pub fn standard() -> Self {
        Self {
            qualifiers: vec![VersionQualifier::Standard],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: ArtistId,
    pub name: String,
    pub sort_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Artist {
    pub fn new(id: ArtistId, name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(DomainError::EmptyName { entity: "artist" });
        }
        let now = Utc::now();
        Ok(Self {
            id,
            name,
            sort_name: None,
            created_at: now,
            updated_at: now,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: AlbumId,
    pub title: String,
    pub release_date: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LibraryFolderStatus {
    #[default]
    Idle,
    Queued,
    Scanning,
    Complete,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LocalFileIndexStatus {
    #[default]
    Pending,
    Indexed,
    Missing,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFolder {
    pub id: LibraryFolderId,
    pub path: PathBuf,
    pub normalized_path_key: String,
    pub enabled: bool,
    pub status: LibraryFolderStatus,
    pub scan_generation: u64,
    pub last_scan_started_at: Option<DateTime<Utc>>,
    pub last_scan_finished_at: Option<DateTime<Utc>>,
    pub last_scan_error: Option<String>,
    pub file_count: u64,
    pub indexed_track_count: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub directories_visited: u64,
    pub candidates: u64,
    pub unchanged_skipped: u64,
    pub new_files: u64,
    pub changed_files: u64,
    pub renamed_files: u64,
    pub missing_files: u64,
    pub unsupported_skipped: u64,
    pub metadata_failures: u64,
    pub artwork_failures: u64,
    pub database_failures: u64,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub folder_id: LibraryFolderId,
    pub status: LibraryFolderStatus,
    pub current_file: Option<PathBuf>,
    pub processed: u64,
    pub candidates: u64,
    pub summary: Option<ScanSummary>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LibrarySort {
    #[default]
    Title,
    Artist,
    DateAdded,
    DateModified,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPageRequest {
    #[serde(default)]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    #[serde(default)]
    pub sort: LibrarySort,
    #[serde(default)]
    pub descending: bool,
    pub folder_id: Option<LibraryFolderId>,
}

impl Default for LibraryPageRequest {
    fn default() -> Self {
        Self {
            page: 0,
            page_size: default_page_size(),
            sort: LibrarySort::default(),
            descending: false,
            folder_id: None,
        }
    }
}

fn default_page_size() -> u32 {
    50
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTrack {
    pub track_id: TrackId,
    pub source_id: SourceId,
    pub folder_id: LibraryFolderId,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub path: PathBuf,
    pub available: bool,
    pub availability_detail: Option<String>,
    pub index_status: LocalFileIndexStatus,
    pub status_detail: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub modified_at: Option<DateTime<Utc>>,
    pub codec: Option<String>,
    pub container: Option<String>,
    pub bitrate_kbps: Option<u64>,
    pub sample_rate_hz: Option<u64>,
    pub bit_depth: Option<u16>,
    pub content_fingerprint: Option<String>,
    pub artwork_cache_key: Option<String>,
    pub artwork_mime_type: Option<String>,
    pub artwork_path: Option<PathBuf>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
    pub items: Vec<LibraryTrack>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
    pub has_next: bool,
    pub sort: LibrarySort,
    pub descending: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    pub folders: Vec<LibraryFolder>,
    pub indexed_track_count: u64,
    pub available_track_count: u64,
    pub is_scanning: bool,
}

impl Album {
    pub fn new(id: AlbumId, title: impl Into<String>) -> Result<Self, DomainError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(DomainError::EmptyName { entity: "album" });
        }
        let now = Utc::now();
        Ok(Self {
            id,
            title,
            release_date: None,
            created_at: now,
            updated_at: now,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFileSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub library_folder_id: Option<LibraryFolderId>,
    pub normalized_path_key: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub modified_at: Option<DateTime<Utc>>,
    pub content_fingerprint: Option<String>,
    pub container: Option<String>,
    pub codec: Option<String>,
    pub bitrate_kbps: Option<u64>,
    pub sample_rate_hz: Option<u64>,
    pub bit_depth: Option<u16>,
    pub index_status: LocalFileIndexStatus,
    pub status_detail: Option<String>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_indexed_at: Option<DateTime<Utc>>,
    pub last_seen_generation: u64,
    pub artwork_cache_key: Option<String>,
    pub artwork_mime_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSource {
    pub id: SourceId,
    pub track_id: TrackId,
    pub provider_kind: ProviderKind,
    pub provider_item_id: String,
    pub source_uri: Option<Url>,
    pub duration_ms: Option<u64>,
    pub version: VersionInfo,
    pub available: bool,
    pub availability_detail: Option<String>,
    pub capabilities: SourceCapabilities,
    pub local_file: Option<LocalFileSource>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TrackSource {
    pub fn new(
        id: SourceId,
        track_id: TrackId,
        provider_kind: ProviderKind,
        provider_item_id: impl Into<String>,
        capabilities: SourceCapabilities,
    ) -> Result<Self, DomainError> {
        let provider_item_id = provider_item_id.into();
        if provider_item_id.trim().is_empty() {
            return Err(DomainError::EmptyProviderItemId);
        }
        if provider_kind == ProviderKind::Spotify
            && (capabilities.playback
                || capabilities.downloads
                || capabilities.lyrics
                || capabilities.lyrics_metadata)
        {
            return Err(DomainError::SpotifyAudioCapability);
        }
        let now = Utc::now();
        Ok(Self {
            id,
            track_id,
            provider_kind,
            provider_item_id,
            source_uri: None,
            duration_ms: None,
            version: VersionInfo::default(),
            available: true,
            availability_detail: None,
            capabilities,
            local_file: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn new_local(
        id: SourceId,
        track_id: TrackId,
        path: PathBuf,
        capabilities: SourceCapabilities,
    ) -> Result<Self, DomainError> {
        Self::new_local_with_provider_item_id(
            id,
            track_id,
            path,
            Uuid::new_v4().to_string(),
            capabilities,
        )
    }

    pub fn new_local_with_provider_item_id(
        id: SourceId,
        track_id: TrackId,
        path: PathBuf,
        provider_item_id: impl Into<String>,
        capabilities: SourceCapabilities,
    ) -> Result<Self, DomainError> {
        let mut source = Self::new(
            id,
            track_id,
            ProviderKind::Local,
            provider_item_id,
            capabilities,
        )?;
        source.local_file = Some(LocalFileSource {
            source_id: id,
            path,
            library_folder_id: None,
            normalized_path_key: None,
            file_size_bytes: None,
            modified_at: None,
            content_fingerprint: None,
            container: None,
            codec: None,
            bitrate_kbps: None,
            sample_rate_hz: None,
            bit_depth: None,
            index_status: LocalFileIndexStatus::Pending,
            status_detail: None,
            last_seen_at: None,
            last_indexed_at: None,
            last_seen_generation: 0,
            artwork_cache_key: None,
            artwork_mime_type: None,
        });
        Ok(source)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedTrack {
    pub id: TrackId,
    pub title: String,
    pub normalized_title: String,
    pub artists: Vec<Artist>,
    pub album: Option<Album>,
    pub duration_ms: Option<u64>,
    pub version: VersionInfo,
    pub sources: Vec<TrackSource>,
    pub preferred_source_id: Option<SourceId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UnifiedTrack {
    pub fn new(
        id: TrackId,
        title: impl Into<String>,
        artists: Vec<Artist>,
        album: Option<Album>,
        duration_ms: Option<u64>,
        version: VersionInfo,
        sources: Vec<TrackSource>,
    ) -> Result<Self, DomainError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(DomainError::EmptyTitle);
        }

        let mut artist_ids = HashSet::new();
        for artist in &artists {
            if !artist_ids.insert(artist.id) {
                return Err(DomainError::DuplicateArtistId {
                    artist_id: artist.id,
                });
            }
        }

        let mut source_ids = HashSet::new();
        for source in &sources {
            if source.track_id != id {
                return Err(DomainError::SourceBelongsToDifferentTrack {
                    source_id: source.id,
                });
            }
            if !source_ids.insert(source.id) {
                return Err(DomainError::DuplicateSourceId {
                    source_id: source.id,
                });
            }
        }

        let now = Utc::now();
        Ok(Self {
            id,
            normalized_title: normalize_title(&title),
            title,
            artists,
            album,
            duration_ms,
            version,
            sources,
            preferred_source_id: None,
            created_at: now,
            updated_at: now,
        })
    }
}

fn normalize_title(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capabilities() -> SourceCapabilities {
        SourceCapabilities {
            search: true,
            playback: true,
            metadata: true,
            artwork: true,
            lyrics: false,
            downloads: true,
            popularity: true,
            release_date: true,
            lyrics_metadata: false,
        }
    }

    #[test]
    fn typed_ids_round_trip_through_display_and_serde() {
        let id = TrackId::new();
        let encoded = serde_json::to_string(&id).unwrap();
        let decoded: TrackId = serde_json::from_str(&encoded).unwrap();

        assert_eq!(id, decoded);
        assert_eq!(id.to_string(), encoded.trim_matches('"'));
        assert_eq!(id, id.to_string().parse().unwrap());
    }

    #[test]
    fn provider_kind_and_capabilities_use_stable_wire_names() {
        assert_eq!(
            serde_json::to_string(&ProviderKind::Youtube).unwrap(),
            "\"youtube\""
        );
        let encoded = serde_json::to_value(capabilities()).unwrap();
        assert_eq!(encoded["releaseDate"], true);
        assert_eq!(encoded["lyricsMetadata"], false);
    }

    #[test]
    fn version_info_rejects_duplicate_qualifiers() {
        let result = VersionInfo::new(vec![VersionQualifier::Live, VersionQualifier::Live]);

        assert!(matches!(
            result,
            Err(DomainError::DuplicateVersionQualifier { .. })
        ));
    }

    #[test]
    fn unified_track_can_hold_multiple_artists_and_sources() {
        let track_id = TrackId::new();
        let artist_a = Artist::new(ArtistId::new(), "Test Artist A").unwrap();
        let artist_b = Artist::new(ArtistId::new(), "Test Artist B").unwrap();
        let local = TrackSource::new_local(
            SourceId::new(),
            track_id,
            PathBuf::from("C:\\Music\\signal-test.flac"),
            SourceCapabilities {
                playback: true,
                metadata: true,
                ..SourceCapabilities::default()
            },
        )
        .unwrap();
        let youtube = TrackSource::new(
            SourceId::new(),
            track_id,
            ProviderKind::Youtube,
            "fake-video-id",
            capabilities(),
        )
        .unwrap();

        let track = UnifiedTrack::new(
            track_id,
            "  Signal   Test ",
            vec![artist_a, artist_b],
            None,
            Some(180_000),
            VersionInfo::standard(),
            vec![local, youtube],
        )
        .unwrap();

        assert_eq!(track.artists.len(), 2);
        assert_eq!(track.sources.len(), 2);
        assert_eq!(track.normalized_title, "signal test");
    }

    #[test]
    fn spotify_cannot_be_created_with_non_metadata_capabilities() {
        let result = TrackSource::new(
            SourceId::new(),
            TrackId::new(),
            ProviderKind::Spotify,
            "spotify-track-id",
            SourceCapabilities {
                lyrics_metadata: true,
                ..SourceCapabilities::default()
            },
        );

        assert_eq!(result, Err(DomainError::SpotifyAudioCapability));
    }
}
