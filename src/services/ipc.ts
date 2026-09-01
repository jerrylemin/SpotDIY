import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { z } from "zod";

import type {
  AppStatus,
  AbLoopPreset,
  AbLoopPresetId,
  AbLoopState,
  Bookmark,
  BookmarkId,
  DownloadMode,
  DownloadSnapshot,
  DownloadTask,
  DownloadTaskId,
  MediaToolsSnapshot,
  LibraryFolder,
  LibraryFolderId,
  LibraryPage,
  LibraryPageRequest,
  LibraryTrack,
  LibraryStatus,
  LyricsCandidate,
  LyricsDocument,
  LyricsSourceKind,
  LyricsSyncKind,
  ManualLyricsMode,
  PlaybackAudioDevice,
  PlaybackBackendHealth,
  PlaybackErrorCode,
  PlaybackErrorDto,
  PlaybackPhase,
  PlaybackSnapshot,
  PlaybackSourceOption,
  ProviderKind,
  ProviderSearchEvent,
  ProviderSearchSection,
  QueueEntryId,
  RepeatMode,
  SearchCompleted,
  SearchId,
  SearchLens,
  SearchRequest,
  SearchResult,
  SearchStarted,
  ScanProgress,
  SettingValue,
  SettingsSnapshot,
  SpotifyAuthorizationRequest,
  SpotifySetupStatus,
  ClearFusionOverrideRequest,
  FusionEvaluation,
  FusionOverride,
  FusionOverrideRequest,
  SourceResolution,
  TrackPlaybackRequest,
  TrackId,
  SourceId,
  Playlist,
  PlaylistId,
  PlaylistItem,
  PlaylistItemId,
  BranchChange,
  BranchMergeResult,
  Tag,
  TagId,
  TrackCollectionState,
  TrackInspector,
  QueueSection,
  QueueSnapshot,
  QueueSnapshotEntryId,
  QueueSnapshotId,
  QueueSnapshotSummary,
  QueueWorkspace,
} from "../types/domain";
import { spotThemeDefinitionSchema } from "../features/theme/theme-schema";

const providerKindSchema = z.enum(["local", "youtube", "soundcloud", "spotify"]);
const sourceCapabilitiesSchema = z.object({
  search: z.boolean(),
  playback: z.boolean(),
  metadata: z.boolean(),
  artwork: z.boolean(),
  lyrics: z.boolean(),
  downloads: z.boolean(),
  popularity: z.boolean(),
  releaseDate: z.boolean(),
  lyricsMetadata: z.boolean(),
}).strict();

const appStatusSchema = z.object({
  version: z.string(),
  runtime: z.enum(["tauri", "browser-preview"]),
  storageMode: z.enum(["standard", "portable"]),
  firstRun: z.boolean(),
  tracksIndexed: z.number().int().nonnegative(),
  musicFolders: z.array(z.string()),
  providers: z.array(
    z.object({
      kind: providerKindSchema,
      label: z.string(),
      configured: z.boolean(),
      available: z.boolean(),
      runtimeStatus: z.enum(["unknown", "ready", "missing", "unsupported", "broken", "disabled"]),
      capabilities: sourceCapabilitiesSchema,
      detail: z.string(),
    }).strict(),
  ),
  mediaTools: z.object({
    ytDlp: z.object({
      status: z.enum(["unknown", "ready", "missing", "unsupported", "broken", "disabled"]),
      version: z.string().nullable(),
      detail: z.string().nullable(),
    }).strict(),
    ffmpeg: z.object({
      status: z.enum(["unknown", "ready", "missing", "unsupported", "broken", "disabled"]),
      version: z.string().nullable(),
      detail: z.string().nullable(),
    }).strict(),
  }).strict(),
}).strict();

const searchIdSchema = z.string().min(1).transform((value) => value as SearchId);
const searchLensSchema = z.enum(["all", "tracks", "artists", "albums", "playlists", "local", "youtube", "soundcloud", "spotify"]);
const searchEntityKindSchema = z.enum(["track", "artist", "album", "playlist"]);
const searchSortFieldSchema = z.enum(["relevance", "popularity", "newest", "oldest", "duration", "dateAdded", "downloaded", "audioQuality"]);
const searchSortDirectionSchema = z.enum(["ascending", "descending"]);
const searchRequestSchema = z.object({
  query: z.string().trim().min(1).max(256),
  lens: searchLensSchema,
  sortField: searchSortFieldSchema,
  sortDirection: searchSortDirectionSchema,
  limit: z.number().int().min(1).max(50),
}).strict();
const partialDateSchema = z.object({
  value: z.string().min(4),
  precision: z.enum(["year", "month", "day"]),
}).strict();
const searchResultSchema = z.object({
  provider: providerKindSchema,
  entityKind: searchEntityKindSchema,
  providerItemId: z.string().min(1),
  canonicalUrl: z.string().url().nullable(),
  title: z.string(),
  artists: z.array(z.string()),
  album: z.string().nullable(),
  durationMs: z.number().int().nonnegative().nullable(),
  artworkUrl: z.string().url().nullable(),
  publishedAt: partialDateSchema.nullable(),
  engagementCount: z.number().int().nonnegative().nullable(),
  engagementKind: z.enum(["views", "plays"]).nullable(),
  explicit: z.boolean().nullable(),
  localTrackId: z.string().min(1).transform((value) => value as TrackId).nullable(),
  localSourceId: z.string().min(1).transform((value) => value as SourceId).nullable(),
  originalRank: z.number().int().nonnegative(),
}).strict();
const providerSearchStateSchema = z.enum(["idle", "loading", "ready", "failed", "cancelled"]);
const providerSearchErrorCodeSchema = z.enum(["unavailable", "timeout", "cancelled", "rate_limited", "quota_exceeded", "disabled", "invalid_response", "failed"]);
const providerSearchErrorSchema = z.object({
  code: providerSearchErrorCodeSchema,
  detail: z.string().nullable(),
  retryAfterSeconds: z.number().int().nonnegative().nullable(),
}).strict();
const providerSearchSectionSchema = z.object({
  provider: providerKindSchema,
  state: providerSearchStateSchema,
  results: z.array(searchResultSchema),
  error: providerSearchErrorSchema.nullable(),
}).strict();
const providerSearchEventSchema = z.object({
  searchId: searchIdSchema,
  section: providerSearchSectionSchema,
}).strict();
const searchStartedSchema = z.object({ searchId: searchIdSchema }).strict();
const searchCompletedSchema = z.object({ searchId: searchIdSchema }).strict();
const spotifySetupStatusSchema = z.object({
  enabled: z.boolean(),
  configured: z.boolean(),
  available: z.boolean(),
  state: z.enum(["disabled", "setup_required", "connected", "unavailable"]),
  market: z.string().regex(/^[A-Z]{2}$/).nullable(),
  detail: z.string().nullable(),
}).strict();
const spotifyAuthorizationRequestSchema = z.object({
  authorizationUrl: z.string().url(),
  redirectUri: z.string().url(),
}).strict();

const themeSchema = z.enum(["dark", "light", "system", "custom"]);
const layoutProfileSchema = z.enum(["comfortable", "compact", "dense"]);
const sourcePreferenceOrderSchema = z
  .array(providerKindSchema)
  .length(4)
  .refine((value) => new Set(value).size === value.length, "Provider preference order cannot contain duplicates.");
const settingsSnapshotSchema = z.object({
  theme: themeSchema,
  layoutProfile: layoutProfileSchema,
  customTheme: spotThemeDefinitionSchema.nullable(),
  downloadsDirectory: z.string().nullable(),
  sourcePreferenceOrder: sourcePreferenceOrderSchema,
  firstRun: z.boolean(),
  storageMode: z.enum(["standard", "portable"]),
});
const settingValueSchema = z.discriminatedUnion("key", [
  z.object({ key: z.literal("theme"), value: themeSchema }),
  z.object({ key: z.literal("layoutProfile"), value: layoutProfileSchema }),
  z.object({ key: z.literal("customTheme"), value: spotThemeDefinitionSchema.nullable() }),
  z.object({ key: z.literal("downloadsDirectory"), value: z.string().nullable() }),
  z.object({ key: z.literal("sourcePreferenceOrder"), value: sourcePreferenceOrderSchema }),
]);

const downloadTaskIdSchema = z.string().min(1).transform((value) => value as DownloadTaskId);
const downloadModeSchema = z.enum(["audio", "video"]);
const downloadStateSchema = z.enum(["queued", "resolving", "downloading", "postprocessing", "completed", "failed", "cancelled"]);
const sourceQualityProvenanceSchema = z.enum(["providerEncoded", "unknown"]);
const downloadErrorCodeSchema = z.enum([
  "invalidRequest",
  "unsupportedProvider",
  "invalidProviderUrl",
  "downloadDirectoryNotConfigured",
  "downloadDirectoryInvalid",
  "sourceNotFound",
  "sourceTrackMismatch",
  "toolMissing",
  "toolBroken",
  "processFailed",
  "outputInvalid",
  "finalizationFailed",
  "cancelled",
  "persistenceFailed",
  "shuttingDown",
  "unknown",
]);
const downloadToolStatusSchema = z.object({
  status: z.enum(["unknown", "ready", "missing", "unsupported", "broken", "disabled"]),
  version: z.string().nullable(),
  detail: z.string().nullable(),
}).strict();
const mediaToolsSnapshotSchema = z.object({
  ytDlp: downloadToolStatusSchema,
  ffmpeg: downloadToolStatusSchema,
}).strict();
const downloadTaskSchema = z.object({
  id: downloadTaskIdSchema,
  providerKind: providerKindSchema,
  providerItemId: z.string().min(1),
  canonicalUrl: z.string().url(),
  targetTrackId: z.string().min(1).transform((value) => value as TrackId).nullable(),
  targetSourceId: z.string().min(1).transform((value) => value as SourceId).nullable(),
  title: z.string(),
  artists: z.array(z.string()),
  artworkUrl: z.string().url().nullable(),
  mode: downloadModeSchema,
  state: downloadStateSchema,
  destinationDirectory: z.string().min(1),
  outputPath: z.string().min(1).nullable(),
  outputExtension: z.string().min(1).nullable(),
  outputCodec: z.string().min(1).nullable(),
  sourceQualityProvenance: sourceQualityProvenanceSchema,
  transcoded: z.boolean(),
  expectedBytes: z.number().int().nonnegative().nullable(),
  downloadedBytes: z.number().int().nonnegative(),
  progressPermille: z.number().int().min(0).max(1000),
  speedBytesPerSecond: z.number().int().nonnegative().nullable(),
  etaSeconds: z.number().int().nonnegative().nullable(),
  retryCount: z.number().int().nonnegative(),
  errorCode: downloadErrorCodeSchema.nullable(),
  errorDetail: z.string().nullable(),
  createdAt: z.string(),
  updatedAt: z.string(),
  startedAt: z.string().nullable(),
  completedAt: z.string().nullable(),
  outputMissing: z.boolean(),
}).strict();
const downloadSnapshotSchema = z.object({
  revision: z.number().int().nonnegative(),
  tasks: z.array(downloadTaskSchema),
  maxConcurrent: z.number().int().min(1).max(4),
  downloadsDirectory: z.string().min(1).nullable(),
  tools: mediaToolsSnapshotSchema,
}).strict();

