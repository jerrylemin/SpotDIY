import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  acceptFusionCandidate,
  clearFusionOverride,
  evaluateFusionCandidate,
  getSourceResolution,
  setFusionOverride,
} from "../src/services/ipc";
import type { SearchResult, TrackId } from "../src/types/domain";

const targetTrackId = "track-target" as TrackId;
const candidate: SearchResult = {
  provider: "youtube",
  entityKind: "track",
  providerItemId: "video-1",
  canonicalUrl: "https://youtu.be/video-1",
  title: "Artist - Signal Test",
  artists: ["Artist"],
  album: null,
  durationMs: 180_000,
  artworkUrl: null,
  publishedAt: null,
  engagementCount: null,
  engagementKind: null,
  explicit: null,
  localTrackId: null,
  localSourceId: null,
  originalRank: 0,
};

const evaluation = {
  targetTrackId,
  decision: "auto_merge",
  scoreBps: 9_000,
  thresholdBps: 8_800,
  titleScoreBps: 10_000,
  artistScoreBps: 10_000,
  durationScoreBps: 10_000,
  durationDeltaMs: 0,
  candidateQualifiers: [],
  targetQualifiers: [],
  reason: "matched",
};

function enableNativeRuntime() {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
}

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  invokeMock.mockReset();
});

describe("source fusion and resolution IPC contracts", () => {
  it("forwards strict fusion requests and parses typed results", async () => {
    enableNativeRuntime();
    invokeMock
      .mockResolvedValueOnce(evaluation)
      .mockResolvedValueOnce(evaluation)
      .mockResolvedValueOnce({
        providerKind: "youtube",
        providerItemId: "video-1",
        targetTrackId,
        decision: "merge",
        createdAt: "2026-09-01T00:00:00Z",
        updatedAt: "2026-09-01T00:00:00Z",
      })
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce({
        selectedSourceId: null,
        candidates: [{
          sourceId: "source-youtube",
          provider: "youtube",
          playable: false,
          reason: "provider_playback_not_implemented",
          preferenceRank: 2,
          detail: "YouTube playback is not implemented yet",
        }],
      });

    await expect(evaluateFusionCandidate(candidate, targetTrackId)).resolves.toEqual(evaluation);
    await expect(acceptFusionCandidate(candidate, targetTrackId)).resolves.toEqual(evaluation);
    await expect(setFusionOverride({
      providerKind: "youtube",
      providerItemId: "video-1",
      targetTrackId,
      decision: "merge",
    })).resolves.toMatchObject({ decision: "merge" });
    await expect(clearFusionOverride({
      providerKind: "youtube",
      providerItemId: "video-1",
      targetTrackId,
    })).resolves.toBeUndefined();
    await expect(getSourceResolution(targetTrackId)).resolves.toMatchObject({ selectedSourceId: null });

    expect(invokeMock).toHaveBeenNthCalledWith(1, "evaluate_fusion_candidate", {
      candidate,
      targetTrackId,
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "accept_fusion_candidate", {
      candidate,
      targetTrackId,
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "set_fusion_override", {
      providerKind: "youtube",
      providerItemId: "video-1",
      targetTrackId,
      decision: "merge",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(4, "clear_fusion_override", {
      providerKind: "youtube",
      providerItemId: "video-1",
      targetTrackId,
    });
    expect(invokeMock).toHaveBeenNthCalledWith(5, "get_source_resolution", { trackId: targetTrackId });
  });

  it("rejects malformed fusion and resolution responses", async () => {
    enableNativeRuntime();
    invokeMock.mockResolvedValueOnce({ ...evaluation, unexpected: true });
    await expect(evaluateFusionCandidate(candidate, targetTrackId)).rejects.toThrow();

    invokeMock.mockResolvedValueOnce({ selectedSourceId: null, candidates: [{
      sourceId: "source-youtube",
      provider: "youtube",
      playable: false,
      reason: "provider_playback_not_implemented",
      preferenceRank: 2,
      detail: null,
      rawUrl: "https://youtu.be/video-1",
    }] });
    await expect(getSourceResolution(targetTrackId)).rejects.toThrow();
  });
});
