import { useMemo, useState } from "react";

import { Button } from "../common/Button";
import { StatusChip } from "../common/StatusChip";
import { SpotIcon } from "../icons/SpotIcon";
import { useWindowsIntegration } from "../../hooks/useWindowsIntegration";
import type { GlobalShortcutBinding, OutputProfile, OverlayKind } from "../../types/domain";

const overlayLabels: Record<OverlayKind, string> = {
  mini: "Mini",
  edge: "Edge",
  lyrics: "Lyrics",
  gaming: "Gaming",
};

const shortcutLabels: Record<GlobalShortcutBinding["action"], string> = {
  playPause: "Play / Pause",
  next: "Next",
  previous: "Previous",
  volumeUp: "Volume +5%",
  volumeDown: "Volume -5%",
  showHideMain: "Show / Hide main",
  toggleMiniOverlay: "Mini overlay",
  toggleLyricsOverlay: "Lyrics overlay",
  toggleGamingOverlay: "Gaming overlay",
};

function statusLabel(value: string | undefined): string {
  if (!value) return "Loading";
  return value.charAt(0).toUpperCase() + value.slice(1);
}

function chipStatus(value: string | undefined): "neutral" | "success" | "warning" | "danger" | "info" {
  if (value === "ready" || value === "registered" || value === "open") return "success";
  if (value === "conflict" || value === "unsupported" || value === "failed" || value === "error") return "warning";
  if (value === "invalid") return "danger";
  return "neutral";
}

function profileCopy(profile: OutputProfile): string {
  return `${profile.audioDeviceName} · ${profile.volumePercent}%${profile.muted ? " · muted" : ""}`;
}

function ShortcutEditor({
  binding,
  status,
  onSave,
}: {
  binding: GlobalShortcutBinding;
  status: { status: string; detail: string | null } | undefined;
  onSave: (binding: GlobalShortcutBinding) => void;
}) {
  const [accelerator, setAccelerator] = useState(binding.accelerator);
  const [enabled, setEnabled] = useState(binding.enabled);
  const changed = accelerator !== binding.accelerator || enabled !== binding.enabled;
  return (
    <div className="windows-shortcut-row">
      <div className="windows-shortcut-copy"><strong>{shortcutLabels[binding.action]}</strong><span>{status?.detail ?? "Native registration status"}</span></div>
      <input aria-label={`${shortcutLabels[binding.action]} accelerator`} className="windows-shortcut-input" onChange={(event) => setAccelerator(event.target.value)} value={accelerator} />
      <label className="windows-inline-checkbox"><input aria-label={`Enable ${shortcutLabels[binding.action]}`} checked={enabled} onChange={(event) => setEnabled(event.target.checked)} type="checkbox" /><span>Enabled</span></label>
      {changed ? <Button onClick={() => onSave({ ...binding, accelerator, enabled })} size="sm" type="button" variant="quiet">Save</Button> : null}
      <StatusChip status={chipStatus(status?.status)}>{statusLabel(status?.status)}</StatusChip>
    </div>
  );
}

