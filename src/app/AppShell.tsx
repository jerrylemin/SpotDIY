import { useEffect } from "react";
import { Outlet } from "@tanstack/react-router";

import { useAppStatus } from "../hooks/useAppStatus";
import { useUiStore } from "../stores/ui-store";
import { CommandPalette } from "../components/shell/CommandPalette";
import { PlayerBar } from "../components/shell/PlayerBar";
import { Sidebar } from "../components/shell/Sidebar";
import { Topbar } from "../components/shell/Topbar";

export function AppShell() {
  const appStatus = useAppStatus();
  const toggleCommandPalette = useUiStore((state) => state.toggleCommandPalette);
  const setCommandPaletteOpen = useUiStore((state) => state.setCommandPaletteOpen);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        toggleCommandPalette();
      }
      if (event.key === "Escape") setCommandPaletteOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [setCommandPaletteOpen, toggleCommandPalette]);

  return (
    <div className="app-shell">
      <Sidebar />
      <div className="content-shell">
        <Topbar status={appStatus.data} statusError={appStatus.isError} />
        <main className="page-content"><Outlet /></main>
        <PlayerBar />
      </div>
      <CommandPalette />
    </div>
  );
}
