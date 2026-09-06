import { describe, expect, it } from "vitest";

import { deriveSearchResultActions, downloadModesForResult } from "../src/features/actions/track-actions";
import type { SearchResult } from "../src/types/domain";

const baseResult: SearchResult = {
  provider: "youtube",
  entityKind: "track",
  providerItemId: "video-1",
  canonicalUrl: "https://www.youtube.com/watch?v=video-1",
  title: "Source result",
  artists: ["Artist"],
  album: null,
  durationMs: 120_000,
  artworkUrl: null,
  publishedAt: null,
  engagementCount: null,
  engagementKind: null,
  explicit: null,
  localTrackId: null,
  localSourceId: null,
  originalRank: 1,
};

describe("capability-aware search actions", () => {
  it("enables online playback and native provider downloads when the runtime is ready", () => {
    const actions = deriveSearchResultActions(baseResult, {
      nativeRuntime: true,
      downloadsAvailable: true,
      downloadReadiness: { ytDlpStatus: "ready", ffmpegStatus: "ready", mpvStatus: "ready", downloadDirectoryStatus: "ready" },
    });
    expect(actions.find((action) => action.id === "play")).toMatchObject({ enabled: true, reason: undefined });
    expect(actions.find((action) => action.id === "download")).toMatchObject({ enabled: true });
  });

  it("enables Spotify source-matched playback and native audio downloads", () => {
    const spotify = { ...baseResult, provider: "spotify" as const, canonicalUrl: "https://open.spotify.com/track/1" };
    expect(downloadModesForResult(spotify)).toEqual(["audio"]);
    const actions = deriveSearchResultActions(spotify, {
      nativeRuntime: true,
      downloadsAvailable: true,
      downloadReadiness: {
        ytDlpStatus: "ready",
        ffmpegStatus: "ready",
        mpvStatus: "ready",
        spotifyStatus: "ready",
        downloadDirectoryStatus: "ready",
      },
    });
    expect(actions.find((action) => action.id === "play")).toMatchObject({ enabled: true, reason: undefined });
    expect(actions.find((action) => action.id === "download")).toMatchObject({ enabled: true, downloadModes: ["audio"] });
  });

  it("keeps SoundCloud audio-only and reports native readiness failures", () => {
    const soundcloud = {
      ...baseResult,
      provider: "soundcloud" as const,
      canonicalUrl: "https://soundcloud.com/artist/track",
    };
    expect(downloadModesForResult(soundcloud)).toEqual(["audio"]);
    const actions = deriveSearchResultActions(soundcloud, {
      nativeRuntime: true,
      downloadsAvailable: true,
      downloadReadiness: {
        ytDlpStatus: "missing",
        ffmpegStatus: "ready",
        downloadDirectoryStatus: "ready",
      },
    });
    expect(actions.find((action) => action.id === "download")).toMatchObject({
      label: "Download audio",
      enabled: false,
      reason: "yt-dlp is not available.",
      downloadModes: ["audio"],
    });
  });

  it("offers persisted local actions without treating a search result as online media", () => {
    const local = { ...baseResult, provider: "local" as const, localTrackId: "track-1" as SearchResult["localTrackId"], localSourceId: "source-1" as SearchResult["localSourceId"], canonicalUrl: null };
    const actions = deriveSearchResultActions(local, { nativeRuntime: true });
    expect(actions.map((action) => action.id)).toEqual(["play", "play-next", "queue", "inspect", "open-location"]);
    expect(actions.every((action) => action.enabled)).toBe(true);
  });
});
