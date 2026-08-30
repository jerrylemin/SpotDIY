export type ProviderKind = "local" | "youtube" | "soundcloud" | "spotify";
export type TrackId = string & { readonly __brand: "TrackId" };
export type ArtistId = string & { readonly __brand: "ArtistId" };
export type AlbumId = string & { readonly __brand: "AlbumId" };
export type SourceId = string & { readonly __brand: "SourceId" };

export type RouteId =
  | "home"
  | "search"
  | "library"
  | "playlists"
  | "downloads"
  | "settings";

export interface SourceCapabilities {
  search: boolean;
  playback: boolean;
  metadata: boolean;
  artwork: boolean;
  lyrics: boolean;
  downloads: boolean;
  popularity: boolean;
  releaseDate: boolean;
  lyricsMetadata: boolean;
}

export type VersionQualifier =
  | "standard"
  | "studio"
  | "live"
  | "acoustic"
  | "remix"
  | "remaster"
  | "cover"
  | "instrumental"
  | "karaoke"
  | "spedUp"
  | "slowed"
  | "unknown";

export interface VersionInfo {
  qualifiers: VersionQualifier[];
}

export interface Artist {
  id: ArtistId;
  name: string;
  sortName: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface Album {
  id: AlbumId;
  title: string;
  releaseDate: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface LocalFileSource {
  sourceId: SourceId;
  path: string;
  fileSizeBytes: number | null;
  modifiedAt: string | null;
  contentFingerprint: string | null;
  codec: string | null;
  bitrateKbps: number | null;
  sampleRateHz: number | null;
  bitDepth: number | null;
}

export interface TrackSource {
  id: SourceId;
  trackId: TrackId;
  providerKind: ProviderKind;
  providerItemId: string;
  sourceUri: string | null;
  durationMs: number | null;
  version: VersionInfo;
  available: boolean;
  availabilityDetail: string | null;
  capabilities: SourceCapabilities;
  localFile: LocalFileSource | null;
  createdAt: string;
  updatedAt: string;
}

export interface UnifiedTrack {
  id: TrackId;
  title: string;
  normalizedTitle: string;
  artists: Artist[];
  album: Album | null;
  durationMs: number | null;
  version: VersionInfo;
  sources: TrackSource[];
  preferredSourceId: SourceId | null;
  createdAt: string;
  updatedAt: string;
}

export type Theme = "dark" | "light" | "system";
export type StorageMode = "standard" | "portable";

export interface SettingsSnapshot {
  theme: Theme;
  downloadsDirectory: string | null;
  sourcePreferenceOrder: ProviderKind[];
  firstRun: boolean;
  storageMode: StorageMode;
}

export type SettingValue =
  | { key: "theme"; value: Theme }
  | { key: "downloadsDirectory"; value: string | null }
  | { key: "sourcePreferenceOrder"; value: ProviderKind[] };

export interface ProviderStatus {
  kind: ProviderKind;
  label: string;
  configured: boolean;
  available: boolean;
  capabilities: SourceCapabilities;
  detail: string;
}

export interface AppStatus {
  version: string;
  runtime: "tauri" | "browser-preview";
  storageMode: "standard" | "portable";
  firstRun: boolean;
  tracksIndexed: number;
  musicFolders: string[];
  providers: ProviderStatus[];
}

export interface NavItem {
  id: RouteId;
  label: string;
  shortLabel: string;
  icon: "home" | "search" | "library" | "playlist" | "download" | "settings";
}
