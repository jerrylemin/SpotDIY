import type { SpotThemeDefinition } from "../features/theme/theme-schema";

export type ProviderKind = "local" | "youtube" | "soundcloud" | "spotify";
export type TrackId = string & { readonly __brand: "TrackId" };
export type ArtistId = string & { readonly __brand: "ArtistId" };
export type AlbumId = string & { readonly __brand: "AlbumId" };
export type SourceId = string & { readonly __brand: "SourceId" };
export type LibraryFolderId = string & { readonly __brand: "LibraryFolderId" };
export type ArtworkId = string & { readonly __brand: "ArtworkId" };
export type QueueEntryId = string & { readonly __brand: "QueueEntryId" };
export type PlaylistId = string & { readonly __brand: "PlaylistId" };
export type PlaylistItemId = string & { readonly __brand: "PlaylistItemId" };
export type TagId = string & { readonly __brand: "TagId" };
export type QueueSnapshotId = string & { readonly __brand: "QueueSnapshotId" };
export type QueueSnapshotEntryId = string & { readonly __brand: "QueueSnapshotEntryId" };
export type BookmarkId = string & { readonly __brand: "BookmarkId" };
export type AbLoopPresetId = string & { readonly __brand: "AbLoopPresetId" };
export type ListeningSessionId = string & { readonly __brand: "ListeningSessionId" };
export type PlayHistoryId = string & { readonly __brand: "PlayHistoryId" };
export type SmartPlaylistId = string & { readonly __brand: "SmartPlaylistId" };

export type RouteId =
  | "home"
  | "search"
  | "library"
  | "playlists"
  | "downloads"
  | "lyrics"
  | "analytics"
  | "settings"
  | "music-map"
  | "library-galaxy"
  | "theme-studio";

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

export type LyricsSourceKind = "manual" | "sidecar" | "embedded" | "lrclib";
export type LyricsSyncKind = "plain" | "timed" | "instrumental";

export interface LyricsCue {
  startMs: number;
  lines: string[];
}

export interface LyricsAttribution {
  label: string;
  provider: string;
  url: string | null;
}

export interface LyricsDocument {
  trackId: TrackId;
  source: LyricsSourceKind;
  syncKind: LyricsSyncKind;
  plainText: string | null;
  cues: LyricsCue[];
  instrumental: boolean;
  editable: boolean;
  attribution: LyricsAttribution | null;
}

export interface LyricsCandidate {
  providerRecordId: number;
  trackName: string;
  artistName: string;
  albumName: string | null;
  durationMs: number | null;
  instrumental: boolean;
  hasPlain: boolean;
  hasSynced: boolean;
}

export type ManualLyricsMode = "plain" | "lrc";

export interface Bookmark {
  id: BookmarkId;
  trackId: TrackId;
  positionMs: number;
  note: string;
  createdAt: string;
  updatedAt: string;
}

export interface AbLoopPreset {
  id: AbLoopPresetId;
  trackId: TrackId;
  name: string;
  aMs: number;
  bMs: number;
  createdAt: string;
  updatedAt: string;
}

export type HistoryOutcome = "completed" | "skipped" | "stopped" | "interrupted";

export interface HistoryEntry {
  id: PlayHistoryId;
  sessionId: ListeningSessionId | null;
  trackId: TrackId | null;
  sourceId: SourceId | null;
  titleSnapshot: string;
  artists: string[];
  albumSnapshot: string | null;
  providerKind: ProviderKind | null;
  startedAt: string;
  endedAt: string;
  localDate: string;
  localHour: number;
  localWeekday: number;
  listenedMs: number;
  durationMs: number | null;
  outcome: HistoryOutcome;
  qualifiedPlay: boolean;
  createdAt: string;
}

export interface ListeningSession {
  id: ListeningSessionId;
  startedAt: string;
  endedAt: string;
  label: string | null;
  eventCount: number;
  listenedMs: number;
}

