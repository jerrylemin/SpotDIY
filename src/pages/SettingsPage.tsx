import { useState } from "react";
import { Link } from "@tanstack/react-router";

import { ProviderBadge } from "../components/common/ProviderBadge";
import { Button } from "../components/common/Button";
import { BackupSection } from "../components/backup/BackupSection";
import { SegmentedControl } from "../components/common/SegmentedControl";
import { StatusChip } from "../components/common/StatusChip";
import { IconGallery } from "../components/icons/IconGallery";
import { SpotIcon } from "../components/icons/SpotIcon";
import { WindowsIntegrationSettingsSection } from "../components/settings/WindowsIntegrationSettingsSection";
import { LAYOUT_PROFILE_LABELS, LAYOUT_PROFILES } from "../features/layout/layout-profiles";
import { MAX_THEME_BYTES } from "../features/theme/theme-schema";
import { useTheme } from "../features/theme/theme-controller-model";
import { useAppStatus } from "../hooks/useAppStatus";
import {
  clearFfmpegPath,
  clearMpvPath,
  clearYtDlpPath,
  configureFfmpeg,
  configureMpv,
  configureYtDlp,
  isTauriRuntime,
  rescanFfmpeg,
  rescanMpv,
  rescanYtDlp,
} from "../services/ipc";
import type { ProviderKind } from "../types/domain";

const providerOrder: ProviderKind[] = ["local", "youtube", "soundcloud", "spotify"];

const fallbackCapabilities = {
  search: false,
  playback: false,
  metadata: false,
  artwork: false,
  lyrics: false,
  downloads: false,
  popularity: false,
  releaseDate: false,
  lyricsMetadata: false,
};

function providerName(kind: ProviderKind): string {
  switch (kind) {
    case "local":
      return "Local library";
    case "youtube":
      return "YouTube";
    case "soundcloud":
      return "SoundCloud";
    case "spotify":
      return "Spotify";
  }
}

function runtimeToolLabel(status: string | undefined): string {
  switch (status) {
    case "ready":
      return "Ready";
    case "missing":
      return "Missing";
    case "broken":
      return "Broken";
    case "unsupported":
      return "Unsupported";
    case "disabled":
      return "Disabled";
    default:
      return "Unknown";
  }
}

