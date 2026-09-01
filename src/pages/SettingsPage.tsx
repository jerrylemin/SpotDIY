import { useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { ProviderBadge } from "../components/common/ProviderBadge";
import { SpotIcon } from "../components/icons/SpotIcon";
import { useAppStatus } from "../hooks/useAppStatus";
import {
  IpcError,
  beginSpotifyAuthorization,
  disconnectSpotify,
  getSpotifySetupStatus,
  subscribeToSpotifyAuthState,
} from "../services/ipc";
import type { ProviderKind, SpotifySetupStatus } from "../types/domain";

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
      return "Spotify catalog";
  }
}

function statusLabel(status: SpotifySetupStatus | undefined): string {
  switch (status?.state) {
    case "connected":
      return "Connected";
    case "setup_required":
      return "Setup required";
    case "unavailable":
      return "Unavailable";
    default:
      return "Disabled";
  }
}

function errorMessage(error: unknown): string {
  if (error instanceof IpcError && error.message) {
    return error.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return "Spotify setup could not be updated.";
}

export function SettingsPage() {
  const appStatus = useAppStatus();
  const queryClient = useQueryClient();
  const spotify = useQuery({
    queryKey: ["spotify-setup"],
    queryFn: getSpotifySetupStatus,
    staleTime: Number.POSITIVE_INFINITY,
    retry: 1,
  });
  const [clientId, setClientId] = useState("");
  const [market, setMarket] = useState(spotify.data?.market ?? "US");
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    if (spotify.data?.market) {
      setMarket(spotify.data.market);
    }
  }, [spotify.data?.market]);

  useEffect(() => {
    let mounted = true;
    let unsubscribe: (() => void) | undefined;
    void subscribeToSpotifyAuthState((next) => {
      if (mounted) {
        queryClient.setQueryData(["spotify-setup"], next);
      }
    }).then((stop) => {
      if (mounted) {
        unsubscribe = stop;
      } else {
        stop();
      }
    }).catch(() => undefined);
    return () => {
      mounted = false;
      unsubscribe?.();
    };
  }, [queryClient]);

  const spotifyStatus = spotify.data;
  const providers = providerOrder.map((kind) => appStatus.data?.providers.find((provider) => provider.kind === kind) ?? {
    kind,
    label: providerName(kind),
    configured: false,
    available: false,
    runtimeStatus: "unknown" as const,
    capabilities: fallbackCapabilities,
    detail: "Provider status is not available yet.",
  });

  async function setupSpotify() {
    setBusy(true);
    setActionError(null);
    try {
      await beginSpotifyAuthorization(clientId, market);
      await queryClient.invalidateQueries({ queryKey: ["spotify-setup"] });
    } catch (error) {
      setActionError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function disconnect() {
    setBusy(true);
    setActionError(null);
    try {
      queryClient.setQueryData(["spotify-setup"], await disconnectSpotify());
    } catch (error) {
      setActionError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page-stack settings-page">
      <section className="page-intro"><div><span className="eyebrow">SETTINGS</span><h1>Make it <em>yours.</em></h1><p>Local storage, source connections, and the shape of the player live here.</p></div><span className="version-label">SpotDIY v{appStatus.data?.version ?? "0.1.0"}</span></section>
      <section className="settings-section">
        <div className="settings-section-heading"><span className="eyebrow">SOURCE CONNECTIONS</span><p>Optional online sources augment your local library. Spotify provides catalog metadata only and never enters the playback or download path.</p></div>
        <div className="settings-source-list">
          {providers.map((provider) => <div className="settings-source-row" key={provider.kind}><ProviderBadge kind={provider.kind} /><div className="settings-source-copy"><strong>{provider.label}</strong><span>{provider.detail}</span></div><span className={`source-connection-status ${provider.configured ? "connected" : "not-connected"}`}>{provider.configured ? "Connected" : provider.kind === "spotify" ? statusLabel(spotifyStatus) : "Not connected"}</span>{provider.kind === "spotify" && spotifyStatus?.state === "connected" ? <button className="button button-small" disabled={busy} onClick={() => void disconnect()} type="button">Disconnect</button> : null}</div>)}
        </div>
        <div className="spotify-setup-card">
          <div className="spotify-setup-heading"><div><span className="eyebrow">SPOTIFY CATALOG</span><strong>PKCE authorization</strong></div><span className={`source-connection-status ${spotifyStatus?.state === "connected" ? "connected" : "not-connected"}`}>{statusLabel(spotifyStatus)}</span></div>
          <p>{spotifyStatus?.detail ?? "Checking Spotify setup status."}</p>
          {spotifyStatus?.state === "setup_required" ? <div className="spotify-setup-form"><label><span>Client ID</span><input aria-label="Spotify client ID" onChange={(event) => setClientId(event.target.value)} placeholder="Paste your Spotify client ID" value={clientId} /></label><label><span>Market</span><input aria-label="Spotify market" maxLength={2} onChange={(event) => setMarket(event.target.value.toUpperCase())} value={market} /></label><button className="button button-primary" disabled={busy || clientId.trim().length === 0} onClick={() => void setupSpotify()} type="button">{busy ? "Waiting for authorization…" : "Set up source"}</button></div> : null}
          {actionError ? <div className="library-inline-error" role="alert"><SpotIcon name="alert" size={15} />{actionError}</div> : null}
          {spotifyStatus?.state === "disabled" ? <span className="settings-muted-note">Enable the Spotify developer gate in the native environment before authorizing.</span> : null}
        </div>
      </section>
      <section className="settings-section"><div className="settings-section-heading"><span className="eyebrow">STORAGE</span><p>SpotDIY keeps application data local. Music remains in the folders you select.</p></div><div className="storage-grid"><div className="storage-card storage-card-active"><div><SpotIcon name="library" size={21} /><span className="storage-card-label">Standard mode</span></div><strong>Windows LocalAppData</strong><p>Recommended for a normal installation.</p><span className="storage-card-state">CURRENT MODE</span></div><div className="storage-card"><div><SpotIcon name="folder" size={21} /><span className="storage-card-label">Portable mode</span></div><strong>Folder-contained data</strong><p>Keep database, covers, lyrics, and cache beside SpotDIY.exe.</p><button className="text-link" disabled title="Portable mode is implemented in the storage slice" type="button">Configure mode <SpotIcon name="arrow" size={14} /></button></div></div></section>
      <section className="settings-footer"><Link className="text-link" to="/library">Open local library <SpotIcon name="arrow" size={14} /></Link><span><span className="status-dot status-dot-active" /> No telemetry by default</span></section>
    </div>
  );
}
