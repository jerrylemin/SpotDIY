import { useEffect, useState } from "react";

import { SpotIcon } from "../icons/SpotIcon";

interface VolumeControlProps {
  muted: boolean;
  volumePercent: number;
  disabled?: boolean;
  pending?: boolean;
  onToggleMuted: () => void;
  onSetVolume: (volumePercent: number) => void;
}

export function VolumeControl({
  muted,
  volumePercent,
  disabled = false,
  pending = false,
  onToggleMuted,
  onSetVolume,
}: VolumeControlProps) {
  const [draftVolume, setDraftVolume] = useState(volumePercent);

  useEffect(() => {
    setDraftVolume(volumePercent);
  }, [volumePercent]);

  return (
    <div className="player-volume">
      <button
        aria-label={muted ? "Unmute" : "Mute"}
        aria-pressed={muted}
        className={`player-icon-button player-mode-button${muted ? " player-mode-button-active" : ""}`}
        disabled={disabled || pending}
        onClick={onToggleMuted}
        type="button"
      >
        <SpotIcon name={muted || draftVolume === 0 ? "mute" : "volume"} size={17} />
      </button>
      <input
        aria-label="Playback volume"
        disabled={disabled || pending}
        max={100}
        min={0}
        onBlur={() => onSetVolume(draftVolume)}
        onChange={(event) => setDraftVolume(Number(event.target.value))}
        onMouseUp={() => onSetVolume(draftVolume)}
        step={1}
        type="range"
        value={draftVolume}
      />
      <span>{draftVolume}%</span>
    </div>
  );
}
