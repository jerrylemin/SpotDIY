export type ProviderKind = "local" | "youtube" | "soundcloud" | "spotify";
export type TrackId = string & { readonly __brand: "TrackId" };
export type ArtistId = string & { readonly __brand: "ArtistId" };
export type AlbumId = string & { readonly __brand: "AlbumId" };
export type SourceId = string & { readonly __brand: "SourceId" };
export type LibraryFolderId = string & { readonly __brand: "LibraryFolderId" };
export type ArtworkId = string & { readonly __brand: "ArtworkId" };

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
  libraryFolderId: LibraryFolderId | null;
  normalizedPathKey: string | null;
  fileSizeBytes: number | null;
  modifiedAt: string | null;
  contentFingerprint: string | null;
  container: string | null;
  codec: string | null;
  bitrateKbps: number | null;
  sampleRateHz: number | null;
  bitDepth: number | null;
  indexStatus: "pending" | "indexed" | "missing" | "error";
  statusDetail: string | null;
  lastSeenAt: string | null;
  lastIndexedAt: string | null;
  lastSeenGeneration: number;
  artworkCacheKey: string | null;
  artworkMimeType: string | null;
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

export type LibraryFolderStatus = "idle" | "queued" | "scanning" | "complete" | "failed";
export type LocalFileIndexStatus = "pending" | "indexed" | "missing" | "error";
export type LibrarySort = "title" | "artist" | "dateAdded" | "dateModified";

export interface LibraryFolder {
  id: LibraryFolderId;
  path: string;
  normalizedPathKey: string;
  enabled: boolean;
  status: LibraryFolderStatus;
  scanGeneration: number;
  lastScanStartedAt: string | null;
  lastScanFinishedAt: string | null;
  lastScanError: string | null;
  fileCount: number;
  indexedTrackCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface ScanSummary {
  directoriesVisited: number;
  candidates: number;
  unchangedSkipped: number;
  newFiles: number;
  changedFiles: number;
  renamedFiles: number;
  missingFiles: number;
  unsupportedSkipped: number;
  metadataFailures: number;
  artworkFailures: number;
  databaseFailures: number;
  elapsedMs: number;
}

export interface ScanProgress {
  folderId: LibraryFolderId;
  status: LibraryFolderStatus;
  currentFile: string | null;
  processed: number;
  candidates: number;
  summary: ScanSummary | null;
  startedAt: string | null;
  finishedAt: string | null;
  error: string | null;
}

export interface LibraryStatus {
  folders: LibraryFolder[];
  indexedTrackCount: number;
  availableTrackCount: number;
  isScanning: boolean;
}

export interface LibraryTrack {
  trackId: TrackId;
  sourceId: SourceId;
  folderId: LibraryFolderId;
  title: string;
  artists: string[];
  album: string | null;
  durationMs: number | null;
  path: string;
  available: boolean;
  availabilityDetail: string | null;
  indexStatus: LocalFileIndexStatus;
  statusDetail: string | null;
  fileSizeBytes: number | null;
  modifiedAt: string | null;
  codec: string | null;
  container: string | null;
  bitrateKbps: number | null;
  sampleRateHz: number | null;
  bitDepth: number | null;
  contentFingerprint: string | null;
  artworkCacheKey: string | null;
  artworkMimeType: string | null;
  artworkPath: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface LibraryPageRequest {
  page: number;
  pageSize: number;
  sort: LibrarySort;
  descending: boolean;
  folderId: LibraryFolderId | null;
}

export interface LibraryPage {
  items: LibraryTrack[];
  page: number;
  pageSize: number;
  total: number;
  hasNext: boolean;
  sort: LibrarySort;
  descending: boolean;
}

export interface NavItem {
  id: RouteId;
  label: string;
  shortLabel: string;
  icon: "home" | "search" | "library" | "playlist" | "download" | "settings";
}
