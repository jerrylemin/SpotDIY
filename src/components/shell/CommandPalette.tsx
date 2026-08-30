import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";

import { useUiStore } from "../../stores/ui-store";
import { SpotIcon, type SpotIconName } from "../icons/SpotIcon";

interface Command {
  id: string;
  label: string;
  hint: string;
  icon: SpotIconName;
  path?: "/" | "/search" | "/library" | "/playlists" | "/downloads" | "/settings";
  disabled?: boolean;
}

const commands: Command[] = [
  { id: "search", label: "Search sources", hint: "Find music across your sources", icon: "search", path: "/search" },
  { id: "library", label: "Open library", hint: "Browse local files and quality", icon: "library", path: "/library" },
  { id: "playlists", label: "Open playlists", hint: "Curate and organize listening", icon: "playlist", path: "/playlists" },
  { id: "downloads", label: "Open downloads", hint: "View offline tasks and files", icon: "download", path: "/downloads" },
  { id: "settings", label: "Open settings", hint: "Storage, sources, and appearance", icon: "settings", path: "/settings" },
  { id: "play", label: "Play current queue", hint: "Queue is empty", icon: "play", disabled: true },
  { id: "queue", label: "Open queue", hint: "Queue is empty", icon: "queue", disabled: true },
];

export function CommandPalette() {
  const open = useUiStore((state) => state.commandPaletteOpen);
  const setOpen = useUiStore((state) => state.setCommandPaletteOpen);
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const filteredCommands = useMemo(
    () => commands.filter((command) => `${command.label} ${command.hint}`.toLowerCase().includes(query.toLowerCase())),
    [query],
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
        <div className="command-footer"><span><kbd>↑↓</kbd> Navigate</span><span><kbd>↵</kbd> Open</span><span><kbd>ESC</kbd> Close</span></div>
      </section>
    </div>
  );
}
