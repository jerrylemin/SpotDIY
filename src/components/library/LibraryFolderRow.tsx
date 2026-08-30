import { SpotIcon } from "../icons/SpotIcon";
import type { LibraryFolder, LibraryFolderId, ScanProgress } from "../../types/domain";

interface LibraryFolderRowProps {
  folder: LibraryFolder;
  progress: ScanProgress | null;
  actionPending: boolean;
  onRescan: (folderId: LibraryFolderId) => void;
  onRemove: (folder: LibraryFolder) => void;
}

function formatLastScan(value: string | null): string {
  if (!value) {
    return "Not scanned yet";
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? "Scan time unavailable"
    : `Last scanned ${new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(date)}`;
}

function statusLabel(folder: LibraryFolder, progress: ScanProgress | null): string {
  if (progress?.folderId === folder.id && progress.status === "scanning") {
    return "Scanning";
  }
  if (progress?.folderId === folder.id && progress.status === "queued") {
    return "Queued";
  }
  switch (folder.status) {
    case "complete":
      return "Indexed";
    case "failed":
      return "Needs attention";
    case "scanning":
      return "Scanning";
    case "queued":
      return "Queued";
    default:
      return "Ready to scan";
  }
}

export function LibraryFolderRow({
  folder,
  progress,
  actionPending,
  onRescan,
  onRemove,
}: LibraryFolderRowProps) {
  const folderProgress = progress?.folderId === folder.id ? progress : null;
  const isScanning = folderProgress?.status === "scanning" || folder.status === "scanning";
  const progressLabel = folderProgress
    ? folderProgress.candidates > 0
      ? `${folderProgress.processed} of ${folderProgress.candidates} files checked`
      : "Preparing files…"
    : null;

  return (
    <article className="library-folder-row" data-testid={`library-folder-${folder.id}`}>
      <div className="library-folder-heading">
        <div className="library-folder-icon">
          <SpotIcon name="folder" size={19} />
        </div>
        <div className="library-folder-copy">
          <strong title={folder.path}>{folder.path}</strong>
          <span>
            {folder.indexedTrackCount} indexed tracks · {folder.fileCount} files
          </span>
        </div>
        <span className={`library-status-chip library-status-${folder.status}`}>
          {statusLabel(folder, progress)}
        </span>
      </div>
      <div className="library-folder-footer">
        <span>{progressLabel ?? formatLastScan(folder.lastScanFinishedAt)}</span>
        <div className="library-folder-actions">
          <button
            className="button button-quiet button-small"
            disabled={actionPending || !folder.enabled}
            onClick={() => onRescan(folder.id)}
            title="Scan this folder for new or changed files"
            type="button"
          >
            <SpotIcon name="refresh" size={14} />
            {isScanning ? "Queue refresh" : "Rescan"}
          </button>
          <button
            aria-label={`Remove ${folder.path}`}
            className="icon-button library-remove-button"
            disabled={actionPending}
            onClick={() => onRemove(folder)}
            title="Remove folder from SpotDIY"
            type="button"
          >
            <SpotIcon name="trash" size={15} />
          </button>
        </div>
      </div>
      {folder.lastScanError ? (
        <p className="library-inline-error" role="alert">
          <SpotIcon name="alert" size={14} />
          {folder.lastScanError}
        </p>
      ) : null}
      {folderProgress?.currentFile ? (
        <p className="library-progress-file" title={folderProgress.currentFile}>
          Reading {folderProgress.currentFile}
        </p>
      ) : null}
    </article>
  );
}
