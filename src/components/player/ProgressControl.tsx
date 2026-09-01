import { useEffect, useState } from "react";

import type { AbLoopState } from "../../types/domain";

interface ProgressControlProps {
  positionMs: number;
  durationMs: number | null;
  disabled?: boolean;
  pending?: boolean;
  onSeek: (positionMs: number) => void;
  bookmarkPositions?: number[];
  abLoop?: AbLoopState;
}

function formatClock(durationMs: number): string {
  const totalSeconds = Math.floor(durationMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}

export function ProgressControl({
  positionMs,
  durationMs,
  disabled = false,
  pending = false,
  onSeek,
  bookmarkPositions = [],
  abLoop,
}: ProgressControlProps) {
  const safeDuration = durationMs ?? 0;
  const [draftPosition, setDraftPosition] = useState(positionMs);
  const [scrubbing, setScrubbing] = useState(false);

  useEffect(() => {
    if (!scrubbing) {
      setDraftPosition(positionMs);
    }
  }, [positionMs, scrubbing]);

  const commitSeek = () => {
    if (!scrubbing) {
      return;
    }
    setScrubbing(false);
    onSeek(draftPosition);
  };

  return (
    <label className="player-progress" aria-label="Playback progress">
      <span>{formatClock(draftPosition)}</span>
      <span className="player-progress-track">
        <input
          aria-label="Seek within current track"
          disabled={disabled || pending || safeDuration === 0}
          max={safeDuration}
          min={0}
          onBlur={commitSeek}
          onChange={(event) => {
            setScrubbing(true);
            setDraftPosition(Number(event.target.value));
          }}
          onKeyUp={(event) => {
            if (event.key === "ArrowLeft" || event.key === "ArrowRight" || event.key === "Home" || event.key === "End") {
              commitSeek();
            }
          }}
          onMouseUp={commitSeek}
          step={1000}
          type="range"
          value={Math.min(draftPosition, safeDuration)}
        />
        {safeDuration > 0 ? bookmarkPositions.map((positionMs, index) => (
          <span
            aria-hidden="true"
            className="player-progress-marker player-progress-marker-bookmark"
            key={`bookmark-${positionMs}-${index}`}
            style={{ left: `${Math.min(100, Math.max(0, positionMs / safeDuration * 100))}%` }}
          />
        )) : null}
        {safeDuration > 0 && abLoop?.aMs !== null && abLoop?.aMs !== undefined ? (
          <span aria-hidden="true" className="player-progress-marker player-progress-marker-a" style={{ left: `${Math.min(100, Math.max(0, abLoop.aMs / safeDuration * 100))}%` }} />
        ) : null}
        {safeDuration > 0 && abLoop?.bMs !== null && abLoop?.bMs !== undefined ? (
          <span aria-hidden="true" className="player-progress-marker player-progress-marker-b" style={{ left: `${Math.min(100, Math.max(0, abLoop.bMs / safeDuration * 100))}%` }} />
        ) : null}
      </span>
      <span>{formatClock(safeDuration)}</span>
    </label>
  );
}
