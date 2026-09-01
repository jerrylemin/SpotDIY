import { convertFileSrc } from "@tauri-apps/api/core";
import { Link } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { usePlayback } from "../../hooks/usePlayback";
import { useBookmarks } from "../../hooks/useLyrics";
import { useQueue } from "../../hooks/useQueue";
import { useTrackInspector } from "../../hooks/useTrackInspector";
import { isTauriRuntime } from "../../services/ipc";
import { useUiStore } from "../../stores/ui-store";
import { SourceSwitcher } from "./SourceSwitcher";
import { PlaybackControls } from "./PlaybackControls";
import { ProgressControl } from "./ProgressControl";
import { VolumeControl } from "./VolumeControl";
import { SpotIcon } from "../icons/SpotIcon";

function formatDuration(durationMs: number | null): string {
  if (durationMs === null) return "—";
  const seconds = Math.floor(durationMs / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function qualityFacts(source: NonNullable<ReturnType<typeof useTrackInspector>["data"]>["sources"][number] | null): string[] {
  if (!source) return [];
  return Array.from(new Set([
    source.quality.codec,
    source.quality.container,
    source.quality.bitrateKbps === null ? null : `${source.quality.bitrateKbps} kbps`,
    source.quality.sampleRateHz === null ? null : `${source.quality.sampleRateHz / 1000} kHz`,
    source.quality.bitDepth === null ? null : `${source.quality.bitDepth}-bit`,
  ].filter((value): value is string => Boolean(value))));
}

export function NowPlayingPanel() {
  const playback = usePlayback();
  const queue = useQueue();
  const bookmarks = useBookmarks(playback.snapshot.currentTrackId);
  const setPlayerMode = useUiStore((state) => state.setPlayerMode);
  const setQueueDrawerOpen = useUiStore((state) => state.setQueueDrawerOpen);
  const openTrackInspector = useUiStore((state) => state.openTrackInspector);
  const [artworkFailed, setArtworkFailed] = useState(false);
  const snapshot = playback.snapshot;
  const hasTrack = snapshot.currentTrackId !== null;
  const inspector = useTrackInspector(snapshot.currentTrackId, hasTrack);
  const inspectorSource = inspector.data?.sources.find((source) => source.sourceId === snapshot.currentSourceId) ?? null;
  const artworkSource = snapshot.artworkPath
    ? isTauriRuntime()
      ? convertFileSrc(snapshot.artworkPath, "asset")
      : snapshot.artworkPath
    : null;

  useEffect(() => setArtworkFailed(false), [artworkSource]);

  return (
    <section aria-label="Expanded now playing" aria-modal="false" className="now-playing-panel" role="dialog">
      <header className="now-playing-header">
        <div><span className="eyebrow accent-eyebrow">NOW PLAYING</span><h2>{hasTrack ? snapshot.title : "Nothing queued"}</h2><p>{hasTrack ? snapshot.artists.join(" · ") || "Unknown artist" : "Choose a local track to start listening."}</p></div>
        <div className="now-playing-header-actions">
          <button aria-label="Open queue" className="button button-quiet button-small" onClick={() => setQueueDrawerOpen(true)} type="button"><SpotIcon name="queue" size={14} /> Queue</button>
          <button aria-label="Use standard player" className="icon-button" onClick={() => setPlayerMode("standard")} title="Use standard player" type="button"><SpotIcon name="collapse" size={17} /></button>
          <button aria-label="Close expanded now playing" className="icon-button" onClick={() => setPlayerMode("standard")} title="Close expanded now playing" type="button"><SpotIcon name="close" size={18} /></button>
        </div>
      </header>

      <div className="now-playing-grid">
        <div className="now-playing-artwork">
          {artworkSource && !artworkFailed ? <img alt={`${snapshot.title ?? "Track"} artwork`} onError={() => setArtworkFailed(true)} src={artworkSource} /> : <SpotIcon name={hasTrack ? "library" : "play"} size={54} />}
          <span className={`player-phase-chip player-phase-${snapshot.phase}`}>{snapshot.phase === "idle" ? "Nothing queued" : snapshot.phase}</span>
        </div>
        <div className="now-playing-controls">
          <PlaybackControls
            disabled={playback.initializing}
            onClearQueue={() => { void playback.clearQueue(); }}
            onCycleRepeat={() => { void playback.cycleRepeatMode(); }}
            onNext={() => { void playback.nextTrack(); }}
            onPrevious={() => { void playback.previousTrack(); }}
            onTogglePlayPause={() => { void playback.togglePlayPause(); }}
            onToggleShuffle={() => { void playback.toggleShuffle(); }}
            pending={playback.pending}
            snapshot={snapshot}
          />
          <ProgressControl
            abLoop={snapshot.abLoop}
            bookmarkPositions={bookmarks.data?.map((bookmark) => bookmark.positionMs)}
            disabled={!hasTrack || snapshot.phase === "failed" || snapshot.phase === "recovering"}
            durationMs={snapshot.durationMs}
            onSeek={(positionMs) => { void playback.seekPlayback(positionMs); }}
            pending={playback.pending}
            positionMs={snapshot.positionMs}
          />
          <SourceSwitcher
            currentSourceId={snapshot.currentSourceId}
            disabled={!hasTrack || playback.pending}
            onSwitch={(sourceId) => { if (snapshot.currentTrackId) void playback.switchSource(snapshot.currentTrackId, sourceId); }}
            sources={snapshot.sources}
          />
          <VolumeControl disabled={snapshot.phase === "failed" || snapshot.phase === "recovering"} muted={snapshot.muted} onSetVolume={(volumePercent) => { void playback.setVolume(volumePercent); }} onToggleMuted={() => { void playback.toggleMuted(); }} pending={playback.pending} volumePercent={snapshot.volumePercent} />
        </div>
      </div>

      <div className="now-playing-lower-grid">
        <div className="now-playing-facts">
          <span className="eyebrow">SOURCE CONTEXT</span>
          <div><span>Album</span><strong>{snapshot.album ?? "Unavailable"}</strong></div>
          <div><span>Duration</span><strong>{formatDuration(snapshot.durationMs)}</strong></div>
          <div><span>Queue position</span><strong>{snapshot.queueLength === 0 || snapshot.queueIndex === null ? "—" : `${snapshot.queueIndex + 1} of ${snapshot.queueLength}`}</strong></div>
          {snapshot.error ? <p className="now-playing-error" role="alert">{snapshot.error.summary}</p> : null}
          <div className="now-playing-quality">
            <span className="eyebrow">QUALITY / PROVENANCE</span>
            {inspector.isLoading ? <span className="inspector-muted">Reading current source details…</span> : inspectorSource ? <><div><span>Provider</span><strong>{inspectorSource.provider}</strong></div><div><span>Availability</span><strong>{inspectorSource.available ? "Available" : inspectorSource.availabilityDetail ?? "Unavailable"}</strong></div><div><span>Version</span><strong>{inspectorSource.versionQualifiers.join(" · ") || "Unspecified"}</strong></div><div className="now-playing-quality-facts">{qualityFacts(inspectorSource).map((fact) => <span key={fact}>{fact}</span>)}</div></> : <span className="inspector-muted">Quality is unavailable for this source.</span>}
          </div>
          <div className="now-playing-fact-actions">
            <Link className="button button-quiet button-small" to="/lyrics"><SpotIcon name="lyrics" size={14} /> Lyrics</Link>
            {hasTrack ? <button className="button button-quiet button-small" onClick={() => openTrackInspector(snapshot.currentTrackId!)} type="button"><SpotIcon name="info" size={14} /> Inspect track</button> : null}
          </div>
        </div>
        <div className="now-playing-queue-preview">
          <div className="now-playing-subheading"><span className="eyebrow">UP NEXT</span><span>{queue.workspace.upNext.length} queued</span></div>
          {queue.workspace.upNext.slice(0, 4).map((entry) => <div className="now-playing-queue-entry" key={entry.id}><strong>{entry.title ?? `Track ${entry.trackId}`}</strong><span>{entry.artists.join(" · ") || "Unknown artist"}</span></div>)}
          {queue.workspace.upNext.length === 0 ? <p className="inspector-muted">Nothing is waiting in the persistent queue.</p> : null}
        </div>
      </div>
    </section>
  );
}
