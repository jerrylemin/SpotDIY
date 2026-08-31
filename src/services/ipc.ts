import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { z } from "zod";

import type {
  AppStatus,
  LibraryFolder,
  LibraryFolderId,
  LibraryPage,
  LibraryPageRequest,
  LibraryTrack,
  LibraryStatus,
  PlaybackAudioDevice,
  PlaybackBackendHealth,
  PlaybackErrorCode,
  PlaybackErrorDto,
  PlaybackPhase,
  PlaybackSnapshot,
  PlaybackSourceOption,
  ProviderKind,
  QueueEntryId,
  RepeatMode,
  ScanProgress,
  SettingValue,
  SettingsSnapshot,
  TrackPlaybackRequest,
  TrackId,
  SourceId,
} from "../types/domain";

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
});

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
      capabilities: sourceCapabilitiesSchema,
      detail: z.string(),
    }),
  ),
});

const themeSchema = z.enum(["dark", "light", "system"]);
const sourcePreferenceOrderSchema = z
  .array(providerKindSchema)
  .length(4)
  .refine((value) => new Set(value).size === value.length, "Provider preference order cannot contain duplicates.");
const settingsSnapshotSchema = z.object({
  theme: themeSchema,
  downloadsDirectory: z.string().nullable(),
  sourcePreferenceOrder: sourcePreferenceOrderSchema,
  firstRun: z.boolean(),
  storageMode: z.enum(["standard", "portable"]),
});
const settingValueSchema = z.discriminatedUnion("key", [
  z.object({ key: z.literal("theme"), value: themeSchema }),
  z.object({ key: z.literal("downloadsDirectory"), value: z.string().nullable() }),
  z.object({ key: z.literal("sourcePreferenceOrder"), value: sourcePreferenceOrderSchema }),
]);

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
const queueEntryIdSchema = z.string().transform((value) => value as QueueEntryId);
const trackIdSchema = z.string().transform((value) => value as TrackId);
const sourceIdSchema = z.string().transform((value) => value as SourceId);
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
  "sourceSwitchFailed",
  "localFileMissing",
  "loadFailed",
  "seekFailed",
  "deviceUnavailable",
  "queueEmpty",
  "invalidState",
  "backendUnavailable",
  "backendDisconnected",
  "backendTimeout",
  "backendProtocol",
  "backendOperation",
  "libraryOperation",
  "controllerBusy",
  "controllerUnavailable",
  "recoveryRetrying",
  "recoveryExhausted",
  "shuttingDown",
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
}).strict();
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

export class IpcError extends Error {
  public constructor(message: string, public readonly cause?: unknown) {
    super(message);
    this.name = "IpcError";
  }
}

export const LIBRARY_PROGRESS_EVENT = "library://scan-progress";
export const PLAYBACK_STATE_EVENT = "playback://state";

type PlaybackSnapshotListener = (snapshot: PlaybackSnapshot) => void;

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
  sourceSwitchFailed: "SpotDIY could not switch playback sources.",
  localFileMissing: "The local file is missing or unavailable.",
  loadFailed: "SpotDIY could not load that track.",
  seekFailed: "SpotDIY could not seek within the current track.",
  deviceUnavailable: "That audio device is unavailable.",
  queueEmpty: "The playback queue is empty.",
  invalidState: "That control is unavailable right now.",
  backendUnavailable: "The playback backend is unavailable.",
  backendDisconnected: "The playback backend disconnected.",
  backendTimeout: "The playback backend timed out.",
  backendProtocol: "The playback backend returned an invalid response.",
  backendOperation: "The playback backend rejected that operation.",
  libraryOperation: "SpotDIY could not resolve that local source.",
  controllerBusy: "Playback is busy. Try again in a moment.",
  controllerUnavailable: "Playback is temporarily unavailable.",
  recoveryRetrying: "SpotDIY is retrying the playback backend.",
  recoveryExhausted: "Playback recovery is exhausted.",
  shuttingDown: "SpotDIY is shutting down playback.",
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
  currentIndex: number | null;
  listeners: Set<PlaybackSnapshotListener>;
  positionTimer: number | null;
  transitionTimer: number | null;
  nextRevision: number;
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

