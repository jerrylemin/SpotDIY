import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import type { AppStatus, ProviderKind } from "../types/domain";

const providerKindSchema = z.enum(["local", "youtube", "soundcloud", "spotify"]);

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
      capabilities: z.object({
        search: z.boolean(),
        playback: z.boolean(),
        metadata: z.boolean(),
        artwork: z.boolean(),
        lyrics: z.boolean(),
        downloads: z.boolean(),
      }),
      detail: z.string(),
    }),
  ),
});

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
        },
        detail: "Connect Client Credentials locally to search the catalog.",
      },
    ],
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
