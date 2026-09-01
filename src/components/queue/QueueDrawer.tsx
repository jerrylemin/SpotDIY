import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useDroppable,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useEffect, useMemo, useState } from "react";

import { useQueue } from "../../hooks/useQueue";
import { isTauriRuntime } from "../../services/ipc";
import { useUiStore } from "../../stores/ui-store";
import type { QueueEntryId, QueueSection, QueueWorkspaceEntry } from "../../types/domain";
import { SpotIcon } from "../icons/SpotIcon";

const sectionLabels: Record<QueueSection, string> = {
  up_next: "UP NEXT",
  later: "LATER",
  autoplay: "AUTOPLAY",
};

function formatPosition(positionMs: number): string {
  const totalSeconds = Math.floor(positionMs / 1000);
  return `${Math.floor(totalSeconds / 60)}:${String(totalSeconds % 60).padStart(2, "0")}`;
}

function entryTitle(entry: QueueWorkspaceEntry): string {
  return entry.title ?? `Track ${entry.trackId}`;
}

function entrySubtitle(entry: QueueWorkspaceEntry): string {
  if (entry.artists.length > 0) {
    return entry.artists.join(" · ");
  }
  return entry.album ?? "Unknown artist";
}

interface SortableQueueEntryProps {
  entry: QueueWorkspaceEntry;
  editable: boolean;
  onPin: (entry: QueueWorkspaceEntry) => void;
  onRemove: (entry: QueueWorkspaceEntry) => void;
}

function SortableQueueEntry({ entry, editable, onPin, onRemove }: SortableQueueEntryProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: entry.id,
    data: { section: entry.section, index: entry.position },
  });
  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div
      className={`queue-entry${isDragging ? " queue-entry-dragging" : ""}${entry.pinned ? " queue-entry-pinned" : ""}`}
      ref={setNodeRef}
      style={style}
    >
      <button
        aria-label={`Drag ${entryTitle(entry)}`}
        className="queue-drag-handle"
        disabled={!editable}
        ref={setActivatorNodeRef}
        type="button"
        {...attributes}
        {...listeners}
      >
        <span aria-hidden="true">⋮⋮</span>
      </button>
      <div className="queue-entry-copy">
        <strong title={entryTitle(entry)}>{entryTitle(entry)}</strong>
        <span>{entrySubtitle(entry)}</span>
      </div>
      <div className="queue-entry-actions">
        <button
          aria-label={`${entry.pinned ? "Unpin" : "Pin"} ${entryTitle(entry)}`}
          aria-pressed={entry.pinned}
          className={`queue-entry-action${entry.pinned ? " queue-entry-action-active" : ""}`}
          disabled={!editable}
          onClick={() => onPin(entry)}
          title={entry.pinned ? "Unpin entry" : "Keep entry when clearing this section"}
          type="button"
        >
          {entry.pinned ? "Pinned" : "Pin"}
        </button>
        <button
          aria-label={`Remove ${entryTitle(entry)} from queue`}
          className="queue-entry-action queue-entry-remove"
          disabled={!editable}
          onClick={() => onRemove(entry)}
          type="button"
        >
          Remove
        </button>
      </div>
    </div>
  );
}

interface QueueSectionListProps {
  section: QueueSection;
  entries: QueueWorkspaceEntry[];
  editable: boolean;
  onClear: (section: QueueSection) => void;
  onPin: (entry: QueueWorkspaceEntry) => void;
  onRemove: (entry: QueueWorkspaceEntry) => void;
}

function QueueSectionList({ section, entries, editable, onClear, onPin, onRemove }: QueueSectionListProps) {
  const { isOver, setNodeRef } = useDroppable({
    id: `queue-section-${section}`,
    data: { section },
  });

  return (
    <section className={`queue-section${isOver ? " queue-section-over" : ""}`} ref={setNodeRef}>
      <div className="queue-section-heading">
        <div>
          <span className="eyebrow">{sectionLabels[section]}</span>
          <span className="queue-section-count">{entries.length}</span>
        </div>
        <button
          className="queue-clear-section"
          disabled={!editable || entries.every((entry) => entry.pinned)}
          onClick={() => onClear(section)}
          type="button"
        >
          Clear
        </button>
      </div>
      <SortableContext items={entries.map((entry) => entry.id)} strategy={verticalListSortingStrategy}>
        <div className="queue-section-list">
          {entries.map((entry) => (
            <SortableQueueEntry entry={entry} editable={editable} key={entry.id} onPin={onPin} onRemove={onRemove} />
          ))}
          {entries.length === 0 ? (
            <div className="queue-section-empty">{section === "autoplay" ? "Autoplay recommendations are not enabled yet." : "Nothing waiting here."}</div>
          ) : null}
        </div>
      </SortableContext>
    </section>
  );
}

