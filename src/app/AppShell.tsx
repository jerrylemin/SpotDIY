import { useEffect } from "react";
import { Outlet } from "@tanstack/react-router";

import { useAppStatus } from "../hooks/useAppStatus";
import { useUiStore } from "../stores/ui-store";
import { CommandPalette } from "../components/shell/CommandPalette";
import { PlayerBar } from "../components/shell/PlayerBar";
import { QueueDrawer } from "../components/queue/QueueDrawer";
import { SearchResultInspector, TrackInspector } from "../components/inspector/TrackInspector";
import { Sidebar } from "../components/shell/Sidebar";
import { Topbar } from "../components/shell/Topbar";

export function AppShell() {
  const appStatus = useAppStatus();
  const toggleCommandPalette = useUiStore((state) => state.toggleCommandPalette);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        toggleCommandPalette();
        return;
      }
      if (event.key !== "Escape" || event.defaultPrevented) {
        return;
      }

      const state = useUiStore.getState();
      if (state.commandPaletteOpen) {
        event.preventDefault();
        state.setCommandPaletteOpen(false);
      } else if (state.inspector.kind !== "closed") {
        event.preventDefault();
        state.closeInspector();
      } else if (state.queueDrawerOpen) {
        event.preventDefault();
        state.setQueueDrawerOpen(false);
      } else if (state.playerMode === "expanded") {
        event.preventDefault();
        state.setPlayerMode("standard");
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [toggleCommandPalette]);

  useEffect(() => {
    const timers = new WeakMap<HTMLElement, number>();
    const onScroll = (event: Event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement) || target.scrollHeight <= target.clientHeight) {
        return;
      }
      target.classList.add("is-scrolling");
      const timer = timers.get(target);
      if (timer !== undefined) {
        window.clearTimeout(timer);
      }
      timers.set(target, window.setTimeout(() => {
        target.classList.remove("is-scrolling");
        timers.delete(target);
      }, 850));
    };

    document.addEventListener("scroll", onScroll, true);
    return () => document.removeEventListener("scroll", onScroll, true);
  }, []);

  const inspector = useUiStore((state) => state.inspector);
  const closeInspector = useUiStore((state) => state.closeInspector);

  return (
    <div className="app-shell">
      <Sidebar />
      <div className="content-shell">
        <Topbar status={appStatus.data} statusError={appStatus.isError} />
        <main className="page-content"><Outlet /></main>
        <PlayerBar />
        <QueueDrawer />
      </div>
      {inspector.kind === "track" ? <TrackInspector manageEscape onClose={closeInspector} trackId={inspector.trackId} /> : null}
      {inspector.kind === "search" ? <SearchResultInspector manageEscape onClose={closeInspector} result={inspector.result} /> : null}
      <CommandPalette />
    </div>
  );
}