export function SettingsPage() {
  const appStatus = useAppStatus();
  const appearance = useTheme();
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [appearanceError, setAppearanceError] = useState<string | null>(null);
  const [exportedThemeJson, setExportedThemeJson] = useState<string | null>(null);

  const providers = providerOrder.map((kind) => appStatus.data?.providers.find((provider) => provider.kind === kind) ?? {
    kind,
    label: providerName(kind),
    configured: false,
    available: false,
    runtimeStatus: "unknown" as const,
    capabilities: fallbackCapabilities,
    detail: "Provider status is not available yet.",
  });

  async function runMpvAction(action: () => Promise<unknown>, fallback: string) {
    setBusy(true);
    setActionError(null);
    try {
      await action();
      await appStatus.refetch();
    } catch (error) {
      setActionError(error instanceof Error && error.message ? error.message : fallback);
    } finally {
      setBusy(false);
    }
  }

  function configureMpvAction() {
    return runMpvAction(configureMpv, "MPV could not be configured.");
  }

  function clearMpvAction() {
    return runMpvAction(clearMpvPath, "The custom MPV path could not be cleared.");
  }

  function rescanMpvAction() {
    return runMpvAction(rescanMpv, "MPV could not be re-scanned.");
  }

  function configureYtDlpAction() {
    return runMpvAction(configureYtDlp, "yt-dlp could not be configured.");
  }

  function clearYtDlpAction() {
    return runMpvAction(clearYtDlpPath, "The custom yt-dlp path could not be cleared.");
  }

  function rescanYtDlpAction() {
    return runMpvAction(rescanYtDlp, "yt-dlp could not be re-scanned.");
  }

  function configureFfmpegAction() {
    return runMpvAction(configureFfmpeg, "FFmpeg could not be configured.");
  }

  function clearFfmpegAction() {
    return runMpvAction(clearFfmpegPath, "The custom FFmpeg path could not be cleared.");
  }

  function rescanFfmpegAction() {
    return runMpvAction(rescanFfmpeg, "FFmpeg could not be re-scanned.");
  }

  function toolActions(key: "ytDlp" | "ffmpeg" | "mpv") {
    const actions = key === "ytDlp"
      ? { configure: configureYtDlpAction, clear: clearYtDlpAction, rescan: rescanYtDlpAction }
      : key === "ffmpeg"
        ? { configure: configureFfmpegAction, clear: clearFfmpegAction, rescan: rescanFfmpegAction }
        : { configure: configureMpvAction, clear: clearMpvAction, rescan: rescanMpvAction };
    return <div className="settings-tool-actions"><button aria-label="Configure…" className="button button-small icon-only-button" disabled={busy || !isTauriRuntime()} onClick={() => { void actions.configure(); }} title="Configure executable path" type="button"><SpotIcon name="settings" size={14} /> Configure…</button><button aria-label="Clear custom path" className="button button-small button-quiet icon-only-button" disabled={busy || !isTauriRuntime()} onClick={() => { void actions.clear(); }} title="Clear custom executable path" type="button"><SpotIcon name="trash" size={14} /> Clear custom path</button><button aria-label="Re-scan" className="button button-small button-quiet icon-only-button" disabled={busy || !isTauriRuntime()} onClick={() => { void actions.rescan(); }} title="Re-scan executable" type="button"><SpotIcon name="refresh" size={14} /> Re-scan</button></div>;
  }

  function appearanceErrorMessage(error: unknown): string {
    if (error instanceof Error && error.message) {
      return error.message;
    }
    return "Appearance settings could not be updated.";
  }

  async function changeTheme(theme: Parameters<typeof appearance.setTheme>[0]) {
    setAppearanceError(null);
    setExportedThemeJson(null);
    try {
      await appearance.setTheme(theme);
    } catch (error) {
      setAppearanceError(appearanceErrorMessage(error));
    }
  }

  async function changeLayout(layoutProfile: Parameters<typeof appearance.setLayoutProfile>[0]) {
    setAppearanceError(null);
    try {
      await appearance.setLayoutProfile(layoutProfile);
    } catch (error) {
      setAppearanceError(appearanceErrorMessage(error));
    }
  }

  async function importTheme(file: File | undefined) {
    if (!file) {
      return;
    }
    setAppearanceError(null);
    setExportedThemeJson(null);
    try {
      if (file.size > MAX_THEME_BYTES) {
        throw new Error(`Theme package exceeds the ${MAX_THEME_BYTES} byte limit.`);
      }
      await appearance.importCustomTheme(await file.text());
    } catch (error) {
      setAppearanceError(appearanceErrorMessage(error));
    }
  }

  function exportTheme() {
    setAppearanceError(null);
    try {
      const json = appearance.exportCustomTheme();
      if (!json) {
        throw new Error("Import a valid custom theme before exporting it.");
      }
      setExportedThemeJson(json);
      if (typeof Blob !== "undefined" && typeof URL !== "undefined" && typeof URL.createObjectURL === "function") {
        const url = URL.createObjectURL(new Blob([json], { type: "application/json" }));
        const anchor = document.createElement("a");
        anchor.href = url;
        anchor.download = `${appearance.settings?.customTheme?.name ?? "spotdiy-theme"}.json`;
        anchor.click();
        URL.revokeObjectURL(url);
      }
    } catch (error) {
      setAppearanceError(appearanceErrorMessage(error));
    }
  }

  async function resetTheme() {
    setAppearanceError(null);
    setExportedThemeJson(null);
    try {
      await appearance.resetCustomTheme();
    } catch (error) {
      setAppearanceError(appearanceErrorMessage(error));
    }
  }

  const themeLabel = appearance.theme === "custom" ? "Custom" : appearance.theme[0].toUpperCase() + appearance.theme.slice(1);
  const resolvedSystemLabel = appearance.resolvedSystemTheme === "dark" ? "Dark" : "Light";
  const themeOptions = [
    { value: "dark" as const, label: "Dark" },
    { value: "light" as const, label: "Light" },
    { value: "system" as const, label: "System" },
    { value: "custom" as const, label: "Custom", disabled: !appearance.settings?.customTheme && appearance.theme !== "custom" },
  ];
  const layoutOptions = LAYOUT_PROFILES.map((value) => ({ value, label: LAYOUT_PROFILE_LABELS[value] }));

  return (
    <div className="page-stack settings-page">
      <section className="page-intro"><div><span className="eyebrow">SETTINGS</span><h1>Make it <em>yours.</em></h1><p>Local storage, source connections, and the shape of the player live here.</p></div><span className="version-label">SpotDIY v{appStatus.data?.version ?? "0.1.0"}</span></section>
      <section className="settings-section appearance-section">
        <div className="settings-section-heading"><span className="eyebrow">APPEARANCE</span><p>Choose the visual mode and density of the desktop workspace. These settings change presentation only.</p></div>
        <div className="appearance-grid">
          <div className="appearance-control">
            <div className="appearance-control-heading"><div><strong>Theme</strong><span>Current: {themeLabel}</span></div><StatusChip status={appearance.resolvedTheme === "dark" ? "neutral" : "info"}>{appearance.resolvedTheme === "dark" ? "Dark surfaces" : "Light surfaces"}</StatusChip></div>
            <SegmentedControl label="Theme" onChange={(value) => { void changeTheme(value); }} options={themeOptions} value={appearance.theme} />
            <span className="settings-muted-note">System resolves to {resolvedSystemLabel} right now. The stored value remains System.</span>
          </div>
          <div className="appearance-control">
            <div className="appearance-control-heading"><div><strong>Layout density</strong><span>Current: {LAYOUT_PROFILE_LABELS[appearance.settings?.layoutProfile ?? "comfortable"]}</span></div><SpotIcon name="layout" size={18} /></div>
            <SegmentedControl label="Layout density" onChange={(value) => { void changeLayout(value); }} options={layoutOptions} value={appearance.settings?.layoutProfile ?? "comfortable"} />
            <span className="settings-muted-note">Compact and Dense preserve a minimum 32px interactive hit area.</span>
          </div>
        </div>
        <div className="appearance-custom-panel">
          <div className="appearance-control-heading"><div><strong>Custom theme package</strong><span>{appearance.settings?.customTheme ? appearance.settings.customTheme.name : "No custom theme imported"}</span></div><SpotIcon name="theme" size={18} /></div>
          <p className="settings-muted-note">Import a validated JSON theme package. Colors are data-only #RRGGBB tokens; CSS and filesystem paths are never accepted.</p>
          <div className="appearance-actions">
            <label className="button button-secondary appearance-file-label">Import JSON<input accept="application/json,.json" aria-label="Import custom theme JSON" className="appearance-file-input" onChange={(event) => { void importTheme(event.target.files?.[0]); event.currentTarget.value = ""; }} type="file" /></label>
            <Button disabled={!appearance.settings?.customTheme} onClick={exportTheme} size="sm" type="button" variant="quiet">Export JSON</Button>
            <Button disabled={!appearance.settings?.customTheme} onClick={() => { void resetTheme(); }} size="sm" type="button" variant="danger">Reset</Button>
          </div>
          {appearance.settings?.customTheme ? <span className="settings-muted-note">Active custom theme: {appearance.settings.customTheme.name}</span> : null}
          {exportedThemeJson ? <textarea aria-label="Exported custom theme JSON" className="appearance-export" readOnly value={exportedThemeJson} /> : null}
          {appearance.error || appearanceError ? <div className="library-inline-error" role="alert"><SpotIcon name="alert" size={15} /><span>{appearanceError ?? appearance.error}</span></div> : null}
        </div>
        {import.meta.env.DEV ? (
          <details className="design-system-inspector">
            <summary><span>Design-system inspector</span><SpotIcon name="expand" size={16} /></summary>
            <div className="design-system-inspector-body"><IconGallery /></div>
          </details>
        ) : null}
      </section>
      <WindowsIntegrationSettingsSection />
      <section className="settings-section">
        <div className="settings-section-heading"><span className="eyebrow">SOURCE CONNECTIONS</span><p>Optional online sources augment your local library. Spotify search and source-matched MP3 downloads use spotdl; no Spotify developer app is required.</p></div>
        <div className="settings-source-list">
          {providers.map((provider) => <div className="settings-source-row" key={provider.kind}><ProviderBadge kind={provider.kind} /><div className="settings-source-copy"><strong>{provider.label}</strong><span>{provider.detail}</span></div><span className={`source-connection-status ${provider.available ? "connected" : "not-connected"}`}>{provider.kind === "spotify" ? (provider.available ? "Ready" : "Unavailable") : provider.configured ? "Connected" : "Not connected"}</span></div>)}
        </div>
      </section>
      <section className="settings-section"><div className="settings-section-heading"><span className="eyebrow">MEDIA TOOLS</span><p>Download and online playback use validated local binaries. Paths stay in the native boundary; only health, version, and actionable detail are shown here.</p></div><div className="settings-tool-list">{(["ytDlp", "ffmpeg", "mpv"] as const).map((key) => { const tool = appStatus.data?.mediaTools[key]; const label = key === "ytDlp" ? "yt-dlp" : key === "ffmpeg" ? "FFmpeg" : "MPV"; return <div className="settings-tool-row" key={key}><div><strong>{label}</strong><span>{tool?.version ?? "Version unavailable"}</span></div><span className={`source-connection-status ${tool?.status === "ready" ? "connected" : "not-connected"}`}>{runtimeToolLabel(tool?.status)}</span><p>{tool?.detail ?? "Tool health is not available yet."}</p>{toolActions(key)}</div>; })}</div>{actionError ? <div className="library-inline-error" role="alert"><SpotIcon name="alert" size={15} /><span>{actionError}</span></div> : null}</section>
      <BackupSection />
      <section className="settings-footer"><Link className="text-link" to="/library">Open local library <SpotIcon name="arrow" size={14} /></Link><span><span className="status-dot status-dot-active" /> No telemetry by default</span></section>
    </div>
  );
}
