import type {
  DownloadDirectoryStatus,
  DownloadMode,
  ProviderKind,
  ProviderRuntimeStatus,
  SearchResult,
} from "../../types/domain";

export type SearchResultActionId = "play" | "play-next" | "queue" | "inspect" | "open-location" | "open-source" | "download";

export interface DownloadReadiness {
  ytDlpStatus: ProviderRuntimeStatus;
  ffmpegStatus: ProviderRuntimeStatus;
  downloadDirectoryStatus: DownloadDirectoryStatus;
  mpvStatus?: ProviderRuntimeStatus;
  spotifyStatus?: ProviderRuntimeStatus;
}

export interface SearchResultAction {
  id: SearchResultActionId;
  label: string;
  enabled: boolean;
  reason?: string;
  downloadModes?: DownloadMode[];
}

export interface SearchResultActionOptions {
  nativeRuntime: boolean;
  downloadsAvailable?: boolean;
  downloadReadiness?: DownloadReadiness;
}

export function downloadModesForProvider(provider: ProviderKind): DownloadMode[] {
  switch (provider) {
    case "youtube":
      return ["audio", "video"];
    case "soundcloud":
      return ["audio"];
    case "spotify":
      return ["audio"];
    default:
      return [];
  }
}

export function downloadModesForResult(result: SearchResult): DownloadMode[] {
  return result.entityKind === "track" ? downloadModesForProvider(result.provider) : [];
}

function runtimeToolReason(label: string, status: ProviderRuntimeStatus): string | undefined {
  switch (status) {
    case "ready":
      return undefined;
    case "missing":
      return `${label} is not available.`;
    case "unsupported":
      return `${label} is unsupported.`;
    case "broken":
      return `${label} is not usable.`;
    default:
      return `${label} readiness is not available yet.`;
  }
}

export function downloadReadinessReason(
  provider: ProviderKind,
  mode: DownloadMode,
  options: {
    canonicalUrl: string | null;
    nativeRuntime: boolean;
    downloadsAvailable?: boolean;
    downloadReadiness?: DownloadReadiness;
  },
): string | undefined {
  const modes = downloadModesForProvider(provider);
  if (modes.length === 0) {
    return "This provider does not support downloads.";
  }
  if (!modes.includes(mode)) {
    return `${provider === "soundcloud" ? "SoundCloud" : provider === "spotify" ? "Spotify" : "YouTube"} does not support this download mode.`;
  }
  if (options.canonicalUrl === null) {
    return "No validated provider URL is available.";
  }
  if (!options.nativeRuntime) {
    return "Downloads require the native SpotDIY desktop runtime.";
  }
  if (options.downloadsAvailable === false) {
    return "This provider does not advertise download task creation.";
  }
  const readiness = options.downloadReadiness;
  if (!readiness) {
    return "Download readiness is not available yet.";
  }
  if (provider === "spotify") {
    return runtimeToolReason("spotdl", readiness.spotifyStatus ?? "unknown")
      ?? runtimeToolReason("yt-dlp", readiness.ytDlpStatus)
      ?? (readiness.downloadDirectoryStatus === "missing" ? "Download folder is not configured." : undefined)
      ?? (readiness.downloadDirectoryStatus === "invalid" ? "Download folder is not usable." : undefined)
      ?? runtimeToolReason("FFmpeg", readiness.ffmpegStatus);
  }
  return runtimeToolReason("yt-dlp", readiness.ytDlpStatus)
    ?? (readiness.downloadDirectoryStatus === "missing" ? "Download folder is not configured." : undefined)
    ?? (readiness.downloadDirectoryStatus === "invalid" ? "Download folder is not usable." : undefined)
    ?? (mode === "video" ? runtimeToolReason("FFmpeg", readiness.ffmpegStatus) : undefined);
}

export function isDownloadFolderReadinessReason(reason: string | undefined): boolean {
  return reason === "Download folder is not configured." || reason === "Download folder is not usable.";
}

function onlinePlaybackReason(
  result: SearchResult,
  options: SearchResultActionOptions,
): string | undefined {
  if (result.provider === "spotify") {
    return "Spotify results are not playable online; open Spotify to listen.";
  }
  if (result.canonicalUrl === null) {
    return "No validated provider URL is available";
  }
  if (!options.nativeRuntime) {
    return "Online playback requires the native SpotDIY desktop runtime";
  }
  const status = options.downloadReadiness?.mpvStatus;
  return status === "ready" ? undefined : runtimeToolReason("MPV", status ?? "unknown");
}

export function deriveSearchResultActions(result: SearchResult, options: SearchResultActionOptions): SearchResultAction[] {
  const local = result.provider === "local" && result.localTrackId !== null;
  if (local) {
    return [
      { id: "play", label: "Play now", enabled: true },
      { id: "play-next", label: "Play next", enabled: true },
      { id: "queue", label: "Add to queue", enabled: true },
      { id: "inspect", label: "Inspect", enabled: true },
      {
        id: "open-location",
        label: "Open location",
        enabled: result.localSourceId !== null && options.nativeRuntime,
        reason: options.nativeRuntime ? "No local file source" : "File locations require the native app",
      },
    ];
  }

  const actions: SearchResultAction[] = [
    {
      id: "play",
      label: "Play online",
      enabled: onlinePlaybackReason(result, options) === undefined,
      reason: onlinePlaybackReason(result, options),
    },
    { id: "inspect", label: "Inspect", enabled: true },
    {
      id: "open-source",
      label: result.provider === "spotify" ? "Open on Spotify" : "Open source",
      enabled: result.canonicalUrl !== null,
      reason: "No validated provider URL is available",
    },
  ];

  const downloadModes = downloadModesForResult(result);
  if (downloadModes.length > 0) {
    const reason = downloadReadinessReason(result.provider, downloadModes[0], {
      canonicalUrl: result.canonicalUrl,
      nativeRuntime: options.nativeRuntime,
      downloadsAvailable: options.downloadsAvailable,
      downloadReadiness: options.downloadReadiness,
    });
    actions.push({
      id: "download",
      label: downloadModes.length === 1 ? "Download audio" : "Download",
      enabled: reason === undefined || isDownloadFolderReadinessReason(reason),
      reason,
      downloadModes,
    });
  }

  return actions;
}
