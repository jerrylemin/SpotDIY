import { DndContext, useDraggable, useDroppable, type DragEndEvent } from "@dnd-kit/core";
import { CSS } from "@dnd-kit/utilities";
import type { TrackId } from "../../types/domain";
import { resolveVisualDrop, type VisualDropAction } from "./drag-actions";

interface TrackActionDragPanelProps {
  disabled?: boolean;
  onInbox: () => void;
  onPlayNext: () => void;
  onQueue: () => void;
  trackId: TrackId;
}

function DropTarget({ action, onDrop }: { action: VisualDropAction; onDrop: (action: VisualDropAction) => void }) {
  const { isOver, setNodeRef } = useDroppable({ id: action });
  const labels: Record<VisualDropAction, string> = { "play-next": "PLAY NEXT", queue: "ADD TO QUEUE", inbox: "INBOX" };
  return <button className={`visual-drop-target${isOver ? " visual-drop-target-over" : ""}`} onClick={() => onDrop(action)} ref={setNodeRef} type="button">{labels[action]}</button>;
}

function DragChip({ disabled, trackId }: { disabled?: boolean; trackId: TrackId }) {
  const { attributes, listeners, setNodeRef, transform } = useDraggable({ id: `visual-track:${trackId}`, disabled });
  return <button
    aria-label="Drag selected track to an action target"
    className="visual-drag-chip"
    disabled={disabled}
    ref={setNodeRef}
    style={{ transform: CSS.Translate.toString(transform) }}
    type="button"
    {...attributes}
    {...listeners}
  >
    <span aria-hidden="true">⠿</span> Drag track
  </button>;
}

export function TrackActionDragPanel({ disabled, onInbox, onPlayNext, onQueue, trackId }: TrackActionDragPanelProps) {
  const run = (action: VisualDropAction) => {
    if (action === "play-next") onPlayNext();
    if (action === "queue") onQueue();
    if (action === "inbox") onInbox();
  };
  const onDragEnd = (event: DragEndEvent) => {
    const source = typeof event.active.id === "string" ? event.active.id.replace(/^visual-track:/, "") : null;
    const target = typeof event.over?.id === "string" ? event.over.id : null;
    const drop = resolveVisualDrop(source, target);
    if (drop && drop.trackId === trackId) run(drop.action);
  };
  return (
    <DndContext onDragEnd={onDragEnd}>
      <div aria-label="Drag actions" className="visual-drag-actions">
        <DragChip disabled={disabled} trackId={trackId} />
        <DropTarget action="play-next" onDrop={run} />
        <DropTarget action="queue" onDrop={run} />
        <DropTarget action="inbox" onDrop={run} />
      </div>
    </DndContext>
  );
}