export function QueueDrawer() {
  const open = useUiStore((state) => state.queueDrawerOpen);
  const setOpen = useUiStore((state) => state.setQueueDrawerOpen);
  const queue = useQueue();
  const [snapshotMessage, setSnapshotMessage] = useState<string | null>(null);
  const [snapshotNames, setSnapshotNames] = useState<Awaited<ReturnType<typeof queue.listSnapshots>>>([]);
  const listSnapshots = queue.listSnapshots;
  const editable = isTauriRuntime();
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  const sectionEntries = useMemo(() => ({
    up_next: queue.workspace.upNext,
    later: queue.workspace.later,
    autoplay: queue.workspace.autoplay,
  }), [queue.workspace.autoplay, queue.workspace.later, queue.workspace.upNext]);

  useEffect(() => {
    if (!open || !editable) {
      return;
    }
    void listSnapshots().then(setSnapshotNames).catch(() => undefined);
  }, [editable, listSnapshots, open]);

  if (!open) {
    return null;
  }

  const handleDragEnd = (event: DragEndEvent) => {
    const activeSection = event.active.data.current?.section as QueueSection | undefined;
    const overSection = event.over?.data.current?.section as QueueSection | undefined
      ?? (typeof event.over?.id === "string" && event.over.id.startsWith("queue-section-")
        ? event.over.id.replace("queue-section-", "") as QueueSection
        : undefined);
    if (!activeSection || !overSection || !event.over || activeSection === "autoplay" || overSection === "autoplay") {
      return;
    }
    const sourceEntries = sectionEntries[activeSection];
    const targetEntries = sectionEntries[overSection];
    const sourceIndex = sourceEntries.findIndex((entry) => entry.id === event.active.id);
    if (sourceIndex < 0) {
      return;
    }
    const overIndex = targetEntries.findIndex((entry) => entry.id === event.over?.id);
    const targetIndex = overIndex < 0 ? targetEntries.length : overIndex;
    if (activeSection === overSection && sourceIndex === targetIndex) {
      return;
    }
    void queue.moveEntry(event.active.id as QueueEntryId, overSection, targetIndex).catch(() => undefined);
  };

  const refreshSnapshots = () => {
    void listSnapshots().then(setSnapshotNames).catch(() => undefined);
  };

  const saveSnapshot = () => {
    const name = window.prompt("Name this queue snapshot (1–80 characters):", "Evening set");
    if (!name) {
      return;
    }
    setSnapshotMessage(null);
    void queue.saveSnapshot(name)
      .then(() => {
        setSnapshotMessage(`Saved “${name.trim()}”.`);
        refreshSnapshots();
      })
      .catch(() => undefined);
  };

  return (
    <div className="queue-drawer-layer" role="presentation">
      <button aria-label="Close queue" className="queue-drawer-backdrop" onClick={() => setOpen(false)} type="button" />
      <aside aria-label="Persistent queue" className="queue-drawer" role="dialog">
        <div className="queue-drawer-header">
          <div>
            <span className="eyebrow">PLAYBACK WORKSPACE</span>
            <h2>Queue</h2>
            <p>{queue.workspace.current ? `${queue.workspace.upNext.length + queue.workspace.later.length} tracks waiting` : "Nothing is playing"}</p>
          </div>
          <button aria-label="Close queue" className="icon-button" onClick={() => setOpen(false)} type="button"><SpotIcon name="close" size={18} /></button>
        </div>

        {queue.error ? <div className="queue-alert" role="alert"><SpotIcon name="alert" size={15} /><span>{queue.error}</span></div> : null}
        {!editable ? <div className="queue-preview-note" role="status">Queue editing and snapshots are available in the native SpotDIY app.</div> : null}
        {snapshotMessage ? <div className="queue-success" role="status">{snapshotMessage}</div> : null}

        <div className="queue-drawer-body">
          <section className="queue-current-card">
            <div className="queue-current-label"><span className="eyebrow">CURRENT</span><span>{formatPosition(queue.workspace.currentPositionMs)}</span></div>
            {queue.workspace.current ? (
              <div className="queue-current-copy">
                <strong>{entryTitle(queue.workspace.current)}</strong>
                <span>{entrySubtitle(queue.workspace.current)}</span>
              </div>
            ) : <span className="queue-section-empty">Playback will appear here.</span>}
          </section>

          <DndContext onDragEnd={handleDragEnd} sensors={sensors}>
            <QueueSectionList editable={editable} entries={queue.workspace.upNext} onClear={queue.clearSection} onPin={(entry) => void queue.setEntryPinned(entry.id, !entry.pinned)} onRemove={(entry) => void queue.removeEntry(entry.id)} section="up_next" />
            <QueueSectionList editable={editable} entries={queue.workspace.later} onClear={queue.clearSection} onPin={(entry) => void queue.setEntryPinned(entry.id, !entry.pinned)} onRemove={(entry) => void queue.removeEntry(entry.id)} section="later" />
            <QueueSectionList editable={false} entries={queue.workspace.autoplay} onClear={queue.clearSection} onPin={(entry) => void queue.setEntryPinned(entry.id, !entry.pinned)} onRemove={(entry) => void queue.removeEntry(entry.id)} section="autoplay" />
          </DndContext>

          <section className="queue-snapshots">
            <div className="queue-section-heading">
              <div><span className="eyebrow">SNAPSHOTS</span><span className="queue-section-count">{snapshotNames.length}</span></div>
              <button className="button button-quiet button-small" disabled={!editable} onClick={saveSnapshot} type="button">Save current</button>
            </div>
            {snapshotNames.length === 0 ? <p className="queue-section-empty">Save a named queue to restore it later.</p> : (
              <div className="queue-snapshot-list">
                {snapshotNames.map((snapshot) => (
                  <div className="queue-snapshot-row" key={snapshot.id}>
                    <div><strong>{snapshot.name}</strong><span>{snapshot.entryCount} tracks · {new Date(snapshot.createdAt).toLocaleDateString()}</span></div>
                    <div>
                      <button className="queue-entry-action" disabled={!editable} onClick={() => { void queue.restoreSnapshot(snapshot.id); }} type="button">Restore</button>
                      <button className="queue-entry-action queue-entry-remove" disabled={!editable} onClick={() => { if (window.confirm(`Delete snapshot “${snapshot.name}”?`)) { void queue.deleteSnapshot(snapshot.id).then(setSnapshotNames); } }} type="button">Delete</button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>
        </div>
      </aside>
    </div>
  );
}
