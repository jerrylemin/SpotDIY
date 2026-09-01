use std::sync::OnceLock;

use regex::Regex;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

use crate::domain::{UnifiedTrack, VersionInfo, VersionQualifier};
use crate::search::types::SearchResult;

const GUARDED_QUALIFIERS: [VersionQualifier; 10] = [
    VersionQualifier::Live,
    VersionQualifier::Acoustic,
    VersionQualifier::Remix,
    VersionQualifier::Remaster,
    VersionQualifier::Cover,
    VersionQualifier::Instrumental,
    VersionQualifier::Karaoke,
    VersionQualifier::SpedUp,
    VersionQualifier::Slowed,
    VersionQualifier::Nightcore,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedCandidate {
    pub normalized_title: String,
    pub normalized_artists: Vec<String>,
    pub artist_key: String,
    pub guarded_qualifiers: Vec<VersionQualifier>,
    pub duration_ms: Option<u64>,
}

impl NormalizedCandidate {
    pub fn title(&self) -> &str {
        &self.normalized_title
    }

    pub fn artists(&self) -> &[String] {
        &self.normalized_artists
    }

    pub fn qualifiers(&self) -> &[VersionQualifier] {
        &self.guarded_qualifiers
    }
}

pub fn normalize_search_result(result: &SearchResult) -> NormalizedCandidate {
    normalize_parts(&result.title, &result.artists, result.duration_ms, &[])
}

pub fn normalize_candidate(result: &SearchResult) -> NormalizedCandidate {
    normalize_search_result(result)
}

pub fn normalize_unified_track(track: &UnifiedTrack) -> NormalizedCandidate {
    normalize_parts(
        &track.title,
        &track
            .artists
            .iter()
            .map(|artist| artist.name.clone())
            .collect::<Vec<_>>(),
        track.duration_ms,
        &track.version.qualifiers,
    )
}

pub fn normalize_title(value: &str) -> String {
    normalize_basic(value)
}

pub fn normalize_artist_list(values: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        for part in split_artist_expression(strip_topic_suffix(value)) {
            let part = normalize_basic(&part);
            if !part.is_empty() {
                normalized.push(part);
            }
        }
    }
    normalized.sort();
    normalized.dedup();
    normalized
}

pub fn guarded_version_qualifiers(value: &str) -> Vec<VersionQualifier> {
    let mut qualifiers = Vec::new();
    add_text_qualifiers(value, &mut qualifiers);
    sort_qualifiers(&mut qualifiers);
    qualifiers
}

pub fn guarded_qualifiers(version: &VersionInfo) -> Vec<VersionQualifier> {
    let mut qualifiers = version
        .qualifiers
        .iter()
        .copied()
        .filter(|qualifier| GUARDED_QUALIFIERS.contains(qualifier))
        .collect::<Vec<_>>();
    sort_qualifiers(&mut qualifiers);
    qualifiers
}

fn normalize_parts(
    title: &str,
    artists: &[String],
    duration_ms: Option<u64>,
    explicit_qualifiers: &[VersionQualifier],
) -> NormalizedCandidate {
    let (title, mut featured_artists, mut title_qualifiers) = strip_title_annotations(title);
    let mut normalized_artists = normalize_artist_list(artists);
    normalized_artists.extend(normalize_artist_list(&featured_artists));
    normalized_artists.sort();
    normalized_artists.dedup();

    let title = strip_artist_prefix(&title, &normalized_artists);
    title_qualifiers.extend(
        explicit_qualifiers
            .iter()
            .copied()
            .filter(|qualifier| GUARDED_QUALIFIERS.contains(qualifier)),
    );
    sort_qualifiers(&mut title_qualifiers);
    featured_artists.clear();

    NormalizedCandidate {
        normalized_title: normalize_basic(&title),
        artist_key: normalized_artists.join("|"),
        normalized_artists,
        guarded_qualifiers: title_qualifiers,
        duration_ms,
    }
}

