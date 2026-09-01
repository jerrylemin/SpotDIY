import { describe, expect, it } from "vitest";

import { deriveSearchResultActions } from "../src/features/actions/track-actions";
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
  it("keeps online playback visible but disabled and enables native provider downloads", () => {
    const actions = deriveSearchResultActions(baseResult, { nativeRuntime: true });
    expect(actions.find((action) => action.id === "play")).toMatchObject({ enabled: false, reason: "Online playback is not implemented" });
    expect(actions.find((action) => action.id === "download")).toMatchObject({ enabled: true });
  });

  it("treats Spotify as metadata-only and keeps browser downloads disabled", () => {
    const spotify = { ...baseResult, provider: "spotify" as const, canonicalUrl: "https://open.spotify.com/track/1" };
    const actions = deriveSearchResultActions(spotify, { nativeRuntime: false });
    expect(actions.find((action) => action.id === "play")?.reason).toBe("Spotify is metadata-only");
    expect(actions.find((action) => action.id === "download")?.reason).toBe("Spotify downloads are not supported");
    expect(actions.find((action) => action.id === "download")?.enabled).toBe(false);
  });

  it("offers persisted local actions without treating a search result as online media", () => {
    const local = { ...baseResult, provider: "local" as const, localTrackId: "track-1" as SearchResult["localTrackId"], localSourceId: "source-1" as SearchResult["localSourceId"], canonicalUrl: null };
    const actions = deriveSearchResultActions(local, { nativeRuntime: true });
    expect(actions.map((action) => action.id)).toEqual(["play", "play-next", "queue", "inspect", "open-location"]);
    expect(actions.every((action) => action.enabled)).toBe(true);
  });
});
