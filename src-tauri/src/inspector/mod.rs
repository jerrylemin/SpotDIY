use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::db::repository::{RepositoryError, TrackRepository};
use crate::db::Database;
use crate::domain::{ProviderKind, SourceCapabilities, SourceId, TrackId, VersionQualifier};
use crate::playlists::{PlaylistError, PlaylistMembership, PlaylistService, Tag};
use crate::sources::validate_provider_url;

#[derive(Debug, Error)]
pub enum InspectorError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Playlist(#[from] PlaylistError),
    #[error("track {0} was not found")]
    TrackNotFound(TrackId),
}

#[derive(Clone)]
pub struct TrackInspectorService {
    database: Database,
    playlists: PlaylistService,
}

impl TrackInspectorService {
    pub fn new(database: Database, playlists: PlaylistService) -> Self {
        Self {
            database,
            playlists,
        }
    }

    pub fn get_track_inspector(&self, track_id: TrackId) -> Result<TrackInspector, InspectorError> {
        let track = TrackRepository::new(&self.database)
            .get(track_id)?
            .ok_or(InspectorError::TrackNotFound(track_id))?;
        let collection_state = self
            .playlists
            .get_track_collection_states(&[track_id])?
            .into_iter()
            .next()
            .ok_or(InspectorError::TrackNotFound(track_id))?;

        Ok(TrackInspector {
            track_id: track.id,
            title: track.title,
            artists: track
                .artists
                .into_iter()
                .map(|artist| artist.name)
                .collect(),
            album: track.album.map(|album| album.title),
            duration_ms: track.duration_ms,
            version_qualifiers: track.version.qualifiers,
            preferred_source_id: track.preferred_source_id,
            collection_state: InspectorCollectionState {
                liked: collection_state.liked,
                rating: collection_state.rating,
                tags: collection_state.tags,
                playlist_memberships: collection_state.playlist_memberships,
                in_inbox: collection_state.in_inbox,
            },
            sources: track.sources.into_iter().map(source_dto).collect(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInspector {
    pub track_id: TrackId,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub version_qualifiers: Vec<VersionQualifier>,
    pub preferred_source_id: Option<SourceId>,
    pub collection_state: InspectorCollectionState,
    pub sources: Vec<InspectorSource>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorCollectionState {
    pub liked: bool,
    pub rating: Option<u8>,
    pub tags: Vec<Tag>,
    pub playlist_memberships: Vec<PlaylistMembership>,
    pub in_inbox: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSource {
    pub source_id: SourceId,
    pub provider: ProviderKind,
    pub provider_item_id: String,
    pub available: bool,
    pub availability_detail: Option<String>,
    pub capabilities: InspectorCapabilities,
    pub duration_ms: Option<u64>,
    pub version_qualifiers: Vec<VersionQualifier>,
    pub quality: InspectorQuality,
    pub canonical_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorCapabilities {
    pub search: bool,
    pub metadata: bool,
    pub artwork: bool,
    pub playback: bool,
    pub lyrics: bool,
    pub downloads: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorQuality {
    pub container: Option<String>,
    pub codec: Option<String>,
    pub bitrate_kbps: Option<u64>,
    pub sample_rate_hz: Option<u64>,
    pub bit_depth: Option<u16>,
}

fn source_dto(source: crate::domain::TrackSource) -> InspectorSource {
    let quality = source
        .local_file
        .as_ref()
        .map(|local| InspectorQuality {
            container: local.container.clone(),
            codec: local.codec.clone(),
            bitrate_kbps: local.bitrate_kbps,
            sample_rate_hz: local.sample_rate_hz,
            bit_depth: local.bit_depth,
        })
        .unwrap_or_default();

    InspectorSource {
        source_id: source.id,
        provider: source.provider_kind,
        provider_item_id: source.provider_item_id,
        available: source.available,
        availability_detail: source.availability_detail,
        capabilities: InspectorCapabilities::from(source.capabilities),
        duration_ms: source.duration_ms,
        version_qualifiers: source.version.qualifiers,
        quality,
        canonical_url: canonical_url(source.provider_kind, source.source_uri.as_ref()),
    }
}

impl From<SourceCapabilities> for InspectorCapabilities {
    fn from(capabilities: SourceCapabilities) -> Self {
        Self {
            search: capabilities.search,
            metadata: capabilities.metadata,
            artwork: capabilities.artwork,
            playback: capabilities.playback,
            lyrics: capabilities.lyrics,
            downloads: capabilities.downloads,
        }
    }
}

fn canonical_url(provider: ProviderKind, source_uri: Option<&Url>) -> Option<String> {
    if provider == ProviderKind::Local {
        return None;
    }
    source_uri
        .and_then(|url| validate_provider_url(provider, url.as_str()).ok())
        .map(|url| url.as_url().as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, TempDatabasePath};
    use crate::domain::{Artist, SourceCapabilities, TrackSource, UnifiedTrack, VersionInfo};
    use crate::playlists::PlaylistService;
    use chrono::Utc;
    use std::path::PathBuf;

    fn capabilities(playback: bool, downloads: bool) -> SourceCapabilities {
        SourceCapabilities {
            search: true,
            playback,
            metadata: true,
            artwork: true,
            lyrics: downloads,
            downloads,
            popularity: true,
            release_date: true,
            lyrics_metadata: true,
        }
    }

    fn make_track() -> UnifiedTrack {
        let track_id = TrackId::new();
        let local_id = SourceId::new();
        let youtube_id = SourceId::new();
        let spotify_id = SourceId::new();
        let now = Utc::now();
        let artist = Artist {
            id: crate::domain::ArtistId::new(),
            name: "Inspector Artist".to_owned(),
            sort_name: None,
            created_at: now,
            updated_at: now,
        };
        let mut local = TrackSource::new_local_with_provider_item_id(
            local_id,
            track_id,
            PathBuf::from(r"C:\Music\private.flac"),
            "local-inspector",
            capabilities(true, false),
        )
        .unwrap();
        let local_file = local.local_file.as_mut().unwrap();
        local_file.container = Some("flac".to_owned());
        local_file.codec = Some("flac".to_owned());
        local_file.bitrate_kbps = Some(1200);
        local_file.sample_rate_hz = Some(96_000);
        local_file.bit_depth = Some(24);
        local.duration_ms = Some(201_000);

        let mut youtube = TrackSource::new(
            youtube_id,
            track_id,
            ProviderKind::Youtube,
            "youtube-inspector",
            capabilities(false, true),
        )
        .unwrap();
        youtube.source_uri = Some(Url::parse("https://www.youtube.com/watch?v=inspector").unwrap());
        youtube.available = false;
        youtube.availability_detail = Some("yt-dlp is not available".to_owned());
        youtube.duration_ms = Some(202_000);
        youtube.version = VersionInfo {
            qualifiers: vec![VersionQualifier::Live],
        };

        let mut spotify = TrackSource::new(
            spotify_id,
            track_id,
            ProviderKind::Spotify,
            "spotify-inspector",
            SourceCapabilities {
                search: true,
                playback: false,
                metadata: true,
                artwork: true,
                lyrics: false,
                downloads: false,
                popularity: true,
                release_date: true,
                lyrics_metadata: false,
            },
        )
        .unwrap();
        spotify.source_uri = Some(Url::parse("https://open.spotify.com/track/inspector").unwrap());

        UnifiedTrack::new(
            track_id,
            "Inspector Track",
            vec![artist],
            None,
            Some(201_000),
            VersionInfo::default(),
            vec![local, youtube, spotify],
        )
        .unwrap()
    }

    #[test]
    fn inspector_returns_purpose_built_sources_without_local_paths() {
        let path = TempDatabasePath::new("inspector-data");
        let database = Database::open(path.path()).unwrap();
        let track = make_track();
        let track_id = track.id;
        crate::db::repository::TrackRepository::new(&database)
            .create(&track)
            .unwrap();
        let playlists = PlaylistService::new(database.clone());
        playlists.set_track_liked(track_id, true).unwrap();
        let inspector = TrackInspectorService::new(database, playlists)
            .get_track_inspector(track_id)
            .unwrap();

        assert_eq!(inspector.track_id, track_id);
        assert_eq!(inspector.sources.len(), 3);
        let local = inspector
            .sources
            .iter()
            .find(|source| source.provider == ProviderKind::Local)
            .unwrap();
        assert_eq!(local.canonical_url, None);
        assert_eq!(local.quality.codec.as_deref(), Some("flac"));
        assert!(inspector.sources.iter().any(|source| {
            source.provider == ProviderKind::Youtube
                && source.canonical_url.as_deref()
                    == Some("https://www.youtube.com/watch?v=inspector")
                && !source.available
        }));
        let spotify = inspector
            .sources
            .iter()
            .find(|source| source.provider == ProviderKind::Spotify)
            .unwrap();
        assert!(!spotify.capabilities.playback);
        assert!(!spotify.capabilities.downloads);
        assert!(inspector.collection_state.liked);
        let json = serde_json::to_string(&inspector).unwrap();
        assert!(!json.contains("private.flac"));
        assert!(!json.contains("C:\\\\Music"));
    }

    #[test]
    fn inspector_reports_missing_tracks() {
        let path = TempDatabasePath::new("inspector-missing");
        let database = Database::open(path.path()).unwrap();
        let service = TrackInspectorService::new(database.clone(), PlaylistService::new(database));

        assert!(matches!(
            service.get_track_inspector(TrackId::new()),
            Err(InspectorError::TrackNotFound(_))
        ));
    }
}
