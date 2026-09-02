import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { usePlayback } from "../../hooks/usePlayback";
import { useWindowsIntegration } from "../../hooks/useWindowsIntegration";
import { isTauriRuntime } from "../../services/ipc";
import { SpotIcon } from "../icons/SpotIcon";
import { OverlayFrame, OverlayProgress, OverlayTransport } from "./OverlayFrame";

function artworkSource(path: string | null): string | null {
  if (!path) return null;
  return isTauriRuntime() ? convertFileSrc(path, "asset") : path;
}

export function MiniOverlay() {
  const playback = usePlayback();
  const windows = useWindowsIntegration();
  const [artworkFailed, setArtworkFailed] = useState(false);
  const snapshot = playback.snapshot;
  const hasTrack = snapshot.currentTrackId !== null;
  const source = artworkSource(snapshot.artworkPath);
  const canTransport = hasTrack || snapshot.queueLength > 0;

  useEffect(() => setArtworkFailed(false), [source]);

  return (
    <OverlayFrame kind="mini" title="Mini" onClose={() => { void windows.closeOverlay("mini"); }}>
      <div className="spot-overlay-main-row">
        <div className="spot-overlay-artwork">
          {source && !artworkFailed ? <img alt="" onError={() => setArtworkFailed(true)} src={source} /> : <SpotIcon name={hasTrack ? "library" : "play"} size={22} />}
        </div>
        <div className="spot-overlay-track-copy">
          <strong>{snapshot.title ?? "Nothing queued"}</strong>
          <span>{hasTrack ? snapshot.artists.join(" · ") || "Unknown artist" : "Choose a local track to start listening."}</span>
          <OverlayProgress durationMs={snapshot.durationMs} positionMs={snapshot.positionMs} />
        </div>
        <OverlayTransport
          canTransport={canTransport}
          onNext={() => { void playback.nextTrack(); }}
          onPrevious={() => { void playback.previousTrack(); }}
          onToggle={() => { void playback.togglePlayPause(); }}
          pending={playback.pending}
          playing={snapshot.phase === "playing" || snapshot.phase === "seeking"}
        />
      </div>
    </OverlayFrame>
  );
}
