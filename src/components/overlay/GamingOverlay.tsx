import { useState } from "react";

import { usePlayback } from "../../hooks/usePlayback";
import { useWindowsIntegration } from "../../hooks/useWindowsIntegration";
import { SpotIcon } from "../icons/SpotIcon";
import { OverlayFrame, OverlayProgress } from "./OverlayFrame";

export function GamingOverlay() {
  const playback = usePlayback();
  const windows = useWindowsIntegration();
  const [clickThroughBusy, setClickThroughBusy] = useState(false);
  const snapshot = playback.snapshot;
  const integration = windows.snapshot;
  const clickThrough = integration?.gamingClickThrough ?? false;
  const canTransport = snapshot.currentTrackId !== null || snapshot.queueLength > 0;
  const playing = snapshot.phase === "playing" || snapshot.phase === "seeking";

  async function toggleClickThrough() {
    setClickThroughBusy(true);
    try {
      await windows.setGamingClickThrough(!clickThrough);
    } catch {
      // The Windows hook keeps the actionable native error visible.
    } finally {
      setClickThroughBusy(false);
    }
  }

  return (
    <OverlayFrame kind="gaming" title="Gaming" onClose={() => { void windows.closeOverlay("gaming"); }}>
      <div className="spot-overlay-gaming-row">
        <div className="spot-overlay-gaming-copy">
          <strong>{snapshot.title ?? "Nothing queued"}</strong>
          <span>{snapshot.artists.join(" · ") || "Choose a track"}</span>
          <OverlayProgress durationMs={snapshot.durationMs} positionMs={snapshot.positionMs} />
        </div>
        <div className="spot-overlay-gaming-actions">
          <button aria-label={playing ? "Pause" : "Play"} className="spot-overlay-play" disabled={!canTransport || playback.pending} onClick={() => { void playback.togglePlayPause(); }} type="button"><SpotIcon name={playing ? "pause" : "play"} size={14} /></button>
          <button aria-label="Next track" className="spot-overlay-control" disabled={!canTransport || playback.pending} onClick={() => { void playback.nextTrack(); }} type="button"><SpotIcon name="next" size={15} /></button>
        </div>
      </div>
      <div className="spot-overlay-gaming-footer">
        <span className={`spot-overlay-state${clickThrough ? " spot-overlay-state-active" : ""}`}><span className="spot-overlay-state-dot" /> {clickThrough ? "Click-through on" : "Interactive"}</span>
        <button className="spot-overlay-toggle" disabled={clickThroughBusy || windows.loading} onClick={() => { void toggleClickThrough(); }} type="button">{clickThrough ? "Disable click-through" : "Enable click-through"}</button>
      </div>
      <p className="spot-overlay-warning">Best with windowed or borderless games. Exclusive fullscreen can cover desktop overlays.</p>
      {windows.error ? <p className="spot-overlay-error" role="alert">{windows.error}</p> : null}
    </OverlayFrame>
  );
}
