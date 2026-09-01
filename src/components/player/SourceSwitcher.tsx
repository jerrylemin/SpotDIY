import type { PlaybackSourceOption, SourceId } from "../../types/domain";
import { ProviderBadge } from "../common/ProviderBadge";

interface SourceSwitcherProps {
  currentSourceId: SourceId | null;
  disabled?: boolean;
  onSwitch: (sourceId: SourceId) => void;
  sources: readonly PlaybackSourceOption[];
}

function sourceLabel(source: PlaybackSourceOption): string {
  return `${source.label} · ${source.provider === "local" ? "Local" : source.provider === "youtube" ? "YouTube" : source.provider === "soundcloud" ? "SoundCloud" : "Spotify"}`;
}

export function SourceSwitcher({ currentSourceId, disabled = false, onSwitch, sources }: SourceSwitcherProps) {
  if (sources.length === 0) {
    return <span className="player-source-empty">No source selected</span>;
  }

  const selectedSource = sources.find((source) => source.sourceId === currentSourceId) ?? sources[0];
  return (
    <label className="player-source-switcher">
      <span className="player-source-switcher-label">SOURCE</span>
      <span className="player-source-switcher-control">
        <ProviderBadge kind={selectedSource.provider} />
        <select aria-label="Playback source" disabled={disabled} onChange={(event) => onSwitch(event.target.value as SourceId)} title={selectedSource.availabilityDetail ?? "Choose an available playback source"} value={currentSourceId ?? selectedSource.sourceId}>
          {sources.map((source) => <option disabled={!source.available} key={source.sourceId} value={source.sourceId}>{sourceLabel(source)}{source.available ? "" : " · unavailable"}</option>)}
        </select>
      </span>
    </label>
  );
}