fn strip_title_annotations(value: &str) -> (String, Vec<String>, Vec<VersionQualifier>) {
    let mut working = value.trim().to_owned();
    let mut featured_artists = Vec::new();
    let mut qualifiers = Vec::new();

    loop {
        let Some((start, content)) = trailing_annotation(&working) else {
            break;
        };
        let (features, annotation_qualifiers, presentation_noise) = classify_annotation(content);
        if features.is_empty() && annotation_qualifiers.is_empty() && !presentation_noise {
            break;
        }
        featured_artists.extend(features);
        qualifiers.extend(annotation_qualifiers);
        working.truncate(start);
        working = working.trim_end().to_owned();
    }

    loop {
        let Some(separator) = working.rfind(" - ") else {
            break;
        };
        let suffix = working[separator + 3..].trim();
        let (features, suffix_qualifiers, presentation_noise) = classify_annotation(suffix);
        if features.is_empty() && suffix_qualifiers.is_empty() && !presentation_noise {
            break;
        }
        featured_artists.extend(features);
        qualifiers.extend(suffix_qualifiers);
        working.truncate(separator);
        working = working.trim_end().to_owned();
    }

    if let Some(captures) = feature_suffix_regex().captures(&working) {
        if let Some(feature_match) = captures.get(1) {
            featured_artists.extend(split_artist_expression(feature_match.as_str().to_owned()));
        }
        working.truncate(captures.get(0).expect("full regex match").start());
        working = working.trim_end().to_owned();
    }

    (working, featured_artists, qualifiers)
}

fn trailing_annotation(value: &str) -> Option<(usize, &str)> {
    let last = value.chars().last()?;
    let opening = match last {
        ')' => '(',
        ']' => '[',
        _ => return None,
    };
    let start = value.rfind(opening)?;
    (start < value.len() - 1)
        .then_some((start, &value[start + opening.len_utf8()..value.len() - 1]))
}

fn classify_annotation(value: &str) -> (Vec<String>, Vec<VersionQualifier>, bool) {
    if let Some(marker) = feature_regex().find(value) {
        let mut features = Vec::new();
        let before = value[..marker.start()].trim();
        let after = value[marker.end()..].trim();
        if !before.is_empty() {
            features.extend(split_artist_expression(before.to_owned()));
        }
        if !after.is_empty() {
            features.extend(split_artist_expression(after.to_owned()));
        }
        return (features, Vec::new(), false);
    }
    let qualifiers = guarded_version_qualifiers(value);
    let presentation_noise = is_presentation_noise(value);
    (Vec::new(), qualifiers, presentation_noise)
}

fn is_presentation_noise(value: &str) -> bool {
    matches!(
        normalize_basic(value).as_str(),
        "official audio"
            | "official video"
            | "official music video"
            | "official lyrics"
            | "official lyric video"
            | "lyrics"
            | "lyric video"
            | "visualizer"
            | "audio"
            | "video"
    )
}

fn add_text_qualifiers(value: &str, output: &mut Vec<VersionQualifier>) {
    let normalized = normalize_basic(value);
    let words = normalized.split_whitespace().collect::<Vec<_>>();
    let has = |word: &str| words.iter().any(|candidate| *candidate == word);
    if normalized == "live"
        || normalized.starts_with("live session")
        || normalized.starts_with("live version")
        || normalized.contains(" live at ")
        || normalized.ends_with(" live session")
    {
        output.push(VersionQualifier::Live);
    }
    if has("acoustic") {
        output.push(VersionQualifier::Acoustic);
    }
    if normalized.contains("remix") {
        output.push(VersionQualifier::Remix);
    }
    if normalized.contains("remaster") || normalized.contains("remastered") {
        output.push(VersionQualifier::Remaster);
    }
    if has("cover") {
        output.push(VersionQualifier::Cover);
    }
    if has("instrumental") {
        output.push(VersionQualifier::Instrumental);
    }
    if has("karaoke") {
        output.push(VersionQualifier::Karaoke);
    }
    if has("sped") && has("up") {
        output.push(VersionQualifier::SpedUp);
    }
    if has("slowed") {
        output.push(VersionQualifier::Slowed);
    }
    if has("nightcore") {
        output.push(VersionQualifier::Nightcore);
    }
}

fn strip_artist_prefix(value: &str, normalized_artists: &[String]) -> String {
    let Some((left, right)) = value.split_once(" - ") else {
        return value.to_owned();
    };
    let prefix_artists = normalize_artist_list(&[left.to_owned()]);
    if !prefix_artists.is_empty() && prefix_artists == normalized_artists
        || prefix_artists.len() == 1 && normalized_artists.contains(&prefix_artists[0])
    {
        return right.trim().to_owned();
    }
    value.to_owned()
}

fn strip_topic_suffix(value: &str) -> String {
    let Some((left, right)) = value.rsplit_once(" - ") else {
        return value.to_owned();
    };
    if normalize_basic(right) == "topic" {
        left.trim().to_owned()
    } else {
        value.to_owned()
    }
}

