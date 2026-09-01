use std::cmp::Ordering;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{ProviderKind, SourceId, TrackSource, UnifiedTrack};
use crate::library::{LibraryError, LibraryService};
use crate::settings::{SettingsError, SettingsRepository};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceResolutionReason {
    PreferredSource,
    Playable,
    Unavailable,
    LocalFileMissing,
    SourceDoesNotSupportPlayback,
    ProviderPlaybackNotImplemented,
    MetadataOnly,
}

impl SourceResolutionReason {
    pub const fn default_detail(self) -> Option<&'static str> {
        match self {
            Self::PreferredSource | Self::Playable => None,
            Self::Unavailable => Some("source is unavailable"),
            Self::LocalFileMissing => Some("local file is missing or unavailable"),
            Self::SourceDoesNotSupportPlayback => Some("source does not support playback"),
            Self::ProviderPlaybackNotImplemented => {
                Some("online provider playback is not implemented yet")
            }
            Self::MetadataOnly => Some("provider is metadata-only"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceResolutionCandidate {
    pub source_id: SourceId,
    pub provider: ProviderKind,
    pub playable: bool,
    pub reason: SourceResolutionReason,
    pub preference_rank: u32,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceResolution {
    pub selected_source_id: Option<SourceId>,
    pub candidates: Vec<SourceResolutionCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceReadiness {
    pub playable: bool,
    pub reason: SourceResolutionReason,
    pub detail: Option<String>,
}

impl SourceReadiness {
    pub fn playable() -> Self {
        Self {
            playable: true,
            reason: SourceResolutionReason::Playable,
            detail: None,
        }
    }

    pub fn unavailable(reason: SourceResolutionReason, detail: Option<String>) -> Self {
        Self {
            playable: false,
            reason,
            detail,
        }
    }
}

pub trait SourceReadinessProbe: Send + Sync {
    fn probe(&self, track: &UnifiedTrack, source: &TrackSource) -> SourceReadiness;
}

impl<F> SourceReadinessProbe for F
where
    F: for<'track, 'source> Fn(&'track UnifiedTrack, &'source TrackSource) -> SourceReadiness
        + Send
        + Sync,
{
    fn probe(&self, track: &UnifiedTrack, source: &TrackSource) -> SourceReadiness {
        self(track, source)
    }
}

#[derive(Debug, Error)]
pub enum SourceResolverError {
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error("source {source_id} was not found for track {track_id}")]
    SourceNotFound {
        track_id: crate::domain::TrackId,
        source_id: SourceId,
    },
}

#[derive(Clone)]
pub struct SourceResolver {
    library: LibraryService,
    probe: Arc<dyn SourceReadinessProbe>,
}

impl SourceResolver {
    pub fn new(library: LibraryService) -> Self {
        let probe = Arc::new(ProductionSourceReadinessProbe {
            library: library.clone(),
        });
        Self { library, probe }
    }

    pub fn with_probe<P>(library: LibraryService, probe: P) -> Self
    where
        P: SourceReadinessProbe + 'static,
    {
        Self {
            library,
            probe: Arc::new(probe),
        }
    }

    pub fn new_with_probe<P>(library: LibraryService, probe: P) -> Self
    where
        P: SourceReadinessProbe + 'static,
    {
        Self::with_probe(library, probe)
    }

    pub fn resolve(&self, track: &UnifiedTrack) -> Result<SourceResolution, SourceResolverError> {
        let preference_order =
            SettingsRepository::new(self.library.database()).get_source_preference_order()?;
        let mut ordered_sources = Vec::new();
        for (preference_rank, provider) in preference_order.iter().copied().enumerate() {
            let mut provider_sources = track
                .sources
                .iter()
                .filter(|source| source.provider_kind == provider)
                .collect::<Vec<_>>();
            if provider == ProviderKind::Local {
                provider_sources.sort_by(|left, right| local_quality_order(left, right));
            } else {
                provider_sources.sort_by_key(|source| source.id.to_string());
            }
            ordered_sources.extend(
                provider_sources
                    .into_iter()
                    .map(|source| (source, preference_rank as u32)),
            );
        }

        let mut candidates = ordered_sources
            .into_iter()
            .map(|(source, preference_rank)| {
                let readiness = self.probe.probe(track, source);
                SourceResolutionCandidate {
                    source_id: source.id,
                    provider: source.provider_kind,
                    playable: readiness.playable,
                    reason: readiness.reason,
                    preference_rank,
                    detail: readiness
                        .detail
                        .or_else(|| readiness.reason.default_detail().map(str::to_owned)),
                }
            })
            .collect::<Vec<_>>();

        let preferred_playable = track.preferred_source_id.and_then(|preferred_source_id| {
            candidates.iter().position(|candidate| {
                candidate.source_id == preferred_source_id && candidate.playable
            })
        });
        let selected_source_id = preferred_playable
            .and_then(|index| candidates.get(index).map(|candidate| candidate.source_id))
            .or_else(|| {
                candidates
                    .iter()
                    .find(|candidate| candidate.playable)
                    .map(|candidate| candidate.source_id)
            });
        if let Some(selected_source_id) = selected_source_id {
            if let Some(candidate) = candidates
                .iter_mut()
                .find(|candidate| candidate.source_id == selected_source_id)
            {
                if track.preferred_source_id == Some(selected_source_id) {
                    candidate.reason = SourceResolutionReason::PreferredSource;
                    candidate.detail = None;
                }
            }
        }

        if let Some(index) = preferred_playable {
            let preferred = candidates.remove(index);
            candidates.insert(0, preferred);
        }
        Ok(SourceResolution {
            selected_source_id,
            candidates,
        })
    }

    pub fn resolve_exact_source(
        &self,
        track: &UnifiedTrack,
        source_id: SourceId,
    ) -> Result<SourceResolutionCandidate, SourceResolverError> {
        let source = track
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .ok_or(SourceResolverError::SourceNotFound {
                track_id: track.id,
                source_id,
            })?;
        let preference_rank = SettingsRepository::new(self.library.database())
            .get_source_preference_order()?
            .iter()
            .position(|provider| *provider == source.provider_kind)
            .unwrap_or(usize::MAX) as u32;
        let readiness = self.probe.probe(track, source);
        Ok(SourceResolutionCandidate {
            source_id,
            provider: source.provider_kind,
            playable: readiness.playable,
            reason: readiness.reason,
            preference_rank,
            detail: readiness
                .detail
                .or_else(|| readiness.reason.default_detail().map(str::to_owned)),
        })
    }

    pub fn library(&self) -> &LibraryService {
        &self.library
    }
}

struct ProductionSourceReadinessProbe {
    library: LibraryService,
}

impl SourceReadinessProbe for ProductionSourceReadinessProbe {
    fn probe(&self, track: &UnifiedTrack, source: &TrackSource) -> SourceReadiness {
        if !source.available {
            let detail = source.availability_detail.clone();
            return SourceReadiness::unavailable(
                if source.provider_kind == ProviderKind::Local
                    && detail
                        .as_deref()
                        .is_some_and(|detail| detail.to_ascii_lowercase().contains("missing"))
                {
                    SourceResolutionReason::LocalFileMissing
                } else {
                    SourceResolutionReason::Unavailable
                },
                detail,
            );
        }
        match source.provider_kind {
            ProviderKind::Local => {
                if !source.capabilities.playback {
                    return SourceReadiness::unavailable(
                        SourceResolutionReason::SourceDoesNotSupportPlayback,
                        Some("source does not advertise playback capability".to_owned()),
                    );
                }
                match self.library.resolve_playback_path(track.id, source.id) {
                    Ok(_) => SourceReadiness::playable(),
                    Err(error) => local_error_readiness(error),
                }
            }
            ProviderKind::Youtube | ProviderKind::Soundcloud => SourceReadiness::unavailable(
                SourceResolutionReason::ProviderPlaybackNotImplemented,
                Some(format!(
                    "{} playback is not implemented yet",
                    source.provider_kind
                )),
            ),
            ProviderKind::Spotify => SourceReadiness::unavailable(
                SourceResolutionReason::MetadataOnly,
                Some("Spotify is metadata-only".to_owned()),
            ),
        }
    }
}

fn local_error_readiness(error: LibraryError) -> SourceReadiness {
    let detail = Some(error.to_string());
    let reason = match &error {
        LibraryError::SourceUnavailable { detail, .. }
            if detail.to_ascii_lowercase().contains("missing")
                || detail.to_ascii_lowercase().contains("unavailable") =>
        {
            SourceResolutionReason::LocalFileMissing
        }
        LibraryError::SourceUnavailable { .. } => SourceResolutionReason::Unavailable,
        _ => SourceResolutionReason::Unavailable,
    };
    SourceReadiness::unavailable(reason, detail)
}

fn local_quality_order(left: &TrackSource, right: &TrackSource) -> Ordering {
    let left_file = left.local_file.as_ref();
    let right_file = right.local_file.as_ref();
    is_lossless(right_file.map(|file| file.codec.as_deref()).flatten())
        .cmp(&is_lossless(
            left_file.map(|file| file.codec.as_deref()).flatten(),
        ))
        .then_with(|| {
            right_file
                .and_then(|file| file.bit_depth)
                .unwrap_or_default()
                .cmp(
                    &left_file
                        .and_then(|file| file.bit_depth)
                        .unwrap_or_default(),
                )
        })
        .then_with(|| {
            right_file
                .and_then(|file| file.sample_rate_hz)
                .unwrap_or_default()
                .cmp(
                    &left_file
                        .and_then(|file| file.sample_rate_hz)
                        .unwrap_or_default(),
                )
        })
        .then_with(|| {
            right_file
                .and_then(|file| file.bitrate_kbps)
                .unwrap_or_default()
                .cmp(
                    &left_file
                        .and_then(|file| file.bitrate_kbps)
                        .unwrap_or_default(),
                )
        })
        .then_with(|| left.id.to_string().cmp(&right.id.to_string()))
}

fn is_lossless(codec: Option<&str>) -> bool {
    let Some(codec) = codec.map(str::trim).map(str::to_ascii_lowercase) else {
        return false;
    };
    codec == "flac"
        || codec == "alac"
        || codec.starts_with("pcm_")
        || codec == "wavpack"
        || codec == "ape"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::TrackRepository;
    use crate::db::{Database, TempDatabasePath};
    use crate::domain::{
        Artist, ArtistId, LocalFileIndexStatus, LocalFileSource, SourceCapabilities, SourceId,
        TrackId, VersionInfo,
    };
    use std::path::PathBuf;

    fn library() -> (Database, LibraryService) {
        let database = Database::open(TempDatabasePath::new("resolver").path()).unwrap();
        let artwork = tempfile::tempdir().unwrap();
        let library = LibraryService::new(database.clone(), artwork.path()).unwrap();
        (database, library)
    }

    fn track_with_sources(
        database: &Database,
        track_id: TrackId,
        sources: Vec<TrackSource>,
    ) -> UnifiedTrack {
        let track = UnifiedTrack::new(
            track_id,
            "Resolver Track",
            vec![Artist::new(ArtistId::new(), "Artist").unwrap()],
            None,
            Some(180_000),
            VersionInfo::standard(),
            sources,
        )
        .unwrap();
        TrackRepository::new(database).create(&track).unwrap();
        track
    }

    fn remote(track_id: TrackId, provider: ProviderKind, item: &str) -> TrackSource {
        TrackSource::new(
            SourceId::new(),
            track_id,
            provider,
            item,
            crate::ipc::provider_capabilities(provider),
        )
        .unwrap()
    }

    fn local(track_id: TrackId, codec: &str, id: SourceId) -> TrackSource {
        let path = PathBuf::from(format!("C:\\Music\\resolver-{id}.flac"));
        let mut source = TrackSource::new_local_with_provider_item_id(
            id,
            track_id,
            path.clone(),
            format!("local-{id}"),
            SourceCapabilities {
                playback: true,
                metadata: true,
                ..SourceCapabilities::default()
            },
        )
        .unwrap();
        source.local_file = Some(LocalFileSource {
            source_id: id,
            path,
            library_folder_id: None,
            normalized_path_key: None,
            file_size_bytes: None,
            modified_at: None,
            content_fingerprint: None,
            container: None,
            codec: Some(codec.to_owned()),
            bitrate_kbps: Some(320),
            sample_rate_hz: Some(44_100),
            bit_depth: Some(16),
            index_status: LocalFileIndexStatus::Indexed,
            status_detail: None,
            last_seen_at: None,
            last_indexed_at: None,
            last_seen_generation: 0,
            artwork_cache_key: None,
            artwork_mime_type: None,
        });
        source
    }

    #[test]
    fn production_resolver_reports_online_sources_as_not_implemented() {
        let (database, library) = library();
        let track_id = TrackId::new();
        let track = track_with_sources(
            &database,
            track_id,
            vec![
                remote(track_id, ProviderKind::Youtube, "video"),
                remote(track_id, ProviderKind::Soundcloud, "track"),
                remote(track_id, ProviderKind::Spotify, "album-track"),
            ],
        );
        let resolution = SourceResolver::new(library).resolve(&track).unwrap();
        assert_eq!(resolution.selected_source_id, None);
        assert_eq!(
            resolution
                .candidates
                .iter()
                .find(|candidate| candidate.provider == ProviderKind::Youtube)
                .unwrap()
                .reason,
            SourceResolutionReason::ProviderPlaybackNotImplemented
        );
        assert_eq!(
            resolution
                .candidates
                .iter()
                .find(|candidate| candidate.provider == ProviderKind::Soundcloud)
                .unwrap()
                .reason,
            SourceResolutionReason::ProviderPlaybackNotImplemented
        );
        assert_eq!(
            resolution
                .candidates
                .iter()
                .find(|candidate| candidate.provider == ProviderKind::Spotify)
                .unwrap()
                .reason,
            SourceResolutionReason::MetadataOnly
        );
    }

    #[test]
    fn preferred_playable_source_wins_and_is_explained() {
        let (database, library) = library();
        let track_id = TrackId::new();
        let youtube = remote(track_id, ProviderKind::Youtube, "video");
        let soundcloud = remote(track_id, ProviderKind::Soundcloud, "track");
        let mut track = track_with_sources(&database, track_id, vec![youtube.clone(), soundcloud]);
        track.preferred_source_id = Some(youtube.id);
        let resolver =
            SourceResolver::with_probe(library, |_track: &UnifiedTrack, _source: &TrackSource| {
                SourceReadiness::playable()
            });

        let resolution = resolver.resolve(&track).unwrap();

        assert_eq!(resolution.selected_source_id, Some(youtube.id));
        assert_eq!(resolution.candidates[0].source_id, youtube.id);
        assert_eq!(
            resolution.candidates[0].reason,
            SourceResolutionReason::PreferredSource
        );
    }

    #[test]
    fn unavailable_preferred_source_falls_back_to_next_playable_provider() {
        let (database, library) = library();
        let track_id = TrackId::new();
        let youtube = remote(track_id, ProviderKind::Youtube, "video");
        let soundcloud = remote(track_id, ProviderKind::Soundcloud, "track");
        let mut track = track_with_sources(
            &database,
            track_id,
            vec![youtube.clone(), soundcloud.clone()],
        );
        track.preferred_source_id = Some(youtube.id);
        crate::settings::SettingsRepository::new(&database)
            .set_setting(crate::settings::SettingValue::SourcePreferenceOrder(vec![
                ProviderKind::Youtube,
                ProviderKind::Soundcloud,
                ProviderKind::Local,
                ProviderKind::Spotify,
            ]))
            .unwrap();
        let resolver = SourceResolver::with_probe(
            library,
            move |_track: &UnifiedTrack, source: &TrackSource| {
                if source.id == youtube.id {
                    SourceReadiness::unavailable(
                        SourceResolutionReason::ProviderPlaybackNotImplemented,
                        Some("preferred provider is not playable".to_owned()),
                    )
                } else {
                    SourceReadiness::playable()
                }
            },
        );

        let resolution = resolver.resolve(&track).unwrap();

        assert_eq!(resolution.selected_source_id, Some(soundcloud.id));
        assert_eq!(resolution.candidates[0].source_id, youtube.id);
        assert_eq!(resolution.candidates[1].source_id, soundcloud.id);
        assert_eq!(
            resolution.candidates[0].reason,
            SourceResolutionReason::ProviderPlaybackNotImplemented
        );
    }

    #[test]
    fn fake_future_provider_readiness_respects_preference_order() {
        let (database, library) = library();
        let track_id = TrackId::new();
        let youtube = remote(track_id, ProviderKind::Youtube, "video");
        let soundcloud = remote(track_id, ProviderKind::Soundcloud, "track");
        let track = track_with_sources(
            &database,
            track_id,
            vec![youtube.clone(), soundcloud.clone()],
        );
        crate::settings::SettingsRepository::new(&database)
            .set_setting(crate::settings::SettingValue::SourcePreferenceOrder(vec![
                ProviderKind::Soundcloud,
                ProviderKind::Youtube,
                ProviderKind::Local,
                ProviderKind::Spotify,
            ]))
            .unwrap();
        let resolver =
            SourceResolver::with_probe(library, |_track: &UnifiedTrack, _source: &TrackSource| {
                SourceReadiness::playable()
            });
        let resolution = resolver.resolve(&track).unwrap();
        assert_eq!(resolution.selected_source_id, Some(soundcloud.id));
        assert_eq!(resolution.candidates[0].provider, ProviderKind::Soundcloud);
    }

    #[test]
    fn local_quality_orders_lossless_before_lossy_and_uses_stable_id() {
        let (database, library) = library();
        let track_id = TrackId::new();
        let lossy_id = SourceId::new();
        let lossless_id = SourceId::new();
        let track = track_with_sources(
            &database,
            track_id,
            vec![
                local(track_id, "mp3", lossy_id),
                local(track_id, "flac", lossless_id),
            ],
        );
        let resolver =
            SourceResolver::with_probe(library, |_track: &UnifiedTrack, _source: &TrackSource| {
                SourceReadiness::playable()
            });
        let resolution = resolver.resolve(&track).unwrap();
        assert_eq!(resolution.selected_source_id, Some(lossless_id));
        assert_eq!(resolution.candidates[0].source_id, lossless_id);
    }

    #[test]
    fn local_quality_ties_use_stable_source_id_order() {
        let (database, library) = library();
        let track_id = TrackId::new();
        let first = SourceId::new();
        let second = SourceId::new();
        let track = track_with_sources(
            &database,
            track_id,
            vec![
                local(track_id, "flac", second),
                local(track_id, "flac", first),
            ],
        );
        let resolver =
            SourceResolver::with_probe(library, |_track: &UnifiedTrack, _source: &TrackSource| {
                SourceReadiness::playable()
            });

        let resolution = resolver.resolve(&track).unwrap();

        let expected = [first, second]
            .into_iter()
            .min_by(|left, right| left.to_string().cmp(&right.to_string()))
            .unwrap();
        assert_eq!(resolution.selected_source_id, Some(expected));
        assert_eq!(resolution.candidates[0].source_id, expected);
    }

    #[test]
    fn local_file_and_capability_failures_have_typed_explanations() {
        let (database, library) = library();
        let track_id = TrackId::new();
        let mut missing = local(track_id, "flac", SourceId::new());
        missing.available = false;
        missing.availability_detail = Some("local file is missing".to_owned());
        let no_playback = TrackSource::new_local(
            SourceId::new(),
            track_id,
            PathBuf::from("C:\\Music\\resolver-no-playback.flac"),
            SourceCapabilities {
                metadata: true,
                ..SourceCapabilities::default()
            },
        )
        .unwrap();
        let track = track_with_sources(&database, track_id, vec![missing, no_playback]);
        let resolution = SourceResolver::new(library).resolve(&track).unwrap();

        assert_eq!(resolution.selected_source_id, None);
        assert!(resolution.candidates.iter().any(|candidate| {
            candidate.reason == SourceResolutionReason::LocalFileMissing
                && candidate.detail.as_deref() == Some("local file is missing")
        }));
        assert!(resolution.candidates.iter().any(|candidate| {
            candidate.reason == SourceResolutionReason::SourceDoesNotSupportPlayback
                && candidate.detail.is_some()
        }));
    }
}