export function WindowsIntegrationSettingsSection() {
  const windows = useWindowsIntegration();
  const snapshot = windows.snapshot;
  const [profileName, setProfileName] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingProfile, setEditingProfile] = useState<OutputProfile | null>(null);
  const smtcEnabled = snapshot?.smtcStatus !== "disabled";
  const overlayStates = useMemo(() => new Map(snapshot?.overlays.map((overlay) => [overlay.kind, overlay]) ?? []), [snapshot?.overlays]);

  const updateWindowsSettings = (next: { smtcEnabled?: boolean; globalShortcutsEnabled?: boolean }) => {
    const current = {
      smtcEnabled,
      globalShortcutsEnabled: snapshot?.globalShortcutsEnabled ?? false,
      ...next,
    };
    void windows.setWindowsIntegrationSettings(current);
  };

  async function createProfile() {
    if (!profileName.trim()) return;
    await windows.createOutputProfile(profileName);
    setProfileName("");
  }

  function startEdit(profile: OutputProfile) {
    setEditingId(profile.id);
    setEditingProfile({ ...profile });
  }

  async function saveProfile() {
    if (!editingProfile) return;
    await windows.updateOutputProfile(editingProfile);
    setEditingId(null);
    setEditingProfile(null);
  }

  return (
    <section className="settings-section windows-integration-section">
      <div className="settings-section-heading"><span className="eyebrow">WINDOWS &amp; OVERLAYS</span><p>Native desktop controls stay narrow, local, and safe. Overlay windows are session-only.</p></div>
      <div className="windows-integration-grid">
        <div className="windows-integration-card">
          <div className="windows-card-heading"><div><strong>Windows media controls</strong><span>System Media Transport Controls</span></div><StatusChip status={chipStatus(snapshot?.smtcStatus)}>{statusLabel(snapshot?.smtcStatus)}</StatusChip></div>
          <label className="windows-toggle-row"><span><strong>SMTC enabled</strong><small>Expose current title, artist, album, and play state to Windows.</small></span><input aria-label="SMTC enabled" checked={smtcEnabled} onChange={(event) => updateWindowsSettings({ smtcEnabled: event.target.checked })} type="checkbox" /></label>
          {snapshot?.smtcDetail ? <p className="windows-status-detail">{snapshot.smtcDetail}</p> : null}
        </div>

        <div className="windows-integration-card windows-shortcuts-card">
          <div className="windows-card-heading"><div><strong>Global shortcuts</strong><span>Rust-owned system-wide bindings</span></div><label className="windows-toggle-label"><input aria-label="Global shortcuts enabled" checked={snapshot?.globalShortcutsEnabled ?? false} onChange={(event) => { void windows.setGlobalShortcutsEnabled(event.target.checked); }} type="checkbox" /> Enabled</label></div>
          <p className="windows-muted-copy">Shortcuts are disabled by default. Conflicts are isolated to the affected action.</p>
          <div className="windows-shortcut-list">
            {snapshot?.shortcutStatuses.map((status) => {
              const binding: GlobalShortcutBinding = {
                action: status.action,
                accelerator: status.accelerator,
                enabled: status.enabled,
              };
              return <ShortcutEditor binding={binding} key={binding.action} onSave={(next) => { void windows.updateGlobalShortcut(next); }} status={status} />;
            })}
          </div>
          <Button onClick={() => { void windows.resetGlobalShortcuts(); }} size="sm" type="button" variant="quiet">Restore shortcut defaults</Button>
        </div>
      </div>

      <div className="windows-integration-card">
        <div className="windows-card-heading"><div><strong>Overlay windows</strong><span>Always-on-top desktop surfaces</span></div><StatusChip status={snapshot?.platformSupported ? "success" : "neutral"}>{snapshot?.platformSupported ? "Native ready" : "Desktop app only"}</StatusChip></div>
        <div className="windows-overlay-buttons">
          {(Object.keys(overlayLabels) as OverlayKind[]).map((kind) => {
            const overlay = overlayStates.get(kind);
            return <Button key={kind} onClick={() => { void windows.toggleOverlay(kind); }} size="sm" type="button" variant="quiet"><SpotIcon name={kind === "lyrics" ? "lyrics" : kind === "gaming" ? "play" : "expand"} size={15} />{overlayLabels[kind]} <StatusChip status={chipStatus(overlay?.status)}>{statusLabel(overlay?.status)}</StatusChip></Button>;
          })}
        </div>
        <div className="windows-gaming-warning"><SpotIcon name="info" size={16} /><span>Gaming Overlay is a standard always-on-top desktop window. Best with windowed or borderless games; exclusive fullscreen can cover desktop overlays.</span></div>
        <label className="windows-toggle-row windows-gaming-toggle"><span><strong>Gaming click-through</strong><small>Click-through is session-only and starts disabled. Rescue: Ctrl+Alt+Shift+G.</small></span><input aria-label="Gaming click-through" checked={snapshot?.gamingClickThrough ?? false} disabled={overlayStates.get("gaming")?.status !== "open"} onChange={(event) => { void windows.setGamingClickThrough(event.target.checked).catch(() => undefined); }} type="checkbox" /></label>
      </div>

      <div className="windows-integration-card">
        <div className="windows-card-heading"><div><strong>Output profiles</strong><span>Save the current device, volume, and mute state</span></div><span className="windows-profile-count">{snapshot?.outputProfiles.length ?? 0} / 16</span></div>
        <div className="windows-profile-create"><input aria-label="New output profile name" onChange={(event) => setProfileName(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void createProfile(); }} placeholder="Profile name" value={profileName} /><Button disabled={!profileName.trim()} onClick={() => { void createProfile(); }} size="sm" type="button" variant="primary">Create from current output</Button></div>
        <div className="windows-profile-list">
          {snapshot?.outputProfiles.map((profile) => editingId === profile.id && editingProfile ? <div className="windows-profile-row windows-profile-edit" key={profile.id}>
            <input aria-label="Edit output profile name" onChange={(event) => setEditingProfile({ ...editingProfile, name: event.target.value })} value={editingProfile.name} />
            <input aria-label="Edit output device" onChange={(event) => setEditingProfile({ ...editingProfile, audioDeviceName: event.target.value })} value={editingProfile.audioDeviceName} />
            <input aria-label="Edit output volume" max={100} min={0} onChange={(event) => setEditingProfile({ ...editingProfile, volumePercent: Number(event.target.value) })} type="number" value={editingProfile.volumePercent} />
            <label className="windows-inline-checkbox"><input aria-label="Edit output mute" checked={editingProfile.muted} onChange={(event) => setEditingProfile({ ...editingProfile, muted: event.target.checked })} type="checkbox" /> Muted</label>
            <Button onClick={() => { void saveProfile(); }} size="sm" type="button" variant="primary">Save</Button><Button onClick={() => { setEditingId(null); setEditingProfile(null); }} size="sm" type="button" variant="quiet">Cancel</Button>
          </div> : <div className="windows-profile-row" key={profile.id}>
            <div><strong>{profile.name}</strong><span>{profileCopy(profile)}</span></div>
            <div className="windows-profile-actions"><Button onClick={() => { void windows.applyOutputProfile(profile.id); }} size="sm" type="button" variant="quiet">Apply</Button><Button onClick={() => startEdit(profile)} size="sm" type="button" variant="quiet">Edit</Button><Button onClick={() => { void windows.deleteOutputProfile(profile.id); }} size="sm" type="button" variant="danger"><SpotIcon name="trash" size={14} /></Button></div>
          </div>)}
          {snapshot?.outputProfiles.length === 0 ? <p className="windows-muted-copy">No output profiles yet. Create one from the current playback output.</p> : null}
        </div>
      </div>
      {windows.error ? <div className="library-inline-error" role="alert"><SpotIcon name="alert" size={15} />{windows.error}</div> : null}
    </section>
  );
}