export interface AnalyticsOverview {
  listenedMs: number;
  qualifiedPlays: number;
  skips: number;
  uniqueTracks: number;
  uniqueArtists: number;
  sessionCount: number;
}

export interface ListeningHeatmapCell {
  weekday: number;
  hour: number;
  listenedMs: number;
}

export interface TopTrack {
  trackId: TrackId | null;
  title: string;
  artists: string[];
  listenedMs: number;
  qualifiedPlays: number;
  playCount: number;
}

export interface TopArtist {
  name: string;
  listenedMs: number;
  qualifiedPlays: number;
  playCount: number;
}

export interface TasteTimelineMonth {
  month: string;
  listenedMs: number;
  qualifiedPlays: number;
  topTracks: string[];
  topArtists: string[];
}

export interface Paged<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}

export interface ReopenQueueEntry {
  trackId: TrackId;
  requestedSourceId: SourceId | null;
}

export interface ReopenQueueResult {
  entries: ReopenQueueEntry[];
  droppedCount: number;
}

export type SmartSortMode = "title" | "artist" | "dateAdded" | "lastPlayed" | "playCount" | "rating" | "duration" | "audioQuality";
export type SmartSortDirection = "asc" | "desc";
export type SmartField = "artist" | "album" | "genre" | "year" | "dateAdded" | "lastPlayed" | "playCount" | "skipCount" | "rating" | "liked" | "downloaded" | "provider" | "audioQuality" | "duration" | "tag";
export type SmartOperation = "contains" | "equals" | "before" | "after" | "between" | "never" | "greaterThanOrEqual" | "lessThanOrEqual" | "absent" | "true" | "false" | "has" | "lacks" | "is";
export type SmartScalar = string | number;
export type SmartValue = string | number | boolean | { from: SmartScalar; to: SmartScalar };
export type SmartRule =
  | { type: "group"; operator: "and" | "or"; children: SmartRule[] }
  | { type: "predicate"; field: SmartField; operation: SmartOperation; value: SmartValue | null };

export interface SmartPlaylist {
  id: SmartPlaylistId;
  name: string;
  rule: SmartRule;
  sortMode: SmartSortMode;
  sortDirection: SmartSortDirection;
  limitCount: number | null;
  createdAt: string;
  updatedAt: string;
}

export interface SmartPlaylistInput {
  name: string;
  rule: SmartRule;
  sortMode: SmartSortMode;
  sortDirection: SmartSortDirection;
  limitCount: number | null;
}

export type AudioQuality = "lossless" | "lossy" | "unknown";
export interface SmartTrack {
  trackId: TrackId;
  title: string;
  artists: string[];
  album: string | null;
  durationMs: number | null;
  dateAdded: string;
  lastPlayed: string | null;
  playCount: number;
  rating: number | null;
  audioQuality: AudioQuality;
}

export type SmartPlaylistPreview = Paged<SmartTrack>;

export type SmartShufflePool = "library" | "liked" | { smartPlaylist: SmartPlaylistId };
export interface SmartShuffleOptions {
  familiarity: number;
  variety: number;
  freshness: number;
  count: number;
  recentTrackIds?: TrackId[];
}

export interface ListeningModeState {
  privateSession: boolean;
  temporary: boolean;
}

export type ListeningModeReason = "privateEnabled" | "privateDisabled" | "temporaryEntered" | "temporaryExited" | "privateLockedByTemporary";
export interface ListeningModeChange {
  state: ListeningModeState;
  reason: ListeningModeReason;
}

export type Theme = "dark" | "light" | "system" | "custom";
export type LayoutProfile = "comfortable" | "compact" | "dense";
export type StorageMode = "standard" | "portable";

export interface SettingsSnapshot {
  theme: Theme;
  layoutProfile: LayoutProfile;
  customTheme: SpotThemeDefinition | null;
  downloadsDirectory: string | null;
  sourcePreferenceOrder: ProviderKind[];
  firstRun: boolean;
  storageMode: StorageMode;
  windowsIntegration: WindowsIntegrationSettings;
  globalShortcuts: GlobalShortcutBinding[];
  outputProfiles: OutputProfile[];
}

