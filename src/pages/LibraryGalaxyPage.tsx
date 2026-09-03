import { useEffect, useMemo, useRef, useState } from "react";

import { EmptyState } from "../components/common/EmptyState";
import { SpotIcon } from "../components/icons/SpotIcon";
import { buildGalaxyLayout, type GalaxyClusterMode } from "../features/library-galaxy/layout";
import { TrackActionDragPanel } from "../features/visual-exploration/TrackActionDragPanel";
import { VisualTrackActions } from "../features/visual-exploration/VisualTrackActions";
import { usePlayback } from "../hooks/usePlayback";
import { useVisualLibraryDataset } from "../hooks/useVisualLibraryDataset";
import { useUiStore } from "../stores/ui-store";
import { addTrackToInbox } from "../services/ipc";
import type { TrackId, VisualDatasetRequest, VisualTrackPoint } from "../types/domain";

const EMPTY_REQUEST: VisualDatasetRequest = { query: null, genre: null, artist: null, likedOnly: false, limit: 5_000 };
const EMPTY_TRACKS: VisualTrackPoint[] = [];

export function LibraryGalaxyPage() {
  const openInspector = useUiStore((state) => state.openTrackInspector);
  const playback = usePlayback();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const canvasWrapRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ x: number; y: number; viewX: number; viewY: number; moved: boolean } | null>(null);
  const [query, setQuery] = useState("");
  const [genre, setGenre] = useState("");
  const [artist, setArtist] = useState("");
  const [likedOnly, setLikedOnly] = useState(false);
  const [clusterMode, setClusterMode] = useState<GalaxyClusterMode>("artist");
  const [selectedId, setSelectedId] = useState<TrackId | null>(null);
  const [hoveredId, setHoveredId] = useState<TrackId | null>(null);
  const [size, setSize] = useState({ width: 900, height: 600 });
  const [view, setView] = useState({ x: 0, y: 0, scale: 1 });
  const request = { ...EMPTY_REQUEST, query: query.trim() || null, genre: genre || null, artist: artist || null, likedOnly };
  const dataset = useVisualLibraryDataset(request);
  const tracks = dataset.data?.tracks ?? EMPTY_TRACKS;
  const layout = useMemo(() => buildGalaxyLayout(tracks, size.width, size.height, clusterMode), [clusterMode, size.height, size.width, tracks]);
  const selectedTrack = tracks.find((track) => track.trackId === selectedId) ?? null;
  const hoveredTrack = tracks.find((track) => track.trackId === hoveredId) ?? null;
  const genres = useMemo(() => [...new Set(tracks.flatMap((track) => track.genres))].sort((a, b) => a.localeCompare(b)), [tracks]);
  const artists = useMemo(() => [...new Set(tracks.flatMap((track) => track.artists))].sort((a, b) => a.localeCompare(b)), [tracks]);

  useEffect(() => {
    const element = canvasWrapRef.current;
    if (!element || typeof ResizeObserver === "undefined") return undefined;
    const update = () => setSize({ width: Math.max(320, element.clientWidth), height: Math.max(380, element.clientHeight) });
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const context = canvas.getContext("2d");
    if (!context) return;
    const ratio = window.devicePixelRatio || 1;
    canvas.width = size.width * ratio;
    canvas.height = size.height * ratio;
    canvas.style.width = `${size.width}px`;
    canvas.style.height = `${size.height}px`;
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, size.width, size.height);
    const styles = getComputedStyle(document.documentElement);
    const accent = styles.getPropertyValue("--color-accent").trim() || "#D7FF60";
    const muted = styles.getPropertyValue("--color-text-muted").trim() || "#A8A7AE";
    const border = styles.getPropertyValue("--color-border").trim() || "#2E2F36";
    context.font = "11px Segoe UI, sans-serif";
    const clusterAnchors = new Map<string, (typeof layout.points)[number]>();
    for (const point of layout.points) if (!clusterAnchors.has(point.cluster)) clusterAnchors.set(point.cluster, point);
    for (const cluster of layout.clusters) {
      const point = clusterAnchors.get(cluster);
      if (!point) continue;
      const x = point.x * view.scale + view.x;
      const y = point.y * view.scale + view.y;
      context.fillStyle = muted;
      context.fillText(cluster, x + 14, y - 14);
    }
    for (const point of layout.points) {
      const x = point.x * view.scale + view.x;
      const y = point.y * view.scale + view.y;
      if (x < -20 || y < -20 || x > size.width + 20 || y > size.height + 20) continue;
      const radius = point.radius * Math.min(1.4, view.scale);
      context.beginPath();
      context.arc(x, y, radius + (point.track.audioQuality === "lossless" ? 2 : 0), 0, Math.PI * 2);
      context.strokeStyle = point.track.audioQuality === "lossless" ? accent : border;
      context.lineWidth = point.track.audioQuality === "lossless" ? 1.5 : 1;
      context.stroke();
      context.beginPath();
      context.arc(x, y, radius, 0, Math.PI * 2);
      context.fillStyle = point.track.trackId === selectedId || point.track.trackId === hoveredId
        ? accent
        : point.track.liked ? "#8E7BFF" : muted;
      context.globalAlpha = point.track.trackId === selectedId ? 1 : point.track.rating ? 0.9 : 0.65;
      context.fill();
      context.globalAlpha = 1;
    }
  }, [hoveredId, layout, selectedId, size.height, size.width, view]);

  const hit = (event: React.MouseEvent<HTMLCanvasElement>): TrackId | null => {
    const rect = event.currentTarget.getBoundingClientRect();
    const x = (event.clientX - rect.left - view.x) / view.scale;
    const y = (event.clientY - rect.top - view.y) / view.scale;
    let closest: { id: TrackId; distance: number } | null = null;
    for (const point of layout.points) {
      const distance = Math.hypot(point.x - x, point.y - y);
      if (distance <= Math.max(14, point.radius + 6) && (!closest || distance < closest.distance)) closest = { id: point.track.trackId, distance };
    }
    return closest?.id ?? null;
  };
  const reset = () => setView({ x: 0, y: 0, scale: 1 });

  if (dataset.isLoading && !dataset.data) return <div className="page-stack"><EmptyState icon="spark" eyebrow="LIBRARY GALAXY" title="Loading Library Galaxy" description="Plotting your local collection…" /></div>;
  if (dataset.isError) return <div className="page-stack"><EmptyState icon="alert" eyebrow="LIBRARY GALAXY UNAVAILABLE" title="Could not build Library Galaxy" description="The visual library dataset could not be read." action={<button className="button button-primary" onClick={() => void dataset.refetch()} type="button">Try again</button>} /></div>;
  if (tracks.length === 0) return <div className="page-stack"><EmptyState icon="spark" eyebrow="LIBRARY GALAXY" title="No tracks to plot" description="Your library has no tracks to visualize yet." /></div>;

  return (
    <div className="page-stack visual-page galaxy-page">
      <section className="page-intro"><div><span className="eyebrow">EXPLORE / LIBRARY GALAXY</span><h1>Your collection, <em>in orbit.</em></h1><p>A bounded Canvas view clusters your local tracks by a deterministic artist or genre orbit.</p></div><div className="page-intro-stat"><strong>{layout.points.length}</strong><span>points plotted</span></div></section>
      <section aria-label="Library Galaxy filters" className="visual-toolbar">
        <label><span>Search</span><input onChange={(event) => setQuery(event.target.value)} placeholder="Title, artist, album…" value={query} /></label>
        <label><span>Genre</span><select onChange={(event) => setGenre(event.target.value)} value={genre}><option value="">All genres</option>{genres.map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
        <label><span>Artist</span><select onChange={(event) => setArtist(event.target.value)} value={artist}><option value="">All artists</option>{artists.map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
        <button aria-pressed={likedOnly} className={`button button-small ${likedOnly ? "button-primary" : "button-quiet"}`} onClick={() => setLikedOnly((value) => !value)} type="button">Liked only</button>
        <div className="visual-segmented" aria-label="Galaxy cluster mode"><button aria-pressed={clusterMode === "artist"} className="button button-small button-quiet" onClick={() => setClusterMode("artist")} type="button">Primary Artist</button><button aria-pressed={clusterMode === "genre"} className="button button-small button-quiet" onClick={() => setClusterMode("genre")} type="button">Genre</button></div>
        <div className="visual-view-actions"><button className="button button-quiet button-small" onClick={() => setView((current) => ({ ...current, scale: Math.min(3, current.scale + 0.15) }))} type="button">＋</button><button className="button button-quiet button-small" onClick={() => setView((current) => ({ ...current, scale: Math.max(0.45, current.scale - 0.15) }))} type="button">−</button><button className="button button-quiet button-small" onClick={reset} type="button">Reset</button></div>
      </section>
      {dataset.data?.truncated ? <div className="library-alert" role="status"><SpotIcon name="info" size={15} /><span>Showing {dataset.data.returnedTracks} of {dataset.data.totalTracks} tracks. Use filters to narrow the visualization.</span></div> : null}
      <section className="visual-workspace galaxy-workspace">
        <div className="visual-canvas-panel"><div className="visual-panel-heading"><div><span className="eyebrow">CANVAS 2D</span><h2>Library Galaxy</h2></div><span className="section-note">{layout.clusters.length} clusters</span></div><div className="galaxy-canvas-wrap" ref={canvasWrapRef}><canvas aria-label="Library Galaxy canvas" onClick={(event) => { const id = hit(event); if (id) { setSelectedId(id); openInspector(id); } }} onPointerDown={(event) => { event.currentTarget.setPointerCapture(event.pointerId); dragRef.current = { x: event.clientX, y: event.clientY, viewX: view.x, viewY: view.y, moved: false }; }} onPointerMove={(event) => { const drag = dragRef.current; if (drag && event.buttons === 1) { if (Math.abs(event.clientX - drag.x) + Math.abs(event.clientY - drag.y) > 4) drag.moved = true; setView((current) => ({ ...current, x: drag.viewX + event.clientX - drag.x, y: drag.viewY + event.clientY - drag.y })); } else setHoveredId(hit(event)); }} onPointerUp={() => { dragRef.current = null; }} onPointerLeave={() => setHoveredId(null)} ref={canvasRef} role="img" tabIndex={0} /></div><div className="visual-hover-readout" aria-live="polite">{hoveredTrack ? `${hoveredTrack.title} · ${hoveredTrack.primaryArtist}` : "Hover a point for track details."}</div></div>
        <aside aria-label="Galaxy Navigator" className="visual-navigator"><div className="visual-panel-heading"><div><span className="eyebrow">KEYBOARD FALLBACK</span><h2>Galaxy Navigator</h2></div><span className="section-note">{Math.min(200, tracks.length)} shown</span></div><div className="visual-node-list">{tracks.slice(0, 200).map((track) => <button className={`visual-node-list-item${selectedId === track.trackId ? " visual-node-list-item-selected" : ""}`} key={track.trackId} onClick={() => { setSelectedId(track.trackId); openInspector(track.trackId); }} type="button"><span className="visual-node-kind visual-node-kind-track">track</span><strong>{track.title}</strong><small>{track.primaryArtist}</small></button>)}</div>{selectedTrack ? <div className="visual-selected-panel"><span className="eyebrow">SELECTED TRACK</span><h3>{selectedTrack.title}</h3><p>{selectedTrack.primaryArtist}{selectedTrack.album ? ` · ${selectedTrack.album}` : ""}</p><button className="button button-quiet button-small" onClick={() => openInspector(selectedTrack.trackId)} type="button"><SpotIcon name="info" size={13} /> Inspect</button><VisualTrackActions track={selectedTrack} /><TrackActionDragPanel disabled={playback.pending} onInbox={() => { void addTrackToInbox(selectedTrack.trackId); }} onPlayNext={() => void playback.playNext(selectedTrack.trackId, null)} onQueue={() => void playback.addToQueue(selectedTrack.trackId, null)} trackId={selectedTrack.trackId} /></div> : <p className="visual-helper">Select a point to reveal actions and inspection.</p>}</aside>
      </section>
    </div>
  );
}