function trackToPlaybackSources(track: LibraryTrack): PlaybackSourceOption[] {
  return [{
    sourceId: track.sourceId,
    provider: "local",
    label: "LOCAL",
    available: track.available,
  }];
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
  const currentCanonicalIndex = current
    ? e2eAdapterState.canonicalQueue.findIndex((entry) => playbackRequestKey(entry) === playbackRequestKey(current))
    : -1;
  if (!e2eAdapterState.snapshot.shuffleEnabled || e2eAdapterState.canonicalQueue.length <= 2) {
    e2eAdapterState.activeQueue = [...e2eAdapterState.canonicalQueue];
    if (current) {
      e2eAdapterState.currentIndex = e2eAdapterState.activeQueue.findIndex((entry) => playbackRequestKey(entry) === playbackRequestKey(current));
    }
    return;
  }

  const head = currentCanonicalIndex < 0 ? [] : e2eAdapterState.canonicalQueue.slice(0, currentCanonicalIndex + 1);
  const tail = currentCanonicalIndex < 0
    ? [...e2eAdapterState.canonicalQueue]
    : [...e2eAdapterState.canonicalQueue.slice(currentCanonicalIndex + 1)];
  tail.sort((left, right) => `${right.trackId}:${right.sourceId ?? ""}`.localeCompare(`${left.trackId}:${left.sourceId ?? ""}`));
  e2eAdapterState.activeQueue = [...head, ...tail];
  if (current) {
    e2eAdapterState.currentIndex = e2eAdapterState.activeQueue.findIndex((entry) => playbackRequestKey(entry) === playbackRequestKey(current));
  }
}

function seedE2EPlaybackState() {
  clearE2ETimers();
  selectE2EDevice("auto");
  e2eAdapterState.canonicalQueue = [];
  e2eAdapterState.activeQueue = [];
  e2eAdapterState.currentIndex = null;
  e2eAdapterState.nextRevision = 1;

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
    e2eAdapterState.activeQueue = [...e2eAdapterState.canonicalQueue];
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
    e2eAdapterState.activeQueue = [...e2eAdapterState.canonicalQueue];
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

  return makePlaybackSnapshotForTrack(track, {
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
  if (request.sourceId !== null && request.sourceId !== track.sourceId) {
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
          capabilities: {
            search: true,
            playback: true,
            metadata: true,
            artwork: true,
            lyrics: false,
            downloads: true,
            popularity: true,
            releaseDate: true,
            lyricsMetadata: false,
          },
          detail: "Provider adapter awaits media-tool verification.",
        },
        {
          kind: "soundcloud",
          label: "SoundCloud",
          configured: false,
          available: false,
          capabilities: {
            search: true,
            playback: true,
            metadata: true,
            artwork: true,
            lyrics: false,
            downloads: true,
            popularity: true,
            releaseDate: true,
            lyricsMetadata: false,
          },
          detail: "Provider adapter awaits media-tool verification.",
        },
        {
          kind: "spotify",
          label: "Spotify catalog",
          configured: false,
          available: false,
          capabilities: {
            search: true,
            playback: false,
            metadata: true,
            artwork: true,
            lyrics: false,
            downloads: false,
            popularity: true,
            releaseDate: true,
            lyricsMetadata: false,
          },
          detail: "Connect Client Credentials locally to search the catalog.",
        },
      ],
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
        capabilities: {
          search: true,
          playback: true,
          metadata: true,
          artwork: true,
          lyrics: false,
          downloads: true,
          popularity: true,
          releaseDate: true,
          lyricsMetadata: false,
        },
        detail: "Provider adapter awaits media-tool verification.",
      },
      {
        kind: "soundcloud",
        label: "SoundCloud",
        configured: false,
        available: false,
        capabilities: {
          search: true,
          playback: true,
          metadata: true,
          artwork: true,
          lyrics: false,
          downloads: true,
          popularity: true,
          releaseDate: true,
          lyricsMetadata: false,
        },
        detail: "Provider adapter awaits media-tool verification.",
      },
      {
        kind: "spotify",
        label: "Spotify catalog",
        configured: false,
        available: false,
        capabilities: {
          search: true,
          playback: false,
          metadata: true,
          artwork: true,
          lyrics: false,
          downloads: false,
          popularity: true,
          releaseDate: true,
          lyricsMetadata: false,
        },
        detail: "Connect Client Credentials locally to search the catalog.",
      },
    ],
  };
}

function browserPreviewSettings(): SettingsSnapshot {
  return {
    theme: "dark",
    downloadsDirectory: null,
    sourcePreferenceOrder: ["local", "soundcloud", "youtube", "spotify"],
    firstRun: true,
    storageMode: "standard",
  };
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
      throw new IpcError("Local settings require the native SpotDIY runtime.");
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
    e2eAdapterState.activeQueue = [request];
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
      e2eAdapterState.activeQueue = [parsedRequest];
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
      const priorPosition = e2eAdapterState.snapshot.positionMs;
      const paused = e2eAdapterState.snapshot.phase === "paused";
      e2eAdapterState.canonicalQueue = [parsedRequest];
      e2eAdapterState.activeQueue = [parsedRequest];
      e2eAdapterState.currentIndex = 0;
      return scheduleE2ELoad(0, paused, Math.min(priorPosition, track.durationMs ?? priorPosition));
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

export async function subscribeToPlaybackState(listener: PlaybackSnapshotListener): Promise<() => void> {
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
      } catch {
        // Native events are untrusted input; invalid playback events must not break the UI.
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