const libraryFolderSchema = z.object({
  id: z.string().transform((value) => value as LibraryFolderId),
  path: z.string(),
  normalizedPathKey: z.string(),
  enabled: z.boolean(),
  status: z.enum(["idle", "queued", "scanning", "complete", "failed"]),
  scanGeneration: z.number().int().nonnegative(),
  lastScanStartedAt: z.string().nullable(),
  lastScanFinishedAt: z.string().nullable(),
  lastScanError: z.string().nullable(),
  fileCount: z.number().int().nonnegative(),
  indexedTrackCount: z.number().int().nonnegative(),
  createdAt: z.string(),
  updatedAt: z.string(),
});
const scanSummarySchema = z.object({
  directoriesVisited: z.number().int().nonnegative(),
  candidates: z.number().int().nonnegative(),
  unchangedSkipped: z.number().int().nonnegative(),
  newFiles: z.number().int().nonnegative(),
  changedFiles: z.number().int().nonnegative(),
  renamedFiles: z.number().int().nonnegative(),
  missingFiles: z.number().int().nonnegative(),
  unsupportedSkipped: z.number().int().nonnegative(),
  metadataFailures: z.number().int().nonnegative(),
  artworkFailures: z.number().int().nonnegative(),
  databaseFailures: z.number().int().nonnegative(),
  elapsedMs: z.number().int().nonnegative(),
});
const scanProgressSchema = z.object({
  folderId: z.string().transform((value) => value as LibraryFolderId),
  status: z.enum(["idle", "queued", "scanning", "complete", "failed"]),
  currentFile: z.string().nullable(),
  processed: z.number().int().nonnegative(),
  candidates: z.number().int().nonnegative(),
  summary: scanSummarySchema.nullable(),
  startedAt: z.string().nullable(),
  finishedAt: z.string().nullable(),
  error: z.string().nullable(),
});
const libraryStatusSchema = z.object({
  folders: z.array(libraryFolderSchema),
  indexedTrackCount: z.number().int().nonnegative(),
  availableTrackCount: z.number().int().nonnegative(),
  isScanning: z.boolean(),
});
const libraryPageRequestSchema = z.object({
  page: z.number().int().nonnegative(),
  pageSize: z.number().int().min(1).max(100),
  sort: z.enum(["title", "artist", "dateAdded", "dateModified"]),
  descending: z.boolean(),
  folderId: z.string().transform((value) => value as LibraryFolderId).nullable(),
});
const libraryTrackSchema = z.object({
  trackId: z.string().transform((value) => value as TrackId),
  sourceId: z.string().transform((value) => value as SourceId),
  folderId: z.string().transform((value) => value as LibraryFolderId),
  title: z.string(),
  artists: z.array(z.string()),
  album: z.string().nullable(),
  durationMs: z.number().int().nonnegative().nullable(),
  path: z.string(),
  available: z.boolean(),
  availabilityDetail: z.string().nullable(),
  indexStatus: z.enum(["pending", "indexed", "missing", "error"]),
  statusDetail: z.string().nullable(),
  fileSizeBytes: z.number().int().nonnegative().nullable(),
  modifiedAt: z.string().nullable(),
  codec: z.string().nullable(),
  container: z.string().nullable(),
  bitrateKbps: z.number().int().nonnegative().nullable(),
  sampleRateHz: z.number().int().nonnegative().nullable(),
  bitDepth: z.number().int().nonnegative().nullable(),
  contentFingerprint: z.string().nullable(),
  artworkCacheKey: z.string().nullable(),
  artworkMimeType: z.string().nullable(),
  artworkPath: z.string().nullable(),
  createdAt: z.string(),
  updatedAt: z.string(),
});
const libraryPageSchema = z.object({
  items: z.array(libraryTrackSchema),
  page: z.number().int().nonnegative(),
  pageSize: z.number().int().positive(),
  total: z.number().int().nonnegative(),
  hasNext: z.boolean(),
  sort: z.enum(["title", "artist", "dateAdded", "dateModified"]),
  descending: z.boolean(),
});
const trackIdSchema = z.string().transform((value) => value as TrackId);
const sourceIdSchema = z.string().transform((value) => value as SourceId);
const bookmarkIdSchema = z.string().min(1).transform((value) => value as BookmarkId);
const abLoopPresetIdSchema = z.string().min(1).transform((value) => value as AbLoopPresetId);
const playlistIdSchema = z.string().min(1).transform((value) => value as PlaylistId);
const playlistItemIdSchema = z.string().min(1).transform((value) => value as PlaylistItemId);
const tagIdSchema = z.string().min(1).transform((value) => value as TagId);
const playlistKindSchema = z.enum(["normal", "inbox", "branch"]);
const branchStatusSchema = z.enum(["open", "merged"]);
const playlistItemSchema = z.object({
  id: playlistItemIdSchema,
  playlistId: playlistIdSchema,
  trackId: trackIdSchema,
  requestedSourceId: sourceIdSchema.nullable(),
  position: z.number().int().nonnegative(),
  originBaseItemId: playlistItemIdSchema.nullable(),
  addedAt: z.string(),
  updatedAt: z.string(),
}).strict();
const playlistSchema = z.object({
  id: playlistIdSchema,
  name: z.string().min(1).max(120),
  kind: playlistKindSchema,
  parentPlaylistId: playlistIdSchema.nullable(),
  baseParentRevision: z.number().int().nonnegative().nullable(),
  branchStatus: branchStatusSchema.nullable(),
  revision: z.number().int().nonnegative(),
  createdAt: z.string(),
  updatedAt: z.string(),
  items: z.array(playlistItemSchema),
}).strict();
const branchChangeSchema = z.discriminatedUnion("type", [
  z.object({ type: z.literal("add"), branchItemId: playlistItemIdSchema }).strict(),
  z.object({ type: z.literal("remove"), baseItemId: playlistItemIdSchema }).strict(),
  z.object({
    type: z.literal("move"),
    baseItemId: playlistItemIdSchema,
    targetPosition: z.number().int().nonnegative(),
  }).strict(),
]);
const branchMergeResultSchema = z.object({
  parent: playlistSchema,
  branch: playlistSchema,
}).strict();
const playlistMembershipSchema = z.object({
  playlistId: playlistIdSchema,
  name: z.string(),
  kind: playlistKindSchema,
}).strict();
const tagSchema = z.object({
  id: tagIdSchema,
  name: z.string().min(1).max(64),
  normalizedName: z.string().min(1).max(64),
  createdAt: z.string(),
  updatedAt: z.string(),
}).strict();
const trackCollectionStateSchema = z.object({
  trackId: trackIdSchema,
  liked: z.boolean(),
  rating: z.number().int().min(1).max(5).nullable(),
  tags: z.array(tagSchema),
  playlistMemberships: z.array(playlistMembershipSchema),
  inInbox: z.boolean(),
}).strict();
const playlistErrorCodeSchema = z.enum([
  "invalidName",
  "invalidTagName",
  "playlistNotFound",
  "playlistItemNotFound",
  "trackNotFound",
  "sourceNotFound",
  "sourceMismatch",
  "systemPlaylist",
  "branchExists",
  "cannotBranch",
  "branchNotFound",
  "branchNotOpen",
  "branchAlreadyMerged",
  "branchConflict",
  "invalidBranchChange",
  "emptySelection",
  "invalidPosition",
  "tagNotFound",
  "tagExists",
  "invalidRating",
  "collectionRequestTooLarge",
  "snapshotNotFound",
  "database",
]);
const playlistErrorSchema = z.object({
  code: playlistErrorCodeSchema,
  detail: z.string().min(1),
}).strict();
const queueEntryIdSchema = z.string().transform((value) => value as QueueEntryId);
const versionQualifierSchema = z.enum([
  "standard",
  "studio",
  "live",
  "acoustic",
  "remix",
  "remaster",
  "cover",
  "instrumental",
  "karaoke",
  "spedUp",
  "slowed",
  "nightcore",
  "unknown",
]);
const trackInspectorQualitySchema = z.object({
  container: z.string().nullable(),
  codec: z.string().nullable(),
  bitrateKbps: z.number().int().nonnegative().nullable(),
  sampleRateHz: z.number().int().nonnegative().nullable(),
  bitDepth: z.number().int().nonnegative().nullable(),
}).strict();
const trackInspectorSourceSchema = z.object({
  sourceId: sourceIdSchema,
  provider: providerKindSchema,
  providerItemId: z.string().min(1),
  available: z.boolean(),
  availabilityDetail: z.string().nullable(),
  capabilities: z.object({
    search: z.boolean(),
    metadata: z.boolean(),
    artwork: z.boolean(),
    playback: z.boolean(),
    lyrics: z.boolean(),
    downloads: z.boolean(),
  }).strict(),
  durationMs: z.number().int().nonnegative().nullable(),
  versionQualifiers: z.array(versionQualifierSchema),
  quality: trackInspectorQualitySchema,
  canonicalUrl: z.string().url().nullable(),
}).strict();
const trackInspectorSchema = z.object({
  trackId: trackIdSchema,
  title: z.string(),
  artists: z.array(z.string()),
  album: z.string().nullable(),
  durationMs: z.number().int().nonnegative().nullable(),
  versionQualifiers: z.array(versionQualifierSchema),
  preferredSourceId: sourceIdSchema.nullable(),
  collectionState: z.object({
    liked: z.boolean(),
    rating: z.number().int().min(1).max(5).nullable(),
    tags: z.array(tagSchema),
    playlistMemberships: z.array(playlistMembershipSchema),
    inInbox: z.boolean(),
  }).strict(),
  sources: z.array(trackInspectorSourceSchema),
}).strict();
const fusionDecisionSchema = z.enum(["already_unified", "forced_merge", "auto_merge", "forced_split", "rejected", "excluded"]);
const fusionReasonSchema = z.enum([
  "matched",
  "provider_excluded",
  "entity_unsupported",
  "already_unified",
  "forced_merge",
  "forced_split",
  "same_provider_requires_manual_merge",
  "title_below_minimum",
  "artist_below_minimum",
  "duration_mismatch",
  "version_mismatch",
  "below_threshold",
  "identity_conflict",
  "ambiguous",
  "invalid_candidate",
]);
const fusionOverrideDecisionSchema = z.enum(["merge", "split"]);
const fusionEvaluationSchema = z.object({
  targetTrackId: trackIdSchema,
  decision: fusionDecisionSchema,
  scoreBps: z.number().int().min(0).max(10_000),
  thresholdBps: z.number().int().min(0).max(10_000),
  titleScoreBps: z.number().int().min(0).max(10_000),
  artistScoreBps: z.number().int().min(0).max(10_000),
  durationScoreBps: z.number().int().min(0).max(10_000),
  durationDeltaMs: z.number().int().nonnegative().nullable(),
  candidateQualifiers: z.array(versionQualifierSchema),
  targetQualifiers: z.array(versionQualifierSchema),
  reason: fusionReasonSchema,
}).strict();
const fusionOverrideSchema = z.object({
  providerKind: providerKindSchema,
  providerItemId: z.string().min(1),
  targetTrackId: trackIdSchema,
  decision: fusionOverrideDecisionSchema,
  createdAt: z.string(),
  updatedAt: z.string(),
}).strict();
const fusionOverrideRequestSchema = z.object({
  providerKind: providerKindSchema,
  providerItemId: z.string().trim().min(1),
  targetTrackId: trackIdSchema,
  decision: fusionOverrideDecisionSchema,
}).strict();
const clearFusionOverrideRequestSchema = z.object({
  providerKind: providerKindSchema,
  providerItemId: z.string().trim().min(1),
  targetTrackId: trackIdSchema,
}).strict();
const sourceResolutionReasonSchema = z.enum([
  "preferred_source",
  "playable",
  "unavailable",
  "local_file_missing",
  "source_does_not_support_playback",
  "provider_playback_not_implemented",
  "metadata_only",
]);
const sourceResolutionCandidateSchema = z.object({
  sourceId: sourceIdSchema,
  provider: providerKindSchema,
  playable: z.boolean(),
  reason: sourceResolutionReasonSchema,
  preferenceRank: z.number().int().nonnegative(),
  detail: z.string().nullable(),
}).strict();
const sourceResolutionSchema = z.object({
  selectedSourceId: sourceIdSchema.nullable(),
  candidates: z.array(sourceResolutionCandidateSchema),
}).strict();
const playbackPhaseSchema = z.enum([
  "idle",
  "loading",
  "playing",
  "paused",
  "seeking",
  "ended",
  "recovering",
  "failed",
  "shuttingDown",
]);
const repeatModeSchema = z.enum(["off", "one", "all"]);
const queueSectionSchema = z.enum(["up_next", "later", "autoplay"]);
const queueSnapshotIdSchema = z.string().min(1).transform((value) => value as QueueSnapshotId);
const queueSnapshotEntryIdSchema = z.string().min(1).transform((value) => value as QueueSnapshotEntryId);
const queueWorkspaceEntrySchema = z.object({
  id: queueEntryIdSchema,
  trackId: trackIdSchema,
  requestedSourceId: sourceIdSchema.nullable(),
  section: queueSectionSchema,
  position: z.number().int().nonnegative(),
  pinned: z.boolean(),
  title: z.string().nullable(),
  artists: z.array(z.string()),
  album: z.string().nullable(),
}).strict();
const queueWorkspaceSchema = z.object({
  revision: z.number().int().nonnegative(),
  current: queueWorkspaceEntrySchema.nullable(),
  upNext: z.array(queueWorkspaceEntrySchema),
  later: z.array(queueWorkspaceEntrySchema),
  autoplay: z.array(queueWorkspaceEntrySchema),
  currentPositionMs: z.number().int().nonnegative(),
  repeatMode: repeatModeSchema,
  shuffleEnabled: z.boolean(),
}).strict();
const queueSnapshotEntrySchema = z.object({
  id: queueSnapshotEntryIdSchema,
  snapshotId: queueSnapshotIdSchema,
  trackId: trackIdSchema,
  requestedSourceId: sourceIdSchema.nullable(),
  section: queueSectionSchema,
  position: z.number().int().nonnegative(),
  pinned: z.boolean(),
  traversalPosition: z.number().int().nonnegative(),
}).strict();
const queueSnapshotSummarySchema = z.object({
  id: queueSnapshotIdSchema,
  name: z.string().min(1).max(80),
  currentTrackId: trackIdSchema.nullable(),
  currentSourceId: sourceIdSchema.nullable(),
  currentPositionMs: z.number().int().nonnegative(),
  repeatMode: repeatModeSchema,
  shuffleEnabled: z.boolean(),
  entryCount: z.number().int().nonnegative(),
  createdAt: z.string(),
}).strict();
const queueSnapshotSchema = queueSnapshotSummarySchema.extend({
  currentSnapshotEntryId: queueSnapshotEntryIdSchema.nullable(),
  historyOrder: z.array(queueSnapshotEntryIdSchema),
  traversalOrder: z.array(queueSnapshotEntryIdSchema),
  entries: z.array(queueSnapshotEntrySchema),
}).strict();

const lyricsSourceKindSchema = z.enum(["manual", "sidecar", "embedded", "lrclib"]);
const lyricsSyncKindSchema = z.enum(["plain", "timed", "instrumental"]);
const lyricsCueSchema = z.object({
  startMs: z.number().int().nonnegative(),
  lines: z.array(z.string()),
}).strict();
const lyricsAttributionSchema = z.object({
  label: z.string().min(1),
  provider: z.string().min(1),
  url: z.string().url().refine((value) => {
    const url = new URL(value);
    return url.protocol === "https:" && url.hostname === "lrclib.net" && url.username === "" && url.password === "" && url.port === "";
  }, "Attribution links must use the LRCLIB HTTPS host.").nullable(),
}).strict();
const lyricsDocumentSchema = z.object({
  trackId: trackIdSchema,
  source: lyricsSourceKindSchema,
  syncKind: lyricsSyncKindSchema,
  plainText: z.string().nullable(),
  cues: z.array(lyricsCueSchema),
  instrumental: z.boolean(),
  editable: z.boolean(),
  attribution: lyricsAttributionSchema.nullable(),
}).strict().transform((value): LyricsDocument => ({
  ...value,
  source: value.source as LyricsSourceKind,
  syncKind: value.syncKind as LyricsSyncKind,
  cues: value.cues as LyricsDocument["cues"],
}));
const lyricsCandidateSchema = z.object({
  providerRecordId: z.number().int().positive(),
  trackName: z.string().min(1),
  artistName: z.string().min(1),
  albumName: z.string().nullable(),
  durationMs: z.number().int().nonnegative().nullable(),
  instrumental: z.boolean(),
  hasPlain: z.boolean(),
  hasSynced: z.boolean(),
}).strict().transform((value): LyricsCandidate => value);
const lyricsErrorCodeSchema = z.enum([
  "trackNotFound",
  "sourceNotFound",
  "sourceMismatch",
  "invalidLyrics",
  "inputTooLarge",
  "invalidUtf8",
  "unsupportedImport",
  "importRead",
  "importCancelled",
  "notFound",
  "rateLimited",
  "provider",
  "invalidCandidate",
  "cacheNotFound",
  "database",
  "local",
]);
const lyricsErrorSchema = z.object({
  code: lyricsErrorCodeSchema,
  detail: z.string().min(1),
  retryAfterSeconds: z.number().int().nonnegative().nullable(),
}).strict();
const bookmarkSchema = z.object({
  id: bookmarkIdSchema,
  trackId: trackIdSchema,
  positionMs: z.number().int().nonnegative(),
  note: z.string().max(500),
  createdAt: z.string(),
  updatedAt: z.string(),
}).strict().transform((value): Bookmark => value);
const abLoopPresetSchema = z.object({
  id: abLoopPresetIdSchema,
  trackId: trackIdSchema,
  name: z.string().min(1).max(80),
  aMs: z.number().int().nonnegative(),
  bMs: z.number().int().positive(),
  createdAt: z.string(),
  updatedAt: z.string(),
}).strict().transform((value): AbLoopPreset => value);
const bookmarkErrorCodeSchema = z.enum([
  "trackNotFound",
  "bookmarkNotFound",
  "presetNotFound",
  "invalidPosition",
  "positionOutsideDuration",
  "noteTooLong",
  "emptyNote",
  "invalidPresetName",
  "duplicatePresetName",
  "invalidLoop",
  "presetTrackMismatch",
  "database",
  "invalidStoredValue",
]);
const bookmarkErrorSchema = z.object({
  code: bookmarkErrorCodeSchema,
  detail: z.string().min(1),
}).strict();
const playbackErrorCodeSchema = z.enum([
  "toolMissing",
  "toolBroken",
  "spawnFailed",
  "ipcConnectTimeout",
  "ipcDisconnected",
  "protocolError",
  "requestTimeout",
  "trackNotFound",
  "sourceNotFound",
  "sourceMismatch",
  "sourceUnavailable",
  "sourceNotPlayable",
  "localFileMissing",
  "loadFailed",
  "seekFailed",
  "deviceUnavailable",
  "queueEmpty",
  "recoveryRetrying",
  "recoveryExhausted",
  "persistenceFailed",
  "queueEntryNotFound",
  "queueEntryImmutable",
  "invalidQueuePosition",
  "snapshotNotFound",
  "shuttingDown",
  "invalidAbLoop",
  "abLoopPresetNotFound",
  "abLoopPresetTrackMismatch",
]);
const playbackBackendHealthSchema = z.object({
  ready: z.boolean(),
  connected: z.boolean(),
  detail: z.string().nullable(),
  recoveryAction: z.string().nullable(),
}).strict();
const playbackAudioDeviceSchema = z.object({
  name: z.string().min(1),
  description: z.string(),
  selected: z.boolean(),
}).strict();
const playbackSourceOptionSchema = z.object({
  sourceId: sourceIdSchema,
  provider: providerKindSchema,
  label: z.string(),
  available: z.boolean(),
  availabilityDetail: z.string().nullable(),
}).strict();
const abLoopStateSchema = z.object({
  aMs: z.number().int().nonnegative().nullable(),
  bMs: z.number().int().nonnegative().nullable(),
  active: z.boolean(),
}).strict().transform((value): AbLoopState => value);
const playbackErrorSchema = z.object({
  code: playbackErrorCodeSchema,
  summary: z.string().min(1),
  retryable: z.boolean(),
}).strict().transform((value): PlaybackErrorDto => ({
  code: value.code as PlaybackErrorCode,
  summary: value.summary,
  retryable: value.retryable,
}));
const playbackSnapshotSchema = z.object({
  revision: z.number().int().nonnegative(),
  phase: playbackPhaseSchema,
  currentQueueEntryId: queueEntryIdSchema.nullable(),
  currentTrackId: trackIdSchema.nullable(),
  currentSourceId: sourceIdSchema.nullable(),
  title: z.string().nullable(),
  artists: z.array(z.string()),
  album: z.string().nullable(),
  artworkPath: z.string().nullable(),
  sources: z.array(playbackSourceOptionSchema),
  positionMs: z.number().int().nonnegative(),
  durationMs: z.number().int().nonnegative().nullable(),
  volumePercent: z.number().int().min(0).max(100),
  muted: z.boolean(),
  repeatMode: repeatModeSchema,
  shuffleEnabled: z.boolean(),
  queueLength: z.number().int().nonnegative(),
  queueIndex: z.number().int().nonnegative().nullable(),
  selectedAudioDevice: z.string(),
  backendHealth: playbackBackendHealthSchema,
  recovering: z.boolean(),
  error: playbackErrorSchema.nullable(),
  abLoop: abLoopStateSchema,
}).strict().transform((value): PlaybackSnapshot => ({
  revision: value.revision,
  phase: value.phase as PlaybackPhase,
  currentQueueEntryId: value.currentQueueEntryId,
  currentTrackId: value.currentTrackId,
  currentSourceId: value.currentSourceId,
  title: value.title,
  artists: value.artists,
  album: value.album,
  artworkPath: value.artworkPath,
  sources: value.sources as PlaybackSourceOption[],
  positionMs: value.positionMs,
  durationMs: value.durationMs,
  volumePercent: value.volumePercent,
  muted: value.muted,
  repeatMode: value.repeatMode as RepeatMode,
  shuffleEnabled: value.shuffleEnabled,
  queueLength: value.queueLength,
  queueIndex: value.queueIndex,
  selectedAudioDevice: value.selectedAudioDevice,
  backendHealth: value.backendHealth as PlaybackBackendHealth,
  recovering: value.recovering,
  error: value.error,
  abLoop: value.abLoop,
}));
const trackPlaybackRequestSchema = z.object({
  trackId: trackIdSchema,
  sourceId: sourceIdSchema.nullable(),
}).strict();
const playbackSourceRequestSchema = z.object({
  trackId: trackIdSchema,
  sourceId: sourceIdSchema,
}).strict();
const playbackDeviceNameSchema = z.string().trim().min(1);
const playbackSeekSchema = z.number().int().nonnegative();
const playbackVolumeSchema = z.number().int().min(0).max(100);
const manualLyricsModeSchema = z.enum(["plain", "lrc"]);

export class IpcError extends Error {
  public constructor(message: string, public readonly cause?: unknown) {
    super(message);
    this.name = "IpcError";
  }
}

