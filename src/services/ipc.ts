import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { z } from "zod";

import type {
  AppStatus,
  LibraryFolder,
  LibraryFolderId,
  LibraryPage,
  LibraryPageRequest,
  LibraryStatus,
  ProviderKind,
  ScanProgress,
  SettingValue,
  SettingsSnapshot,
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

export class IpcError extends Error {
  public constructor(message: string, public readonly cause?: unknown) {
    super(message);
    this.name = "IpcError";
  }
}

export const LIBRARY_PROGRESS_EVENT = "library://scan-progress";

function browserPreviewStatus(): AppStatus {
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
  return {
    folders: [],
    indexedTrackCount: 0,
    availableTrackCount: 0,
    isScanning: false,
  };
}

function browserPreviewLibraryPage(request: LibraryPageRequest): LibraryPage {
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
