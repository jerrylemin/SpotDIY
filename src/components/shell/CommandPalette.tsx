import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";

import { usePlayback } from "../../hooks/usePlayback";
import { useUiStore } from "../../stores/ui-store";
import { SpotIcon, type SpotIconName } from "../icons/SpotIcon";

interface Command {
  id: string;
  label: string;
  hint: string;
  icon: SpotIconName;
  path?: "/" | "/search" | "/library" | "/playlists" | "/downloads" | "/settings";
  action?: () => void;
  disabled?: boolean;
}

export function CommandPalette() {
  const open = useUiStore((state) => state.commandPaletteOpen);
  const setOpen = useUiStore((state) => state.setCommandPaletteOpen);
  const navigate = useNavigate();
  const playback = usePlayback();
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);

  const queueReady = playback.snapshot.currentTrackId !== null || playback.snapshot.queueLength > 0;
  const transportCommands = useMemo<Command[]>(() => [
    {
      id: "play-pause",
      label: playback.snapshot.phase === "playing" ? "Pause playback" : "Play / Pause",
      hint: queueReady ? "Toggle the current playback state" : "Queue is empty",
      icon: playback.snapshot.phase === "playing" ? "pause" : "play",
      disabled: !queueReady || playback.pending,
      action: () => { void playback.togglePlayPause(); },
    },
    {
      id: "next-track",
      label: "Next track",
      hint: queueReady ? "Advance within the playback queue" : "Queue is empty",
      icon: "next",
      disabled: !queueReady || playback.pending,
      action: () => { void playback.nextTrack(); },
    },
    {
      id: "previous-track",
      label: "Previous track",
      hint: queueReady ? "Restart or move to the previous queue entry" : "Queue is empty",
      icon: "previous",
      disabled: !queueReady || playback.pending,
      action: () => { void playback.previousTrack(); },
    },
    {
      id: "clear-queue",
      label: "Clear queue",
      hint: playback.snapshot.queueLength > 0 ? "Stop playback and empty the transient queue" : "Queue is empty",
      icon: "trash",
      disabled: playback.snapshot.queueLength === 0 || playback.pending,
      action: () => { void playback.clearQueue(); },
    },
  ], [playback, queueReady]);

  const commands = useMemo<Command[]>(() => [
    { id: "search", label: "Search sources", hint: "Find music across your sources", icon: "search", path: "/search" },
    { id: "library", label: "Open library", hint: "Browse local files and playback actions", icon: "library", path: "/library" },
    { id: "playlists", label: "Open playlists", hint: "Curate and organize listening", icon: "playlist", path: "/playlists" },
    { id: "downloads", label: "Open downloads", hint: "View offline tasks and files", icon: "download", path: "/downloads" },
    { id: "settings", label: "Open settings", hint: "Storage, sources, and appearance", icon: "settings", path: "/settings" },
    ...transportCommands,
  ], [transportCommands]);

  const filteredCommands = useMemo(
    () => commands.filter((command) => `${command.label} ${command.hint}`.toLowerCase().includes(query.toLowerCase())),
    [commands, query],
  );

  useEffect(() => {
    if (open) {
      setQuery("");
      setSelected(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  if (!open) return null;

  const execute = (command: Command | undefined) => {
    if (!command || command.disabled) return;
    setOpen(false);
    command.action?.();
    if (command.path) navigate({ to: command.path });
  };

  return (
    <div className="command-backdrop" onMouseDown={() => setOpen(false)}>
      <section aria-label="Command palette" className="command-palette" onMouseDown={(event) => event.stopPropagation()}>
        <div className="command-input-wrap">
          <SpotIcon name="command" size={20} />
          <input
            aria-label="Search commands"
            onChange={(event) => { setQuery(event.target.value); setSelected(0); }}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown") { event.preventDefault(); setSelected((value) => Math.min(value + 1, filteredCommands.length - 1)); }
              if (event.key === "ArrowUp") { event.preventDefault(); setSelected((value) => Math.max(value - 1, 0)); }
              if (event.key === "Enter") { event.preventDefault(); execute(filteredCommands[selected]); }
              if (event.key === "Escape") setOpen(false);
            }}
            placeholder="What do you want to do?"
            ref={inputRef}
            value={query}
          />
          <kbd>ESC</kbd>
        </div>
        <div className="command-list">
          {filteredCommands.map((command, index) => (
            <button
              className={`command-item${index === selected ? " command-item-selected" : ""}`}
              disabled={command.disabled}
              key={command.id}
              onClick={() => execute(command)}
              onMouseEnter={() => setSelected(index)}
              type="button"
            >
              <span className="command-icon"><SpotIcon name={command.icon} size={17} /></span>
              <span className="command-copy"><strong>{command.label}</strong><small>{command.hint}</small></span>
              {index === selected && !command.disabled ? <SpotIcon name="arrow" size={16} /> : null}
            </button>
          ))}
          {filteredCommands.length === 0 ? <p className="command-empty">No command matches “{query}”.</p> : null}
        </div>
        <div className="command-footer"><span><kbd>↑↓</kbd> Navigate</span><span><kbd>↵</kbd> Run</span><span><kbd>ESC</kbd> Close</span></div>
      </section>
    </div>
  );
}
