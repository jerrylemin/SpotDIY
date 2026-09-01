import type { SearchResult } from "../../types/domain";

export type SearchResultActionId = "play" | "play-next" | "queue" | "inspect" | "open-location" | "open-source" | "download";

export interface SearchResultAction {
  id: SearchResultActionId;
  label: string;
  enabled: boolean;
  reason?: string;
}

export interface SearchResultActionOptions {
  nativeRuntime: boolean;
  downloadsAvailable?: boolean;
}

function supportedDownloadProvider(result: SearchResult): boolean {
  return result.entityKind === "track" && (result.provider === "youtube" || result.provider === "soundcloud");
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

  const providerName = result.provider === "spotify" ? "Spotify" : result.provider === "youtube" ? "YouTube" : "SoundCloud";
  return [
    {
      id: "play",
      label: "Play online",
      enabled: false,
      reason: result.provider === "spotify" ? "Spotify is metadata-only" : "Online playback is not implemented",
    },
    { id: "inspect", label: "Inspect", enabled: true },
    {
      id: "open-source",
      label: result.provider === "spotify" ? "Open on Spotify" : "Open source",
      enabled: result.canonicalUrl !== null,
      reason: "No validated provider URL is available",
    },
    {
      id: "download",
      label: "Download",
      enabled: supportedDownloadProvider(result) && result.canonicalUrl !== null && options.nativeRuntime && options.downloadsAvailable !== false,
      reason: result.provider === "spotify"
        ? "Spotify downloads are not supported"
        : options.downloadsAvailable === false
          ? "This provider does not advertise downloads"
        : options.nativeRuntime
          ? "This result cannot be downloaded"
          : `${providerName} downloads require the native app`,
    },
  ];
}
