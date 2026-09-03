import { describe, expect, it } from "vitest";

import { buildMusicMapGraph } from "../src/features/music-map/layout";
import { buildGalaxyLayout } from "../src/features/library-galaxy/layout";
import { resolveVisualDrop } from "../src/features/visual-exploration/drag-actions";
import { visibleRadialActions } from "../src/components/radial-menu/radial-actions";
import { sampleAccentFromPixels } from "../src/features/theme/theme-studio/dynamic-accent";
import type { VisualTrackPoint } from "../src/types/domain";

const track = (id: string, overrides: Partial<VisualTrackPoint> = {}): VisualTrackPoint => ({
  trackId: id as VisualTrackPoint["trackId"],
  title: `Track ${id}`,
  primaryArtist: "Artist",
  artists: ["Artist"],
  album: "Album",
  genres: ["Electronic"],
  year: 2026,
  dateAdded: "2026-01-01T00:00:00Z",
  lastPlayed: null,
  liked: false,
  rating: null,
  qualifiedPlays: 0,
  listenedMs: 0,
  audioQuality: "unknown",
  providerCount: 1,
  artworkPath: null,
  ...overrides,
});

describe("advanced visual exploration pure contracts", () => {
  it("builds stable relational Music Map nodes and real edges", () => {
    const tracks = [track("b"), track("a", { genres: ["Ambient"] })];
    const first = buildMusicMapGraph(tracks);
    const second = buildMusicMapGraph(tracks);

    expect(first).toEqual(second);
    expect(first.nodes.filter((node) => node.kind === "track")).toHaveLength(2);
    expect(first.edges.every((edge) => first.nodes.some((node) => node.id === edge.source))).toBe(true);
    expect(first.nodes.length).toBeLessThanOrEqual(1_500);
    expect(first.edges.length).toBeLessThanOrEqual(3_500);
  });

  it("keeps Galaxy coordinates deterministic and bounded", () => {
    const tracks = Array.from({ length: 5_100 }, (_, index) => track(`track-${index}`, {
      primaryArtist: `Artist ${index % 11}`,
    }));
    const first = buildGalaxyLayout(tracks, 1200, 700, "artist");
    const second = buildGalaxyLayout(tracks, 1200, 700, "artist");

    expect(first).toEqual(second);
    expect(first.points).toHaveLength(5_000);
    expect(first.points.every((point) => point.x >= 0 && point.x <= 1200 && point.y >= 0 && point.y <= 700)).toBe(true);
  });

  it("maps only completed valid drops to mutations", () => {
    expect(resolveVisualDrop("track-1", "play-next")).toEqual({ trackId: "track-1", action: "play-next" });
    expect(resolveVisualDrop("track-1", "queue")).toEqual({ trackId: "track-1", action: "queue" });
    expect(resolveVisualDrop("track-1", "inbox")).toEqual({ trackId: "track-1", action: "inbox" });
    expect(resolveVisualDrop("track-1", null)).toBeNull();
  });

  it("shows at most eight radial entries and a More fallback", () => {
    const actions = Array.from({ length: 10 }, (_, index) => ({ id: `${index}`, label: `Action ${index}`, onSelect: () => undefined }));
    const visible = visibleRadialActions(actions);
    expect(visible.visible).toHaveLength(7);
    expect(visible.more).toHaveLength(3);
  });

  it("samples a bounded accent and falls back when contrast cannot be met", () => {
    const bright = sampleAccentFromPixels(new Uint8ClampedArray([220, 240, 80, 255]), "#101113", "#17181D");
    expect(bright.accent).toMatch(/^#[0-9A-F]{6}$/);
    expect(["#000000", "#FFFFFF"]).toContain(bright.accentContrast);
    expect(sampleAccentFromPixels(new Uint8ClampedArray(), "#101113", "#17181D")).toBeNull();
  });
});
