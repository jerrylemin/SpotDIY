use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Local,
    Youtube,
    Soundcloud,
    Spotify,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCapabilities {
    pub search: bool,
    pub playback: bool,
    pub metadata: bool,
    pub artwork: bool,
    pub lyrics: bool,
    pub downloads: bool,
}