export const LIBRARY_PROGRESS_EVENT = "library://scan-progress";
export const PLAYBACK_STATE_EVENT = "playback://state";
export const QUEUE_STATE_EVENT = "queue://state";
export const DOWNLOAD_STATE_EVENT = "downloads://state";

type PlaybackSnapshotListener = (snapshot: PlaybackSnapshot) => void;
type PlaybackSnapshotErrorListener = (error: IpcError) => void;
export type QueueWorkspaceListener = (workspace: QueueWorkspace) => void;
export type QueueWorkspaceErrorListener = (error: IpcError) => void;
export type DownloadSnapshotListener = (snapshot: DownloadSnapshot) => void;
export type DownloadSnapshotErrorListener = (error: IpcError) => void;

const playbackErrorSummary: Record<PlaybackErrorCode, string> = {
  toolMissing: "mpv is not available on this machine.",
  toolBroken: "The playback tool could not be validated.",
  spawnFailed: "SpotDIY could not start the playback backend.",
  ipcConnectTimeout: "The playback backend did not accept a connection in time.",
  ipcDisconnected: "The playback backend disconnected.",
  protocolError: "SpotDIY received an invalid playback response.",
  requestTimeout: "The playback backend did not respond in time.",
  trackNotFound: "That track is no longer available in the local library.",
  sourceNotFound: "That playback source could not be found.",
  sourceMismatch: "That source does not belong to the requested track.",
  sourceUnavailable: "That source is currently unavailable.",
  sourceNotPlayable: "That source cannot be played by SpotDIY.",
  localFileMissing: "The local file is missing or unavailable.",
  loadFailed: "SpotDIY could not load that track.",
  seekFailed: "SpotDIY could not seek within the current track.",
  deviceUnavailable: "That audio device is unavailable.",
  queueEmpty: "The playback queue is empty.",
  recoveryRetrying: "SpotDIY is retrying the playback backend.",
  recoveryExhausted: "Playback recovery is exhausted.",
  persistenceFailed: "SpotDIY could not save the playback queue.",
  queueEntryNotFound: "That queue entry is no longer available.",
  queueEntryImmutable: "The current or consumed queue entry cannot be changed.",
  invalidQueuePosition: "That queue position is invalid.",
  snapshotNotFound: "That queue snapshot could not be found.",
  shuttingDown: "SpotDIY is shutting down playback.",
  invalidAbLoop: "That A/B loop is invalid.",
  abLoopPresetNotFound: "That A/B loop preset could not be found.",
  abLoopPresetTrackMismatch: "That A/B loop preset belongs to another track.",
};

const emptyPlaybackSnapshot = (): PlaybackSnapshot => ({
  revision: 0,
  phase: "idle",
  currentQueueEntryId: null,
  currentTrackId: null,
  currentSourceId: null,
  title: null,
  artists: [],
  album: null,
  artworkPath: null,
  sources: [],
  positionMs: 0,
  durationMs: null,
  volumePercent: 100,
  muted: false,
  repeatMode: "off",
  shuffleEnabled: false,
  queueLength: 0,
  queueIndex: null,
  selectedAudioDevice: "auto",
  backendHealth: {
    ready: false,
    connected: false,
    detail: null,
    recoveryAction: null,
  },
  recovering: false,
  error: null,
  abLoop: {
    aMs: null,
    bMs: null,
    active: false,
  },
});

function isPlaybackE2EAdapterEnabled(): boolean {
  return !isTauriRuntime() && import.meta.env.DEV && import.meta.env.VITE_SPOTDIY_E2E === "1";
}

const e2eLibraryFolder: LibraryFolder = {
  id: "folder-e2e" as LibraryFolderId,
  path: "C:\\Synthetic Music",
  normalizedPathKey: "c:\\synthetic music",
  enabled: true,
  status: "complete",
  scanGeneration: 1,
  lastScanStartedAt: "2026-08-31T00:00:00Z",
  lastScanFinishedAt: "2026-08-31T00:00:01Z",
  lastScanError: null,
  fileCount: 2,
  indexedTrackCount: 2,
  createdAt: "2026-08-31T00:00:00Z",
  updatedAt: "2026-08-31T00:00:01Z",
};

const e2eLibraryTracks: LibraryTrack[] = [
  {
    trackId: "track-e2e-1" as TrackId,
    sourceId: "source-e2e-1" as SourceId,
    folderId: e2eLibraryFolder.id,
    title: "Night Drive - Neon Over Water (Extended Live Session)",
    artists: ["Luna Max"],
    album: "Afterglow",
    durationMs: 185000,
    path: "C:\\Synthetic Music\\night-drive.flac",
    available: true,
    availabilityDetail: null,
    indexStatus: "indexed",
    statusDetail: null,
    fileSizeBytes: 12_340_000,
    modifiedAt: "2026-08-31T00:00:00Z",
    codec: "FLAC",
    container: "FLAC",
    bitrateKbps: null,
    sampleRateHz: 44100,
    bitDepth: 16,
    contentFingerprint: "fixture-night-drive",
    artworkCacheKey: null,
    artworkMimeType: null,
    artworkPath: null,
    createdAt: "2026-08-31T00:00:00Z",
    updatedAt: "2026-08-31T00:00:00Z",
  },
  {
    trackId: "track-e2e-2" as TrackId,
    sourceId: "source-e2e-2" as SourceId,
    folderId: e2eLibraryFolder.id,
    title: "Static Bloom",
    artists: ["Mira Vale"],
    album: "Soft Signals",
    durationMs: 214000,
    path: "C:\\Synthetic Music\\static-bloom.flac",
    available: true,
    availabilityDetail: null,
    indexStatus: "indexed",
    statusDetail: null,
    fileSizeBytes: 13_800_000,
    modifiedAt: "2026-08-31T00:00:00Z",
    codec: "FLAC",
    container: "FLAC",
    bitrateKbps: null,
    sampleRateHz: 48000,
    bitDepth: 24,
    contentFingerprint: "fixture-static-bloom",
    artworkCacheKey: null,
    artworkMimeType: null,
    artworkPath: null,
    createdAt: "2026-08-31T00:00:00Z",
    updatedAt: "2026-08-31T00:00:00Z",
  },
];

const e2eTrackMap = new Map(e2eLibraryTracks.map((track) => [track.trackId, track]));
const e2eDevices: PlaybackAudioDevice[] = [
  { name: "auto", description: "Default output", selected: true },
  { name: "headphones", description: "USB Headphones", selected: false },
];

function selectE2EDevice(name: string) {
  for (const device of e2eDevices) {
    device.selected = device.name === name;
  }
}

interface PlaybackE2EAdapterState {
  snapshot: PlaybackSnapshot;
  canonicalQueue: TrackPlaybackRequest[];
  activeQueue: TrackPlaybackRequest[];
  canonicalQueueIds: QueueEntryId[];
  activeQueueIds: QueueEntryId[];
  currentIndex: number | null;
  listeners: Set<PlaybackSnapshotListener>;
  positionTimer: number | null;
  transitionTimer: number | null;
  nextRevision: number;
  nextQueueEntryId: number;
}

const e2eAdapterState: PlaybackE2EAdapterState = {
  snapshot: emptyPlaybackSnapshot(),
  canonicalQueue: [],
  activeQueue: [],
  currentIndex: null,
  listeners: new Set(),
  positionTimer: null,
  transitionTimer: null,
  nextRevision: 1,
  canonicalQueueIds: [],
  activeQueueIds: [],
  nextQueueEntryId: 1,
};

function e2ePlaybackScenario(): "default" | "toolMissing" | "recovering" | "failed" {
  if (typeof window === "undefined") {
    return "default";
  }

  const scenario = new URLSearchParams(window.location.search).get("playbackScenario");
  if (scenario === "toolMissing" || scenario === "recovering" || scenario === "failed") {
    return scenario;
  }

  return "default";
}

function summarizePlaybackError(code: PlaybackErrorCode, detail: string | null): string {
  return detail ?? playbackErrorSummary[code];
}

function createPlaybackError(code: PlaybackErrorCode, detail: string | null, retryable: boolean): PlaybackErrorDto {
  return {
    code,
    summary: summarizePlaybackError(code, detail),
    retryable,
  };
}

function nextE2EQueueEntryId(): QueueEntryId {
  return `queue-entry-e2e-${e2eAdapterState.nextQueueEntryId++}` as QueueEntryId;
}

function e2eAlternateSourceId(track: LibraryTrack): SourceId {
  return `${track.sourceId}-alternate` as SourceId;
}

function trackToPlaybackSources(track: LibraryTrack): PlaybackSourceOption[] {
  return [
    {
      sourceId: track.sourceId,
      provider: "local",
      label: "LOCAL",
      available: track.available,
      availabilityDetail: track.availabilityDetail,
    },
    {
      sourceId: e2eAlternateSourceId(track),
      provider: "youtube",
      label: "YT",
      available: true,
      availabilityDetail: null,
    },
  ];
}

function makePlaybackSnapshotForTrack(track: LibraryTrack, overrides: Partial<PlaybackSnapshot> = {}): PlaybackSnapshot {
  return {
    ...emptyPlaybackSnapshot(),
    revision: e2eAdapterState.nextRevision++,
    phase: "idle",
    currentTrackId: track.trackId,
    currentSourceId: track.sourceId,
    title: track.title,
    artists: track.artists,
    album: track.album,
    artworkPath: track.artworkPath,
    sources: trackToPlaybackSources(track),
    durationMs: track.durationMs,
    queueLength: 1,
    queueIndex: 0,
    selectedAudioDevice: e2eDevices[0].name,
    backendHealth: {
      ready: true,
      connected: true,
      detail: null,
      recoveryAction: null,
    },
    ...overrides,
  };
}

function setE2ESnapshot(snapshot: PlaybackSnapshot): PlaybackSnapshot {
  e2eAdapterState.snapshot = snapshot;
  for (const listener of e2eAdapterState.listeners) {
    listener(snapshot);
  }
  return snapshot;
}

function clearE2ETimers() {
  if (e2eAdapterState.positionTimer !== null) {
    window.clearInterval(e2eAdapterState.positionTimer);
    e2eAdapterState.positionTimer = null;
  }
  if (e2eAdapterState.transitionTimer !== null) {
    window.clearTimeout(e2eAdapterState.transitionTimer);
    e2eAdapterState.transitionTimer = null;
  }
}

function currentE2ETrack(): LibraryTrack | null {
  const current = e2eAdapterState.currentIndex === null ? null : e2eAdapterState.activeQueue[e2eAdapterState.currentIndex];
  return current ? e2eTrackMap.get(current.trackId) ?? null : null;
}

function playbackRequestKey(request: TrackPlaybackRequest): string {
  return `${request.trackId}:${request.sourceId ?? ""}`;
}

function rebuildE2EActiveQueue() {
  const current = e2eAdapterState.currentIndex === null
    ? null
    : e2eAdapterState.activeQueue[e2eAdapterState.currentIndex] ?? null;
  const currentEntryId = e2eAdapterState.currentIndex === null
    ? null
    : e2eAdapterState.activeQueueIds[e2eAdapterState.currentIndex] ?? null;
  const currentCanonicalIndex = currentEntryId === null
    ? current
      ? e2eAdapterState.canonicalQueue.findIndex((entry) => playbackRequestKey(entry) === playbackRequestKey(current))
      : -1
    : e2eAdapterState.canonicalQueueIds.indexOf(currentEntryId);
  if (!e2eAdapterState.snapshot.shuffleEnabled || e2eAdapterState.canonicalQueue.length <= 2) {
    e2eAdapterState.activeQueue = [...e2eAdapterState.canonicalQueue];
    e2eAdapterState.activeQueueIds = [...e2eAdapterState.canonicalQueueIds];
    if (currentEntryId !== null) {
      e2eAdapterState.currentIndex = e2eAdapterState.activeQueueIds.indexOf(currentEntryId);
    } else if (current) {
      e2eAdapterState.currentIndex = e2eAdapterState.activeQueue.findIndex((entry) => playbackRequestKey(entry) === playbackRequestKey(current));
    }
    return;
  }

  const splitIndex = currentCanonicalIndex < 0
    ? e2eAdapterState.canonicalQueue.length
    : currentCanonicalIndex + 1;
  const headPairs = e2eAdapterState.canonicalQueue
    .slice(0, splitIndex)
    .map((request, index) => ({
      request,
      id: e2eAdapterState.canonicalQueueIds[index],
    }));
  const tailPairs = e2eAdapterState.canonicalQueue
    .slice(splitIndex)
    .map((request, index) => ({
      request,
      id: e2eAdapterState.canonicalQueueIds[splitIndex + index],
    }));
  tailPairs.sort((left, right) => playbackRequestKey(right.request).localeCompare(playbackRequestKey(left.request)));
  e2eAdapterState.activeQueue = [...headPairs, ...tailPairs].map(({ request }) => request);
  e2eAdapterState.activeQueueIds = [...headPairs, ...tailPairs].map(({ id }) => id);
  if (current) {
    e2eAdapterState.currentIndex = currentEntryId === null
      ? e2eAdapterState.activeQueue.findIndex((entry) => playbackRequestKey(entry) === playbackRequestKey(current))
      : e2eAdapterState.activeQueueIds.indexOf(currentEntryId);
  }
}

function seedE2EPlaybackState() {
  clearE2ETimers();
  selectE2EDevice("auto");
  e2eAdapterState.canonicalQueue = [];
  e2eAdapterState.activeQueue = [];
  e2eAdapterState.canonicalQueueIds = [];
  e2eAdapterState.activeQueueIds = [];
  e2eAdapterState.currentIndex = null;
  e2eAdapterState.nextRevision = 1;
  e2eAdapterState.nextQueueEntryId = 1;

  const scenario = e2ePlaybackScenario();
  if (scenario === "toolMissing") {
    setE2ESnapshot({
      ...emptyPlaybackSnapshot(),
      revision: e2eAdapterState.nextRevision++,
      phase: "failed",
      backendHealth: {
        ready: false,
        connected: false,
        detail: "mpv is unavailable in the browser playback adapter.",
        recoveryAction: "Retry the playback backend",
      },
      error: createPlaybackError("toolMissing", null, true),
    });
    return;
  }

  if (scenario === "recovering") {
    const track = e2eLibraryTracks[0];
    e2eAdapterState.canonicalQueue = [{ trackId: track.trackId, sourceId: track.sourceId }];
    e2eAdapterState.canonicalQueueIds = [nextE2EQueueEntryId()];
    e2eAdapterState.activeQueue = [...e2eAdapterState.canonicalQueue];
    e2eAdapterState.activeQueueIds = [...e2eAdapterState.canonicalQueueIds];
    e2eAdapterState.currentIndex = 0;
    setE2ESnapshot(makePlaybackSnapshotForTrack(track, {
      phase: "recovering",
      recovering: true,
      backendHealth: {
        ready: false,
        connected: false,
        detail: "Reconnecting to the playback backend…",
        recoveryAction: "Retry the playback backend",
      },
    }));
    return;
  }

  if (scenario === "failed") {
    const track = e2eLibraryTracks[0];
    e2eAdapterState.canonicalQueue = [{ trackId: track.trackId, sourceId: track.sourceId }];
    e2eAdapterState.canonicalQueueIds = [nextE2EQueueEntryId()];
    e2eAdapterState.activeQueue = [...e2eAdapterState.canonicalQueue];
    e2eAdapterState.activeQueueIds = [...e2eAdapterState.canonicalQueueIds];
    e2eAdapterState.currentIndex = 0;
    setE2ESnapshot(makePlaybackSnapshotForTrack(track, {
      phase: "failed",
      error: createPlaybackError("recoveryExhausted", null, true),
      backendHealth: {
        ready: false,
        connected: false,
        detail: "Playback recovery is exhausted.",
        recoveryAction: "Retry the playback backend",
      },
    }));
    return;
  }

  setE2ESnapshot({
    ...emptyPlaybackSnapshot(),
    revision: e2eAdapterState.nextRevision++,
    backendHealth: {
      ready: true,
      connected: true,
      detail: null,
      recoveryAction: null,
    },
  });
}

function ensureE2EPlaybackState() {
  if (e2eAdapterState.snapshot.revision === 0) {
    seedE2EPlaybackState();
  }
}

function withNextRevision(snapshot: PlaybackSnapshot, overrides: Partial<PlaybackSnapshot>): PlaybackSnapshot {
  return {
    ...snapshot,
    ...overrides,
    revision: e2eAdapterState.nextRevision++,
  };
}

function startE2EPlaybackTicker() {
  clearE2ETimers();
  const tick = () => {
    const track = currentE2ETrack();
    if (!track || e2eAdapterState.snapshot.phase !== "playing") {
      return;
    }

    const durationMs = track.durationMs ?? e2eAdapterState.snapshot.durationMs ?? 0;
    const nextPosition = Math.min(durationMs, e2eAdapterState.snapshot.positionMs + 1_000);
    if (nextPosition >= durationMs && durationMs > 0) {
      setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, {
        phase: "ended",
        positionMs: durationMs,
      }));
      clearE2ETimers();
      return;
    }

    setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, { positionMs: nextPosition }));
  };

  e2eAdapterState.positionTimer = window.setInterval(tick, 1_000);
}

