import { useState } from "react";

import { Button } from "../common/Button";
import { StatusChip } from "../common/StatusChip";
import { SpotIcon } from "../icons/SpotIcon";
import { ImportPreview } from "./ImportPreview";
import { useBackup } from "../../hooks/useBackup";
import type { SpotDiyExportOptions, StorageMode } from "../../types/domain";

const initialOptions: SpotDiyExportOptions = {
  includeLocalAudio: false,
  includeArtworkCache: false,
  includeSidecarLyrics: false,
};

function modeLabel(mode: StorageMode | undefined): string {
  return mode === "portable" ? "Portable" : "Standard";
}

export function BackupSection() {
  const backup = useBackup();
  const [options, setOptions] = useState(initialOptions);
  const [modeMessage, setModeMessage] = useState<string | null>(null);

  async function exportBackup() {
    await backup.exportBackup(options);
  }

  async function switchMode() {
    const target: StorageMode = backup.storage?.mode === "portable" ? "standard" : "portable";
    setModeMessage(null);
    try {
      await backup.switchMode(target);
      setModeMessage(`${modeLabel(target)} mode is prepared. Restart SpotDIY to activate it.`);
    } catch {
      // The hook exposes the actionable native error.
    }
  }

  return (
    <section className="settings-section backup-section">
      <div className="settings-section-heading"><span className="eyebrow">STORAGE &amp; BACKUP</span><p>Keep durable SpotDIY state local, portable, and recoverable. Archive secrets are excluded by design.</p></div>
      <div className="backup-storage-card">
        <div className="backup-card-heading"><div><strong>Current storage</strong><span>Backend-resolved application paths</span></div><StatusChip status={backup.storage?.restartRequired ? "warning" : "success"}>{backup.loading ? "Loading" : backup.storage?.restartRequired ? "Restart required" : `${modeLabel(backup.storage?.mode)} active`}</StatusChip></div>
        <div className="backup-path-grid">
          <span><small>Mode</small><strong>{modeLabel(backup.storage?.mode)}</strong></span>
          <span><small>Data root</small><code>{backup.storage?.dataRoot ?? "Loading…"}</code></span>
          <span><small>Database</small><code>{backup.storage?.databasePath ?? "Loading…"}</code></span>
          <span><small>Cache</small><code>{backup.storage?.cacheRoot ?? "Loading…"}</code></span>
        </div>
        <div className="backup-mode-actions"><Button disabled={backup.busy || backup.loading} onClick={() => { void switchMode(); }} size="sm" type="button" variant="quiet">Prepare {backup.storage?.mode === "portable" ? "Standard" : "Portable"} Mode</Button><span>Database is copied safely; the mode marker changes last. Restart is explicit.</span></div>
        {modeMessage ? <p className="settings-muted-note">{modeMessage}</p> : null}
      </div>
      <div className="backup-storage-card">
        <div className="backup-card-heading"><div><strong>SpotDIY backup archive</strong><span>Deterministic .spotdiy ZIP · metadata first</span></div><SpotIcon name="download" size={18} /></div>
        <div className="backup-option-list">
          <label><input checked={options.includeLocalAudio} onChange={(event) => setOptions((current) => ({ ...current, includeLocalAudio: event.target.checked }))} type="checkbox" /><span>Include local audio</span></label>
          <label><input checked={options.includeArtworkCache} onChange={(event) => setOptions((current) => ({ ...current, includeArtworkCache: event.target.checked }))} type="checkbox" /><span>Include trusted artwork cache</span></label>
          <label><input checked={options.includeSidecarLyrics} disabled={!options.includeLocalAudio} onChange={(event) => setOptions((current) => ({ ...current, includeSidecarLyrics: event.target.checked }))} type="checkbox" /><span>Include sidecar lyrics (.lrc)</span></label>
        </div>
        <div className="backup-action-row"><Button disabled={backup.busy || backup.loading} onClick={() => { void exportBackup(); }} size="sm" type="button" variant="primary"><SpotIcon name="download" size={15} />Export .spotdiy</Button><Button disabled={backup.busy || backup.loading || backup.preview !== null} onClick={() => { void backup.prepareImport(); }} size="sm" type="button" variant="quiet"><SpotIcon name="file" size={15} />Import .spotdiy</Button></div>
        <p className="settings-muted-note">Credentials, access tokens, provider payloads, runtime WAL/SHM files, and download temp files are never included.</p>
      </div>
      {backup.preview ? <ImportPreview busy={backup.busy} onCancel={() => { void backup.cancelImport(backup.preview?.importId ?? ""); }} onConfirm={() => { void backup.commitImport(backup.preview?.importId ?? ""); }} preview={backup.preview} /> : null}
      {backup.storage?.pendingImport && !backup.preview ? <div className="backup-warning"><SpotIcon name="alert" size={16} /><span>A restore is pending. Review the import preview before restarting.</span></div> : null}
      {backup.error ? <div className="library-inline-error" role="alert"><SpotIcon name="alert" size={15} /><span>{backup.error}</span></div> : null}
    </section>
  );
}
