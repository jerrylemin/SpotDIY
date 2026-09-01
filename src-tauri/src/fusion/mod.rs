pub mod matcher;
pub mod normalize;
pub mod overrides;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::repository::{RepositoryError, SourceRepository, TrackRepository};
use crate::db::Database;
use crate::domain::{
    DomainError, ProviderKind, SourceId, TrackId, TrackSource, UnifiedTrack, VersionInfo,
    VersionQualifier,
};
use crate::search::types::{SearchEntityKind, SearchResult};
use crate::sources::validate_provider_url;

pub use overrides::{
    FusionOverride, FusionOverrideDecision, FusionOverrideError, FusionOverrideRepository,
};

pub const AUTOMATIC_MATCH_THRESHOLD_BPS: u16 = matcher::AUTOMATIC_MATCH_THRESHOLD_BPS;
pub const AUTOMATIC_MATCH_AMBIGUITY_GAP_BPS: u16 = matcher::AMBIGUITY_GAP_BPS;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FusionDecision {
    AlreadyUnified,
    ForcedMerge,
    AutoMerge,
    ForcedSplit,
    Rejected,
    Excluded,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FusionReason {
    Matched,
    ProviderExcluded,
    EntityUnsupported,
    AlreadyUnified,
    ForcedMerge,
    ForcedSplit,
    SameProviderRequiresManualMerge,
    TitleBelowMinimum,
    ArtistBelowMinimum,
    DurationMismatch,
    VersionMismatch,
    BelowThreshold,
    IdentityConflict,
    Ambiguous,
    InvalidCandidate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FusionEvaluation {
    pub target_track_id: TrackId,
    pub decision: FusionDecision,
    pub score_bps: u16,
    pub threshold_bps: u16,
    pub title_score_bps: u16,
    pub artist_score_bps: u16,
    pub duration_score_bps: u16,
    pub duration_delta_ms: Option<u64>,
    pub candidate_qualifiers: Vec<VersionQualifier>,
    pub target_qualifiers: Vec<VersionQualifier>,
    pub reason: FusionReason,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FusionSelection {
    pub target_track_id: Option<TrackId>,
    pub decision: FusionDecision,
    pub evaluation: Option<FusionEvaluation>,
    pub candidates: Vec<FusionEvaluation>,
}

#[derive(Debug, Error)]
pub enum FusionServiceError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Override(#[from] FusionOverrideError),
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("candidate provider URL is not valid for {provider}")]
    InvalidProviderUrl { provider: ProviderKind },
}

#[derive(Clone)]
pub struct SourceFusionService {
    database: Database,
}

impl SourceFusionService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn evaluate_candidate(
        &self,
        candidate: &SearchResult,
        target_track_id: TrackId,
    ) -> Result<FusionEvaluation, FusionServiceError> {
        let target = TrackRepository::new(&self.database)
            .get(target_track_id)?
            .ok_or(RepositoryError::TrackNotFound(target_track_id))?;
        self.evaluate(candidate, &target)
    }

    pub fn evaluate(
        &self,
        candidate: &SearchResult,
        target: &UnifiedTrack,
    ) -> Result<FusionEvaluation, FusionServiceError> {
        let candidate_normalized = normalize::normalize_search_result(candidate);
        let target_normalized = normalize::normalize_unified_track(target);
        let scores = matcher::score(&candidate_normalized, &target_normalized);
        let base = || FusionEvaluation {
            target_track_id: target.id,
            decision: FusionDecision::Rejected,
            score_bps: scores.score_bps,
            threshold_bps: matcher::AUTOMATIC_MATCH_THRESHOLD_BPS,
            title_score_bps: scores.title_score_bps,
            artist_score_bps: scores.artist_score_bps,
            duration_score_bps: scores.duration_score_bps,
            duration_delta_ms: scores.duration_delta_ms,
            candidate_qualifiers: candidate_normalized.guarded_qualifiers.clone(),
            target_qualifiers: target_normalized.guarded_qualifiers.clone(),
            reason: FusionReason::BelowThreshold,
        };

        if candidate.provider == ProviderKind::Spotify {
            return Ok(with_decision(
                base(),
                FusionDecision::Excluded,
                FusionReason::ProviderExcluded,
            ));
        }
        if candidate.entity_kind != SearchEntityKind::Track {
            return Ok(with_decision(
                base(),
                FusionDecision::Rejected,
                FusionReason::EntityUnsupported,
            ));
        }
        let provider_item_id = candidate.provider_item_id.trim();
        if provider_item_id.is_empty() {
            return Ok(with_decision(
                base(),
                FusionDecision::Rejected,
                FusionReason::InvalidCandidate,
            ));
        }

        if candidate.provider == ProviderKind::Local {
            if candidate.local_track_id == Some(target.id) {
                return Ok(with_decision(
                    base(),
                    FusionDecision::AlreadyUnified,
                    FusionReason::AlreadyUnified,
                ));
            }
            if candidate.local_track_id.is_some() {
                return Ok(with_decision(
                    base(),
                    FusionDecision::Rejected,
                    FusionReason::IdentityConflict,
                ));
            }
        }

        if target.sources.iter().any(|source| {
            source.provider_kind == candidate.provider
                && source.provider_item_id == provider_item_id
        }) {
            return Ok(with_decision(
                base(),
                FusionDecision::AlreadyUnified,
                FusionReason::AlreadyUnified,
            ));
        }

        let source_repository = SourceRepository::new(&self.database);
        if let Some(existing) =
            source_repository.find_by_provider_identity(candidate.provider, provider_item_id)?
        {
            return Ok(with_decision(
                base(),
                if existing.track_id == target.id {
                    FusionDecision::AlreadyUnified
                } else {
                    FusionDecision::Rejected
                },
                if existing.track_id == target.id {
                    FusionReason::AlreadyUnified
                } else {
                    FusionReason::IdentityConflict
                },
            ));
        }

        let overrides = FusionOverrideRepository::new(&self.database);
        if let Some(override_record) =
            overrides.get_override(candidate.provider, provider_item_id, target.id)?
        {
            if override_record.decision == FusionOverrideDecision::Split {
                return Ok(with_decision(
                    base(),
                    FusionDecision::ForcedSplit,
                    FusionReason::ForcedSplit,
                ));
            }
        }
        if let Some(forced_target) =
            overrides.forced_merge_target(candidate.provider, provider_item_id)?
        {
            if forced_target == target.id {
                return Ok(with_decision(
                    base(),
                    FusionDecision::ForcedMerge,
                    FusionReason::ForcedMerge,
                ));
            }
            return Ok(with_decision(
                base(),
                FusionDecision::Rejected,
                FusionReason::IdentityConflict,
            ));
        }

        if target
            .sources
            .iter()
            .any(|source| source.provider_kind == candidate.provider)
        {
            return Ok(with_decision(
                base(),
                FusionDecision::Rejected,
                FusionReason::SameProviderRequiresManualMerge,
            ));
        }

        match matcher::automatic_match(&candidate_normalized, &target_normalized) {
            Ok(scores) => Ok(evaluation_from_scores(
                target.id,
                &candidate_normalized,
                &target_normalized,
                scores,
                FusionDecision::AutoMerge,
                FusionReason::Matched,
            )),
            Err(rejection) => Ok(with_decision(
                base(),
                FusionDecision::Rejected,
                rejection_reason(rejection),
            )),
        }
    }

    pub fn choose_best(
        &self,
        candidate: &SearchResult,
        targets: &[UnifiedTrack],
    ) -> Result<FusionSelection, FusionServiceError> {
        let evaluations = targets
            .iter()
            .map(|target| self.evaluate(candidate, target))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(evaluation) = evaluations
            .iter()
            .find(|evaluation| evaluation.decision == FusionDecision::ForcedMerge)
        {
            return Ok(FusionSelection {
                target_track_id: Some(evaluation.target_track_id),
                decision: evaluation.decision,
                evaluation: Some(evaluation.clone()),
                candidates: evaluations,
            });
        }
        if let Some(evaluation) = evaluations
            .iter()
            .find(|evaluation| evaluation.decision == FusionDecision::AlreadyUnified)
        {
            return Ok(FusionSelection {
                target_track_id: Some(evaluation.target_track_id),
                decision: evaluation.decision,
                evaluation: Some(evaluation.clone()),
                candidates: evaluations,
            });
        }

        let mut automatic = evaluations
            .iter()
            .filter(|evaluation| evaluation.decision == FusionDecision::AutoMerge)
            .cloned()
            .collect::<Vec<_>>();
        automatic.sort_by(|left, right| {
            right.score_bps.cmp(&left.score_bps).then_with(|| {
                left.target_track_id
                    .to_string()
                    .cmp(&right.target_track_id.to_string())
            })
        });
        if let Some(top) = automatic.first() {
            if let Some(second) = automatic.get(1) {
                if top.score_bps.abs_diff(second.score_bps) < matcher::AMBIGUITY_GAP_BPS {
                    let ambiguous = with_decision(
                        top.clone(),
                        FusionDecision::Rejected,
                        FusionReason::Ambiguous,
                    );
                    return Ok(FusionSelection {
                        target_track_id: None,
                        decision: FusionDecision::Rejected,
                        evaluation: Some(ambiguous),
                        candidates: evaluations,
                    });
                }
            }
            return Ok(FusionSelection {
                target_track_id: Some(top.target_track_id),
                decision: top.decision,
                evaluation: Some(top.clone()),
                candidates: evaluations,
            });
        }

        Ok(FusionSelection {
            target_track_id: None,
            decision: evaluations
                .first()
                .map(|evaluation| evaluation.decision)
                .unwrap_or(FusionDecision::Rejected),
            evaluation: evaluations.first().cloned(),
            candidates: evaluations,
        })
    }

    pub fn accept_match(
        &self,
        candidate: &SearchResult,
        target_track_id: TrackId,
    ) -> Result<FusionEvaluation, FusionServiceError> {
        let target = TrackRepository::new(&self.database)
            .get(target_track_id)?
            .ok_or(RepositoryError::TrackNotFound(target_track_id))?;
        let evaluation = self.evaluate(candidate, &target)?;
        if !matches!(
            evaluation.decision,
            FusionDecision::AutoMerge
                | FusionDecision::ForcedMerge
                | FusionDecision::AlreadyUnified
        ) {
            return Ok(evaluation);
        }
        if evaluation.decision == FusionDecision::AlreadyUnified {
            return Ok(evaluation);
        }
        if !matches!(
            candidate.provider,
            ProviderKind::Youtube | ProviderKind::Soundcloud
        ) {
            return Ok(with_decision(
                evaluation,
                FusionDecision::Excluded,
                FusionReason::ProviderExcluded,
            ));
        }

        let provider_item_id = candidate.provider_item_id.trim();
        let capabilities = crate::ipc::provider_capabilities(candidate.provider);
        let mut source = TrackSource::new(
            SourceId::new(),
            target_track_id,
            candidate.provider,
            provider_item_id,
            capabilities,
        )?;
        source.source_uri = candidate
            .canonical_url
            .as_ref()
            .map(|url| {
                validate_provider_url(candidate.provider, url.as_url().as_str())
                    .map(|safe| safe.as_url().clone())
                    .map_err(|_| FusionServiceError::InvalidProviderUrl {
                        provider: candidate.provider,
                    })
            })
            .transpose()?;
        source.duration_ms = candidate.duration_ms;
        source.version = version_info(&normalize::normalize_search_result(candidate));
        SourceRepository::new(&self.database).attach_source_to_track(&source)?;
        Ok(evaluation)
    }

    pub fn set_override(
        &self,
        provider_kind: ProviderKind,
        provider_item_id: impl Into<String>,
        target_track_id: TrackId,
        decision: FusionOverrideDecision,
    ) -> Result<FusionOverride, FusionServiceError> {
        FusionOverrideRepository::new(&self.database)
            .set_override(provider_kind, provider_item_id, target_track_id, decision)
            .map_err(FusionServiceError::from)
    }

    pub fn clear_override(
        &self,
        provider_kind: ProviderKind,
        provider_item_id: &str,
        target_track_id: TrackId,
    ) -> Result<(), FusionServiceError> {
        FusionOverrideRepository::new(&self.database)
            .clear_override(provider_kind, provider_item_id, target_track_id)
            .map_err(FusionServiceError::from)
    }

    pub fn get_override(
        &self,
        provider_kind: ProviderKind,
        provider_item_id: &str,
        target_track_id: TrackId,
    ) -> Result<Option<FusionOverride>, FusionServiceError> {
        FusionOverrideRepository::new(&self.database)
            .get_override(provider_kind, provider_item_id, target_track_id)
            .map_err(FusionServiceError::from)
    }
}

fn evaluation_from_scores(
    target_track_id: TrackId,
    candidate: &normalize::NormalizedCandidate,
    target: &normalize::NormalizedCandidate,
    scores: matcher::MatchScores,
    decision: FusionDecision,
    reason: FusionReason,
) -> FusionEvaluation {
    FusionEvaluation {
        target_track_id,
        decision,
        score_bps: scores.score_bps,
        threshold_bps: matcher::AUTOMATIC_MATCH_THRESHOLD_BPS,
        title_score_bps: scores.title_score_bps,
        artist_score_bps: scores.artist_score_bps,
        duration_score_bps: scores.duration_score_bps,
        duration_delta_ms: scores.duration_delta_ms,
        candidate_qualifiers: candidate.guarded_qualifiers.clone(),
        target_qualifiers: target.guarded_qualifiers.clone(),
        reason,
    }
}

fn with_decision(
    mut evaluation: FusionEvaluation,
    decision: FusionDecision,
    reason: FusionReason,
) -> FusionEvaluation {
    evaluation.decision = decision;
    evaluation.reason = reason;
    evaluation
}

fn rejection_reason(rejection: matcher::MatcherRejection) -> FusionReason {
    match rejection {
        matcher::MatcherRejection::VersionMismatch => FusionReason::VersionMismatch,
        matcher::MatcherRejection::TitleBelowMinimum => FusionReason::TitleBelowMinimum,
        matcher::MatcherRejection::ArtistBelowMinimum => FusionReason::ArtistBelowMinimum,
        matcher::MatcherRejection::DurationMismatch => FusionReason::DurationMismatch,
        matcher::MatcherRejection::BelowThreshold => FusionReason::BelowThreshold,
    }
}

fn version_info(candidate: &normalize::NormalizedCandidate) -> VersionInfo {
    if candidate.guarded_qualifiers.is_empty() {
        VersionInfo::standard()
    } else {
        VersionInfo::new(candidate.guarded_qualifiers.clone())
            .expect("normalized qualifiers unique")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::TempDatabasePath;
    use crate::domain::{Artist, ArtistId, SourceCapabilities};
    use crate::search::types::SearchEntityKind;

    fn target(database: &Database, title: &str, artist: &str) -> UnifiedTrack {
        let track_id = TrackId::new();
        let track = UnifiedTrack::new(
            track_id,
            title,
            vec![Artist::new(ArtistId::new(), artist).unwrap()],
            None,
            Some(180_000),
            VersionInfo::standard(),
            vec![],
        )
        .unwrap();
        TrackRepository::new(database).create(&track).unwrap();
        track
    }

    fn candidate(provider: ProviderKind, title: &str, artist: &str, item: &str) -> SearchResult {
        SearchResult {
            provider,
            entity_kind: SearchEntityKind::Track,
            provider_item_id: item.to_owned(),
            canonical_url: None,
            title: title.to_owned(),
            artists: vec![artist.to_owned()],
            album: None,
            duration_ms: Some(180_000),
            artwork_url: None,
            published_at: None,
            engagement_count: None,
            engagement_kind: None,
            explicit: None,
            local_track_id: None,
            local_source_id: None,
            original_rank: 0,
        }
    }

    #[test]
    fn automatic_cross_provider_match_is_conservative_and_ephemeral() {
        let path = TempDatabasePath::new("fusion-service");
        let database = Database::open(path.path()).unwrap();
        let target = target(&database, "Signal Test", "Artist");
        let service = SourceFusionService::new(database.clone());
        let candidate = candidate(
            ProviderKind::Youtube,
            "Artist - Signal Test",
            "Artist",
            "video-1",
        );

        let result = service.evaluate(&candidate, &target).unwrap();

        assert_eq!(result.decision, FusionDecision::AutoMerge);
        let source_count: i64 = database
            .with_connection(|connection| {
                connection.query_row("SELECT COUNT(*) FROM track_sources", [], |row| row.get(0))
            })
            .unwrap();
        assert_eq!(source_count, 0);
    }

    #[test]
    fn manual_merge_bypasses_match_guards_but_split_wins_for_target() {
        let path = TempDatabasePath::new("fusion-overrides");
        let database = Database::open(path.path()).unwrap();
        let target = target(&database, "Signal Test", "Artist");
        let service = SourceFusionService::new(database.clone());
        let candidate = candidate(ProviderKind::Soundcloud, "Different", "Other", "track-1");

        service
            .set_override(
                ProviderKind::Soundcloud,
                "track-1",
                target.id,
                FusionOverrideDecision::Merge,
            )
            .unwrap();
        assert_eq!(
            service.evaluate(&candidate, &target).unwrap().decision,
            FusionDecision::ForcedMerge
        );
        service
            .set_override(
                ProviderKind::Soundcloud,
                "track-1",
                target.id,
                FusionOverrideDecision::Split,
            )
            .unwrap();
        assert_eq!(
            service.evaluate(&candidate, &target).unwrap().decision,
            FusionDecision::ForcedSplit
        );
    }

    #[test]
    fn spotify_is_excluded_and_accepting_a_match_persists_one_remote_source() {
        let path = TempDatabasePath::new("fusion-accept");
        let database = Database::open(path.path()).unwrap();
        let target = target(&database, "Signal Test", "Artist");
        let service = SourceFusionService::new(database.clone());
        let mut candidate = candidate(ProviderKind::Youtube, "Signal Test", "Artist", "video-1");
        let accepted = service.accept_match(&candidate, target.id).unwrap();
        assert_eq!(accepted.decision, FusionDecision::AutoMerge);
        let source_count: i64 = database
            .with_connection(|connection| {
                connection.query_row("SELECT COUNT(*) FROM track_sources", [], |row| row.get(0))
            })
            .unwrap();
        assert_eq!(source_count, 1);
        assert!(database
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM local_files WHERE source_id IN (SELECT id FROM track_sources WHERE provider_kind = 'youtube')",
                    [],
                    |row| row.get::<_, i64>(0),
                )
            })
            .unwrap() == 0);

        candidate.provider = ProviderKind::Spotify;
        assert_eq!(
            service.evaluate(&candidate, &target).unwrap().decision,
            FusionDecision::Excluded
        );
    }

    #[test]
    fn best_match_uses_forced_merge_then_ambiguity_guard() {
        let path = TempDatabasePath::new("fusion-best");
        let database = Database::open(path.path()).unwrap();
        let first = target(&database, "Signal Test", "Artist");
        let second = target(&database, "Signal Test", "Artist");
        let service = SourceFusionService::new(database.clone());
        let candidate = candidate(ProviderKind::Youtube, "Signal Test", "Artist", "video-1");
        let selection = service
            .choose_best(&candidate, &[first.clone(), second.clone()])
            .unwrap();
        assert_eq!(selection.target_track_id, None);
        assert_eq!(
            selection.evaluation.unwrap().reason,
            FusionReason::Ambiguous
        );

        service
            .set_override(
                ProviderKind::Youtube,
                "video-1",
                second.id,
                FusionOverrideDecision::Merge,
            )
            .unwrap();
        let selection = service
            .choose_best(&candidate, &[first, second.clone()])
            .unwrap();
        assert_eq!(selection.target_track_id, Some(second.id));
        assert_eq!(selection.decision, FusionDecision::ForcedMerge);
    }

    #[test]
    fn manual_acceptance_keeps_target_metadata_unchanged() {
        let path = TempDatabasePath::new("fusion-metadata");
        let database = Database::open(path.path()).unwrap();
        let target = target(&database, "Signal Test", "Artist");
        let before = TrackRepository::new(&database)
            .get(target.id)
            .unwrap()
            .unwrap();
        let service = SourceFusionService::new(database.clone());
        let candidate = candidate(ProviderKind::Youtube, "Other", "Other", "video-1");
        service
            .set_override(
                ProviderKind::Youtube,
                "video-1",
                target.id,
                FusionOverrideDecision::Merge,
            )
            .unwrap();
        service.accept_match(&candidate, target.id).unwrap();
        let after = TrackRepository::new(&database)
            .get(target.id)
            .unwrap()
            .unwrap();
        assert_eq!(after.title, before.title);
        assert_eq!(after.artists, before.artists);
        assert_eq!(after.duration_ms, before.duration_ms);
        assert_eq!(after.preferred_source_id, before.preferred_source_id);
    }

    #[allow(dead_code)]
    fn _capabilities_are_not_frontend_input() -> SourceCapabilities {
        crate::ipc::provider_capabilities(ProviderKind::Youtube)
    }
}
