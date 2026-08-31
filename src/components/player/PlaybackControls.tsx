import type { PlaybackSnapshot } from "../../types/domain";
import { SpotIcon } from "../icons/SpotIcon";

interface PlaybackControlsProps {
  snapshot: PlaybackSnapshot;
  disabled?: boolean;
  pending?: boolean;
  onPrevious: () => void;
  onTogglePlayPause: () => void;
  onNext: () => void;
  onToggleShuffle: () => void;
  onCycleRepeat: () => void;
  onClearQueue: () => void;
}

function repeatLabel(mode: PlaybackSnapshot["repeatMode"]): string {
  switch (mode) {
    case "one":
      return "Repeat one";
    case "all":
      return "Repeat all";
    default:
      return "Repeat off";
  }
}

export function PlaybackControls({
  snapshot,
  disabled = false,
  pending = false,
  onPrevious,
  onTogglePlayPause,
  onNext,
  onToggleShuffle,
  onCycleRepeat,
  onClearQueue,
}: PlaybackControlsProps) {
  const queueReady = snapshot.currentTrackId !== null || snapshot.queueLength > 0;
  const transportDisabled = disabled || pending || !queueReady;
  const playLabel = snapshot.phase === "playing"
    ? "Pause"
    : snapshot.phase === "ended"
      ? "Replay"
      : "Play";

  return (
    <div className="playback-controls-stack">
      <div className="player-controls">
        <button
          aria-label="Toggle shuffle"
          aria-pressed={snapshot.shuffleEnabled}
          className={`player-icon-button player-mode-button${snapshot.shuffleEnabled ? " player-mode-button-active" : ""}`}
          disabled={disabled || pending || snapshot.queueLength < 2}
          onClick={onToggleShuffle}
          type="button"
        >
          <SpotIcon name="shuffle" size={17} />
        </button>
        <button
          aria-label="Previous track"
          className="player-icon-button"
          disabled={transportDisabled}
          onClick={onPrevious}
          type="button"
        >
          <SpotIcon name="previous" size={18} />
        </button>
        <button
          aria-label={playLabel}
          className={`player-play-button${transportDisabled ? " player-play-button-disabled" : ""}`}
          disabled={transportDisabled}
          onClick={onTogglePlayPause}
          type="button"
        >
          <SpotIcon name={snapshot.phase === "playing" ? "pause" : "play"} size={17} />
        </button>
        <button
          aria-label="Next track"
          className="player-icon-button"
          disabled={transportDisabled}
          onClick={onNext}
          type="button"
        >
          <SpotIcon name="next" size={18} />
        </button>
        <button
          aria-label={repeatLabel(snapshot.repeatMode)}
          className={`player-icon-button player-mode-button${snapshot.repeatMode !== "off" ? " player-mode-button-active" : ""}`}
          disabled={disabled || pending || snapshot.queueLength === 0}
          onClick={onCycleRepeat}
          type="button"
        >
          <SpotIcon name="repeat" size={17} />
        </button>
      </div>
      <div className="player-controls-meta">
        <span>{snapshot.queueLength === 0 ? "Queue empty" : `Queue ${snapshot.queueIndex === null ? "—" : snapshot.queueIndex + 1} of ${snapshot.queueLength}`}</span>
        <button
          className="player-meta-action"
          disabled={disabled || pending || snapshot.queueLength === 0}
          onClick={onClearQueue}
          type="button"
        >
          Clear queue
        </button>
      </div>
    </div>
  );
}
