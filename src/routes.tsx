import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";

import { AppShell } from "./app/AppShell";
import { AnalyticsPage } from "./pages/AnalyticsPage";
import { DownloadsPage } from "./pages/DownloadsPage";
import { HomePage } from "./pages/HomePage";
import { LibraryPage } from "./pages/LibraryPage";
import { LyricsPage } from "./pages/LyricsPage";
import { PlaylistsPage } from "./pages/PlaylistsPage";
import { SearchPage } from "./pages/SearchPage";
import { SettingsPage } from "./pages/SettingsPage";

const rootRoute = createRootRoute({ component: AppShell });
const homeRoute = createRoute({ getParentRoute: () => rootRoute, path: "/", component: HomePage });
const searchRoute = createRoute({ getParentRoute: () => rootRoute, path: "/search", component: SearchPage });
const libraryRoute = createRoute({ getParentRoute: () => rootRoute, path: "/library", component: LibraryPage });
const lyricsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/lyrics", component: LyricsPage });
const playlistsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/playlists", component: PlaylistsPage });
const downloadsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/downloads", component: DownloadsPage });
const analyticsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/analytics", component: AnalyticsPage });
const settingsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/settings", component: SettingsPage });

const routeTree = rootRoute.addChildren([homeRoute, searchRoute, libraryRoute, lyricsRoute, playlistsRoute, downloadsRoute, analyticsRoute, settingsRoute]);

export const router = createRouter({ routeTree, defaultPreload: "intent" });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
