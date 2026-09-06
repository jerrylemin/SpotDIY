import { describe, expect, it } from "vitest";

import { interpolatePlaybackPosition } from "../src/hooks/usePlaybackClock";

describe("playback clock interpolation", () => {
  it("advances only while the authoritative playback phase is playing", () => {
    expect(interpolatePlaybackPosition(1_000, 250, "playing", 10_000)).toBe(1_250);
    expect(interpolatePlaybackPosition(1_000, 250, "paused", 10_000)).toBe(1_000);
    expect(interpolatePlaybackPosition(1_000, 250, "seeking", 10_000)).toBe(1_000);
  });

  it("never moves before zero or past the known duration", () => {
    expect(interpolatePlaybackPosition(0, -50, "playing", 10_000)).toBe(0);
    expect(interpolatePlaybackPosition(9_900, 250, "playing", 10_000)).toBe(10_000);
    expect(interpolatePlaybackPosition(9_900, 250, "playing", null)).toBe(10_150);
  });
});