function queueSnapshotForCurrentTrack(phase: PlaybackPhase, positionMs: number): PlaybackSnapshot {
  const track = currentE2ETrack();
  if (!track) {
    return withNextRevision(e2eAdapterState.snapshot, {
      phase: "idle",
      currentTrackId: null,
      currentSourceId: null,
      title: null,
      artists: [],
      album: null,
      artworkPath: null,
      sources: [],
      positionMs: 0,
      durationMs: null,
      queueLength: 0,
      queueIndex: null,
      error: null,
      recovering: false,
    });
  }

  const currentRequest = e2eAdapterState.currentIndex === null
    ? null
    : e2eAdapterState.activeQueue[e2eAdapterState.currentIndex] ?? null;
  const currentQueueEntryId = e2eAdapterState.currentIndex === null
    ? null
    : e2eAdapterState.activeQueueIds[e2eAdapterState.currentIndex] ?? null;
  return makePlaybackSnapshotForTrack(track, {
    currentQueueEntryId,
    currentSourceId: currentRequest?.sourceId ?? track.sourceId,
    phase,
    positionMs,
    queueLength: e2eAdapterState.activeQueue.length,
    queueIndex: e2eAdapterState.currentIndex,
    repeatMode: e2eAdapterState.snapshot.repeatMode,
    shuffleEnabled: e2eAdapterState.snapshot.shuffleEnabled,
    volumePercent: e2eAdapterState.snapshot.volumePercent,
    muted: e2eAdapterState.snapshot.muted,
    selectedAudioDevice: e2eAdapterState.snapshot.selectedAudioDevice || e2eDevices[0].name,
    backendHealth: e2eAdapterState.snapshot.backendHealth,
    error: null,
    recovering: false,
  });
}

function scheduleE2ELoad(index: number, startPaused = false, positionMs = 0): PlaybackSnapshot {
  ensureE2EPlaybackState();
  e2eAdapterState.currentIndex = index;
  clearE2ETimers();

  const loading = setE2ESnapshot(queueSnapshotForCurrentTrack("loading", positionMs));
  e2eAdapterState.transitionTimer = window.setTimeout(() => {
    const phase: PlaybackPhase = startPaused ? "paused" : "playing";
    const next = setE2ESnapshot(queueSnapshotForCurrentTrack(phase, positionMs));
    if (next.phase === "playing") {
      startE2EPlaybackTicker();
    }
  }, 120);
  return loading;
}

function resolveE2EPlaybackRequest(request: TrackPlaybackRequest): LibraryTrack {
  const track = e2eTrackMap.get(request.trackId);
  if (!track) {
    throw new IpcError("SpotDIY could not find that playback track in the E2E adapter.");
  }
  if (request.sourceId !== null && !trackToPlaybackSources(track).some((source) => source.sourceId === request.sourceId)) {
    throw new IpcError("SpotDIY could not find the requested playback source in the E2E adapter.");
  }
  return track;
}

function advanceE2EQueue(direction: 1 | -1): PlaybackSnapshot {
  const queue = e2eAdapterState.activeQueue;
  if (queue.length === 0) {
    throw new IpcError("SpotDIY could not move within an empty playback queue.");
  }

  const currentIndex = e2eAdapterState.currentIndex ?? 0;
  let nextIndex = currentIndex + direction;
  if (direction > 0 && nextIndex >= queue.length) {
    if (e2eAdapterState.snapshot.repeatMode === "all") {
      nextIndex = 0;
    } else {
      clearE2ETimers();
      return setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, {
        phase: "ended",
        positionMs: e2eAdapterState.snapshot.durationMs ?? e2eAdapterState.snapshot.positionMs,
      }));
    }
  }

  if (direction < 0 && nextIndex < 0) {
    if (e2eAdapterState.snapshot.repeatMode === "all" && queue.length > 1) {
      nextIndex = queue.length - 1;
    } else {
      return setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, { positionMs: 0 }));
    }
  }

  return scheduleE2ELoad(nextIndex, false, 0);
}

function sortLibraryTracks(items: LibraryTrack[], request: LibraryPageRequest): LibraryTrack[] {
  const sorted = [...items].sort((left, right) => {
    switch (request.sort) {
      case "artist":
        return (left.artists[0] ?? "").localeCompare(right.artists[0] ?? "");
      case "dateAdded":
        return left.createdAt.localeCompare(right.createdAt);
      case "dateModified":
        return (left.modifiedAt ?? "").localeCompare(right.modifiedAt ?? "");
      default:
        return left.title.localeCompare(right.title);
    }
  });
  return request.descending ? sorted.reverse() : sorted;
}

function browserPreviewMediaTools(): MediaToolsSnapshot {
  return {
    ytDlp: {
      status: "missing",
      version: null,
      detail: "Downloads require the native SpotDIY desktop runtime.",
    },
    ffmpeg: {
      status: "missing",
      version: null,
      detail: "Downloads require the native SpotDIY desktop runtime.",
    },
  };
}

function browserPreviewStatus(): AppStatus {
  if (isPlaybackE2EAdapterEnabled()) {
    return {
      version: "0.1.0",
      runtime: "browser-preview",
      storageMode: "standard",
      firstRun: false,
      tracksIndexed: e2eLibraryTracks.length,
      musicFolders: [e2eLibraryFolder.path],
      providers: [
        {
          kind: "local",
          label: "Local library",
          configured: true,
          available: true,
          runtimeStatus: "ready",
          capabilities: {
            search: true,
            playback: true,
            metadata: true,
            artwork: true,
            lyrics: true,
            downloads: false,
            popularity: false,
            releaseDate: false,
            lyricsMetadata: true,
          },
          detail: "Synthetic playback fixtures are enabled for browser E2E coverage.",
        },
        {
          kind: "youtube",
          label: "YouTube",
          configured: false,
          available: false,
          runtimeStatus: "missing",
          capabilities: {
            search: true,
            playback: false,
            metadata: true,
            artwork: true,
            lyrics: false,
            downloads: true,
            popularity: true,
            releaseDate: false,
            lyricsMetadata: false,
          },
          detail: "Provider adapter awaits media-tool verification.",
        },
        {
          kind: "soundcloud",
          label: "SoundCloud",
          configured: false,
          available: false,
          runtimeStatus: "missing",
          capabilities: {
            search: true,
            playback: false,
            metadata: true,
            artwork: true,
            lyrics: false,
            downloads: true,
            popularity: true,
            releaseDate: false,
            lyricsMetadata: false,
          },
          detail: "Provider adapter awaits media-tool verification.",
        },
        {
          kind: "spotify",
          label: "Spotify catalog",
          configured: false,
          available: false,
          runtimeStatus: "disabled",
          capabilities: {
            search: true,
            playback: false,
            metadata: true,
            artwork: true,
            lyrics: false,
            downloads: false,
            popularity: false,
            releaseDate: true,
            lyricsMetadata: false,
          },
          detail: "Spotify catalog search is disabled by default.",
        },
      ],
      mediaTools: browserPreviewMediaTools(),
    };
  }

  return {
    version: "0.1.0",
    runtime: "browser-preview",
    storageMode: "standard",
    firstRun: true,
    tracksIndexed: 0,
    musicFolders: [],
    providers: [
      {
        kind: "local",
        label: "Local library",
        configured: false,
        available: true,
        runtimeStatus: "ready",
        capabilities: {
          search: true,
          playback: true,
          metadata: true,
          artwork: true,
          lyrics: true,
          downloads: false,
          popularity: false,
          releaseDate: false,
          lyricsMetadata: true,
        },
        detail: "Add a music folder to begin indexing.",
      },
      {
        kind: "youtube",
        label: "YouTube",
        configured: false,
        available: false,
        runtimeStatus: "missing",
        capabilities: {
          search: true,
          playback: false,
          metadata: true,
          artwork: true,
          lyrics: false,
          downloads: true,
          popularity: true,
          releaseDate: false,
          lyricsMetadata: false,
        },
        detail: "Provider adapter awaits media-tool verification.",
      },
      {
        kind: "soundcloud",
        label: "SoundCloud",
        configured: false,
        available: false,
        runtimeStatus: "missing",
        capabilities: {
          search: true,
          playback: false,
          metadata: true,
          artwork: true,
          lyrics: false,
          downloads: true,
          popularity: true,
          releaseDate: false,
          lyricsMetadata: false,
        },
        detail: "Provider adapter awaits media-tool verification.",
      },
      {
        kind: "spotify",
        label: "Spotify catalog",
        configured: false,
        available: false,
        runtimeStatus: "disabled",
        capabilities: {
          search: true,
          playback: false,
          metadata: true,
          artwork: true,
          lyrics: false,
          downloads: false,
          popularity: false,
          releaseDate: true,
          lyricsMetadata: false,
        },
        detail: "Spotify catalog search is disabled by default.",
      },
    ],
    mediaTools: browserPreviewMediaTools(),
  };
}

let browserPreviewSettingsState: SettingsSnapshot = {
  theme: "dark",
  layoutProfile: "comfortable",
  customTheme: null,
  downloadsDirectory: null,
  sourcePreferenceOrder: ["local", "soundcloud", "youtube", "spotify"],
  firstRun: true,
  storageMode: "standard",
};

function browserPreviewSettings(): SettingsSnapshot {
  return {
    ...browserPreviewSettingsState,
    sourcePreferenceOrder: [...browserPreviewSettingsState.sourcePreferenceOrder],
    customTheme: browserPreviewSettingsState.customTheme
      ? {
        ...browserPreviewSettingsState.customTheme,
        tokens: { ...browserPreviewSettingsState.customTheme.tokens },
      }
      : null,
  };
}

function setBrowserPreviewSetting(setting: SettingValue): SettingsSnapshot {
  const parsedSetting = settingValueSchema.parse(setting) as SettingValue;
  if (parsedSetting.key === "theme" && parsedSetting.value === "custom" && browserPreviewSettingsState.customTheme === null) {
    throw new IpcError("A valid custom theme must be imported before it can be selected.");
  }
  if (parsedSetting.key === "customTheme" && parsedSetting.value === null && browserPreviewSettingsState.theme === "custom") {
    throw new IpcError("Reset the active custom theme by selecting Dark first.");
  }

  browserPreviewSettingsState = {
    ...browserPreviewSettingsState,
    ...(parsedSetting.key === "theme" ? { theme: parsedSetting.value } : {}),
    ...(parsedSetting.key === "layoutProfile" ? { layoutProfile: parsedSetting.value } : {}),
    ...(parsedSetting.key === "customTheme" ? { customTheme: parsedSetting.value } : {}),
    ...(parsedSetting.key === "downloadsDirectory" ? { downloadsDirectory: parsedSetting.value } : {}),
    ...(parsedSetting.key === "sourcePreferenceOrder" ? { sourcePreferenceOrder: [...parsedSetting.value] } : {}),
  };
  return browserPreviewSettings();
}


function browserPreviewLibraryStatus(): LibraryStatus {
  if (isPlaybackE2EAdapterEnabled()) {
    return {
      folders: [e2eLibraryFolder],
      indexedTrackCount: e2eLibraryTracks.length,
      availableTrackCount: e2eLibraryTracks.length,
      isScanning: false,
    };
  }

  return {
    folders: [],
    indexedTrackCount: 0,
    availableTrackCount: 0,
    isScanning: false,
  };
}

function browserPreviewLibraryPage(request: LibraryPageRequest): LibraryPage {
  if (isPlaybackE2EAdapterEnabled()) {
    const filtered = request.folderId
      ? e2eLibraryTracks.filter((track) => track.folderId === request.folderId)
      : e2eLibraryTracks;
    const sorted = sortLibraryTracks(filtered, request);
    const start = request.page * request.pageSize;
    const items = sorted.slice(start, start + request.pageSize);
    return {
      items,
      page: request.page,
      pageSize: request.pageSize,
      total: sorted.length,
      hasNext: start + request.pageSize < sorted.length,
      sort: request.sort,
      descending: request.descending,
    };
  }

  return {
    items: [],
    page: request.page,
    pageSize: request.pageSize,
    total: 0,
    hasNext: false,
    sort: request.sort,
    descending: request.descending,
  };
}

function browserPreviewTrackInspector(trackId: TrackId): TrackInspector {
  const track = e2eTrackMap.get(trackId);
  if (!track) {
    throw new IpcError("That track is not available in the browser preview.");
  }

  return {
    trackId: track.trackId,
    title: track.title,
    artists: track.artists,
    album: track.album,
    durationMs: track.durationMs,
    versionQualifiers: ["standard"],
    preferredSourceId: track.sourceId,
    collectionState: {
      liked: false,
      rating: null,
      tags: [],
      playlistMemberships: [],
      inInbox: false,
    },
    sources: [
      {
        sourceId: track.sourceId,
        provider: "local",
        providerItemId: track.trackId,
        available: track.available,
        availabilityDetail: track.availabilityDetail,
        capabilities: {
          search: true,
          metadata: true,
          artwork: Boolean(track.artworkPath),
          playback: track.available,
          lyrics: false,
          downloads: false,
        },
        durationMs: track.durationMs,
        versionQualifiers: ["standard"],
        quality: {
          container: track.container,
          codec: track.codec,
          bitrateKbps: track.bitrateKbps,
          sampleRateHz: track.sampleRateHz,
          bitDepth: track.bitDepth,
        },
        canonicalUrl: null,
      },
      {
        sourceId: e2eAlternateSourceId(track),
        provider: "youtube",
        providerItemId: "spotdiy-e2e",
        available: true,
        availabilityDetail: "Online playback is not implemented in Plan 11.",
        capabilities: {
          search: true,
          metadata: true,
          artwork: false,
          playback: false,
          lyrics: false,
          downloads: true,
        },
        durationMs: track.durationMs,
        versionQualifiers: ["standard"],
        quality: {
          container: null,
          codec: null,
          bitrateKbps: null,
          sampleRateHz: null,
          bitDepth: null,
        },
        canonicalUrl: "https://www.youtube.com/watch?v=spotdiy-e2e",
      },
    ],
  };
}

export const SEARCH_PROVIDER_UPDATE_EVENT = "search://provider-update";
export const SEARCH_COMPLETED_EVENT = "search://complete";
export const SPOTIFY_AUTH_STATE_EVENT = "spotify://auth-state";

type SearchProviderUpdateListener = (event: ProviderSearchEvent) => void;
type SearchCompletedListener = (event: SearchCompleted) => void;
type SearchEventErrorListener = (error: IpcError) => void;

interface BrowserSearchRun {
  searchId: SearchId;
  timers: number[];
  pending: number;
  completed: boolean;
}

const browserSearchRuns = new Map<SearchId, BrowserSearchRun>();
const browserSearchProviderListeners = new Set<SearchProviderUpdateListener>();
const browserSearchCompletedListeners = new Set<SearchCompletedListener>();
let browserActiveSearchId: SearchId | null = null;

const searchProviderOrder = (lens: SearchLens): ProviderKind[] => {
  switch (lens) {
    case "local":
      return ["local"];
    case "youtube":
      return ["youtube"];
    case "soundcloud":
      return ["soundcloud"];
    case "spotify":
      return ["spotify"];
    case "artists":
    case "albums":
      return ["local"];
    default:
      return ["local", "youtube", "soundcloud"];
  }
};

function makeBrowserSearchId(): SearchId {
  const randomUuid = globalThis.crypto?.randomUUID?.();
  return (randomUuid ?? `browser-search-${Date.now()}-${Math.random().toString(16).slice(2)}`) as SearchId;
}

function emitBrowserSearchProviderEvent(event: ProviderSearchEvent) {
  for (const listener of browserSearchProviderListeners) {
    listener(event);
  }
}

function emitBrowserSearchCompletedEvent(event: SearchCompleted) {
  for (const listener of browserSearchCompletedListeners) {
    listener(event);
  }
}

function browserSearchResult(provider: ProviderKind, query: string, track?: LibraryTrack): SearchResult {
  if (provider === "local" && track) {
    return {
      provider,
      entityKind: "track",
      providerItemId: track.trackId,
      canonicalUrl: null,
      title: track.title,
      artists: track.artists,
      album: track.album,
      durationMs: track.durationMs,
      artworkUrl: null,
      publishedAt: null,
      engagementCount: null,
      engagementKind: null,
      explicit: null,
      localTrackId: track.trackId,
      localSourceId: track.sourceId,
      originalRank: 0,
    };
  }

  const slug = encodeURIComponent(query.trim().toLowerCase().replace(/\s+/g, "-"));
  return {
    provider,
    entityKind: "track",
    providerItemId: `e2e-${provider}-${slug}`,
    canonicalUrl: provider === "youtube"
      ? "https://www.youtube.com/watch?v=spotdiy-e2e"
      : provider === "soundcloud"
        ? "https://soundcloud.com/spotdiy/e2e-result"
        : "https://open.spotify.com/track/spotdiy-e2e",
    title: `${query.trim()} — ${provider === "youtube" ? "YouTube" : provider === "soundcloud" ? "SoundCloud" : "Spotify"}`,
    artists: [provider === "spotify" ? "SpotDIY Catalog" : "SpotDIY E2E"],
    album: provider === "spotify" ? "Catalog fixture" : null,
    durationMs: 198_000,
    artworkUrl: null,
    publishedAt: null,
    engagementCount: null,
    engagementKind: null,
    explicit: null,
    localTrackId: null,
    localSourceId: null,
    originalRank: 0,
  };
}

