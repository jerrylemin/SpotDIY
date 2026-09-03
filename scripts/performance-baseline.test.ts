import { performance } from "node:perf_hooks";

import { expect, test } from "vitest";

import { buildGalaxyLayout } from "../src/features/library-galaxy/layout";
import { buildMusicMapGraph } from "../src/features/music-map/layout";
import type { TrackId, VisualTrackPoint } from "../src/types/domain";

const TRACK_COUNT = 5_000;
const MEASURED_RUNS = 10;

function syntheticTracks(count: number): VisualTrackPoint[] {
  return Array.from({ length: count }, (_, index) => {
    const artistIndex = index % 500;
    const albumIndex = index % 1_000;
    return {
      trackId: `perf-track-${index}` as TrackId,
      title: `Synthetic Track ${index}`,
      primaryArtist: `Synthetic Artist ${artistIndex}`,
      artists: [`Synthetic Artist ${artistIndex}`, `Synthetic Guest ${index % 80}`],
      artistIds: [`perf-artist-${artistIndex}`, `perf-guest-${index % 80}`],
      album: `Synthetic Album ${albumIndex}`,
      albumId: `perf-album-${albumIndex}`,
      genres: [`Synthetic Genre ${index % 32}`],
      year: 2026,
      dateAdded: "2026-09-03T00:00:00Z",
      lastPlayed: null,
      liked: false,
      rating: null,
      qualifiedPlays: index % 4,
      listenedMs: index * 1_000,
      audioQuality: index % 2 === 0 ? "lossless" : "lossy",
      providerCount: 1,
      artworkPath: null,
      canPlayback: true,
      canPreview: true,
      canRevealLocal: true,
    };
  });
}

function measure(operation: () => unknown): { median: number; p95: number; max: number } {
  operation();
  const samples = Array.from({ length: MEASURED_RUNS }, () => {
    const started = performance.now();
    operation();
    return performance.now() - started;
  }).sort((left, right) => left - right);
  return {
    median: samples[Math.floor(samples.length / 2)],
    p95: samples[Math.ceil(samples.length * 0.95) - 1],
    max: samples[samples.length - 1],
  };
}

test("captures reproducible maximum-size visual layout baselines", () => {
  const tracks = syntheticTracks(TRACK_COUNT);
  const musicMap = measure(() => buildMusicMapGraph(tracks));
  const galaxy = measure(() => buildGalaxyLayout(tracks, 1_440, 900, "artist"));
  const graph = buildMusicMapGraph(tracks);
  const layout = buildGalaxyLayout(tracks, 1_440, 900, "artist");
  const result = {
    environment: {
      node: process.version,
      platform: process.platform,
      arch: process.arch,
    },
    data: {
      tracks: TRACK_COUNT,
      musicMapNodes: graph.nodes.length,
      musicMapEdges: graph.edges.length,
      galaxyPoints: layout.points.length,
      iterations: MEASURED_RUNS,
      warmup: 1,
    },
    milliseconds: { musicMap, galaxy },
  };
  console.log(`PERFORMANCE_BASELINE ${JSON.stringify(result)}`);
  expect(graph.nodes.length).toBeLessThanOrEqual(1_500);
  expect(graph.edges.length).toBeLessThanOrEqual(3_500);
  expect(layout.points).toHaveLength(TRACK_COUNT);
  expect(musicMap.p95).toBeLessThanOrEqual(2_000);
  expect(galaxy.p95).toBeLessThanOrEqual(1_500);
}, 30_000);
