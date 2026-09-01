use strsim::jaro_winkler;

use super::normalize::NormalizedCandidate;

pub const AUTOMATIC_MATCH_THRESHOLD_BPS: u16 = 8_800;
pub const HARD_TITLE_MINIMUM_BPS: u16 = 9_000;
pub const HARD_ARTIST_MINIMUM_BPS: u16 = 9_000;
pub const TITLE_WEIGHT_PERCENT: u32 = 55;
pub const ARTIST_WEIGHT_PERCENT: u32 = 35;
pub const DURATION_WEIGHT_PERCENT: u32 = 10;
pub const AMBIGUITY_GAP_BPS: u16 = 300;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatcherRejection {
    VersionMismatch,
    TitleBelowMinimum,
    ArtistBelowMinimum,
    DurationMismatch,
    BelowThreshold,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatchScores {
    pub score_bps: u16,
    pub title_score_bps: u16,
    pub artist_score_bps: u16,
    pub duration_score_bps: u16,
    pub duration_delta_ms: Option<u64>,
}

pub fn score(candidate: &NormalizedCandidate, target: &NormalizedCandidate) -> MatchScores {
    let title_score_bps = similarity_bps(&candidate.normalized_title, &target.normalized_title);
    let artist_score_bps = similarity_bps(&candidate.artist_key, &target.artist_key);
    let (duration_score_bps, duration_delta_ms) =
        duration_score(candidate.duration_ms, target.duration_ms);
    let weighted = (u32::from(title_score_bps) * TITLE_WEIGHT_PERCENT
        + u32::from(artist_score_bps) * ARTIST_WEIGHT_PERCENT
        + u32::from(duration_score_bps) * DURATION_WEIGHT_PERCENT
        + 50)
        / 100;
    MatchScores {
        score_bps: weighted as u16,
        title_score_bps,
        artist_score_bps,
        duration_score_bps,
        duration_delta_ms,
    }
}

pub fn automatic_match(
    candidate: &NormalizedCandidate,
    target: &NormalizedCandidate,
) -> Result<MatchScores, MatcherRejection> {
    if candidate.guarded_qualifiers != target.guarded_qualifiers {
        return Err(MatcherRejection::VersionMismatch);
    }
    let scores = score(candidate, target);
    if scores.title_score_bps < HARD_TITLE_MINIMUM_BPS {
        return Err(MatcherRejection::TitleBelowMinimum);
    }
    if scores.artist_score_bps < HARD_ARTIST_MINIMUM_BPS {
        return Err(MatcherRejection::ArtistBelowMinimum);
    }
    if let (Some(candidate_duration), Some(target_duration)) =
        (candidate.duration_ms, target.duration_ms)
    {
        let hard_guard_ms = hard_duration_guard_ms(candidate_duration, target_duration);
        if scores.duration_delta_ms.unwrap_or_default() > hard_guard_ms {
            return Err(MatcherRejection::DurationMismatch);
        }
    }
    if scores.score_bps < AUTOMATIC_MATCH_THRESHOLD_BPS {
        return Err(MatcherRejection::BelowThreshold);
    }
    Ok(scores)
}

pub fn duration_score(
    candidate_duration_ms: Option<u64>,
    target_duration_ms: Option<u64>,
) -> (u16, Option<u64>) {
    let (Some(candidate_duration_ms), Some(target_duration_ms)) =
        (candidate_duration_ms, target_duration_ms)
    else {
        return (0, None);
    };
    let duration_delta_ms = candidate_duration_ms.abs_diff(target_duration_ms);
    let score = if duration_delta_ms <= 2_000 {
        10_000
    } else if duration_delta_ms <= 5_000 {
        7_000
    } else if duration_delta_ms <= hard_duration_guard_ms(candidate_duration_ms, target_duration_ms)
    {
        2_000
    } else {
        0
    };
    (score, Some(duration_delta_ms))
}

pub fn hard_duration_guard_ms(left_ms: u64, right_ms: u64) -> u64 {
    let longer_ms = left_ms.max(right_ms);
    10_000.max(longer_ms.saturating_mul(4) / 100)
}

fn similarity_bps(left: &str, right: &str) -> u16 {
    if left.is_empty() || right.is_empty() {
        return 0;
    }
    (jaro_winkler(left, right) * 10_000.0)
        .round()
        .clamp(0.0, 10_000.0) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VersionQualifier;
    use crate::fusion::normalize::NormalizedCandidate;

    fn candidate(title: &str, artists: &[&str], duration_ms: Option<u64>) -> NormalizedCandidate {
        NormalizedCandidate {
            normalized_title: title.to_owned(),
            normalized_artists: artists.iter().map(|artist| (*artist).to_owned()).collect(),
            artist_key: artists.join("|"),
            guarded_qualifiers: Vec::new(),
            duration_ms,
        }
    }

    #[test]
    fn perfect_match_without_duration_scores_nine_thousand_and_is_accepted() {
        let left = candidate("song", &["artist"], None);
        let right = candidate("song", &["artist"], Some(180_000));
        let scores = automatic_match(&left, &right).unwrap();
        assert_eq!(scores.score_bps, 9_000);
        assert_eq!(scores.duration_score_bps, 0);
    }

    #[test]
    fn duration_bands_and_hard_guard_are_conservative() {
        assert_eq!(duration_score(Some(180_000), Some(182_000)).0, 10_000);
        assert_eq!(duration_score(Some(180_000), Some(185_000)).0, 7_000);
        assert_eq!(duration_score(Some(180_000), Some(190_000)).0, 2_000);
        assert_eq!(duration_score(Some(180_000), Some(190_001)).0, 0);
        assert!(matches!(
            automatic_match(
                &candidate("song", &["artist"], Some(180_000)),
                &candidate("song", &["artist"], Some(190_001)),
            ),
            Err(MatcherRejection::DurationMismatch)
        ));
    }

    #[test]
    fn title_and_artist_hard_minima_precede_weighted_threshold() {
        let title_mismatch = candidate("different", &["artist"], Some(180_000));
        let same_title = candidate("song", &["artist"], Some(180_000));
        assert!(matches!(
            automatic_match(&title_mismatch, &same_title),
            Err(MatcherRejection::TitleBelowMinimum)
        ));

        let artist_mismatch = candidate("song", &["other"], Some(180_000));
        assert!(matches!(
            automatic_match(&artist_mismatch, &same_title),
            Err(MatcherRejection::ArtistBelowMinimum)
        ));
    }

    #[test]
    fn guarded_versions_must_match_exactly() {
        let mut live = candidate("song", &["artist"], Some(180_000));
        live.guarded_qualifiers = vec![VersionQualifier::Live];
        let standard = candidate("song", &["artist"], Some(180_000));
        assert!(matches!(
            automatic_match(&live, &standard),
            Err(MatcherRejection::VersionMismatch)
        ));
    }

    #[test]
    fn threshold_boundary_is_inclusive() {
        let left = candidate("song", &["artist"], None);
        let right = candidate("song", &["artist"], None);
        assert_eq!(
            score(&left, &right).score_bps,
            AUTOMATIC_MATCH_THRESHOLD_BPS + 200
        );
        assert!(automatic_match(&left, &right).is_ok());
    }
}
