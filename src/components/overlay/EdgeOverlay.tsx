import { usePlayback } from "../../hooks/usePlayback";
import { useWindowsIntegration } from "../../hooks/useWindowsIntegration";
import { SpotIcon } from "../icons/SpotIcon";
import { OverlayFrame, OverlayProgress } from "./OverlayFrame";

export function EdgeOverlay() {
  const playback = usePlayback();
  const windows = useWindowsIntegration();
  const snapshot = playback.snapshot;
  const canTransport = snapshot.currentTrackId !== null || snapshot.queueLength > 0;
  const playing = snapshot.phase === "playing" || snapshot.phase === "seeking";

  return (
    <OverlayFrame kind="edge" title="Edge" onClose={() => { void windows.closeOverlay("edge"); }}>
      <div className="spot-overlay-edge-row">
        <div className="spot-overlay-edge-copy">
          <strong>{snapshot.title ?? "Nothing queued"}</strong>
          <span>{snapshot.artists.join(" · ") || "Choose a track"}</span>
          <OverlayProgress durationMs={snapshot.durationMs} positionMs={snapshot.positionMs} />
        </div>
        <div className="spot-overlay-edge-actions">
          <button aria-label={playing ? "Pause" : "Play"} className="spot-overlay-play" disabled={!canTransport || playback.pending} onClick={() => { void playback.togglePlayPause(); }} type="button"><SpotIcon name={playing ? "pause" : "play"} size={14} /></button>
          <button aria-label="Next track" className="spot-overlay-control" disabled={!canTransport || playback.pending} onClick={() => { void playback.nextTrack(); }} type="button"><SpotIcon name="next" size={15} /></button>
          <label className="spot-overlay-volume" title="Playback volume">
            <SpotIcon name={snapshot.muted || snapshot.volumePercent === 0 ? "mute" : "volume"} size={15} />
            <input aria-label="Playback volume" disabled={playback.pending} max={100} min={0} onChange={(event) => { void playback.setVolume(Number(event.target.value)); }} type="range" value={snapshot.volumePercent} />
          </label>
        </div>
      </div>
    </OverlayFrame>
  );
}