function browserSearchSection(provider: ProviderKind, request: SearchRequest): ProviderSearchSection {
  if (provider === "spotify") {
    return {
      provider,
      state: "failed",
      results: [],
      error: {
        code: "disabled",
        detail: "Spotify catalog search is disabled by default.",
        retryAfterSeconds: null,
      },
    };
  }

  if (provider === "soundcloud") {
    return {
      provider,
      state: "failed",
      results: [],
      error: {
        code: "unavailable",
        detail: "Synthetic partial-provider failure for browser E2E coverage.",
        retryAfterSeconds: null,
      },
    };
  }

  if (provider === "local") {
    const needle = request.query.trim().toLocaleLowerCase();
    const tracks = e2eLibraryTracks.filter((track) => [track.title, track.album ?? "", ...track.artists]
      .some((value) => value.toLocaleLowerCase().includes(needle)));
    return {
      provider,
      state: "ready",
      results: (tracks.length > 0 ? tracks : e2eLibraryTracks).map((track, index) => ({
        ...browserSearchResult(provider, request.query, track),
        originalRank: index,
      })),
      error: null,
    };
  }

  return {
    provider,
    state: "ready",
    results: [browserSearchResult(provider, request.query)],
    error: null,
  };
}

function finishBrowserSearch(run: BrowserSearchRun) {
  if (run.completed) {
    return;
  }
  run.completed = true;
  browserSearchRuns.delete(run.searchId);
  if (browserActiveSearchId === run.searchId) {
    browserActiveSearchId = null;
  }
  emitBrowserSearchCompletedEvent({ searchId: run.searchId });
}

function cancelBrowserSearch(searchId: SearchId): boolean {
  const run = browserSearchRuns.get(searchId);
  if (!run || run.completed) {
    return false;
  }
  for (const timer of run.timers) {
    window.clearTimeout(timer);
  }
  run.timers = [];
  for (const provider of searchProviderOrder("all")) {
    emitBrowserSearchProviderEvent({
      searchId,
      section: {
        provider,
        state: "cancelled",
        results: [],
        error: { code: "cancelled", detail: "Search cancelled.", retryAfterSeconds: null },
      },
    });
  }
  finishBrowserSearch(run);
  return true;
}

function startBrowserSearch(request: SearchRequest): SearchStarted {
  if (browserActiveSearchId) {
    cancelBrowserSearch(browserActiveSearchId);
  }

  const searchId = makeBrowserSearchId();
  const run: BrowserSearchRun = { searchId, timers: [], pending: 0, completed: false };
  browserSearchRuns.set(searchId, run);
  browserActiveSearchId = searchId;
  const providers = searchProviderOrder(request.lens);

  for (const provider of providers) {
    emitBrowserSearchProviderEvent({
      searchId,
      section: { provider, state: "loading", results: [], error: null },
    });
  }

  run.pending = providers.length;
  providers.forEach((provider) => {
    const timer = window.setTimeout(() => {
      if (run.completed || browserActiveSearchId !== searchId) {
        return;
      }
      emitBrowserSearchProviderEvent({
        searchId,
        section: browserSearchSection(provider, request),
      });
      run.pending -= 1;
      if (run.pending === 0) {
        finishBrowserSearch(run);
      }
    }, provider === "local" ? 45 : provider === "soundcloud" ? 180 : 500);
    run.timers.push(timer);
  });

  return { searchId };
}

export function parseProviderSearchEvent(value: unknown): ProviderSearchEvent {
  return providerSearchEventSchema.parse(value);
}

export function parseSearchCompleted(value: unknown): SearchCompleted {
  return searchCompletedSchema.parse(value);
}

export function parseSpotifySetupStatus(value: unknown): SpotifySetupStatus {
  return spotifySetupStatusSchema.parse(value);
}

export async function startSearch(request: SearchRequest): Promise<SearchStarted> {
  try {
    const parsedRequest = searchRequestSchema.parse(request) as SearchRequest;
    if (isPlaybackE2EAdapterEnabled()) {
      return startBrowserSearch(parsedRequest);
    }
    if (!isTauriRuntime()) {
      throw new IpcError("Search requires the native SpotDIY runtime.");
    }
    return searchStartedSchema.parse(await invoke<unknown>("start_search", { request: parsedRequest })) as SearchStarted;
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not start the source search.", error);
  }
}

export async function cancelSearch(): Promise<SearchId | null> {
  if (isPlaybackE2EAdapterEnabled()) {
    const searchId = browserActiveSearchId;
    if (searchId) {
      cancelBrowserSearch(searchId);
    }
    return searchId;
  }
  if (!isTauriRuntime()) {
    throw new IpcError("Search cancellation requires the native SpotDIY runtime.");
  }
  try {
    return z.union([searchIdSchema, z.null()]).parse(await invoke<unknown>("cancel_search"));
  } catch (error) {
    throw new IpcError("SpotDIY could not cancel the source search.", error);
  }
}

export async function subscribeToSearchProviderUpdates(
  listener: SearchProviderUpdateListener,
  onError?: SearchEventErrorListener,
): Promise<() => void> {
  if (isPlaybackE2EAdapterEnabled()) {
    browserSearchProviderListeners.add(listener);
    return () => browserSearchProviderListeners.delete(listener);
  }
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  try {
    return await listen<unknown>(SEARCH_PROVIDER_UPDATE_EVENT, (event) => {
      try {
        listener(parseProviderSearchEvent(event.payload));
      } catch (error) {
        onError?.(new IpcError("SpotDIY received an invalid provider search event.", error));
      }
    });
  } catch (error) {
    throw new IpcError("SpotDIY could not subscribe to provider search updates.", error);
  }
}

export async function subscribeToSearchCompleted(
  listener: SearchCompletedListener,
  onError?: SearchEventErrorListener,
): Promise<() => void> {
  if (isPlaybackE2EAdapterEnabled()) {
    browserSearchCompletedListeners.add(listener);
    return () => browserSearchCompletedListeners.delete(listener);
  }
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  try {
    return await listen<unknown>(SEARCH_COMPLETED_EVENT, (event) => {
      try {
        listener(parseSearchCompleted(event.payload));
      } catch (error) {
        onError?.(new IpcError("SpotDIY received an invalid search completion event.", error));
      }
    });
  } catch (error) {
    throw new IpcError("SpotDIY could not subscribe to search completion updates.", error);
  }
}

export async function getSpotifySetupStatus(): Promise<SpotifySetupStatus> {
  if (!isTauriRuntime()) {
    return {
      enabled: false,
      configured: false,
      available: false,
      state: "disabled",
      market: null,
      detail: "Spotify catalog search is disabled by default.",
    };
  }
  try {
    return parseSpotifySetupStatus(await invoke<unknown>("get_spotify_setup_status"));
  } catch (error) {
    throw new IpcError("SpotDIY could not read Spotify setup status.", error);
  }
}

const spotifyClientIdSchema = z.string().trim().min(1).max(128);
const spotifyMarketSchema = z.string().trim().toUpperCase().regex(/^[A-Z]{2}$/);

export async function beginSpotifyAuthorization(clientId: string, market: string): Promise<SpotifyAuthorizationRequest> {
  try {
    const parsedClientId = spotifyClientIdSchema.parse(clientId);
    const parsedMarket = spotifyMarketSchema.parse(market);
    if (!isTauriRuntime()) {
      throw new IpcError("Spotify authorization requires the native SpotDIY runtime.");
    }
    return spotifyAuthorizationRequestSchema.parse(await invoke<unknown>("begin_spotify_authorization", {
      clientId: parsedClientId,
      market: parsedMarket,
    }));
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not begin Spotify authorization.", error);
  }
}

export async function disconnectSpotify(): Promise<SpotifySetupStatus> {
  if (!isTauriRuntime()) {
    throw new IpcError("Spotify disconnect requires the native SpotDIY runtime.");
  }
  try {
    return parseSpotifySetupStatus(await invoke<unknown>("disconnect_spotify"));
  } catch (error) {
    throw new IpcError("SpotDIY could not disconnect Spotify.", error);
  }
}

const providerUrlHosts: Record<ProviderKind, string[]> = {
  local: [],
  youtube: ["youtube.com", "www.youtube.com", "music.youtube.com", "youtu.be"],
  soundcloud: ["soundcloud.com", "www.soundcloud.com"],
  spotify: ["open.spotify.com", "spotify.com", "www.spotify.com"],
};

function providerUrlIsSafe(provider: ProviderKind, value: string): boolean {
  try {
    const url = new URL(value);
    const hasSensitiveQuery = [...url.searchParams.keys()].some((key) => /token|secret|password|cookie|auth|oauth|code/i.test(key));
    return url.protocol === "https:" && !url.username && !url.password && !hasSensitiveQuery && providerUrlHosts[provider].includes(url.hostname.toLowerCase());
  } catch {
    return false;
  }
}

export async function openProviderResult(provider: ProviderKind, url: string): Promise<void> {
  if (!providerUrlIsSafe(provider, url)) {
    throw new IpcError("SpotDIY refused an unsafe provider result URL.");
  }
  if (!isTauriRuntime()) {
    if (typeof window.open === "function") {
      window.open(url, "_blank", "noopener,noreferrer");
      return;
    }
    throw new IpcError("Opening provider results requires a browser window.");
  }
  try {
    await invoke("open_provider_result", { provider, url });
  } catch (error) {
    throw new IpcError("SpotDIY could not open that provider result.", error);
  }
}

export async function evaluateFusionCandidate(
  candidate: SearchResult,
  targetTrackId: TrackId,
): Promise<FusionEvaluation> {
  try {
    const parsedCandidate = searchResultSchema.parse(candidate) as SearchResult;
    const parsedTargetTrackId = trackIdSchema.parse(targetTrackId);
    if (!isTauriRuntime()) {
      throw new IpcError("Source fusion requires the native SpotDIY runtime.");
    }
    return fusionEvaluationSchema.parse(await invoke<unknown>("evaluate_fusion_candidate", {
      candidate: parsedCandidate,
      targetTrackId: parsedTargetTrackId,
    })) as FusionEvaluation;
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not evaluate that source fusion candidate.", error);
  }
}

export async function acceptFusionCandidate(
  candidate: SearchResult,
  targetTrackId: TrackId,
): Promise<FusionEvaluation> {
  try {
    const parsedCandidate = searchResultSchema.parse(candidate) as SearchResult;
    const parsedTargetTrackId = trackIdSchema.parse(targetTrackId);
    if (!isTauriRuntime()) {
      throw new IpcError("Source fusion requires the native SpotDIY runtime.");
    }
    return fusionEvaluationSchema.parse(await invoke<unknown>("accept_fusion_candidate", {
      candidate: parsedCandidate,
      targetTrackId: parsedTargetTrackId,
    })) as FusionEvaluation;
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not accept that source fusion candidate.", error);
  }
}

export async function setFusionOverride(request: FusionOverrideRequest): Promise<FusionOverride> {
  try {
    const parsedRequest = fusionOverrideRequestSchema.parse(request);
    if (!isTauriRuntime()) {
      throw new IpcError("Source fusion overrides require the native SpotDIY runtime.");
    }
    return fusionOverrideSchema.parse(await invoke<unknown>("set_fusion_override", parsedRequest)) as FusionOverride;
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not persist that source fusion override.", error);
  }
}

export async function clearFusionOverride(request: ClearFusionOverrideRequest): Promise<void> {
  try {
    const parsedRequest = clearFusionOverrideRequestSchema.parse(request);
    if (!isTauriRuntime()) {
      throw new IpcError("Source fusion overrides require the native SpotDIY runtime.");
    }
    await invoke("clear_fusion_override", parsedRequest);
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not clear that source fusion override.", error);
  }
}

export async function getSourceResolution(trackId: TrackId): Promise<SourceResolution> {
  try {
    const parsedTrackId = trackIdSchema.parse(trackId);
    if (!isTauriRuntime()) {
      throw new IpcError("Source resolution requires the native SpotDIY runtime.");
    }
    return sourceResolutionSchema.parse(await invoke<unknown>("get_source_resolution", {
      trackId: parsedTrackId,
    })) as SourceResolution;
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not resolve a playback source.", error);
  }
}

export function parseTrackInspector(value: unknown): TrackInspector {
  return trackInspectorSchema.parse(value) as TrackInspector;
}

export async function getTrackInspector(trackId: TrackId): Promise<TrackInspector> {
  try {
    const parsedTrackId = trackIdSchema.parse(trackId);
    if (!isTauriRuntime()) {
      if (isPlaybackE2EAdapterEnabled()) {
        return parseTrackInspector(browserPreviewTrackInspector(parsedTrackId));
      }
      throw new IpcError("Track Inspector requires the native SpotDIY runtime.");
    }
    return parseTrackInspector(await invoke<unknown>("get_track_inspector", {
      trackId: parsedTrackId,
    }));
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not read that track inspector.", error);
  }
}

