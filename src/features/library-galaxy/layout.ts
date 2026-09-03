import type { VisualTrackPoint } from "../../types/domain";

export type GalaxyClusterMode = "artist" | "genre";

export interface GalaxyPoint {
  track: VisualTrackPoint;
  cluster: string;
  clusterKey: string;
  x: number;
  y: number;
  radius: number;
}

export interface GalaxyLayout {
  points: GalaxyPoint[];
  clusters: { key: string; label: string }[];
}

const GOLDEN_ANGLE = Math.PI * (3 - Math.sqrt(5));

export function stableHash(value: string): number {
  let hash = 2_166_136_261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16_777_619);
  }
  return hash >>> 0;
}

function clusterFor(track: VisualTrackPoint, mode: GalaxyClusterMode): { key: string; label: string } {
  const label = (mode === "artist" ? track.primaryArtist : track.genres[0])?.trim() || "Unknown";
  const identity = mode === "artist" ? track.artistIds[0]?.trim() : "";
  return {
    key: `${mode}:${identity || label.toLocaleLowerCase()}`,
    label,
  };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function buildGalaxyLayout(
  tracks: readonly VisualTrackPoint[],
  width: number,
  height: number,
  mode: GalaxyClusterMode,
): GalaxyLayout {
  const safeWidth = Math.max(1, width);
  const safeHeight = Math.max(1, height);
  const boundedTracks = tracks.slice(0, 5_000);
  const clusters = [...new Map(boundedTracks.map((track) => {
    const cluster = clusterFor(track, mode);
    return [cluster.key, cluster] as const;
  }))].map(([, cluster]) => cluster).sort((left, right) => left.label.localeCompare(right.label, undefined, { sensitivity: "base" }) || left.key.localeCompare(right.key));
  const clusterIndex = new Map(clusters.map((cluster, index) => [cluster.key, index]));
  const clusterCounts = new Map<string, number>();
  for (const track of boundedTracks) {
    const cluster = clusterFor(track, mode);
    clusterCounts.set(cluster.key, (clusterCounts.get(cluster.key) ?? 0) + 1);
  }

  const centerRadius = Math.min(safeWidth, safeHeight) * 0.34;
  const spread = Math.min(safeWidth, safeHeight) * 0.1;
  const points = boundedTracks.map((track) => {
    const cluster = clusterFor(track, mode);
    const index = clusterIndex.get(cluster.key) ?? 0;
    const count = clusterCounts.get(cluster.key) ?? 1;
    const centerAngle = index * GOLDEN_ANGLE;
    const centerDistance = clusters.length <= 1 ? 0 : centerRadius * Math.sqrt((index + 1) / clusters.length);
    const centerX = safeWidth / 2 + Math.cos(centerAngle) * centerDistance;
    const centerY = safeHeight / 2 + Math.sin(centerAngle) * centerDistance;
    const hash = stableHash(`${cluster.key}:${track.trackId}`);
    const pointAngle = (hash % 10_000) / 10_000 * Math.PI * 2;
    const pointDistance = spread * (0.25 + ((hash >>> 8) % 10_000) / 10_000 * (0.5 + Math.min(count, 20) / 40));
    const radius = 3 + Math.min(9, Math.sqrt(track.listenedMs / 60_000 + track.qualifiedPlays * 3));
    return {
      track,
      cluster: cluster.label,
      clusterKey: cluster.key,
      x: clamp(centerX + Math.cos(pointAngle) * pointDistance, radius, safeWidth - radius),
      y: clamp(centerY + Math.sin(pointAngle) * pointDistance, radius, safeHeight - radius),
      radius,
    };
  });
  return { points, clusters };
}