export interface StorageStatus {
  mode: StorageMode;
  dataRoot: string;
  databasePath: string;
  cacheRoot: string;
  portableMarkerPresent: boolean;
  restartRequired: boolean;
  pendingImport: boolean;
  lastRollbackPath: string | null;
}

export interface StorageModeSwitchResult {
  mode: StorageMode;
  dataRoot: string;
  databasePath: string;
  cacheRoot: string;
  restartRequired: boolean;
}

export type SpotDiyArchiveEntryKind = "database" | "localAudio" | "artwork" | "sidecarLyrics";

export interface MissingFileReference {
  kind: string;
  trackId: TrackId | null;
  sourceId: SourceId | null;
  path: string;
}

export interface MissingFileReport {
  totalLocalReferences: number;
  availableLocalReferences: number;
  missingLocalReferences: number;
  completedDownloadReferences: number;
  missingDownloadOutputs: number;
  firstMissing: MissingFileReference[];
}

export interface ImportPreview {
  importId: string;
  archiveVersion: number;
  appVersion: string;
  databaseSchemaVersion: number;
  sourceStorageMode: StorageMode;
  entryCount: number;
  includedAudioCount: number;
  includedArtworkCount: number;
  includedSidecarLyricsCount: number;
  missing: MissingFileReport;
  checksumValid: boolean;
  restoredAudioPlannedCount: number;
}

export interface SpotDiyExportOptions {
  includeLocalAudio: boolean;
  includeArtworkCache: boolean;
  includeSidecarLyrics: boolean;
}

export interface ImportCommitResult {
  importId: string;
  restartRequired: boolean;
  preview: ImportPreview;
}

export interface WindowsIntegrationSettings {
  smtcEnabled: boolean;
  globalShortcutsEnabled: boolean;
}

export type GlobalShortcutAction =
  | "playPause"
  | "next"
  | "previous"
  | "volumeUp"
  | "volumeDown"
  | "showHideMain"
  | "toggleMiniOverlay"
  | "toggleLyricsOverlay"
  | "toggleGamingOverlay";

export interface GlobalShortcutBinding {
  action: GlobalShortcutAction;
  accelerator: string;
  enabled: boolean;
}

export type OverlayKind = "mini" | "edge" | "lyrics" | "gaming";
export type OverlayStatus = "closed" | "open" | "error";
export type TrayStatus = "ready" | "failed";
export type SmtcStatus = "ready" | "disabled" | "unsupported" | "failed";
export type ShortcutRegistrationStatus = "disabled" | "registered" | "conflict" | "invalid" | "failed";

export interface ShortcutStatus {
  action: GlobalShortcutAction;
  accelerator: string;
  enabled: boolean;
  status: ShortcutRegistrationStatus;
  detail: string | null;
}

export interface OverlaySnapshot {
  kind: OverlayKind;
  status: OverlayStatus;
  detail: string | null;
}

export interface OutputProfile {
  id: string;
  name: string;
  audioDeviceName: string;
  volumePercent: number;
  muted: boolean;
}

export interface WindowsIntegrationSnapshot {
  revision: number;
  platformSupported: boolean;
  trayStatus: TrayStatus;
  trayDetail: string | null;
  smtcStatus: SmtcStatus;
  smtcDetail: string | null;
  globalShortcutsEnabled: boolean;
  shortcutStatuses: ShortcutStatus[];
  overlays: OverlaySnapshot[];
  gamingClickThrough: boolean;
  outputProfiles: OutputProfile[];
}

export type GamingClickThroughErrorCode = "rescueUnavailable" | "nativeCallFailed" | "overlayUnavailable";

export interface GamingClickThroughError {
  code: GamingClickThroughErrorCode;
  detail: string;
}

export type OutputProfileApplyErrorCode = "invalidProfile" | "deviceUnavailable" | "applyFailed";

