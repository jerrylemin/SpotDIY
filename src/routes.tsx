import { createRootRoute, createRoute, createRouter, lazyRouteComponent } from "@tanstack/react-router";

import { AppShell } from "./app/AppShell";

const rootRoute = createRootRoute({ component: AppShell });
const homeRoute = createRoute({ getParentRoute: () => rootRoute, path: "/", component: lazyRouteComponent(() => import("./pages/HomePage"), "HomePage") });
const searchRoute = createRoute({ getParentRoute: () => rootRoute, path: "/search", component: lazyRouteComponent(() => import("./pages/SearchPage"), "SearchPage") });
const libraryRoute = createRoute({ getParentRoute: () => rootRoute, path: "/library", component: lazyRouteComponent(() => import("./pages/LibraryPage"), "LibraryPage") });
const lyricsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/lyrics", component: lazyRouteComponent(() => import("./pages/LyricsPage"), "LyricsPage") });
const playlistsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/playlists", component: lazyRouteComponent(() => import("./pages/PlaylistsPage"), "PlaylistsPage") });
const downloadsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/downloads", component: lazyRouteComponent(() => import("./pages/DownloadsPage"), "DownloadsPage") });
const analyticsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/analytics", component: lazyRouteComponent(() => import("./pages/AnalyticsPage"), "AnalyticsPage") });
const settingsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/settings", component: lazyRouteComponent(() => import("./pages/SettingsPage"), "SettingsPage") });
const musicMapRoute = createRoute({ getParentRoute: () => rootRoute, path: "/music-map", component: lazyRouteComponent(() => import("./pages/MusicMapPage"), "MusicMapPage") });
const libraryGalaxyRoute = createRoute({ getParentRoute: () => rootRoute, path: "/library-galaxy", component: lazyRouteComponent(() => import("./pages/LibraryGalaxyPage"), "LibraryGalaxyPage") });
const themeStudioRoute = createRoute({ getParentRoute: () => rootRoute, path: "/theme-studio", component: lazyRouteComponent(() => import("./pages/ThemeStudioPage"), "ThemeStudioPage") });

const routeTree = rootRoute.addChildren([homeRoute, searchRoute, libraryRoute, lyricsRoute, playlistsRoute, downloadsRoute, analyticsRoute, settingsRoute, musicMapRoute, libraryGalaxyRoute, themeStudioRoute]);

export const router = createRouter({ routeTree, defaultPreload: "intent" });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
