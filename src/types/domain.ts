export type ProviderKind = "local" | "youtube" | "soundcloud" | "spotify";

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
}

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
