export type ProviderKind = "local" | "youtube" | "soundcloud" | "spotify";
export type TrackId = string & { readonly __brand: "TrackId" };
export type ArtistId = string & { readonly __brand: "ArtistId" };
export type AlbumId = string & { readonly __brand: "AlbumId" };
export type SourceId = string & { readonly __brand: "SourceId" };
export type LibraryFolderId = string & { readonly __brand: "LibraryFolderId" };
export type ArtworkId = string & { readonly __brand: "ArtworkId" };
export type QueueEntryId = string & { readonly __brand: "QueueEntryId" };

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
  | "nightcore"
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
  runtimeStatus: ProviderRuntimeStatus;
  capabilities: SourceCapabilities;
  detail: string;
}

export type ProviderRuntimeStatus = "unknown" | "ready" | "missing" | "unsupported" | "broken" | "disabled";

export interface AppStatus {
  version: string;
  runtime: "tauri" | "browser-preview";
  storageMode: "standard" | "portable";
  firstRun: boolean;
  tracksIndexed: number;
  musicFolders: string[];
  providers: ProviderStatus[];
  mediaTools: MediaToolsSnapshot;
}

export type DownloadTaskId = string & { readonly __brand: "DownloadTaskId" };
export type DownloadMode = "audio" | "video";
export type DownloadState = "queued" | "resolving" | "downloading" | "postprocessing" | "completed" | "failed" | "cancelled";
export type SourceQualityProvenance = "providerEncoded" | "unknown";
export type DownloadErrorCode =
  | "invalidRequest"
  | "unsupportedProvider"
  | "invalidProviderUrl"
  | "downloadDirectoryNotConfigured"
  | "downloadDirectoryInvalid"
  | "sourceNotFound"
  | "sourceTrackMismatch"
  | "toolMissing"
  | "toolBroken"
  | "processFailed"
  | "outputInvalid"
  | "finalizationFailed"
  | "cancelled"
  | "persistenceFailed"
  | "shuttingDown"
  | "unknown";

export interface DownloadToolStatus {
  status: ProviderRuntimeStatus;
  version: string | null;
  detail: string | null;
}

export interface MediaToolsSnapshot {
  ytDlp: DownloadToolStatus;
  ffmpeg: DownloadToolStatus;
}

export interface DownloadTask {
  id: DownloadTaskId;
  providerKind: ProviderKind;
  providerItemId: string;
  canonicalUrl: string;
  targetTrackId: TrackId | null;
  targetSourceId: SourceId | null;
  title: string;
  artists: string[];
  artworkUrl: string | null;
  mode: DownloadMode;
  state: DownloadState;
  destinationDirectory: string;
  outputPath: string | null;
  outputExtension: string | null;
  outputCodec: string | null;
  sourceQualityProvenance: SourceQualityProvenance;
  transcoded: boolean;
  expectedBytes: number | null;
  downloadedBytes: number;
  progressPermille: number;
  speedBytesPerSecond: number | null;
  etaSeconds: number | null;
  retryCount: number;
  errorCode: DownloadErrorCode | null;
  errorDetail: string | null;
  createdAt: string;
  updatedAt: string;
  startedAt: string | null;
  completedAt: string | null;
  outputMissing: boolean;
}

export interface DownloadSnapshot {
  revision: number;
  tasks: DownloadTask[];
  maxConcurrent: number;
  downloadsDirectory: string | null;
  tools: MediaToolsSnapshot;
}

export type SearchId = string & { readonly __brand: "SearchId" };
export type SearchLens = "all" | "tracks" | "artists" | "albums" | "playlists" | "local" | "youtube" | "soundcloud" | "spotify";
export type SearchEntityKind = "track" | "artist" | "album" | "playlist";
export type SearchSortField = "relevance" | "popularity" | "newest" | "oldest" | "duration" | "dateAdded" | "downloaded" | "audioQuality";
export type SearchSortDirection = "ascending" | "descending";

export interface SearchRequest {
  query: string;
  lens: SearchLens;
  sortField: SearchSortField;
  sortDirection: SearchSortDirection;
  limit: number;
}

export type EngagementKind = "views" | "plays";
export type PartialDatePrecision = "year" | "month" | "day";

export interface PartialDate {
  value: string;
  precision: PartialDatePrecision;
}

export interface SearchResult {
  provider: ProviderKind;
  entityKind: SearchEntityKind;
  providerItemId: string;
  canonicalUrl: string | null;
  title: string;
  artists: string[];
  album: string | null;
  durationMs: number | null;
  artworkUrl: string | null;
  publishedAt: PartialDate | null;
  engagementCount: number | null;
  engagementKind: EngagementKind | null;
  explicit: boolean | null;
  localTrackId: TrackId | null;
  localSourceId: SourceId | null;
  originalRank: number;
}

export type FusionOverrideDecision = "merge" | "split";
export type FusionDecision = "already_unified" | "forced_merge" | "auto_merge" | "forced_split" | "rejected" | "excluded";
export type FusionReason =
  | "matched"
  | "provider_excluded"
  | "entity_unsupported"
  | "already_unified"
  | "forced_merge"
  | "forced_split"
  | "same_provider_requires_manual_merge"
  | "title_below_minimum"
  | "artist_below_minimum"
  | "duration_mismatch"
  | "version_mismatch"
  | "below_threshold"
  | "identity_conflict"
  | "ambiguous"
  | "invalid_candidate";