export async function subscribeToSpotifyAuthState(
  listener: (status: SpotifySetupStatus) => void,
  onError?: SearchEventErrorListener,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  try {
    return await listen<unknown>(SPOTIFY_AUTH_STATE_EVENT, (event) => {
      try {
        listener(parseSpotifySetupStatus(event.payload));
      } catch (error) {
        onError?.(new IpcError("SpotDIY received an invalid Spotify auth event.", error));
      }
    });
  } catch (error) {
    throw new IpcError("SpotDIY could not subscribe to Spotify auth updates.", error);
  }
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function parsePlaybackSnapshot(value: unknown): PlaybackSnapshot {
  return playbackSnapshotSchema.parse(value);
}

export function parsePlaybackAudioDevices(value: unknown): PlaybackAudioDevice[] {
  return z.array(playbackAudioDeviceSchema).parse(value);
}

export async function getAppStatus(): Promise<AppStatus> {
  if (!isTauriRuntime()) {
    return browserPreviewStatus();
  }

  try {
    const response = await invoke<unknown>("get_app_status");
    return appStatusSchema.parse(response);
  } catch (error) {
    throw new IpcError("SpotDIY could not read its native application status.", error);
  }
}

export async function getSettingsSnapshot(): Promise<SettingsSnapshot> {
  if (!isTauriRuntime()) {
    return browserPreviewSettings();
  }

  try {
    const response = await invoke<unknown>("get_settings_snapshot");
    return settingsSnapshotSchema.parse(response);
  } catch (error) {
    throw new IpcError("SpotDIY could not read its local settings.", error);
  }
}

export async function setSetting(setting: SettingValue): Promise<SettingsSnapshot> {
  try {
    const parsedSetting = settingValueSchema.parse(setting);
    if (!isTauriRuntime()) {
      return setBrowserPreviewSetting(parsedSetting as SettingValue);
    }

    const response = await invoke<unknown>("set_setting", { setting: parsedSetting });
    return settingsSnapshotSchema.parse(response);
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not persist that local setting.", error);
  }
}

function browserPreviewDownloadSnapshot(): DownloadSnapshot {
  return {
    revision: 0,
    tasks: [],
    maxConcurrent: 2,
    downloadsDirectory: null,
    tools: browserPreviewMediaTools(),
  };
}

export function parseDownloadSnapshot(value: unknown): DownloadSnapshot {
  return downloadSnapshotSchema.parse(value) as DownloadSnapshot;
}

export async function getDownloadSnapshot(): Promise<DownloadSnapshot> {
  if (!isTauriRuntime()) {
    return browserPreviewDownloadSnapshot();
  }
  try {
    return parseDownloadSnapshot(await invoke<unknown>("get_download_snapshot"));
  } catch (error) {
    throw new IpcError("SpotDIY could not read its download queue.", error);
  }
}

export async function queueSearchResultDownload(
  result: SearchResult,
  mode: DownloadMode,
): Promise<DownloadTask> {
  try {
    const parsedResult = searchResultSchema.parse(result) as SearchResult;
    const parsedMode = downloadModeSchema.parse(mode) as DownloadMode;
    if (!isTauriRuntime()) {
      throw new IpcError("Downloads require the native SpotDIY desktop runtime.");
    }
    return downloadTaskSchema.parse(await invoke<unknown>("queue_search_result_download", {
      result: parsedResult,
      mode: parsedMode,
    })) as DownloadTask;
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not queue that provider download.", error);
  }
}

export async function queueSourceDownload(
  trackId: TrackId,
  sourceId: SourceId,
  mode: DownloadMode,
): Promise<DownloadTask> {
  try {
    const parsedTrackId = trackIdSchema.parse(trackId);
    const parsedSourceId = sourceIdSchema.parse(sourceId);
    const parsedMode = downloadModeSchema.parse(mode) as DownloadMode;
    if (!isTauriRuntime()) {
      throw new IpcError("Downloads require the native SpotDIY desktop runtime.");
    }
    return downloadTaskSchema.parse(await invoke<unknown>("queue_source_download", {
      trackId: parsedTrackId,
      sourceId: parsedSourceId,
      mode: parsedMode,
    })) as DownloadTask;
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not queue that library source download.", error);
  }
}

export async function cancelDownload(taskId: DownloadTaskId): Promise<DownloadTask> {
  try {
    const parsedTaskId = downloadTaskIdSchema.parse(taskId);
    if (!isTauriRuntime()) {
      throw new IpcError("Download controls require the native SpotDIY runtime.");
    }
    return downloadTaskSchema.parse(await invoke<unknown>("cancel_download", { taskId: parsedTaskId })) as DownloadTask;
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not cancel that download.", error);
  }
}

export async function retryDownload(taskId: DownloadTaskId): Promise<DownloadTask> {
  try {
    const parsedTaskId = downloadTaskIdSchema.parse(taskId);
    if (!isTauriRuntime()) {
      throw new IpcError("Download controls require the native SpotDIY runtime.");
    }
    return downloadTaskSchema.parse(await invoke<unknown>("retry_download", { taskId: parsedTaskId })) as DownloadTask;
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not retry that download.", error);
  }
}

export async function setDownloadConcurrency(maxConcurrent: number): Promise<DownloadSnapshot> {
  try {
    const parsedMaxConcurrent = z.number().int().min(1).max(4).parse(maxConcurrent);
    if (!isTauriRuntime()) {
      throw new IpcError("Download settings require the native SpotDIY runtime.");
    }
    return parseDownloadSnapshot(await invoke<unknown>("set_download_concurrency", {
      maxConcurrent: parsedMaxConcurrent,
    }));
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not update download concurrency.", error);
  }
}

export async function openDownloadLocation(taskId: DownloadTaskId): Promise<void> {
  try {
    const parsedTaskId = downloadTaskIdSchema.parse(taskId);
    if (!isTauriRuntime()) {
      throw new IpcError("Opening download folders requires the native SpotDIY runtime.");
    }
    await invoke("open_download_location", { taskId: parsedTaskId });
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not open that download folder.", error);
  }
}

const downloadDirectorySelectionSchema = z.union([z.string().trim().min(1), z.null()]);

export async function pickDownloadDirectory(): Promise<string | null> {
  if (!isTauriRuntime()) {
    throw new IpcError("Choosing a download folder requires the native SpotDIY runtime.");
  }
  try {
    return downloadDirectorySelectionSchema.parse(
      await open({ directory: true, multiple: false, title: "Choose download folder" }),
    );
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not open the download folder picker.", error);
  }
}

export async function subscribeToDownloadState(
  listener: DownloadSnapshotListener,
  onError?: DownloadSnapshotErrorListener,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  try {
    return await listen<unknown>(DOWNLOAD_STATE_EVENT, (event) => {
      try {
        listener(parseDownloadSnapshot(event.payload));
      } catch (error) {
        onError?.(new IpcError("SpotDIY received an invalid download state event.", error));
      }
    });
  } catch (error) {
    throw new IpcError("SpotDIY could not subscribe to download updates.", error);
  }
}

export async function getLibraryFolders(): Promise<LibraryFolder[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  try {
    return z.array(libraryFolderSchema).parse(await invoke<unknown>("get_library_folders"));
  } catch (error) {
    throw new IpcError("SpotDIY could not read its local library folders.", error);
  }
}

const libraryFolderPathsSchema = z.array(z.string().trim().min(1));
const libraryFolderSelectionSchema = z.union([z.string(), z.array(z.string())]).nullable();

export async function pickLibraryFolders(): Promise<string[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  try {
    const selection = libraryFolderSelectionSchema.parse(
      await open({ directory: true, multiple: true, title: "Choose music folders" }),
    );
    const paths = selection === null ? [] : Array.isArray(selection) ? selection : [selection];
    return libraryFolderPathsSchema.parse(paths);
  } catch (error) {
    throw new IpcError("SpotDIY could not open the library folder picker.", error);
  }
}

export async function addLibraryFolders(paths: string[]): Promise<LibraryFolder[]> {
  try {
    const parsedPaths = libraryFolderPathsSchema.parse(paths);
    if (parsedPaths.length === 0) {
      return [];
    }
    if (!isTauriRuntime()) {
      throw new IpcError("Adding library folders requires the native SpotDIY runtime.");
    }
    return z.array(libraryFolderSchema).parse(await invoke<unknown>("add_library_folders", { paths: parsedPaths }));
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not add those library folders.", error);
  }
}

export async function removeLibraryFolder(folderId: LibraryFolderId): Promise<void> {
  if (!isTauriRuntime()) {
    throw new IpcError("Removing library folders requires the native SpotDIY runtime.");
  }
  try {
    await invoke("remove_library_folder", { folderId });
  } catch (error) {
    throw new IpcError("SpotDIY could not remove that library folder.", error);
  }
}

export async function getLibraryStatus(): Promise<LibraryStatus> {
  if (!isTauriRuntime()) {
    return browserPreviewLibraryStatus();
  }
  try {
    return libraryStatusSchema.parse(await invoke<unknown>("get_library_status"));
  } catch (error) {
    throw new IpcError("SpotDIY could not read local library status.", error);
  }
}

export async function rescanLibraryFolder(folderId: LibraryFolderId): Promise<void> {
  if (!isTauriRuntime()) {
    throw new IpcError("Scanning library folders requires the native SpotDIY runtime.");
  }
  try {
    await invoke("rescan_library_folder", { folderId });
  } catch (error) {
    throw new IpcError("SpotDIY could not start that library scan.", error);
  }
}

export async function rescanAllLibraryFolders(): Promise<void> {
  if (!isTauriRuntime()) {
    throw new IpcError("Scanning library folders requires the native SpotDIY runtime.");
  }
  try {
    await invoke("rescan_all_library_folders");
  } catch (error) {
    throw new IpcError("SpotDIY could not start the library scan.", error);
  }
}

export async function getLibraryPage(request: LibraryPageRequest): Promise<LibraryPage> {
  try {
    const parsedRequest = libraryPageRequestSchema.parse(request);
    if (!isTauriRuntime()) {
      return browserPreviewLibraryPage(parsedRequest);
    }
    return libraryPageSchema.parse(await invoke<unknown>("get_library_page", { request: parsedRequest }));
  } catch (error) {
    throw new IpcError("SpotDIY could not read the local library page.", error);
  }
}

async function invokePlaylist<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  parse: (value: unknown) => T,
  message: string,
): Promise<T> {
  try {
    const response = args ? await invoke<unknown>(command, args) : await invoke<unknown>(command);
    return parse(response);
  } catch (error) {
    const typedError = playlistErrorSchema.safeParse(error);
    if (typedError.success) {
      throw new IpcError(typedError.data.detail, typedError.data);
    }
    throw new IpcError(message, error);
  }
}

function nativePlaylistRequired(message: string): never {
  throw new IpcError(message);
}

export async function listPlaylists(): Promise<Playlist[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return invokePlaylist("list_playlists", undefined, (value) => z.array(playlistSchema).parse(value) as Playlist[], "SpotDIY could not read playlists.");
}

export async function getPlaylist(playlistId: PlaylistId): Promise<Playlist | null> {
  const parsedId = playlistIdSchema.parse(playlistId);
  if (!isTauriRuntime()) {
    return null;
  }
  return invokePlaylist(
    "get_playlist",
    { playlistId: parsedId },
    (value) => z.union([playlistSchema, z.null()]).parse(value) as Playlist | null,
    "SpotDIY could not read that playlist.",
  );
}

export async function createPlaylist(name: string): Promise<Playlist> {
  const parsedName = z.string().trim().min(1).max(120).parse(name);
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Creating playlists requires the native SpotDIY runtime.");
  }
  return invokePlaylist("create_playlist", { name: parsedName }, (value) => playlistSchema.parse(value) as Playlist, "SpotDIY could not create that playlist.");
}

export async function renamePlaylist(playlistId: PlaylistId, name: string): Promise<Playlist> {
  const parsedId = playlistIdSchema.parse(playlistId);
  const parsedName = z.string().trim().min(1).max(120).parse(name);
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Renaming playlists requires the native SpotDIY runtime.");
  }
  return invokePlaylist("rename_playlist", { playlistId: parsedId, name: parsedName }, (value) => playlistSchema.parse(value) as Playlist, "SpotDIY could not rename that playlist.");
}

export async function deletePlaylist(playlistId: PlaylistId): Promise<void> {
  const parsedId = playlistIdSchema.parse(playlistId);
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Deleting playlists requires the native SpotDIY runtime.");
  }
  await invokePlaylist("delete_playlist", { playlistId: parsedId }, () => undefined, "SpotDIY could not delete that playlist.");
}

export async function duplicatePlaylist(playlistId: PlaylistId, requestedName?: string): Promise<Playlist> {
  const parsedId = playlistIdSchema.parse(playlistId);
  const parsedName = requestedName === undefined ? undefined : z.string().trim().min(1).max(120).parse(requestedName);
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Duplicating playlists requires the native SpotDIY runtime.");
  }
  return invokePlaylist("duplicate_playlist", { playlistId: parsedId, requestedName: parsedName ?? null }, (value) => playlistSchema.parse(value) as Playlist, "SpotDIY could not duplicate that playlist.");
}

export async function addPlaylistItem(playlistId: PlaylistId, trackId: TrackId, requestedSourceId: SourceId | null): Promise<PlaylistItem> {
  const parsed = {
    playlistId: playlistIdSchema.parse(playlistId),
    trackId: trackIdSchema.parse(trackId),
    requestedSourceId: requestedSourceId === null ? null : sourceIdSchema.parse(requestedSourceId),
  };
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Adding playlist items requires the native SpotDIY runtime.");
  }
  return invokePlaylist("add_playlist_item", parsed, (value) => playlistItemSchema.parse(value) as PlaylistItem, "SpotDIY could not add that playlist item.");
}

export async function removePlaylistItem(playlistId: PlaylistId, itemId: PlaylistItemId): Promise<void> {
  const args = { playlistId: playlistIdSchema.parse(playlistId), itemId: playlistItemIdSchema.parse(itemId) };
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Removing playlist items requires the native SpotDIY runtime.");
  }
  await invokePlaylist("remove_playlist_item", args, () => undefined, "SpotDIY could not remove that playlist item.");
}

export async function reorderPlaylistItem(playlistId: PlaylistId, itemId: PlaylistItemId, targetPosition: number): Promise<Playlist> {
  const args = {
    playlistId: playlistIdSchema.parse(playlistId),
    itemId: playlistItemIdSchema.parse(itemId),
    targetPosition: z.number().int().nonnegative().parse(targetPosition),
  };
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Reordering playlists requires the native SpotDIY runtime.");
  }
  return invokePlaylist("reorder_playlist_item", args, (value) => playlistSchema.parse(value) as Playlist, "SpotDIY could not reorder that playlist.");
}

export async function createPlaylistBranch(parentPlaylistId: PlaylistId, name: string): Promise<Playlist> {
  const args = { parentPlaylistId: playlistIdSchema.parse(parentPlaylistId), name: z.string().trim().min(1).max(120).parse(name) };
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Creating playlist branches requires the native SpotDIY runtime.");
  }
  return invokePlaylist("create_playlist_branch", args, (value) => playlistSchema.parse(value) as Playlist, "SpotDIY could not create that playlist branch.");
}

export async function getBranchChanges(branchPlaylistId: PlaylistId): Promise<BranchChange[]> {
  const args = { branchPlaylistId: playlistIdSchema.parse(branchPlaylistId) };
  if (!isTauriRuntime()) {
    return [];
  }
  return invokePlaylist("get_branch_changes", args, (value) => z.array(branchChangeSchema).parse(value) as BranchChange[], "SpotDIY could not read branch changes.");
}

export async function mergeBranchChanges(branchPlaylistId: PlaylistId, selectedChanges: BranchChange[]): Promise<BranchMergeResult> {
  const args = { branchPlaylistId: playlistIdSchema.parse(branchPlaylistId), selectedChanges: z.array(branchChangeSchema).min(1).parse(selectedChanges) };
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Merging playlist branches requires the native SpotDIY runtime.");
  }
  return invokePlaylist("merge_branch_changes", args, (value) => branchMergeResultSchema.parse(value) as BranchMergeResult, "SpotDIY could not merge the selected branch changes.");
}

export async function discardPlaylistBranch(branchPlaylistId: PlaylistId): Promise<void> {
  const args = { branchPlaylistId: playlistIdSchema.parse(branchPlaylistId) };
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Discarding playlist branches requires the native SpotDIY runtime.");
  }
  await invokePlaylist("discard_playlist_branch", args, () => undefined, "SpotDIY could not discard that playlist branch.");
}

export async function playPlaylist(playlistId: PlaylistId, itemIds: PlaylistItemId[]): Promise<PlaybackSnapshot> {
  const args = { playlistId: playlistIdSchema.parse(playlistId), itemIds: z.array(playlistItemIdSchema).min(1).parse(itemIds) };
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Playing playlists requires the native SpotDIY runtime.");
  }
  return invokePlayback("play_playlist", args, parsePlaybackSnapshot, "SpotDIY could not start that playlist.");
}

export async function queuePlaylist(playlistId: PlaylistId, itemIds: PlaylistItemId[]): Promise<PlaybackSnapshot> {
  const args = { playlistId: playlistIdSchema.parse(playlistId), itemIds: z.array(playlistItemIdSchema).min(1).parse(itemIds) };
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Queuing playlists requires the native SpotDIY runtime.");
  }
  return invokePlayback("queue_playlist", args, parsePlaybackSnapshot, "SpotDIY could not add that playlist to the queue.");
}

export async function getTrackCollectionStates(trackIds: TrackId[]): Promise<TrackCollectionState[]> {
  const parsedIds = z.array(trackIdSchema).max(100).parse(trackIds);
  if (!isTauriRuntime()) {
    return parsedIds.map((trackId) => ({ trackId, liked: false, rating: null, tags: [], playlistMemberships: [], inInbox: false }));
  }
  return invokePlaylist("get_track_collection_states", { trackIds: parsedIds }, (value) => z.array(trackCollectionStateSchema).parse(value) as TrackCollectionState[], "SpotDIY could not read track collection state.");
}

export async function setTrackLiked(trackId: TrackId, liked: boolean): Promise<boolean> {
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Likes require the native SpotDIY runtime.");
  }
  return invokePlaylist("set_track_liked", { trackId: trackIdSchema.parse(trackId), liked: z.boolean().parse(liked) }, (value) => z.boolean().parse(value), "SpotDIY could not update that like.");
}

export async function setTrackRating(trackId: TrackId, rating: number | null): Promise<number | null> {
  const parsedRating = rating === null ? null : z.number().int().min(1).max(5).parse(rating);
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Ratings require the native SpotDIY runtime.");
  }
  return invokePlaylist("set_track_rating", { trackId: trackIdSchema.parse(trackId), rating: parsedRating }, (value) => z.number().int().min(1).max(5).nullable().parse(value), "SpotDIY could not update that rating.");
}

export async function listTags(): Promise<Tag[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return invokePlaylist("list_tags", undefined, (value) => z.array(tagSchema).parse(value) as Tag[], "SpotDIY could not read tags.");
}

export async function createTag(name: string): Promise<Tag> {
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Tags require the native SpotDIY runtime.");
  }
  return invokePlaylist("create_tag", { name: z.string().trim().min(1).max(64).parse(name) }, (value) => tagSchema.parse(value) as Tag, "SpotDIY could not create that tag.");
}

export async function renameTag(tagId: TagId, name: string): Promise<Tag> {
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Tags require the native SpotDIY runtime.");
  }
  return invokePlaylist("rename_tag", { tagId: tagIdSchema.parse(tagId), name: z.string().trim().min(1).max(64).parse(name) }, (value) => tagSchema.parse(value) as Tag, "SpotDIY could not rename that tag.");
}

export async function deleteTag(tagId: TagId): Promise<void> {
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Tags require the native SpotDIY runtime.");
  }
  await invokePlaylist("delete_tag", { tagId: tagIdSchema.parse(tagId) }, () => undefined, "SpotDIY could not delete that tag.");
}

export async function addTrackTag(trackId: TrackId, tagId: TagId): Promise<void> {
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Track tags require the native SpotDIY runtime.");
  }
  await invokePlaylist("add_track_tag", { trackId: trackIdSchema.parse(trackId), tagId: tagIdSchema.parse(tagId) }, () => undefined, "SpotDIY could not apply that tag.");
}

export async function removeTrackTag(trackId: TrackId, tagId: TagId): Promise<void> {
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("Track tags require the native SpotDIY runtime.");
  }
  await invokePlaylist("remove_track_tag", { trackId: trackIdSchema.parse(trackId), tagId: tagIdSchema.parse(tagId) }, () => undefined, "SpotDIY could not remove that tag.");
}

export async function addTrackToInbox(trackId: TrackId): Promise<PlaylistItem> {
  if (!isTauriRuntime()) {
    return nativePlaylistRequired("The Inbox requires the native SpotDIY runtime.");
  }
  return invokePlaylist("add_track_to_inbox", { trackId: trackIdSchema.parse(trackId) }, (value) => playlistItemSchema.parse(value) as PlaylistItem, "SpotDIY could not add that track to the Inbox.");
}

