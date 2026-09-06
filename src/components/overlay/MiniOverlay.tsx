import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { usePlayback } from "../../hooks/usePlayback";
import { usePlaybackClock } from "../../hooks/usePlaybackClock";
import { useWindowsIntegration } from "../../hooks/useWindowsIntegration";
import { isTauriRuntime } from "../../services/ipc";
import { SpotIcon } from "../icons/SpotIcon";
import { ProgressControl } from "../player/ProgressControl";
import { VolumeControl } from "../player/VolumeControl";
import { OverlayFrame, OverlayTransport } from "./OverlayFrame";

function artworkSource(path: string | null): string | null {
  if (!path) return null;
  return isTauriRuntime() ? convertFileSrc(path, "asset") : path;
}

export function MiniOverlay() {
  const playback = usePlayback();
  const windows = useWindowsIntegration();
  const [artworkFailed, setArtworkFailed] = useState(false);
  const snapshot = playback.snapshot;
  const visualPositionMs = usePlaybackClock(snapshot);
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
          <ProgressControl
            disabled={!hasTrack || snapshot.phase === "failed" || snapshot.phase === "recovering"}
            durationMs={snapshot.durationMs}
            onSeek={(positionMs) => { void playback.seekPlayback(positionMs); }}
            pending={playback.pending}
            positionMs={visualPositionMs}
          />
        </div>
        <div className="spot-overlay-compact-controls">
          <OverlayTransport
            canTransport={canTransport}
            onNext={() => { void playback.nextTrack(); }}
            onPrevious={() => { void playback.previousTrack(); }}
            onToggle={() => { void playback.togglePlayPause(); }}
            pending={playback.pending}
            playing={snapshot.phase === "playing" || snapshot.phase === "seeking"}
          />
          <VolumeControl
            disabled={snapshot.phase === "failed" || snapshot.phase === "recovering"}
            muted={snapshot.muted}
            onSetVolume={(volumePercent) => { void playback.setVolume(volumePercent); }}
            onToggleMuted={() => { void playback.toggleMuted(); }}
            pending={playback.pending}
            volumePercent={snapshot.volumePercent}
          />
        </div>
      </div>
    </OverlayFrame>
  );
}
