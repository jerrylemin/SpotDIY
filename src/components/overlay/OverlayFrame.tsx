import type { MouseEvent, ReactNode } from "react";

import { isTauriRuntime } from "../../services/ipc";
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
    <section
      aria-label={`${title} overlay`}
      className={`spot-overlay spot-overlay-${kind}`}
      data-overlay-kind={kind}
      onMouseDown={(event) => { void startOverlayDrag(event); }}
    >
      <button aria-label={`Close ${title} overlay`} className="spot-overlay-close" onClick={onClose} type="button">
        <SpotIcon name="close" size={16} />
      </button>
      {children}
    </section>
  );
}

async function startOverlayDrag(event: MouseEvent<HTMLElement>) {
  if (event.button !== 0 || (event.target instanceof Element && event.target.closest("button, input, a, select, textarea"))) {
    return;
  }
  if (!isTauriRuntime()) {
    return;
  }
  event.preventDefault();
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().startDragging();
  } catch {
    // The overlay remains usable if the native drag command is unavailable.
  }
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