export async function getLyrics(trackId: TrackId, currentSourceId: SourceId | null = null): Promise<LyricsDocument | null> {
  const parsedTrackId = trackIdSchema.parse(trackId);
  const parsedSourceId = currentSourceId === null ? null : sourceIdSchema.parse(currentSourceId);
  if (!isTauriRuntime()) {
    return null;
  }
  return invokeLyrics(
    "get_lyrics",
    { trackId: parsedTrackId, currentSourceId: parsedSourceId },
    (value) => value === null ? null : lyricsDocumentSchema.parse(value),
    "SpotDIY could not read lyrics for that track.",
  );
}

export async function saveManualLyrics(
  trackId: TrackId,
  mode: ManualLyricsMode,
  text: string,
): Promise<LyricsDocument> {
  if (!isTauriRuntime()) {
    throw new IpcError("Editing lyrics requires the native SpotDIY runtime.");
  }
  const parsedTrackId = trackIdSchema.parse(trackId);
  const parsedMode = manualLyricsModeSchema.parse(mode);
  const parsedText = z.string().parse(text);
  return invokeLyrics(
    "save_manual_lyrics",
    { trackId: parsedTrackId, mode: parsedMode, text: parsedText },
    (value) => lyricsDocumentSchema.parse(value),
    "SpotDIY could not save those lyrics.",
  );
}

export async function deleteManualLyrics(trackId: TrackId): Promise<void> {
  if (!isTauriRuntime()) {
    throw new IpcError("Editing lyrics requires the native SpotDIY runtime.");
  }
  await invokeLyrics(
    "delete_manual_lyrics",
    { trackId: trackIdSchema.parse(trackId) },
    () => undefined,
    "SpotDIY could not delete the manual lyrics.",
  );
}

export async function pickAndImportLyricsFile(trackId: TrackId): Promise<LyricsDocument> {
  if (!isTauriRuntime()) {
    throw new IpcError("Lyrics file import requires the native SpotDIY runtime.");
  }
  return invokeLyrics(
    "pick_and_import_lyrics_file",
    { trackId: trackIdSchema.parse(trackId) },
    (value) => lyricsDocumentSchema.parse(value),
    "SpotDIY could not import that lyrics file.",
  );
}

export async function findLrclibBest(trackId: TrackId): Promise<LyricsDocument> {
  if (!isTauriRuntime()) {
    throw new IpcError("Online lyrics lookup requires the native SpotDIY runtime.");
  }
  return invokeLyrics(
    "find_lrclib_best",
    { trackId: trackIdSchema.parse(trackId) },
    (value) => lyricsDocumentSchema.parse(value),
    "SpotDIY could not look up lyrics from LRCLIB.",
  );
}

export async function searchLrclib(trackId: TrackId): Promise<LyricsCandidate[]> {
  if (!isTauriRuntime()) {
    throw new IpcError("Online lyrics search requires the native SpotDIY runtime.");
  }
  return invokeLyrics(
    "search_lrclib",
    { trackId: trackIdSchema.parse(trackId) },
    (value) => z.array(lyricsCandidateSchema).parse(value),
    "SpotDIY could not search LRCLIB.",
  );
}

export async function selectLrclibCandidate(trackId: TrackId, providerRecordId: number): Promise<LyricsDocument> {
  if (!isTauriRuntime()) {
    throw new IpcError("Online lyrics selection requires the native SpotDIY runtime.");
  }
  const parsedRecordId = z.number().int().positive().parse(providerRecordId);
  return invokeLyrics(
    "select_lrclib_candidate",
    { trackId: trackIdSchema.parse(trackId), providerRecordId: parsedRecordId },
    (value) => lyricsDocumentSchema.parse(value),
    "SpotDIY could not select those LRCLIB lyrics.",
  );
}

export async function clearCachedLrclib(trackId: TrackId): Promise<void> {
  if (!isTauriRuntime()) {
    throw new IpcError("Lyrics cache controls require the native SpotDIY runtime.");
  }
  await invokeLyrics(
    "clear_cached_lrclib",
    { trackId: trackIdSchema.parse(trackId) },
    () => undefined,
    "SpotDIY could not clear the LRCLIB cache.",
  );
}

export async function listBookmarks(trackId: TrackId): Promise<Bookmark[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return invokeBookmarks(
    "list_bookmarks",
    { trackId: trackIdSchema.parse(trackId) },
    (value) => z.array(bookmarkSchema).parse(value),
    "SpotDIY could not read bookmarks for that track.",
  );
}

export async function createBookmark(trackId: TrackId, positionMs: number, note: string): Promise<Bookmark> {
  if (!isTauriRuntime()) {
    throw new IpcError("Bookmarks require the native SpotDIY runtime.");
  }
  return invokeBookmarks(
    "create_bookmark",
    {
      trackId: trackIdSchema.parse(trackId),
      positionMs: playbackSeekSchema.parse(positionMs),
      note: z.string().parse(note),
    },
    (value) => bookmarkSchema.parse(value),
    "SpotDIY could not create that bookmark.",
  );
}

export async function updateBookmark(bookmarkId: BookmarkId, positionMs: number, note: string): Promise<Bookmark> {
  if (!isTauriRuntime()) {
    throw new IpcError("Bookmarks require the native SpotDIY runtime.");
  }
  return invokeBookmarks(
    "update_bookmark",
    {
      bookmarkId: bookmarkIdSchema.parse(bookmarkId),
      positionMs: playbackSeekSchema.parse(positionMs),
      note: z.string().parse(note),
    },
    (value) => bookmarkSchema.parse(value),
    "SpotDIY could not update that bookmark.",
  );
}

export async function deleteBookmark(bookmarkId: BookmarkId): Promise<void> {
  if (!isTauriRuntime()) {
    throw new IpcError("Bookmarks require the native SpotDIY runtime.");
  }
  await invokeBookmarks(
    "delete_bookmark",
    { bookmarkId: bookmarkIdSchema.parse(bookmarkId) },
    () => undefined,
    "SpotDIY could not delete that bookmark.",
  );
}

export async function revealLocalFile(sourceId: SourceId): Promise<void> {
  if (!isTauriRuntime()) {
    throw new IpcError("File locations require the native SpotDIY runtime.");
  }
  try {
    await invoke("reveal_local_file", { sourceId });
  } catch (error) {
    throw new IpcError("SpotDIY could not reveal that local file.", error);
  }
}

export function parseScanProgress(value: unknown): ScanProgress {
  return scanProgressSchema.parse(value) as ScanProgress;
}

async function invokePlayback<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  parse: (value: unknown) => T,
  message: string,
): Promise<T> {
  try {
    const response = args ? await invoke<unknown>(command, args) : await invoke<unknown>(command);
    return parse(response);
  } catch (error) {
    const typedError = playbackErrorSchema.safeParse(error);
    if (typedError.success) {
      throw new IpcError(typedError.data.summary, typedError.data);
    }
    throw new IpcError(message, error);
  }
}

async function invokeLyrics<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  parse: (value: unknown) => T,
  message: string,
): Promise<T> {
  try {
    const response = args ? await invoke<unknown>(command, args) : await invoke<unknown>(command);
    return parse(response);
  } catch (error) {
    const typedError = lyricsErrorSchema.safeParse(error);
    if (typedError.success) {
      throw new IpcError(typedError.data.detail, typedError.data);
    }
    throw new IpcError(message, error);
  }
}

async function invokeBookmarks<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  parse: (value: unknown) => T,
  message: string,
): Promise<T> {
  try {
    const response = args ? await invoke<unknown>(command, args) : await invoke<unknown>(command);
    return parse(response);
  } catch (error) {
    const typedError = bookmarkErrorSchema.safeParse(error);
    if (typedError.success) {
      throw new IpcError(typedError.data.detail, typedError.data);
    }
    throw new IpcError(message, error);
  }
}

function e2ePlaybackSnapshot(): PlaybackSnapshot {
  ensureE2EPlaybackState();
  return e2eAdapterState.snapshot;
}

function e2eQueueSnapshot(): PlaybackSnapshot {
  return setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, {
    queueLength: e2eAdapterState.activeQueue.length,
    queueIndex: e2eAdapterState.currentIndex,
  }));
}

function e2eAppendTrack(request: TrackPlaybackRequest, insertNext: boolean): PlaybackSnapshot {
  ensureE2EPlaybackState();
  resolveE2EPlaybackRequest(request);

  if (e2eAdapterState.canonicalQueue.length === 0) {
    e2eAdapterState.canonicalQueue = [request];
    e2eAdapterState.canonicalQueueIds = [nextE2EQueueEntryId()];
    e2eAdapterState.activeQueue = [request];
    e2eAdapterState.activeQueueIds = [...e2eAdapterState.canonicalQueueIds];
    e2eAdapterState.currentIndex = null;
    return e2eQueueSnapshot();
  }

  const currentIndex = e2eAdapterState.currentIndex;
  const current = currentIndex === null ? null : e2eAdapterState.activeQueue[currentIndex] ?? null;
  const currentCanonicalIndex = current
    ? e2eAdapterState.canonicalQueue.findIndex((entry) => playbackRequestKey(entry) === playbackRequestKey(current))
    : -1;
  const insertionIndex = insertNext
    ? currentCanonicalIndex < 0 ? 0 : currentCanonicalIndex + 1
    : e2eAdapterState.canonicalQueue.length;
  e2eAdapterState.canonicalQueue.splice(insertionIndex, 0, request);
  e2eAdapterState.canonicalQueueIds.splice(insertionIndex, 0, nextE2EQueueEntryId());
  rebuildE2EActiveQueue();
  return e2eQueueSnapshot();
}

export async function getPlaybackSnapshot(): Promise<PlaybackSnapshot> {
  if (isPlaybackE2EAdapterEnabled()) {
    return e2ePlaybackSnapshot();
  }
  if (!isTauriRuntime()) {
    return {
      ...emptyPlaybackSnapshot(),
      backendHealth: {
        ready: false,
        connected: false,
        detail: "Playback controls are available in the native SpotDIY app.",
        recoveryAction: null,
      },
    };
  }

  return invokePlayback("get_playback_snapshot", undefined, parsePlaybackSnapshot, "SpotDIY could not read the playback state.");
}

export async function playTrack(request: TrackPlaybackRequest): Promise<PlaybackSnapshot> {
  try {
    const parsedRequest = trackPlaybackRequestSchema.parse(request);
    if (isPlaybackE2EAdapterEnabled()) {
      const track = resolveE2EPlaybackRequest(parsedRequest);
      e2eAdapterState.canonicalQueue = [parsedRequest];
      e2eAdapterState.canonicalQueueIds = [nextE2EQueueEntryId()];
      e2eAdapterState.activeQueue = [parsedRequest];
      e2eAdapterState.activeQueueIds = [...e2eAdapterState.canonicalQueueIds];
      e2eAdapterState.currentIndex = 0;
      return scheduleE2ELoad(0, false, 0) ?? makePlaybackSnapshotForTrack(track, { phase: "loading" });
    }
    if (!isTauriRuntime()) {
      throw new IpcError("Playback controls require the native SpotDIY runtime.");
    }
    return await invokePlayback("play_track", {
      trackId: parsedRequest.trackId,
      sourceId: parsedRequest.sourceId,
    }, parsePlaybackSnapshot, "SpotDIY could not start playback for that track.");
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not start playback for that track.", error);
  }
}

export async function enqueueTrack(request: TrackPlaybackRequest): Promise<PlaybackSnapshot> {
  try {
    const parsedRequest = trackPlaybackRequestSchema.parse(request);
    if (isPlaybackE2EAdapterEnabled()) {
      return e2eAppendTrack(parsedRequest, false);
    }
    if (!isTauriRuntime()) {
      throw new IpcError("Adding to the queue requires the native SpotDIY runtime.");
    }
    return await invokePlayback("enqueue_track", {
      trackId: parsedRequest.trackId,
      sourceId: parsedRequest.sourceId,
    }, parsePlaybackSnapshot, "SpotDIY could not add that track to the queue.");
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not add that track to the queue.", error);
  }
}

export async function playTrackNext(request: TrackPlaybackRequest): Promise<PlaybackSnapshot> {
  try {
    const parsedRequest = trackPlaybackRequestSchema.parse(request);
    if (isPlaybackE2EAdapterEnabled()) {
      return e2eAppendTrack(parsedRequest, true);
    }
    if (!isTauriRuntime()) {
      throw new IpcError("Playing next requires the native SpotDIY runtime.");
    }
    return await invokePlayback("play_track_next", {
      trackId: parsedRequest.trackId,
      sourceId: parsedRequest.sourceId,
    }, parsePlaybackSnapshot, "SpotDIY could not queue that track to play next.");
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not queue that track to play next.", error);
  }
}

export async function togglePlayPause(): Promise<PlaybackSnapshot> {
  if (isPlaybackE2EAdapterEnabled()) {
    ensureE2EPlaybackState();
    if (e2eAdapterState.snapshot.phase === "playing") {
      clearE2ETimers();
      return setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, { phase: "paused" }));
    }
    if (e2eAdapterState.snapshot.phase === "paused" || e2eAdapterState.snapshot.phase === "ended") {
      const next = setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, {
        phase: "playing",
        positionMs: e2eAdapterState.snapshot.phase === "ended" ? 0 : e2eAdapterState.snapshot.positionMs,
      }));
      startE2EPlaybackTicker();
      return next;
    }
    if (e2eAdapterState.activeQueue.length > 0) {
      return scheduleE2ELoad(e2eAdapterState.currentIndex ?? 0);
    }
    throw new IpcError("The playback queue is empty.");
  }
  if (!isTauriRuntime()) {
    throw new IpcError("Playback controls require the native SpotDIY runtime.");
  }
  return invokePlayback("toggle_play_pause", undefined, parsePlaybackSnapshot, "SpotDIY could not toggle playback.");
}

export async function seekPlayback(positionMs: number): Promise<PlaybackSnapshot> {
  try {
    const parsedPosition = playbackSeekSchema.parse(positionMs);
    if (isPlaybackE2EAdapterEnabled()) {
      ensureE2EPlaybackState();
      if (e2eAdapterState.snapshot.currentTrackId === null) {
        throw new IpcError("There is no active track to seek.");
      }
      const durationMs = e2eAdapterState.snapshot.durationMs ?? parsedPosition;
      const clamped = Math.min(durationMs, parsedPosition);
      const resumePhase = e2eAdapterState.snapshot.phase === "paused" ? "paused" : "playing";
      clearE2ETimers();
      setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, { phase: "seeking", positionMs: clamped }));
      return await new Promise<PlaybackSnapshot>((resolve) => {
        e2eAdapterState.transitionTimer = window.setTimeout(() => {
          const next = setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, { phase: resumePhase, positionMs: clamped }));
          if (next.phase === "playing") {
            startE2EPlaybackTicker();
          }
          resolve(next);
        }, 80);
      });
    }
    if (!isTauriRuntime()) {
      throw new IpcError("Seeking requires the native SpotDIY runtime.");
    }
    return await invokePlayback("seek_playback", { positionMs: parsedPosition }, parsePlaybackSnapshot, "SpotDIY could not seek within that track.");
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not seek within that track.", error);
  }
}

export async function setAbLoopA(): Promise<PlaybackSnapshot> {
  if (!isTauriRuntime()) {
    throw new IpcError("A/B loop controls require the native SpotDIY runtime.");
  }
  return invokePlayback("set_ab_loop_a", undefined, parsePlaybackSnapshot, "SpotDIY could not set the A loop point.");
}

export async function setAbLoopB(): Promise<PlaybackSnapshot> {
  if (!isTauriRuntime()) {
    throw new IpcError("A/B loop controls require the native SpotDIY runtime.");
  }
  return invokePlayback("set_ab_loop_b", undefined, parsePlaybackSnapshot, "SpotDIY could not set the B loop point.");
}

export async function clearAbLoop(): Promise<PlaybackSnapshot> {
  if (!isTauriRuntime()) {
    throw new IpcError("A/B loop controls require the native SpotDIY runtime.");
  }
  return invokePlayback("clear_ab_loop", undefined, parsePlaybackSnapshot, "SpotDIY could not clear the A/B loop.");
}

export async function saveAbLoopPreset(trackId: TrackId, name: string): Promise<AbLoopPreset> {
  if (!isTauriRuntime()) {
    throw new IpcError("A/B loop presets require the native SpotDIY runtime.");
  }
  return invokePlayback(
    "save_ab_loop_preset",
    { trackId: trackIdSchema.parse(trackId), name: z.string().parse(name) },
    (value) => abLoopPresetSchema.parse(value),
    "SpotDIY could not save that A/B loop preset.",
  );
}

export async function listAbLoopPresets(trackId: TrackId): Promise<AbLoopPreset[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return invokePlayback(
    "list_ab_loop_presets",
    { trackId: trackIdSchema.parse(trackId) },
    (value) => z.array(abLoopPresetSchema).parse(value),
    "SpotDIY could not read A/B loop presets.",
  );
}

export async function applyAbLoopPreset(presetId: AbLoopPresetId): Promise<PlaybackSnapshot> {
  if (!isTauriRuntime()) {
    throw new IpcError("A/B loop presets require the native SpotDIY runtime.");
  }
  return invokePlayback(
    "apply_ab_loop_preset",
    { presetId: abLoopPresetIdSchema.parse(presetId) },
    parsePlaybackSnapshot,
    "SpotDIY could not apply that A/B loop preset.",
  );
}

