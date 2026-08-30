import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import type { AppStatus, ProviderKind, SettingValue, SettingsSnapshot } from "../types/domain";

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

export class IpcError extends Error {
  public constructor(message: string, public readonly cause?: unknown) {
    super(message);
    this.name = "IpcError";
  }
}

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
