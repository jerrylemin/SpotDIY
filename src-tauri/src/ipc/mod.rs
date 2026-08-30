use serde::Serialize;

use crate::domain::{ProviderKind, SourceCapabilities};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub kind: ProviderKind,
    pub capabilities: SourceCapabilities,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub kind: ProviderKind,
    pub label: &'static str,
    pub configured: bool,
    pub available: bool,
    pub capabilities: SourceCapabilities,
    pub detail: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub version: &'static str,
    pub runtime: &'static str,
    pub storage_mode: &'static str,
    pub first_run: bool,
    pub tracks_indexed: u64,
    pub music_folders: Vec<String>,
    pub providers: Vec<ProviderStatus>,
}

const LOCAL_CAPABILITIES: SourceCapabilities = SourceCapabilities {
    search: true,
    playback: true,
    metadata: true,
    artwork: true,
    lyrics: true,
    downloads: false,
};
const VIDEO_CAPABILITIES: SourceCapabilities = SourceCapabilities {
    search: true,
    playback: true,
    metadata: true,
    artwork: true,
    lyrics: false,
    downloads: true,
};
const SPOTIFY_CAPABILITIES: SourceCapabilities = SourceCapabilities {
    search: true,
    playback: false,
    metadata: true,
    artwork: true,
    lyrics: false,
    downloads: false,
};

pub fn app_status(version: &'static str) -> AppStatus {
    AppStatus {
        version,
        runtime: "tauri",
        storage_mode: "standard",
        first_run: true,
        tracks_indexed: 0,
        music_folders: Vec::new(),
        providers: vec![
            ProviderStatus {
                kind: ProviderKind::Local,
                label: "Local library",
                configured: false,
                available: true,
                capabilities: LOCAL_CAPABILITIES,
                detail: "Add a music folder to begin indexing.",
            },
            ProviderStatus {
                kind: ProviderKind::Youtube,
                label: "YouTube",
                configured: false,
                available: false,
                capabilities: VIDEO_CAPABILITIES,
                detail: "Provider adapter awaits media-tool verification.",
            },
            ProviderStatus {
                kind: ProviderKind::Soundcloud,
                label: "SoundCloud",
                configured: false,
                available: false,
                capabilities: VIDEO_CAPABILITIES,
                detail: "Provider adapter awaits media-tool verification.",
            },
            ProviderStatus {
                kind: ProviderKind::Spotify,
                label: "Spotify catalog",
                configured: false,
                available: false,
                capabilities: SPOTIFY_CAPABILITIES,
                detail: "Connect Client Credentials locally to search the catalog.",
            },
        ],
    }
}

pub fn source_capabilities() -> Vec<ProviderCapabilities> {
    vec![
        ProviderCapabilities {
            kind: ProviderKind::Local,
            capabilities: LOCAL_CAPABILITIES,
        },
        ProviderCapabilities {
            kind: ProviderKind::Youtube,
            capabilities: VIDEO_CAPABILITIES,
        },
        ProviderCapabilities {
            kind: ProviderKind::Soundcloud,
            capabilities: VIDEO_CAPABILITIES,
        },
        ProviderCapabilities {
            kind: ProviderKind::Spotify,
            capabilities: SPOTIFY_CAPABILITIES,
        },
    ]
}
