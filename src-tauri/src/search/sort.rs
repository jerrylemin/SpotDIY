use super::types::{SearchEntityKind, SearchLens};
use crate::domain::ProviderKind;

pub fn all_provider_kinds_for_lens(lens: SearchLens) -> &'static [ProviderKind] {
    const ONLINE: &[ProviderKind] = &[
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
        SearchLens::All
        | SearchLens::Tracks
        | SearchLens::Artists
        | SearchLens::Albums
        | SearchLens::Playlists => ONLINE,
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
        SearchLens::Tracks
        | SearchLens::Local
        | SearchLens::Youtube
        | SearchLens::Soundcloud
        | SearchLens::Spotify => TRACKS,
        SearchLens::Artists => ARTISTS,
        SearchLens::Albums => ALBUMS,
        SearchLens::Playlists => PLAYLISTS,
        SearchLens::All => ALL,
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
    fn spotify_lens_selects_only_spotify() {
        assert_eq!(
            all_provider_kinds_for_lens(SearchLens::Spotify),
            &[ProviderKind::Spotify]
        );
    }
}