export interface OutputProfileApplyError {
  code: OutputProfileApplyErrorCode;
  detail: string;
  rollbackSucceeded: boolean;
}

export type SettingValue =
  | { key: "theme"; value: Theme }
  | { key: "layoutProfile"; value: LayoutProfile }
  | { key: "customTheme"; value: SpotThemeDefinition | null }
  | { key: "downloadsDirectory"; value: string | null }
  | { key: "sourcePreferenceOrder"; value: ProviderKind[] }
  | { key: "windowsIntegration"; value: WindowsIntegrationSettings }
  | { key: "globalShortcuts"; value: GlobalShortcutBinding[] }
  | { key: "outputProfiles"; value: OutputProfile[] };

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

export type VisualAudioQuality = "lossless" | "lossy" | "unknown";

export interface VisualTrackPoint {
  trackId: TrackId;
  title: string;
  primaryArtist: string;
  artists: string[];
  artistIds: string[];
  album: string | null;
  albumId: string | null;
  genres: string[];
  year: number | null;
  dateAdded: string;
  lastPlayed: string | null;
  liked: boolean;
  rating: number | null;
  qualifiedPlays: number;
  listenedMs: number;
  audioQuality: VisualAudioQuality;
  providerCount: number;
  artworkPath: string | null;
  canPlayback: boolean;
  canPreview: boolean;
  canRevealLocal: boolean;
}

export interface VisualDatasetRequest {
  query: string | null;
  genre: string | null;
  artist: string | null;
  likedOnly: boolean;
  limit: number;
}

export interface VisualLibraryDataset {
  totalTracks: number;
  returnedTracks: number;
  truncated: boolean;
  tracks: VisualTrackPoint[];
}

export type PreviewPhase = "idle" | "loading" | "playing" | "failed";

export interface PreviewState {
  phase: PreviewPhase;
  trackId: TrackId | null;
  startedAtMs: number | null;
  error: string | null;
}

export type PlaylistKind = "normal" | "inbox" | "branch";
export type BranchStatus = "open" | "merged";

export interface PlaylistItem {
  id: PlaylistItemId;
  playlistId: PlaylistId;
  trackId: TrackId;
  requestedSourceId: SourceId | null;
  position: number;
  originBaseItemId: PlaylistItemId | null;
  addedAt: string;
  updatedAt: string;
}

export interface Playlist {
  id: PlaylistId;
  name: string;
  kind: PlaylistKind;
  parentPlaylistId: PlaylistId | null;
  baseParentRevision: number | null;
  branchStatus: BranchStatus | null;
  revision: number;
  createdAt: string;
  updatedAt: string;
  items: PlaylistItem[];
}

export type BranchChange =
  | { type: "add"; branchItemId: PlaylistItemId }
  | { type: "remove"; baseItemId: PlaylistItemId }
  | { type: "move"; baseItemId: PlaylistItemId; targetPosition: number };

export interface BranchMergeResult {
  parent: Playlist;
  branch: Playlist;
}

export interface PlaylistMembership {
  playlistId: PlaylistId;
  name: string;
  kind: PlaylistKind;
}

export interface Tag {
  id: TagId;
  name: string;
  normalizedName: string;
  createdAt: string;
  updatedAt: string;
}

export interface TrackCollectionState {
  trackId: TrackId;
  liked: boolean;
  rating: number | null;
  tags: Tag[];
  playlistMemberships: PlaylistMembership[];
  inInbox: boolean;
}

export interface TrackInspectorCollectionState {
  liked: boolean;
  rating: number | null;
  tags: Tag[];
  playlistMemberships: PlaylistMembership[];
  inInbox: boolean;
}

export interface TrackInspectorQuality {
  container: string | null;
  codec: string | null;
  bitrateKbps: number | null;
  sampleRateHz: number | null;
  bitDepth: number | null;
}

