import { Button } from "../common/Button";
import { StatusChip } from "../common/StatusChip";
import { SpotIcon } from "../icons/SpotIcon";
import type { ImportPreview as ImportPreviewModel } from "../../types/domain";

function modeLabel(mode: ImportPreviewModel["sourceStorageMode"]): string {
  return mode === "portable" ? "Portable" : "Standard";
}

export function ImportPreview({
  preview,
  busy,
  onConfirm,
  onCancel,
}: {
  preview: ImportPreviewModel;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const missing = preview.missing;
  return (
    <div className="backup-import-preview" aria-label="SpotDIY import preview">
      <div className="backup-card-heading">
        <div><strong>Import preview</strong><span>{preview.importId}</span></div>
        <StatusChip status={preview.checksumValid ? "success" : "danger"}>{preview.checksumValid ? "Checksums valid" : "Checksum failure"}</StatusChip>
      </div>
      <div className="backup-fact-grid">
        <span><small>Archive</small><strong>v{preview.archiveVersion}</strong></span>
        <span><small>App version</small><strong>{preview.appVersion}</strong></span>
        <span><small>DB schema</small><strong>{preview.databaseSchemaVersion}</strong></span>
        <span><small>Source mode</small><strong>{modeLabel(preview.sourceStorageMode)}</strong></span>
        <span><small>Entries</small><strong>{preview.entryCount}</strong></span>
        <span><small>Included audio</small><strong>{preview.includedAudioCount}</strong></span>
        <span><small>Artwork / sidecars</small><strong>{preview.includedArtworkCount} / {preview.includedSidecarLyricsCount}</strong></span>
        <span><small>Audio planned</small><strong>{preview.restoredAudioPlannedCount}</strong></span>
      </div>
      <div className="backup-missing-summary">
        <div><SpotIcon name={missing.missingLocalReferences > 0 || missing.missingDownloadOutputs > 0 ? "alert" : "check"} size={16} /><strong>Missing references</strong></div>
        <span>{missing.missingLocalReferences} of {missing.totalLocalReferences} local files · {missing.missingDownloadOutputs} of {missing.completedDownloadReferences} completed downloads</span>
      </div>
      {missing.firstMissing.length > 0 ? (
        <details className="backup-missing-details">
          <summary>Show first {Math.min(missing.firstMissing.length, 500)} missing paths</summary>
          <ul>
            {missing.firstMissing.slice(0, 10).map((item) => <li key={`${item.kind}-${item.path}`}><span>{item.kind}</span><code>{item.path}</code></li>)}
          </ul>
        </details>
      ) : null}
      <div className="backup-warning"><SpotIcon name="info" size={16} /><span>Import replaces SpotDIY durable application state after restart. Secure provider credentials are not included or changed.</span></div>
      <div className="backup-preview-actions">
        <Button disabled={busy || !preview.checksumValid} onClick={onConfirm} size="sm" type="button" variant="primary">Confirm import after restart</Button>
        <Button disabled={busy} onClick={onCancel} size="sm" type="button" variant="quiet">Cancel import</Button>
      </div>
    </div>
  );
}
