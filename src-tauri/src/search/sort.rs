use super::types::{
    EngagementKind, SearchEntityKind, SearchLens, SearchResult, SearchSortDirection,
    SearchSortField,
};
use crate::domain::{ProviderKind, SourceCapabilities};
use std::cmp::Ordering;

pub fn all_provider_kinds_for_lens(lens: SearchLens) -> &'static [ProviderKind] {
    const TRACK_PROVIDERS: &[ProviderKind] = &[
        ProviderKind::Local,
        ProviderKind::Youtube,
        ProviderKind::Soundcloud,
    ];
    const LOCAL: &[ProviderKind] = &[ProviderKind::Local];
    const YOUTUBE: &[ProviderKind] = &[ProviderKind::Youtube];
    const SOUNDCLOUD: &[ProviderKind] = &[ProviderKind::Soundcloud];
    const SPOTIFY: &[ProviderKind] = &[ProviderKind::Spotify];
    match lens {
        SearchLens::Local => LOCAL,
        SearchLens::Youtube => YOUTUBE,
        SearchLens::Soundcloud => SOUNDCLOUD,
        SearchLens::Spotify => SPOTIFY,
        SearchLens::All | SearchLens::Tracks | SearchLens::Playlists => TRACK_PROVIDERS,
        SearchLens::Artists | SearchLens::Albums => LOCAL,
    }
}

pub fn entities_for_lens(lens: SearchLens) -> &'static [SearchEntityKind] {
    const TRACKS: &[SearchEntityKind] = &[SearchEntityKind::Track];
    const ARTISTS: &[SearchEntityKind] = &[SearchEntityKind::Artist];
    const ALBUMS: &[SearchEntityKind] = &[SearchEntityKind::Album];
    const PLAYLISTS: &[SearchEntityKind] = &[SearchEntityKind::Playlist];
    const ALL: &[SearchEntityKind] = &[
        SearchEntityKind::Track,
        SearchEntityKind::Artist,
        SearchEntityKind::Album,
        SearchEntityKind::Playlist,
    ];
    match lens {
        SearchLens::Tracks | SearchLens::Local | SearchLens::Youtube | SearchLens::Soundcloud => {
            TRACKS
        }
        SearchLens::Spotify => &[
            SearchEntityKind::Track,
            SearchEntityKind::Artist,
            SearchEntityKind::Album,
        ],
        SearchLens::Artists => ARTISTS,
        SearchLens::Albums => ALBUMS,
        SearchLens::Playlists => PLAYLISTS,
        SearchLens::All => ALL,
    }
}

pub(crate) fn sort_provider_results(
    provider: ProviderKind,
    capabilities: SourceCapabilities,
    results: &mut [SearchResult],
    field: SearchSortField,
    direction: SearchSortDirection,
) {
    if !sort_is_supported(provider, capabilities, results, field) {
        results.sort_by(relevance_order);
        return;
    }
    results.sort_by(|left, right| {
        let primary = match field {
            SearchSortField::Relevance => Ordering::Equal,
            SearchSortField::Popularity => {
                nullable_order(left.engagement_count, right.engagement_count, direction)
            }
            SearchSortField::Newest | SearchSortField::Oldest => nullable_order_by(
                left.published_at.as_ref().map(|date| date.value()),
                right.published_at.as_ref().map(|date| date.value()),
                direction,
            ),
            SearchSortField::Duration => {
                nullable_order(left.duration_ms, right.duration_ms, direction)
            }
            SearchSortField::DateAdded
            | SearchSortField::Downloaded
            | SearchSortField::AudioQuality => Ordering::Equal,
        };
        primary.then_with(|| relevance_order(left, right))
    });
}

fn sort_is_supported(
    provider: ProviderKind,
    capabilities: SourceCapabilities,
    results: &[SearchResult],
    field: SearchSortField,
) -> bool {
    match field {
        SearchSortField::Relevance | SearchSortField::Duration => true,
        SearchSortField::Newest | SearchSortField::Oldest => capabilities.release_date,
        SearchSortField::Popularity => {
            let expected = match provider {
                ProviderKind::Youtube => Some(EngagementKind::Views),
                ProviderKind::Soundcloud => Some(EngagementKind::Plays),
                ProviderKind::Local | ProviderKind::Spotify => None,
            };
            capabilities.popularity
                && expected.is_some()
                && results.iter().all(|result| {
                    match (result.engagement_count, result.engagement_kind) {
                        (None, None) => true,
                        (Some(_), kind) => kind == expected,
                        (None, Some(_)) => false,
                    }
                })
        }
        SearchSortField::DateAdded
        | SearchSortField::Downloaded
        | SearchSortField::AudioQuality => false,
    }
}

fn relevance_order(left: &SearchResult, right: &SearchResult) -> Ordering {
    left.original_rank
        .cmp(&right.original_rank)
        .then_with(|| left.provider_item_id.cmp(&right.provider_item_id))
}

fn nullable_order<T: Ord + Copy>(
    left: Option<T>,
    right: Option<T>,
    direction: SearchSortDirection,
) -> Ordering {
    nullable_order_by(left, right, direction)
}

fn nullable_order_by<T: Ord>(
    left: Option<T>,
    right: Option<T>,
    direction: SearchSortDirection,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => match direction {
            SearchSortDirection::Ascending => left.cmp(&right),
            SearchSortDirection::Descending => right.cmp(&left),
        },
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProviderKind;

    #[test]
    fn all_lens_excludes_spotify() {
        assert!(!all_provider_kinds_for_lens(SearchLens::All).contains(&ProviderKind::Spotify));
    }

    #[test]
    fn tracks_lens_uses_local_youtube_and_soundcloud() {
        assert_eq!(
            all_provider_kinds_for_lens(SearchLens::Tracks),
            &[
                ProviderKind::Local,
                ProviderKind::Youtube,
                ProviderKind::Soundcloud
            ]
        );
    }

    #[test]
    fn artists_and_albums_lenses_use_local_only() {
        for lens in [SearchLens::Artists, SearchLens::Albums] {
            assert_eq!(all_provider_kinds_for_lens(lens), &[ProviderKind::Local]);
        }
    }

    #[test]
    fn spotify_lens_selects_only_spotify() {
        assert_eq!(
            all_provider_kinds_for_lens(SearchLens::Spotify),
            &[ProviderKind::Spotify]
        );
    }
}
