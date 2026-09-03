import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";

import { usePlayback } from "../../hooks/usePlayback";
import { useListeningModes } from "../../hooks/useListeningModes";
import { useWindowsIntegration } from "../../hooks/useWindowsIntegration";
import { useUiStore } from "../../stores/ui-store";
import { SpotIcon, type SpotIconName } from "../icons/SpotIcon";

interface Command {
  id: string;
  label: string;
  hint: string;
  icon: SpotIconName;
  path?: "/" | "/search" | "/library" | "/lyrics" | "/playlists" | "/downloads" | "/analytics" | "/settings" | "/music-map" | "/library-galaxy" | "/theme-studio";
  action?: () => void;
  disabled?: boolean;
}

export function CommandPalette() {
  const open = useUiStore((state) => state.commandPaletteOpen);
  const setOpen = useUiStore((state) => state.setCommandPaletteOpen);
  const navigate = useNavigate();
  const playback = usePlayback();
  const windows = useWindowsIntegration();
  const modes = useListeningModes();
  const playerMode = useUiStore((state) => state.playerMode);
  const setPlayerMode = useUiStore((state) => state.setPlayerMode);
  const setQueueDrawerOpen = useUiStore((state) => state.setQueueDrawerOpen);
  const openTrackInspector = useUiStore((state) => state.openTrackInspector);
  const inputRef = useRef<HTMLInputElement>(null);
  const originRef = useRef<HTMLElement | null>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);

  const queueReady = playback.snapshot.currentTrackId !== null || playback.snapshot.queueLength > 0;
  const listeningMode = modes.state.data ?? { privateSession: false, temporary: false };
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
      hint: playback.snapshot.queueLength > 0 ? "Stop playback and empty the persistent queue" : "Queue is empty",
      icon: "trash",
      disabled: playback.snapshot.queueLength === 0 || playback.pending,
      action: () => { void playback.clearQueue(); },
    },
  ], [playback, queueReady]);

  const overlayCommands = useMemo<Command[]>(() => {
    if (!windows.snapshot?.platformSupported) {
      return [];
    }
    return [
      { id: "overlay-mini", label: "Toggle Mini Overlay", hint: "Show or hide the native Mini overlay", icon: "collapse", action: () => { void windows.toggleOverlay("mini"); } },
      { id: "overlay-edge", label: "Toggle Edge Overlay", hint: "Show or hide the native Edge overlay", icon: "expand", action: () => { void windows.toggleOverlay("edge"); } },
      { id: "overlay-lyrics", label: "Toggle Lyrics Overlay", hint: "Show or hide the native Lyrics overlay", icon: "lyrics", action: () => { void windows.toggleOverlay("lyrics"); } },
      { id: "overlay-gaming", label: "Toggle Gaming Overlay", hint: "Show or hide the native Gaming overlay", icon: "play", action: () => { void windows.toggleOverlay("gaming"); } },
      { id: "show-spotdiy", label: "Show SpotDIY", hint: "Bring the main SpotDIY window to the front", icon: "home", action: () => { void windows.showMain(); } },
    ];
  }, [windows]);

  const commands = useMemo<Command[]>(() => [
    { id: "search", label: "Search sources", hint: "Find music across your sources", icon: "search", path: "/search" },
    { id: "library", label: "Open library", hint: "Browse local files and playback actions", icon: "library", path: "/library" },
    { id: "lyrics", label: "Open lyrics", hint: "Follow lyrics and track notes", icon: "lyrics", path: "/lyrics" },
    { id: "playlists", label: "Open playlists", hint: "Curate and organize listening", icon: "playlist", path: "/playlists" },
    { id: "downloads", label: "Open downloads", hint: "View offline tasks and files", icon: "download", path: "/downloads" },
    { id: "analytics", label: "Open analytics", hint: "Review local listening history and patterns", icon: "analytics", path: "/analytics" },
    { id: "music-map", label: "Open Music Map", hint: "Explore genre, artist, album, and track relationships", icon: "spark", path: "/music-map" },
    { id: "library-galaxy", label: "Open Library Galaxy", hint: "Plot your local library in a bounded Canvas workspace", icon: "expand", path: "/library-galaxy" },
    { id: "theme-studio", label: "Open Theme Studio", hint: "Draft themes and preview your workspace", icon: "theme", path: "/theme-studio" },
    { id: "settings", label: "Open settings", hint: "Storage, sources, and appearance", icon: "settings", path: "/settings" },
    {
      id: "private-session",
      label: listeningMode.privateSession ? "Disable Private Session" : "Enable Private Session",
      hint: listeningMode.temporary ? "Temporary Listening keeps Private Session enabled" : "Keep history, sessions, and analytics local but unwritten",
      icon: "info",
      disabled: listeningMode.temporary || modes.privateSession.isPending,
      action: () => modes.privateSession.mutate(!listeningMode.privateSession),
    },
    {
      id: "temporary-listening",
      label: listeningMode.temporary ? "Exit Temporary Listening" : "Enter Temporary Listening",
      hint: listeningMode.temporary ? "Restore the durable queue without autoplay" : "Pause durable writes and restore the queue when you leave",
      icon: "spark",
      disabled: listeningMode.temporary ? modes.temporaryExit.isPending : modes.temporaryEnter.isPending,
      action: () => (listeningMode.temporary ? modes.temporaryExit.mutate() : modes.temporaryEnter.mutate()),
    },
    {
      id: "queue",
      label: "Open queue",
      hint: "Open the persistent playback workspace",
      icon: "queue",
      action: () => setQueueDrawerOpen(true),
    },
    {
      id: "inspect-current",
      label: "Inspect current track",
      hint: playback.snapshot.currentTrackId ? "Open persisted metadata, sources, and collection state" : "Nothing is currently selected",
      icon: "info",
      disabled: playback.snapshot.currentTrackId === null,
      action: () => {
        if (playback.snapshot.currentTrackId) {
          openTrackInspector(playback.snapshot.currentTrackId);
        }
      },
    },
    {
      id: "standard-player",
      label: "Use standard player",
      hint: playerMode === "standard" ? "Standard bottom player is active" : "Use the full transport bar",
      icon: "collapse",
      disabled: playerMode === "standard",
      action: () => setPlayerMode("standard"),
    },
    {
      id: "mini-player",
      label: "Use mini player",
      hint: playerMode === "mini" ? "Mini player is active" : "Keep transport compact while browsing",
      icon: "collapse",
      disabled: playerMode === "mini",
      action: () => setPlayerMode("mini"),
    },
    {
      id: "expanded-player",
      label: "Open expanded now playing",
      hint: playerMode === "expanded" ? "Expanded now playing is active" : "Open the full in-shell player surface",
      icon: "expand",
      disabled: playerMode === "expanded",
      action: () => setPlayerMode("expanded"),
    },
    ...transportCommands,
    ...overlayCommands,
  ], [listeningMode.privateSession, listeningMode.temporary, modes, openTrackInspector, overlayCommands, playback.snapshot.currentTrackId, playerMode, setPlayerMode, setQueueDrawerOpen, transportCommands]);

  const filteredCommands = useMemo(
    () => commands.filter((command) => `${command.label} ${command.hint}`.toLowerCase().includes(query.toLowerCase())),
    [commands, query],
  );

  useEffect(() => {
    if (open) {
      originRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      setQuery("");
      setSelected(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
    return () => {
      if (originRef.current?.isConnected) {
        originRef.current.focus();
      }
      originRef.current = null;
    };
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
              title={command.disabled ? command.hint : undefined}
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