export interface TrackInspectorCapabilities {
  search: boolean;
  metadata: boolean;
  artwork: boolean;
  playback: boolean;
  lyrics: boolean;
  downloads: boolean;
}

export interface TrackInspectorSource {
  sourceId: SourceId;
  provider: ProviderKind;
  providerItemId: string;
  available: boolean;
  availabilityDetail: string | null;
  capabilities: TrackInspectorCapabilities;
  durationMs: number | null;
  versionQualifiers: VersionQualifier[];
  quality: TrackInspectorQuality;
  canonicalUrl: string | null;
}

export interface TrackInspector {
  trackId: TrackId;
  title: string;
  artists: string[];
  album: string | null;
  durationMs: number | null;
  versionQualifiers: VersionQualifier[];
  preferredSourceId: SourceId | null;
  collectionState: TrackInspectorCollectionState;
  sources: TrackInspectorSource[];
}

export type PlaylistErrorCode =
  | "invalidName"
  | "invalidTagName"
  | "playlistNotFound"
  | "playlistItemNotFound"
  | "trackNotFound"
  | "sourceNotFound"
  | "sourceMismatch"
  | "systemPlaylist"
  | "branchExists"
  | "cannotBranch"
  | "branchNotFound"
  | "branchNotOpen"
  | "branchAlreadyMerged"
  | "branchConflict"
  | "invalidBranchChange"
  | "emptySelection"
  | "invalidPosition"
  | "tagNotFound"
  | "tagExists"
  | "invalidRating"
  | "collectionRequestTooLarge"
  | "snapshotNotFound"
  | "database";

export interface PlaylistErrorDto {
  code: PlaylistErrorCode;
  detail: string;
}

export type QueueSection = "up_next" | "later" | "autoplay";

export interface QueueWorkspaceEntry {
  id: QueueEntryId;
  trackId: TrackId;
  requestedSourceId: SourceId | null;
  section: QueueSection;
  position: number;
  pinned: boolean;
  title: string | null;
  artists: string[];
  album: string | null;
}

export interface QueueWorkspace {
  revision: number;
  current: QueueWorkspaceEntry | null;
  upNext: QueueWorkspaceEntry[];
  later: QueueWorkspaceEntry[];
  autoplay: QueueWorkspaceEntry[];
  currentPositionMs: number;
  repeatMode: RepeatMode;
  shuffleEnabled: boolean;
}

export interface QueueSnapshotEntry {
  id: QueueSnapshotEntryId;
  snapshotId: QueueSnapshotId;
  trackId: TrackId;
  requestedSourceId: SourceId | null;
  section: QueueSection;
  position: number;
  pinned: boolean;
  traversalPosition: number;
}

export interface QueueSnapshotSummary {
  id: QueueSnapshotId;
  name: string;
  currentTrackId: TrackId | null;
  currentSourceId: SourceId | null;
  currentPositionMs: number;
  repeatMode: RepeatMode;
  shuffleEnabled: boolean;
  entryCount: number;
  createdAt: string;
}

export interface QueueSnapshot extends QueueSnapshotSummary {
  currentSnapshotEntryId: QueueSnapshotEntryId | null;
  historyOrder: QueueSnapshotEntryId[];
  traversalOrder: QueueSnapshotEntryId[];
  entries: QueueSnapshotEntry[];
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
  | "persistenceFailed"
  | "queueEntryNotFound"
  | "queueEntryImmutable"
  | "invalidQueuePosition"
  | "snapshotNotFound"
  | "shuttingDown"
  | "invalidAbLoop"
  | "abLoopPresetNotFound"
  | "abLoopPresetTrackMismatch";

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

export interface AbLoopState {
  aMs: number | null;
  bMs: number | null;
  active: boolean;
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
  abLoop: AbLoopState;
}

export interface NavItem {
  id: RouteId;
  label: string;
  shortLabel: string;
  icon: "home" | "search" | "library" | "playlist" | "download" | "analytics" | "lyrics" | "settings" | "spark" | "expand" | "theme";
}
