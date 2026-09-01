import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { isTauriRuntime } from "../../services/ipc";
import { usePlayback } from "../../hooks/usePlayback";
import { useUiStore } from "../../stores/ui-store";
import { SourceSwitcher } from "./SourceSwitcher";
import { ProgressControl } from "./ProgressControl";
import { VolumeControl } from "./VolumeControl";
import { SpotIcon } from "../icons/SpotIcon";

function phaseLabel(phase: ReturnType<typeof usePlayback>["snapshot"]["phase"]): string {
  if (phase === "playing") return "Playing";
  if (phase === "loading" || phase === "seeking") return "Loading";
  if (phase === "failed") return "Unavailable";
  if (phase === "recovering") return "Recovering";
  return "Paused";
}

export function MiniPlayer() {
  const playback = usePlayback();
  const queueDrawerOpen = useUiStore((state) => state.queueDrawerOpen);
  const setQueueDrawerOpen = useUiStore((state) => state.setQueueDrawerOpen);
  const setPlayerMode = useUiStore((state) => state.setPlayerMode);
  const openTrackInspector = useUiStore((state) => state.openTrackInspector);
  const [artworkFailed, setArtworkFailed] = useState(false);
  const snapshot = playback.snapshot;
  const hasTrack = snapshot.currentTrackId !== null;
  const queueReady = hasTrack || snapshot.queueLength > 0;
  const artworkSource = snapshot.artworkPath
    ? isTauriRuntime()
      ? convertFileSrc(snapshot.artworkPath, "asset")
      : snapshot.artworkPath
    : null;

  useEffect(() => setArtworkFailed(false), [artworkSource]);

  return (
    <footer aria-label="Mini now playing" className="player-bar player-bar-mini">
      <button
        aria-label={hasTrack ? `Inspect ${snapshot.title ?? "current track"}` : "Inspect current track"}
        className="mini-player-art player-art-placeholder"
        disabled={!hasTrack}
        onClick={() => { if (snapshot.currentTrackId) openTrackInspector(snapshot.currentTrackId); }}
        type="button"
      >
        {artworkSource && !artworkFailed ? <img alt="" onError={() => setArtworkFailed(true)} src={artworkSource} /> : <SpotIcon name={hasTrack ? "library" : "play"} size={18} />}
      </button>
      <div className="mini-player-copy">
        <span className={`player-phase-chip player-phase-${snapshot.phase}`}>{phaseLabel(snapshot.phase)}</span>
        <strong title={snapshot.title ?? undefined}>{snapshot.title ?? "Nothing queued"}</strong>
        <span>{hasTrack ? snapshot.artists.join(" · ") || "Unknown artist" : "Choose a local track to start listening."}</span>
        <ProgressControl disabled={!hasTrack || snapshot.phase === "failed" || snapshot.phase === "recovering"} durationMs={snapshot.durationMs} onSeek={(positionMs) => { void playback.seekPlayback(positionMs); }} pending={playback.pending} positionMs={snapshot.positionMs} />
      </div>
      <div className="mini-player-controls" aria-label="Mini player controls">
        <button aria-label="Previous track" className="player-icon-button" disabled={!queueReady || playback.pending} onClick={() => { void playback.previousTrack(); }} type="button"><SpotIcon name="previous" size={17} /></button>
        <button aria-label={snapshot.phase === "playing" ? "Pause" : "Play"} className="player-play-button" disabled={!queueReady || playback.pending} onClick={() => { void playback.togglePlayPause(); }} type="button"><SpotIcon name={snapshot.phase === "playing" ? "pause" : "play"} size={15} /></button>
        <button aria-label="Next track" className="player-icon-button" disabled={!queueReady || playback.pending} onClick={() => { void playback.nextTrack(); }} type="button"><SpotIcon name="next" size={17} /></button>
      </div>
      <SourceSwitcher
        currentSourceId={snapshot.currentSourceId}
        disabled={!hasTrack || playback.pending}
        onSwitch={(sourceId) => { if (snapshot.currentTrackId) void playback.switchSource(snapshot.currentTrackId, sourceId); }}
        sources={snapshot.sources}
      />
      <VolumeControl disabled={snapshot.phase === "failed" || snapshot.phase === "recovering"} muted={snapshot.muted} onSetVolume={(volumePercent) => { void playback.setVolume(volumePercent); }} onToggleMuted={() => { void playback.toggleMuted(); }} pending={playback.pending} volumePercent={snapshot.volumePercent} />
      <div className="mini-player-actions">
        <button aria-expanded={queueDrawerOpen} aria-label="Open queue" className="button button-quiet button-small" onClick={() => setQueueDrawerOpen(!queueDrawerOpen)} type="button"><SpotIcon name="queue" size={14} /> Queue</button>
        <button aria-label="Open expanded now playing" className="icon-button" onClick={() => setPlayerMode("expanded")} title="Open expanded now playing" type="button"><SpotIcon name="expand" size={17} /></button>
        <button aria-label="Use standard player" className="icon-button" onClick={() => setPlayerMode("standard")} title="Use standard player" type="button"><SpotIcon name="collapse" size={17} /></button>
      </div>
    </footer>
  );
}
