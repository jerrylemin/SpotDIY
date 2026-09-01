import { ProviderBadge } from "../common/ProviderBadge";
import { SpotIcon } from "../icons/SpotIcon";
import { useUiStore } from "../../stores/ui-store";
import type { DownloadTask, DownloadTaskId } from "../../types/domain";

interface DownloadTaskRowProps {
  task: DownloadTask;
  actionPending: boolean;
  onCancel: (taskId: DownloadTaskId) => void;
  onRetry: (taskId: DownloadTaskId) => void;
  onOpenLocation: (taskId: DownloadTaskId) => void;
}

function stateLabel(state: DownloadTask["state"]): string {
  switch (state) {
    case "postprocessing":
      return "Post-processing";
    case "cancelled":
      return "Cancelled";
    default:
      return state.charAt(0).toUpperCase() + state.slice(1);
  }
}

function bytesLabel(value: number | null): string {
  if (value === null) {
    return "Unknown size";
  }
  if (value < 1024) {
    return `${value} B`;
  }
  const units = ["KiB", "MiB", "GiB"];
  let amount = value;
  let unit = "B";
  for (const nextUnit of units) {
    amount /= 1024;
    unit = nextUnit;
    if (amount < 1024 || nextUnit === units[units.length - 1]) {
      break;
    }
  }
  return `${amount.toFixed(amount >= 10 ? 0 : 1)} ${unit}`;
}

function speedLabel(value: number | null): string {
  return value === null ? "Speed unavailable" : `${bytesLabel(value)}/s`;
}

function etaLabel(value: number | null): string {
  if (value === null) {
    return "ETA unavailable";
  }
  const minutes = Math.floor(value / 60);
  const seconds = value % 60;
  return minutes > 0 ? `${minutes}m ${seconds}s left` : `${seconds}s left`;
}

export function DownloadTaskRow({ task, actionPending, onCancel, onRetry, onOpenLocation }: DownloadTaskRowProps) {
  const openTrackInspector = useUiStore((state) => state.openTrackInspector);
  const cancellable = task.state === "queued" || task.state === "resolving" || task.state === "downloading" || task.state === "postprocessing";
  const retryable = task.state === "failed" || task.state === "cancelled";
  const hasOutput = task.state === "completed" && !task.outputMissing;

  return (
    <article className={`download-task-row download-task-state-${task.state}`}>
      <div className="download-task-art">
        {task.artworkUrl ? <img alt={`${task.title} artwork`} src={task.artworkUrl} /> : <SpotIcon name="download" size={20} />}
      </div>
      <div className="download-task-main">
        <div className="download-task-heading">
          <ProviderBadge kind={task.providerKind} />
          <strong title={task.title}>{task.title}</strong>
          <span className="download-mode-chip">{task.mode}</span>
        </div>
        <span className="download-task-artists">{task.artists.length > 0 ? task.artists.join(", ") : "Unknown artist"}</span>
        <span className="download-task-meta">{stateLabel(task.state)} · {task.sourceQualityProvenance === "providerEncoded" ? "Provider encoded" : "Quality unknown"}{task.outputExtension ? ` · .${task.outputExtension}` : ""}</span>
        {task.errorDetail ? <span className="download-task-error" role="alert">{task.errorDetail}</span> : null}
        {task.outputMissing ? <span className="download-task-warning" role="status">Completed record is retained, but the output file is missing.</span> : null}
      </div>
      <div className="download-task-progress">
        <div aria-label={`${task.progressPermille / 10}% downloaded`} className="download-progress-track" role="progressbar" aria-valuemax={1000} aria-valuemin={0} aria-valuenow={task.progressPermille}>
          <span style={{ width: `${task.progressPermille / 10}%` }} />
        </div>
        <div className="download-task-facts"><span>{bytesLabel(task.downloadedBytes)} / {bytesLabel(task.expectedBytes)}</span><span>{speedLabel(task.speedBytesPerSecond)} · {etaLabel(task.etaSeconds)}</span></div>
      </div>
      <div className="download-task-actions">
        {task.targetTrackId ? <button className="text-link" onClick={() => openTrackInspector(task.targetTrackId!)} type="button"><SpotIcon name="info" size={14} /> Inspect track</button> : null}
        {cancellable ? <button className="button button-small" disabled={actionPending} onClick={() => onCancel(task.id)} type="button">Cancel</button> : null}
        {retryable ? <button className="button button-small" disabled={actionPending} onClick={() => onRetry(task.id)} type="button">Retry</button> : null}
        {hasOutput || task.outputMissing ? <button className="text-link" disabled={actionPending} onClick={() => onOpenLocation(task.id)} type="button"><SpotIcon name="folder" size={14} /> Open folder</button> : null}
      </div>
    </article>
  );
}