export async function deleteAbLoopPreset(presetId: AbLoopPresetId): Promise<void> {
  if (!isTauriRuntime()) {
    throw new IpcError("A/B loop presets require the native SpotDIY runtime.");
  }
  await invokePlayback(
    "delete_ab_loop_preset",
    { presetId: abLoopPresetIdSchema.parse(presetId) },
    () => undefined,
    "SpotDIY could not delete that A/B loop preset.",
  );
}

export async function nextTrack(): Promise<PlaybackSnapshot> {
  if (isPlaybackE2EAdapterEnabled()) {
    return advanceE2EQueue(1);
  }
  if (!isTauriRuntime()) {
    throw new IpcError("Skipping tracks requires the native SpotDIY runtime.");
  }
  return invokePlayback("next_track", undefined, parsePlaybackSnapshot, "SpotDIY could not skip to the next track.");
}

export async function previousTrack(): Promise<PlaybackSnapshot> {
  if (isPlaybackE2EAdapterEnabled()) {
    ensureE2EPlaybackState();
    if (e2eAdapterState.snapshot.positionMs > 3_000 || e2eAdapterState.currentIndex === null) {
      return setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, { positionMs: 0 }));
    }
    return advanceE2EQueue(-1);
  }
  if (!isTauriRuntime()) {
    throw new IpcError("Skipping tracks requires the native SpotDIY runtime.");
  }
  return invokePlayback("previous_track", undefined, parsePlaybackSnapshot, "SpotDIY could not return to the previous track.");
}

export async function setPlaybackVolume(volumePercent: number): Promise<PlaybackSnapshot> {
  try {
    const parsedVolume = playbackVolumeSchema.parse(volumePercent);
    if (isPlaybackE2EAdapterEnabled()) {
      return setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, { volumePercent: parsedVolume }));
    }
    if (!isTauriRuntime()) {
      throw new IpcError("Volume control requires the native SpotDIY runtime.");
    }
    return await invokePlayback("set_playback_volume", { volumePercent: parsedVolume }, parsePlaybackSnapshot, "SpotDIY could not update the playback volume.");
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not update the playback volume.", error);
  }
}

export async function setPlaybackMuted(muted: boolean): Promise<PlaybackSnapshot> {
  if (isPlaybackE2EAdapterEnabled()) {
    return setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, { muted }));
  }
  if (!isTauriRuntime()) {
    throw new IpcError("Mute control requires the native SpotDIY runtime.");
  }
  return invokePlayback("set_playback_muted", { muted }, parsePlaybackSnapshot, "SpotDIY could not update the mute state.");
}

export async function setRepeatMode(repeatMode: RepeatMode): Promise<PlaybackSnapshot> {
  try {
    const parsedRepeatMode = repeatModeSchema.parse(repeatMode);
    if (isPlaybackE2EAdapterEnabled()) {
      return setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, { repeatMode: parsedRepeatMode as RepeatMode }));
    }
    if (!isTauriRuntime()) {
      throw new IpcError("Repeat control requires the native SpotDIY runtime.");
    }
    return await invokePlayback("set_repeat_mode", { repeatMode: parsedRepeatMode }, parsePlaybackSnapshot, "SpotDIY could not update repeat mode.");
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not update repeat mode.", error);
  }
}

export async function setShuffleEnabled(enabled: boolean): Promise<PlaybackSnapshot> {
  if (isPlaybackE2EAdapterEnabled()) {
    ensureE2EPlaybackState();
    e2eAdapterState.snapshot = withNextRevision(e2eAdapterState.snapshot, { shuffleEnabled: enabled });
    rebuildE2EActiveQueue();
    return e2eQueueSnapshot();
  }
  if (!isTauriRuntime()) {
    throw new IpcError("Shuffle control requires the native SpotDIY runtime.");
  }
  return invokePlayback("set_shuffle_enabled", { enabled }, parsePlaybackSnapshot, "SpotDIY could not update shuffle mode.");
}

export async function getAudioDevices(): Promise<PlaybackAudioDevice[]> {
  if (isPlaybackE2EAdapterEnabled()) {
    ensureE2EPlaybackState();
    return e2eDevices;
  }
  if (!isTauriRuntime()) {
    return [];
  }
  return invokePlayback("get_audio_devices", undefined, parsePlaybackAudioDevices, "SpotDIY could not read the playback audio devices.");
}

export async function setAudioDevice(name: string): Promise<PlaybackSnapshot> {
  try {
    const parsedName = playbackDeviceNameSchema.parse(name);
    if (isPlaybackE2EAdapterEnabled()) {
      const device = e2eDevices.find((entry) => entry.name === parsedName);
      if (!device) {
        throw new IpcError("SpotDIY could not find that audio device in the playback adapter.");
      }
      selectE2EDevice(device.name);
      return setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, {
        selectedAudioDevice: device.name,
      }));
    }
    if (!isTauriRuntime()) {
      throw new IpcError("Audio device switching requires the native SpotDIY runtime.");
    }
    return await invokePlayback("set_audio_device", { name: parsedName }, parsePlaybackSnapshot, "SpotDIY could not switch the audio device.");
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not switch the audio device.", error);
  }
}

export async function switchPlaybackSource(request: { trackId: TrackId; sourceId: SourceId }): Promise<PlaybackSnapshot> {
  try {
    const parsedRequest = playbackSourceRequestSchema.parse(request);
    if (isPlaybackE2EAdapterEnabled()) {
      const track = resolveE2EPlaybackRequest(parsedRequest);
      ensureE2EPlaybackState();
      const currentIndex = e2eAdapterState.currentIndex;
      const currentRequest = currentIndex === null
        ? null
        : e2eAdapterState.activeQueue[currentIndex] ?? null;
      const currentEntryId = currentIndex === null
        ? null
        : e2eAdapterState.activeQueueIds[currentIndex] ?? null;
      if (currentIndex === null || currentRequest === null || currentRequest.trackId !== parsedRequest.trackId) {
        throw new IpcError("Playback source switching requires the current track.");
      }
      const priorPosition = e2eAdapterState.snapshot.positionMs;
      const paused = e2eAdapterState.snapshot.phase === "paused";
      const canonicalIndex = currentEntryId === null
        ? e2eAdapterState.canonicalQueue.findIndex((entry) => playbackRequestKey(entry) === playbackRequestKey(currentRequest))
        : e2eAdapterState.canonicalQueueIds.indexOf(currentEntryId);
      if (canonicalIndex < 0) {
        throw new IpcError("Playback source switching could not find the current queue entry.");
      }
      e2eAdapterState.canonicalQueue[canonicalIndex] = parsedRequest;
      e2eAdapterState.activeQueue[currentIndex] = parsedRequest;
      return scheduleE2ELoad(currentIndex, paused, Math.min(priorPosition, track.durationMs ?? priorPosition));
    }
    if (!isTauriRuntime()) {
      throw new IpcError("Playback source switching requires the native SpotDIY runtime.");
    }
    return await invokePlayback("switch_playback_source", {
      trackId: parsedRequest.trackId,
      sourceId: parsedRequest.sourceId,
    }, parsePlaybackSnapshot, "SpotDIY could not switch playback sources.");
  } catch (error) {
    if (error instanceof IpcError) {
      throw error;
    }
    throw new IpcError("SpotDIY could not switch playback sources.", error);
  }
}

export async function retryPlaybackBackend(): Promise<PlaybackSnapshot> {
  if (isPlaybackE2EAdapterEnabled()) {
    ensureE2EPlaybackState();
    const recovering = setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, {
      phase: "recovering",
      recovering: true,
      error: null,
      backendHealth: {
        ready: false,
        connected: false,
        detail: "Reconnecting to the playback backend…",
        recoveryAction: "Retry the playback backend",
      },
    }));
    return new Promise<PlaybackSnapshot>((resolve) => {
      e2eAdapterState.transitionTimer = window.setTimeout(() => {
        const baseTrack = currentE2ETrack();
        const next = baseTrack
          ? queueSnapshotForCurrentTrack("paused", e2eAdapterState.snapshot.positionMs)
          : withNextRevision(recovering, {
            phase: "idle",
            recovering: false,
            backendHealth: {
              ready: true,
              connected: true,
              detail: null,
              recoveryAction: null,
            },
          });
        const resolved = setE2ESnapshot(withNextRevision(next, {
          recovering: false,
          backendHealth: {
            ready: true,
            connected: true,
            detail: null,
            recoveryAction: null,
          },
          error: null,
        }));
        resolve(resolved);
      }, 160);
    });
  }
  if (!isTauriRuntime()) {
    throw new IpcError("Playback retry requires the native SpotDIY runtime.");
  }
  return invokePlayback("retry_playback_backend", undefined, parsePlaybackSnapshot, "SpotDIY could not retry the playback backend.");
}

export async function clearPlaybackQueue(): Promise<PlaybackSnapshot> {
  if (isPlaybackE2EAdapterEnabled()) {
    clearE2ETimers();
    e2eAdapterState.canonicalQueue = [];
    e2eAdapterState.activeQueue = [];
    e2eAdapterState.canonicalQueueIds = [];
    e2eAdapterState.activeQueueIds = [];
    e2eAdapterState.currentIndex = null;
    return setE2ESnapshot(withNextRevision(e2eAdapterState.snapshot, {
      ...emptyPlaybackSnapshot(),
      backendHealth: {
        ready: true,
        connected: true,
        detail: null,
        recoveryAction: null,
      },
    }));
  }
  if (!isTauriRuntime()) {
    throw new IpcError("Clearing the queue requires the native SpotDIY runtime.");
  }
  return invokePlayback("clear_playback_queue", undefined, parsePlaybackSnapshot, "SpotDIY could not clear the playback queue.");
}

function browserPreviewQueueWorkspace(): QueueWorkspace {
  ensureE2EPlaybackState();
  const currentIndex = e2eAdapterState.currentIndex;
  const current = currentIndex === null ? null : e2eAdapterState.activeQueue[currentIndex] ?? null;
  const makeEntry = (request: TrackPlaybackRequest, id: QueueEntryId, position: number): QueueWorkspace["later"][number] => {
    const track = e2eTrackMap.get(request.trackId);
    return {
      id,
      trackId: request.trackId,
      requestedSourceId: request.sourceId,
      section: "later",
      position,
      pinned: false,
      title: track?.title ?? null,
      artists: track?.artists ?? [],
      album: track?.album ?? null,
    };
  };
  const later = e2eAdapterState.activeQueue
    .map((request, index) => ({ request, id: e2eAdapterState.activeQueueIds[index], index }))
    .filter(({ index }) => index !== currentIndex)
    .map(({ request, id }, position) => makeEntry(request, id, position));
  const currentEntry = current && currentIndex !== null
    ? makeEntry(current, e2eAdapterState.activeQueueIds[currentIndex], 0)
    : null;
  return {
    revision: e2eAdapterState.snapshot.revision,
    current: currentEntry,
    upNext: [],
    later,
    autoplay: [],
    currentPositionMs: e2eAdapterState.snapshot.positionMs,
    repeatMode: e2eAdapterState.snapshot.repeatMode,
    shuffleEnabled: e2eAdapterState.snapshot.shuffleEnabled,
  };
}

export function parseQueueWorkspace(value: unknown): QueueWorkspace {
  return queueWorkspaceSchema.parse(value) as QueueWorkspace;
}

export async function getQueueWorkspace(): Promise<QueueWorkspace> {
  if (isPlaybackE2EAdapterEnabled()) {
    return browserPreviewQueueWorkspace();
  }
  if (!isTauriRuntime()) {
    return {
      revision: 0,
      current: null,
      upNext: [],
      later: [],
      autoplay: [],
      currentPositionMs: 0,
      repeatMode: "off",
      shuffleEnabled: false,
    };
  }
  return invokePlayback("get_queue_workspace", undefined, parseQueueWorkspace, "SpotDIY could not read the queue workspace.");
}

export async function moveQueueEntry(entryId: QueueEntryId, section: QueueSection, targetIndex: number): Promise<QueueWorkspace> {
  const args = {
    entryId: queueEntryIdSchema.parse(entryId),
    section: queueSectionSchema.parse(section),
    targetIndex: z.number().int().nonnegative().parse(targetIndex),
  };
  if (!isTauriRuntime()) {
    throw new IpcError("Queue editing requires the native SpotDIY runtime.");
  }
  return invokePlayback("move_queue_entry", args, parseQueueWorkspace, "SpotDIY could not move that queue entry.");
}

export async function removeQueueEntry(entryId: QueueEntryId): Promise<QueueWorkspace> {
  if (!isTauriRuntime()) {
    throw new IpcError("Queue editing requires the native SpotDIY runtime.");
  }
  return invokePlayback("remove_queue_entry", { entryId: queueEntryIdSchema.parse(entryId) }, parseQueueWorkspace, "SpotDIY could not remove that queue entry.");
}

export async function setQueueEntryPinned(entryId: QueueEntryId, pinned: boolean): Promise<QueueWorkspace> {
  if (!isTauriRuntime()) {
    throw new IpcError("Queue editing requires the native SpotDIY runtime.");
  }
  return invokePlayback("set_queue_entry_pinned", {
    entryId: queueEntryIdSchema.parse(entryId),
    pinned: z.boolean().parse(pinned),
  }, parseQueueWorkspace, "SpotDIY could not update that queue pin.");
}

export async function clearQueueSection(section: QueueSection): Promise<QueueWorkspace> {
  if (!isTauriRuntime()) {
    throw new IpcError("Queue editing requires the native SpotDIY runtime.");
  }
  return invokePlayback("clear_queue_section", { section: queueSectionSchema.parse(section) }, parseQueueWorkspace, "SpotDIY could not clear that queue section.");
}

export async function saveQueueSnapshot(name: string): Promise<QueueSnapshot> {
  if (!isTauriRuntime()) {
    throw new IpcError("Queue snapshots require the native SpotDIY runtime.");
  }
  return invokePlayback("save_queue_snapshot", { name: z.string().trim().min(1).max(80).parse(name) }, (value) => queueSnapshotSchema.parse(value) as QueueSnapshot, "SpotDIY could not save the queue snapshot.");
}

export async function listQueueSnapshots(): Promise<QueueSnapshotSummary[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return invokePlayback("list_queue_snapshots", undefined, (value) => z.array(queueSnapshotSummarySchema).parse(value) as QueueSnapshotSummary[], "SpotDIY could not read queue snapshots.");
}

export async function restoreQueueSnapshot(snapshotId: QueueSnapshotId): Promise<PlaybackSnapshot> {
  if (!isTauriRuntime()) {
    throw new IpcError("Queue snapshots require the native SpotDIY runtime.");
  }
  return invokePlayback("restore_queue_snapshot", { snapshotId: queueSnapshotIdSchema.parse(snapshotId) }, parsePlaybackSnapshot, "SpotDIY could not restore the queue snapshot.");
}

export async function deleteQueueSnapshot(snapshotId: QueueSnapshotId): Promise<QueueSnapshotSummary[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return invokePlayback("delete_queue_snapshot", { snapshotId: queueSnapshotIdSchema.parse(snapshotId) }, (value) => z.array(queueSnapshotSummarySchema).parse(value) as QueueSnapshotSummary[], "SpotDIY could not delete the queue snapshot.");
}

export async function subscribeToQueueState(
  listener: QueueWorkspaceListener,
  onError?: QueueWorkspaceErrorListener,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    if (isPlaybackE2EAdapterEnabled()) {
      listener(browserPreviewQueueWorkspace());
    }
    return () => undefined;
  }
  try {
    return await listen<unknown>(QUEUE_STATE_EVENT, (event) => {
      try {
        listener(parseQueueWorkspace(event.payload));
      } catch (error) {
        onError?.(new IpcError("SpotDIY received an invalid queue state event.", error));
      }
    });
  } catch (error) {
    throw new IpcError("SpotDIY could not subscribe to queue updates.", error);
  }
}

export async function subscribeToPlaybackState(
  listener: PlaybackSnapshotListener,
  onError?: PlaybackSnapshotErrorListener,
): Promise<() => void> {
  if (isPlaybackE2EAdapterEnabled()) {
    ensureE2EPlaybackState();
    e2eAdapterState.listeners.add(listener);
    listener(e2eAdapterState.snapshot);
    return () => {
      e2eAdapterState.listeners.delete(listener);
      if (e2eAdapterState.listeners.size === 0) {
        clearE2ETimers();
      }
    };
  }
  if (!isTauriRuntime()) {
    return () => undefined;
  }

  try {
    return await listen<unknown>(PLAYBACK_STATE_EVENT, (event) => {
      try {
        listener(parsePlaybackSnapshot(event.payload));
      } catch (error) {
        // Native events are untrusted input; report validation failures without
        // allowing malformed state to reach the store or crash React.
        onError?.(new IpcError("SpotDIY received an invalid playback state event.", error));
      }
    });
  } catch (error) {
    throw new IpcError("SpotDIY could not subscribe to playback updates.", error);
  }
}

export function providerLabel(kind: ProviderKind): string {
  switch (kind) {
    case "local":
      return "LOCAL";
    case "youtube":
      return "YT";
    case "soundcloud":
      return "SC";
    case "spotify":
      return "SP";
  }
}
