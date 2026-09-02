import type { ReactNode } from "react";

import { SpotIcon } from "../icons/SpotIcon";
import type { OverlayKind } from "../../types/domain";

interface OverlayFrameProps {
  kind: OverlayKind;
  title: string;
  onClose: () => void;
  children: ReactNode;
}

export function OverlayFrame({ kind, title, onClose, children }: OverlayFrameProps) {
  return (
    <section aria-label={`${title} overlay`} className={`spot-overlay spot-overlay-${kind}`} data-overlay-kind={kind}>
      <header className="spot-overlay-header">
        <span className="spot-overlay-kicker">SpotDIY · {title}</span>
        <button aria-label={`Close ${title} overlay`} className="spot-overlay-close" onClick={onClose} type="button">
          <SpotIcon name="close" size={16} />
        </button>
      </header>
      {children}
    </section>
  );
}

export function OverlayTransport({
  canTransport,
  playing,
  pending,
  onPrevious,
  onToggle,
  onNext,
}: {
  canTransport: boolean;
  playing: boolean;
  pending: boolean;
  onPrevious: () => void;
  onToggle: () => void;
  onNext: () => void;
}) {
  const disabled = pending || !canTransport;
  return (
    <div aria-label="Overlay playback controls" className="spot-overlay-transport">
      <button aria-label="Previous track" className="spot-overlay-control" disabled={disabled} onClick={onPrevious} type="button"><SpotIcon name="previous" size={16} /></button>
      <button aria-label={playing ? "Pause" : "Play"} className="spot-overlay-play" disabled={disabled} onClick={onToggle} type="button"><SpotIcon name={playing ? "pause" : "play"} size={15} /></button>
      <button aria-label="Next track" className="spot-overlay-control" disabled={disabled} onClick={onNext} type="button"><SpotIcon name="next" size={16} /></button>
    </div>
  );
}

export function OverlayProgress({ positionMs, durationMs }: { positionMs: number; durationMs: number | null }) {
  const percentage = durationMs && durationMs > 0 ? Math.min(100, Math.max(0, positionMs / durationMs * 100)) : 0;
  return <div aria-label="Playback progress" className="spot-overlay-progress"><span style={{ width: `${percentage}%` }} /></div>;
}
