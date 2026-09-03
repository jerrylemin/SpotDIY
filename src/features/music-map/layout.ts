import type { VisualTrackPoint } from "../../types/domain";

export type MusicMapNodeKind = "genre" | "artist" | "album" | "track";

export interface MusicMapNode {
  id: string;
  kind: MusicMapNodeKind;
  label: string;
  trackId?: VisualTrackPoint["trackId"];
}

export interface MusicMapEdge {
  id: string;
  source: string;
  target: string;
}

export interface MusicMapGraph {
  nodes: MusicMapNode[];
  edges: MusicMapEdge[];
}

const layerOrder: Record<MusicMapNodeKind, number> = { genre: 0, artist: 1, album: 2, track: 3 };

function key(value: string): string {
  return value.trim().toLocaleLowerCase();
}

function nodeId(kind: MusicMapNodeKind, label: string): string {
  return `${kind}:${key(label)}`;
}

function addNode(nodes: Map<string, MusicMapNode>, kind: MusicMapNodeKind, label: string, trackId?: VisualTrackPoint["trackId"]): string {
  const trimmed = label.trim();
  if (!trimmed) return "";
  const id = kind === "track" ? `track:${trackId}` : nodeId(kind, trimmed);
  if (!nodes.has(id)) nodes.set(id, { id, kind, label: trimmed, ...(trackId ? { trackId } : {}) });
  return id;
}

export function buildMusicMapGraph(tracks: readonly VisualTrackPoint[], maxNodes = 1_500, maxEdges = 3_500): MusicMapGraph {
  const nodes = new Map<string, MusicMapNode>();
  const edgeIds = new Set<string>();
  const edges: MusicMapEdge[] = [];
  const connect = (source: string, target: string) => {
    if (!source || !target || source === target) return;
    const id = `${source}->${target}`;
    if (!edgeIds.has(id)) {
      edgeIds.add(id);
      edges.push({ id, source, target });
    }
  };

  for (const track of tracks.slice(0, 5_000)) {
    const trackNode = addNode(nodes, "track", track.title, track.trackId);
    const artistNodes = track.artists.map((artist) => addNode(nodes, "artist", artist)).filter(Boolean);
    const genreNodes = track.genres.map((genre) => addNode(nodes, "genre", genre)).filter(Boolean);
    const albumNode = track.album ? addNode(nodes, "album", track.album) : "";

    if (albumNode) connect(albumNode, trackNode);
    for (const artistNode of artistNodes) connect(artistNode, albumNode || trackNode);
    for (const genreNode of genreNodes) {
      for (const artistNode of artistNodes) connect(genreNode, artistNode);
      if (!artistNodes.length) connect(genreNode, albumNode || trackNode);
    }
  }

  const sortedNodes = [...nodes.values()].sort((left, right) =>
    layerOrder[left.kind] - layerOrder[right.kind]
    || left.label.localeCompare(right.label, undefined, { sensitivity: "base" })
    || left.id.localeCompare(right.id));
  const kept = new Set(sortedNodes.slice(0, maxNodes).map((node) => node.id));
  return {
    nodes: sortedNodes.slice(0, maxNodes),
    edges: edges
      .filter((edge) => kept.has(edge.source) && kept.has(edge.target))
      .sort((left, right) => left.id.localeCompare(right.id))
      .slice(0, maxEdges),
  };
}

export function musicMapLayerPosition(kind: MusicMapNodeKind, index: number, count: number): { x: number; y: number } {
  const gap = 640 / (Math.max(1, count) + 1);
  return { x: 100 + layerOrder[kind] * 270, y: gap * (index + 1) };
}