export interface FusionEvaluation {
  targetTrackId: TrackId;
  decision: FusionDecision;
  scoreBps: number;
  thresholdBps: number;
  titleScoreBps: number;
  artistScoreBps: number;
  durationScoreBps: number;
  durationDeltaMs: number | null;
  candidateQualifiers: VersionQualifier[];
  targetQualifiers: VersionQualifier[];
  reason: FusionReason;
}

export interface FusionOverride {
  providerKind: ProviderKind;
  providerItemId: string;
  targetTrackId: TrackId;
  decision: FusionOverrideDecision;
  createdAt: string;
  updatedAt: string;
}

export interface FusionOverrideRequest {
  providerKind: ProviderKind;
  providerItemId: string;
  targetTrackId: TrackId;
  decision: FusionOverrideDecision;
}

export interface ClearFusionOverrideRequest {
  providerKind: ProviderKind;
  providerItemId: string;
  targetTrackId: TrackId;
}

export type ProviderSearchState = "idle" | "loading" | "ready" | "failed" | "cancelled";
export type ProviderSearchErrorCode = "unavailable" | "timeout" | "cancelled" | "rate_limited" | "quota_exceeded" | "disabled" | "invalid_response" | "failed";

export interface ProviderSearchError {
  code: ProviderSearchErrorCode;
  detail: string | null;
  retryAfterSeconds: number | null;
}

export interface ProviderSearchSection {
  provider: ProviderKind;
  state: ProviderSearchState;
  results: SearchResult[];
  error: ProviderSearchError | null;
}

export interface ProviderSearchEvent {
  searchId: SearchId;
  section: ProviderSearchSection;
}

export interface SearchStarted {
  searchId: SearchId;
}

export interface SearchCompleted {
  searchId: SearchId;
}

export type SpotifyAuthState = "disabled" | "setup_required" | "connected" | "unavailable";

export interface SpotifySetupStatus {
  enabled: boolean;
  configured: boolean;
  available: boolean;
  state: SpotifyAuthState;
  market: string | null;
  detail: string | null;
}

export interface SpotifyAuthorizationRequest {
  authorizationUrl: string;
  redirectUri: string;
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

export type PlaybackPhase =
  | "idle"
  | "loading"
  | "playing"
  | "paused"
  | "seeking"
  | "ended"
  | "recovering"
  | "failed"
  | "shuttingDown";

export type RepeatMode = "off" | "one" | "all";

export type PlaybackErrorCode =
  | "toolMissing"
  | "toolBroken"
  | "spawnFailed"
  | "ipcConnectTimeout"
  | "ipcDisconnected"
  | "protocolError"
  | "requestTimeout"
  | "trackNotFound"
  | "sourceNotFound"
  | "sourceMismatch"
  | "sourceUnavailable"
  | "sourceNotPlayable"
  | "localFileMissing"
  | "loadFailed"
  | "seekFailed"
  | "deviceUnavailable"
  | "queueEmpty"
  | "recoveryRetrying"
  | "recoveryExhausted"
  | "shuttingDown";

export interface PlaybackErrorDto {
  code: PlaybackErrorCode;
  summary: string;
  retryable: boolean;
}

export interface PlaybackBackendHealth {
  ready: boolean;
  connected: boolean;
  detail: string | null;
  recoveryAction: string | null;
}

export interface AudioDevice {
  name: string;
  description: string;
  selected: boolean;
}

export type PlaybackAudioDevice = AudioDevice;

export interface PlaybackSourceOption {
  sourceId: SourceId;
  provider: ProviderKind;
  label: string;
  available: boolean;
  availabilityDetail: string | null;
}

export type SourceResolutionReason =
  | "preferred_source"
  | "playable"
  | "unavailable"
  | "local_file_missing"
  | "source_does_not_support_playback"
  | "provider_playback_not_implemented"
  | "metadata_only";

export interface SourceResolutionCandidate {
  sourceId: SourceId;
  provider: ProviderKind;
  playable: boolean;
  reason: SourceResolutionReason;
  preferenceRank: number;
  detail: string | null;
}

export interface SourceResolution {
  selectedSourceId: SourceId | null;
  candidates: SourceResolutionCandidate[];
}

export interface TrackPlaybackRequest {
  trackId: TrackId;
  sourceId: SourceId | null;
}

export interface PlaybackSnapshot {
  revision: number;
  phase: PlaybackPhase;
  currentQueueEntryId: QueueEntryId | null;
  currentTrackId: TrackId | null;
  currentSourceId: SourceId | null;
  title: string | null;
  artists: string[];
  album: string | null;
  artworkPath: string | null;
  sources: PlaybackSourceOption[];
  positionMs: number;
  durationMs: number | null;
  volumePercent: number;
  muted: boolean;
  repeatMode: RepeatMode;
  shuffleEnabled: boolean;
  queueLength: number;
  queueIndex: number | null;
  selectedAudioDevice: string;
  backendHealth: PlaybackBackendHealth;
  recovering: boolean;
  error: PlaybackErrorDto | null;
}

export interface NavItem {
  id: RouteId;
  label: string;
  shortLabel: string;
  icon: "home" | "search" | "library" | "playlist" | "download" | "settings";
}
