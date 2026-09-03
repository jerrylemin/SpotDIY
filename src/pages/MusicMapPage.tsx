import { useMemo, useState } from "react";

import { EmptyState } from "../components/common/EmptyState";
import { SpotIcon } from "../components/icons/SpotIcon";
import { TrackActionDragPanel } from "../features/visual-exploration/TrackActionDragPanel";
import { VisualTrackActions } from "../features/visual-exploration/VisualTrackActions";
import { buildMusicMapGraph, musicMapLayerPosition, type MusicMapNode } from "../features/music-map/layout";
import { useVisualLibraryDataset } from "../hooks/useVisualLibraryDataset";
import { usePlayback } from "../hooks/usePlayback";
import { addTrackToInbox } from "../services/ipc";
import { useUiStore } from "../stores/ui-store";
import type { VisualDatasetRequest, VisualTrackPoint } from "../types/domain";

const EMPTY_REQUEST: VisualDatasetRequest = { query: null, genre: null, artist: null, likedOnly: false, limit: 2_000 };
const EMPTY_TRACKS: VisualTrackPoint[] = [];

function trackLabel(track: VisualTrackPoint): string {
  return `${track.title} · ${track.primaryArtist}`;
}

export function MusicMapPage() {
  const openInspector = useUiStore((state) => state.openTrackInspector);
  const playback = usePlayback();
  const [query, setQuery] = useState("");
  const [genre, setGenre] = useState("");
  const [artist, setArtist] = useState("");
  const [likedOnly, setLikedOnly] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [view, setView] = useState({ x: 0, y: 0, scale: 1 });
  const request = {
    ...EMPTY_REQUEST,
    query: query.trim() || null,
    genre: genre || null,
    artist: artist || null,
    likedOnly,
  };
  const dataset = useVisualLibraryDataset(request);
  const tracks = dataset.data?.tracks ?? EMPTY_TRACKS;
  const graph = useMemo(() => buildMusicMapGraph(tracks), [tracks]);
  const selectedNode = graph.nodes.find((node) => node.id === selectedId) ?? null;
  const selectedTrack = selectedNode?.trackId ? tracks.find((track) => track.trackId === selectedNode.trackId) ?? null : null;
  const genres = useMemo(() => [...new Set(tracks.flatMap((track) => track.genres))].sort((a, b) => a.localeCompare(b)), [tracks]);
  const artists = useMemo(() => [...new Set(tracks.flatMap((track) => track.artists))].sort((a, b) => a.localeCompare(b)), [tracks]);
  const positions = useMemo(() => {
    const byKind = new Map<string, MusicMapNode[]>();
    for (const node of graph.nodes) byKind.set(node.kind, [...(byKind.get(node.kind) ?? []), node]);
    return new Map(graph.nodes.map((node) => [node.id, musicMapLayerPosition(node.kind, (byKind.get(node.kind) ?? []).indexOf(node), byKind.get(node.kind)?.length ?? 1)]));
  }, [graph.nodes]);

  const reset = () => setView({ x: 0, y: 0, scale: 1 });
  const selectNode = (node: MusicMapNode) => {
    setSelectedId(node.id);
    if (node.trackId) openInspector(node.trackId);
  };
  const move = (event: React.PointerEvent<SVGSVGElement>) => {
    if (event.buttons !== 1) return;
    setView((current) => ({ ...current, x: current.x + event.movementX, y: current.y + event.movementY }));
  };

  if (dataset.isLoading && !dataset.data) {
    return <div className="page-stack"><EmptyState icon="analytics" eyebrow="MUSIC MAP" title="Loading Music Map" description="Aggregating your local relationships…" /></div>;
  }
  if (dataset.isError) {
    return <div className="page-stack"><EmptyState icon="alert" eyebrow="MUSIC MAP UNAVAILABLE" title="Could not build your Music Map" description="The visual library dataset could not be read." action={<button className="button button-primary" onClick={() => void dataset.refetch()} type="button">Try again</button>} /></div>;
  }
  if (tracks.length === 0) {
    return <div className="page-stack"><EmptyState icon="analytics" eyebrow="MUSIC MAP" title="No relationships yet" description="Add or index local music to build your Music Map." /></div>;
  }

  return (
    <div className="page-stack visual-page music-map-page">
      <section className="page-intro">
        <div><span className="eyebrow">EXPLORE / MUSIC MAP</span><h1>See the <em>connections.</em></h1><p>Genres, artists, albums, and tracks are linked from your indexed local collection.</p></div>
        <div className="page-intro-stat"><strong>{dataset.data?.returnedTracks ?? tracks.length}</strong><span>tracks in view</span></div>
      </section>
      <section aria-label="Music Map filters" className="visual-toolbar">
        <label><span>Search</span><input onChange={(event) => setQuery(event.target.value)} placeholder="Title, artist, album…" value={query} /></label>
        <label><span>Genre</span><select onChange={(event) => setGenre(event.target.value)} value={genre}><option value="">All genres</option>{genres.map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
        <label><span>Artist</span><select onChange={(event) => setArtist(event.target.value)} value={artist}><option value="">All artists</option>{artists.map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
        <button aria-pressed={likedOnly} className={`button button-small ${likedOnly ? "button-primary" : "button-quiet"}`} onClick={() => setLikedOnly((value) => !value)} type="button">Liked only</button>
        <div className="visual-view-actions"><button className="button button-quiet button-small" onClick={() => setView((current) => ({ ...current, scale: Math.min(2.5, current.scale + 0.15) }))} type="button">＋</button><button className="button button-quiet button-small" onClick={() => setView((current) => ({ ...current, scale: Math.max(0.45, current.scale - 0.15) }))} type="button">−</button><button className="button button-quiet button-small" onClick={reset} type="button">Reset view</button></div>
      </section>
      {dataset.data?.truncated ? <div className="library-alert" role="status"><SpotIcon name="info" size={15} /><span>Showing {dataset.data.returnedTracks} of {dataset.data.totalTracks} tracks. Use filters to narrow the visualization.</span></div> : null}
      <section className="visual-workspace music-map-workspace">
        <div className="visual-canvas-panel">
          <div className="visual-panel-heading"><div><span className="eyebrow">RELATIONAL GRAPH</span><h2>Music Map</h2></div><span className="section-note">{graph.nodes.length} nodes · {graph.edges.length} edges</span></div>
          <svg aria-label="Music Map graph" className="music-map-svg" onPointerMove={move} role="group" viewBox="0 0 1000 700">
            <g transform={`translate(${view.x} ${view.y}) scale(${view.scale})`}>
              {graph.edges.map((edge) => {
                const source = positions.get(edge.source);
                const target = positions.get(edge.target);
                return source && target ? <path className="music-map-edge" d={`M ${source.x} ${source.y} C ${(source.x + target.x) / 2} ${source.y}, ${(source.x + target.x) / 2} ${target.y}, ${target.x} ${target.y}`} key={edge.id} /> : null;
              })}
              {graph.nodes.map((node) => {
                const position = positions.get(node.id);
                if (!position) return null;
                return <g aria-label={`${node.kind}: ${node.label}`} className={`music-map-node music-map-node-${node.kind}${selectedId === node.id ? " music-map-node-selected" : ""}`} key={node.id} onClick={() => selectNode(node)} role="button" tabIndex={0} transform={`translate(${position.x} ${position.y})`} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); selectNode(node); } }}><circle r={node.kind === "track" ? 7 : 10} /><text dx="15" dy="4">{node.label.length > 30 ? `${node.label.slice(0, 30)}…` : node.label}</text></g>;
              })}
            </g>
          </svg>
          <div className="visual-legend"><span><i className="legend-dot legend-genre" />Genre</span><span><i className="legend-dot legend-artist" />Artist</span><span><i className="legend-dot legend-album" />Album</span><span><i className="legend-dot legend-track" />Track</span></div>
        </div>
        <aside aria-label="Map Navigator" className="visual-navigator">
          <div className="visual-panel-heading"><div><span className="eyebrow">KEYBOARD FALLBACK</span><h2>Map Navigator</h2></div><span className="section-note">{Math.min(200, graph.nodes.length)} shown</span></div>
          <div className="visual-node-list">{graph.nodes.slice(0, 200).map((node) => <button className={`visual-node-list-item${selectedId === node.id ? " visual-node-list-item-selected" : ""}`} key={node.id} onClick={() => selectNode(node)} type="button"><span className={`visual-node-kind visual-node-kind-${node.kind}`}>{node.kind}</span><strong>{node.label}</strong></button>)}</div>
          {selectedTrack ? <div className="visual-selected-panel"><span className="eyebrow">SELECTED TRACK</span><h3>{trackLabel(selectedTrack)}</h3><button className="button button-quiet button-small" onClick={() => openInspector(selectedTrack.trackId)} type="button"><SpotIcon name="info" size={13} /> Inspect</button><VisualTrackActions track={selectedTrack} /><TrackActionDragPanel disabled={playback.pending} onInbox={() => { void addTrackToInbox(selectedTrack.trackId); }} onPlayNext={() => void playback.playNext(selectedTrack.trackId, null)} onQueue={() => void playback.addToQueue(selectedTrack.trackId, null)} playbackAllowed={selectedTrack.canPlayback} trackId={selectedTrack.trackId} /></div> : <p className="visual-helper">Select a track node to reveal actions and inspection.</p>}
        </aside>
      </section>
    </div>
  );
}
