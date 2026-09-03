import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";

import { ContextActionMenu, type ContextAction } from "../../components/common/ContextActionMenu";
import { RadialMenu } from "../../components/radial-menu/RadialMenu";
import { SpotIcon } from "../../components/icons/SpotIcon";
import { cancelPreview, getTrackInspector, isTauriRuntime, revealLocalFile, addTrackToInbox } from "../../services/ipc";
import { usePlayback } from "../../hooks/usePlayback";
import { usePreview } from "../../hooks/usePreview";
import { useUiStore } from "../../stores/ui-store";
import type { VisualTrackPoint } from "../../types/domain";

interface VisualTrackActionsProps {
  onActionError?: (message: string) => void;
  track: VisualTrackPoint;
}

function nativeOrE2ePlayback(): boolean {
  return isTauriRuntime() || (import.meta.env.DEV && import.meta.env.VITE_SPOTDIY_E2E === "1");
}

export function VisualTrackActions({ onActionError, track }: VisualTrackActionsProps) {
  const navigate = useNavigate();
  const playback = usePlayback();
  const preview = usePreview();
  const openerRef = useRef<HTMLButtonElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);
  const [radial, setRadial] = useState<{ left: number; top: number } | null>(null);
  const report = useCallback((error: unknown, fallback: string) => onActionError?.(error instanceof Error && error.message ? error.message : fallback), [onActionError]);
  const run = useCallback(async (action: () => Promise<unknown>, fallback: string) => {
    try {
      await action();
    } catch (error) {
      report(error, fallback);
    }
  }, [report]);
  const previewRunning = preview.state.data?.phase === "playing" && preview.state.data.trackId === track.trackId;
  const cancelIfActive = useCallback(() => {
    if (previewRunning) void preview.cancel.mutateAsync();
  }, [preview.cancel, previewRunning]);
  useEffect(() => () => { if (previewRunning) void cancelPreview(); }, [previewRunning]);
  const actions = useMemo<ContextAction[]>(() => {
    const previewBlocked = ["playing", "seeking", "loading", "recovering"].includes(playback.snapshot.phase);
    const playbackEnabled = nativeOrE2ePlayback();
    return [
      { id: "play", label: "Play Now", onSelect: () => { void run(() => playback.playNow(track.trackId, null), "Could not start playback."); }, disabled: !playbackEnabled, disabledReason: "Playback requires the native app." },
      { id: "play-next", label: "Play Next", onSelect: () => { void run(() => playback.playNext(track.trackId, null), "Could not queue the track to play next."); }, disabled: !playbackEnabled, disabledReason: "Playback requires the native app." },
      { id: "queue", label: "Add to Queue", onSelect: () => { void run(() => playback.addToQueue(track.trackId, null), "Could not add the track to the queue."); }, disabled: !playbackEnabled, disabledReason: "Queue actions require the native app." },
      { id: "inbox", label: "Add to Inbox", onSelect: () => { void run(() => addTrackToInbox(track.trackId).then(() => undefined), "Could not add the track to Inbox."); }, disabled: !isTauriRuntime(), disabledReason: "Inbox actions require the native app." },
      { id: "inspect", label: "Inspect", onSelect: () => useUiStore.getState().openTrackInspector(track.trackId) },
      { id: "lyrics", label: "Open Lyrics", onSelect: () => navigate({ to: "/lyrics" }) },
      { id: "reveal", label: "Reveal Local File", onSelect: () => { void run(async () => {
        const inspector = await getTrackInspector(track.trackId);
        const source = inspector.sources.find((item) => item.provider === "local" && item.available);
        if (!source) throw new Error("No available managed local file is attached to this track.");
        await revealLocalFile(source.sourceId);
      }, "Could not reveal the local file."); }, disabled: !isTauriRuntime(), disabledReason: "File locations require the native app." },
      { id: "preview", label: previewRunning ? "Cancel Preview" : "Preview", onSelect: () => { void run(() => previewRunning ? preview.cancel.mutateAsync() : preview.start.mutateAsync(track.trackId), previewRunning ? "Could not cancel the local preview." : "Could not start the local preview."); }, disabled: previewRunning ? preview.cancel.isPending : !playbackEnabled || previewBlocked || preview.start.isPending, disabledReason: previewBlocked ? "Pause playback to preview." : "Preview requires the native app." },
    ];
  }, [navigate, playback, preview, previewRunning, run, track.trackId]);

  return (
    <div
      className="visual-track-actions"
      onContextMenu={(event) => {
        event.preventDefault();
        returnFocusRef.current = event.currentTarget;
        setRadial({ left: event.clientX, top: event.clientY });
      }}
      onKeyDown={(event) => {
        if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
          event.preventDefault();
          returnFocusRef.current = event.currentTarget;
          const rect = event.currentTarget.getBoundingClientRect();
          setRadial({ left: rect.left, top: rect.bottom });
        }
      }}
      onBlur={(event) => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) cancelIfActive(); }}
      onMouseLeave={cancelIfActive}
      role="group"
      tabIndex={0}
    >
      <ContextActionMenu actions={actions} label={`${track.title} actions`}>
        <div className="visual-track-action-buttons">
          <button className="button button-primary button-small" disabled={actions[0].disabled} onClick={actions[0].onSelect} title={actions[0].disabled ? actions[0].disabledReason : undefined} type="button"><SpotIcon name="play" size={13} /> Play</button>
          <button className="button button-quiet button-small" disabled={actions[1].disabled} onClick={actions[1].onSelect} type="button">Next</button>
          <button className="button button-quiet button-small" disabled={actions[2].disabled} onClick={actions[2].onSelect} type="button">Queue</button>
          <button className="button button-quiet button-small" disabled={actions[7].disabled} onClick={actions[7].onSelect} title={actions[7].disabled ? actions[7].disabledReason : undefined} type="button">{actions[7].label}</button>
          <button aria-label="Open radial actions" className="icon-button visual-radial-trigger" onClick={(event) => { returnFocusRef.current = event.currentTarget; const rect = event.currentTarget.getBoundingClientRect(); setRadial({ left: rect.left, top: rect.bottom }); }} ref={openerRef} type="button"><SpotIcon name="spark" size={15} /></button>
        </div>
        {previewRunning ? <span aria-live="polite" className="visual-preview-status" role="status">Preview playing · 8 second local sample</span> : preview.state.data?.phase === "failed" && preview.state.data.trackId === track.trackId ? <span aria-live="polite" className="visual-preview-status visual-preview-status-error" role="status">{preview.state.data.error ?? "Preview failed."}</span> : null}
      </ContextActionMenu>
      {radial ? <RadialMenu actions={actions} anchor={radial} onClose={() => { setRadial(null); (returnFocusRef.current ?? openerRef.current)?.focus(); }} open /> : null}
    </div>
  );
}