fn split_artist_expression(value: String) -> Vec<String> {
    let mut parts = Vec::new();
    for part in feature_regex().split(&value) {
        for part in part.split([',', '&', '/', ';', '|', '×', '+']) {
            for part in artist_word_separator_regex().split(part) {
                let part = part.trim();
                if !part.is_empty() {
                    parts.push(part.to_owned());
                }
            }
        }
    }
    parts
}

fn normalize_basic(value: &str) -> String {
    value
        .nfkd()
        .filter(|character| !is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn sort_qualifiers(qualifiers: &mut Vec<VersionQualifier>) {
    qualifiers.sort_by_key(|qualifier| {
        GUARDED_QUALIFIERS
            .iter()
            .position(|candidate| candidate == qualifier)
            .unwrap_or(usize::MAX)
    });
    qualifiers.dedup();
}

fn feature_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?i)\b(?:feat\.?|ft\.?|featuring)\b").unwrap())
}

fn feature_suffix_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?i)\s+(?:feat\.?|ft\.?|featuring)\s+(.+)$").unwrap())
}

fn artist_word_separator_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?i)\s+(?:and|x)\s+").unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProviderKind;
    use crate::search::types::SearchEntityKind;

    fn result(title: &str, artists: &[&str]) -> SearchResult {
        SearchResult {
            provider: ProviderKind::Youtube,
            entity_kind: SearchEntityKind::Track,
            provider_item_id: "candidate".to_owned(),
            canonical_url: None,
            title: title.to_owned(),
            artists: artists.iter().map(|artist| (*artist).to_owned()).collect(),
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
    fn unicode_punctuation_and_whitespace_are_deterministic() {
        let first = normalize_search_result(&result("  Beyoncé — Halo!  ", &["Beyoncé"]));
        let second = normalize_search_result(&result("Beyonce Halo", &["Beyonce"]));
        assert_eq!(first.normalized_title, second.normalized_title);
        assert_eq!(first.normalized_artists, second.normalized_artists);
    }

    #[test]
    fn featuring_syntax_and_artist_separators_are_equivalent() {
        let first = normalize_search_result(&result("Song (feat. Guest)", &["Artist"]));
        let second = normalize_search_result(&result("Song", &["Artist ft Guest"]));
        let third = normalize_search_result(&result("Song", &["Artist featuring Guest"]));
        assert_eq!(first.normalized_artists, second.normalized_artists);
        assert_eq!(second.normalized_artists, third.normalized_artists);
        assert_eq!(first.normalized_title, "song");
    }

    #[test]
    fn provider_presentation_noise_and_topic_suffixes_are_removed() {
        for suffix in [
            "(Official Audio)",
            "[Official Video]",
            " - Official Audio",
            "(Lyrics)",
            "(Lyric Video)",
            "(Visualizer)",
        ] {
            assert_eq!(
                normalize_search_result(&result(&format!("Song {suffix}"), &["Artist - Topic"]))
                    .normalized_title,
                "song"
            );
        }
        assert_eq!(
            normalize_artist_list(&["Artist - Topic".to_owned()]),
            vec!["artist"]
        );
    }

    #[test]
    fn artist_title_prefix_is_only_removed_when_it_matches() {
        assert_eq!(
            normalize_search_result(&result("Artist - Song", &["Artist"])).normalized_title,
            "song"
        );
        assert_eq!(
            normalize_search_result(&result("Other - Song", &["Artist"])).normalized_title,
            "other song"
        );
    }

    #[test]
    fn live_forever_is_not_a_live_version() {
        let candidate = normalize_search_result(&result("Live Forever", &["Artist"]));
        assert!(candidate.guarded_qualifiers.is_empty());
        assert_eq!(candidate.normalized_title, "live forever");
    }

    #[test]
    fn every_plan_six_guarded_qualifier_is_extracted_from_annotations() {
        let examples = [
            ("Live", VersionQualifier::Live),
            ("Acoustic", VersionQualifier::Acoustic),
            ("Remix", VersionQualifier::Remix),
            ("Remaster", VersionQualifier::Remaster),
            ("Cover", VersionQualifier::Cover),
            ("Instrumental", VersionQualifier::Instrumental),
            ("Karaoke", VersionQualifier::Karaoke),
            ("Sped-Up", VersionQualifier::SpedUp),
            ("Slowed", VersionQualifier::Slowed),
            ("Nightcore", VersionQualifier::Nightcore),
        ];
        for (label, qualifier) in examples {
            let candidate =
                normalize_search_result(&result(&format!("Song ({label})"), &["Artist"]));
            assert_eq!(candidate.guarded_qualifiers, vec![qualifier], "{label}");
        }
    }
}
