import type { PlaybackAudioDevice } from "../../types/domain";
import { SpotIcon } from "../icons/SpotIcon";

interface AudioDeviceMenuProps {
  devices: PlaybackAudioDevice[];
  selectedDeviceName: string;
  disabled?: boolean;
  loading?: boolean;
  onOpen: () => void;
  onSelect: (name: string) => void;
}

export function AudioDeviceMenu({
  devices,
  selectedDeviceName,
  disabled = false,
  loading = false,
  onOpen,
  onSelect,
}: AudioDeviceMenuProps) {
  return (
    <label className="player-device-menu">
      <span>
        <SpotIcon name="device" size={15} />
        Output
      </span>
      <select
        aria-label="Playback audio device"
        disabled={disabled || loading}
        onChange={(event) => onSelect(event.target.value)}
        onFocus={onOpen}
        value={selectedDeviceName}
      >
        {devices.length === 0 ? <option value={selectedDeviceName}>{loading ? "Loading devices…" : "Default output"}</option> : null}
        {devices.map((device) => (
          <option key={device.name} value={device.name}>
            {device.description || device.name}{device.selected ? " (Selected)" : ""}
          </option>
        ))}
      </select>
    </label>
  );
}
