import { Link } from "@tanstack/react-router";

import { useLibraryStatus } from "../../hooks/useLibrary";
import { type NavItem } from "../../types/domain";
import { SpotIcon, SpotLogo } from "../icons/SpotIcon";

const navItems: NavItem[] = [
  { id: "home", label: "Home", shortLabel: "Home", icon: "home" },
  { id: "search", label: "Search", shortLabel: "Search", icon: "search" },
  { id: "library", label: "Your library", shortLabel: "Library", icon: "library" },
  { id: "lyrics", label: "Lyrics & notes", shortLabel: "Lyrics", icon: "lyrics" },
  { id: "playlists", label: "Playlists", shortLabel: "Playlists", icon: "playlist" },
  { id: "downloads", label: "Downloads", shortLabel: "Downloads", icon: "download" },
  { id: "analytics", label: "Analytics", shortLabel: "Analytics", icon: "analytics" },
];

const exploreItems: NavItem[] = [
  { id: "music-map", label: "Music Map", shortLabel: "Map", icon: "spark" },
  { id: "library-galaxy", label: "Library Galaxy", shortLabel: "Galaxy", icon: "expand" },
  { id: "theme-studio", label: "Theme Studio", shortLabel: "Theme", icon: "theme" },
];

function navPath(item: NavItem): "/" | `/${Exclude<NavItem["id"], "home">}` {
  return item.id === "home" ? "/" : `/${item.id}` as `/${Exclude<NavItem["id"], "home">}`;
}

export function Sidebar() {
  const libraryStatus = useLibraryStatus();
  const folderCount = libraryStatus.data?.folders.length ?? 0;

  return (
    <aside className="sidebar">
      <div className="brand-lockup">
        <SpotLogo size={34} />
        <div>
          <span className="brand-name">SpotDIY</span>
          <span className="brand-caption">music / operating environment</span>
        </div>
      </div>

      <nav className="primary-nav" aria-label="Primary navigation">
        <span className="nav-section-label">Workspace</span>
        {navItems.map((item) => (
          <Link
            activeProps={{ className: "nav-item nav-item-active" }}
            className="nav-item"
            key={item.id}
            to={navPath(item)}
          >
            <SpotIcon name={item.icon} size={19} />
            <span>{item.label}</span>
          </Link>
        ))}
        <span className="nav-section-label nav-section-label-explore">Explore</span>
        {exploreItems.map((item) => (
          <Link
            activeProps={{ className: "nav-item nav-item-active" }}
            className="nav-item"
            key={item.id}
            to={navPath(item)}
          >
            <SpotIcon name={item.icon} size={19} />
            <span>{item.label}</span>
          </Link>
        ))}
      </nav>

      <div className="sidebar-spacer" />

      <div className="library-rail-card">
        <div className="library-rail-heading">
          <span className="mini-kicker">LOCAL INDEX</span>
          <span className="index-count">{libraryStatus.data?.indexedTrackCount ?? 0}</span>
        </div>
        <strong>{folderCount === 0 ? "No folders yet" : `${folderCount} folder${folderCount === 1 ? "" : "s"} connected`}</strong>
        <p>{folderCount === 0 ? "Your music stays on your machine." : "Scanning stays incremental."}</p>
        <Link className="rail-action" to="/library">
          <SpotIcon name="folder" size={15} />
          <span>{folderCount === 0 ? "Add a music folder" : "Open library"}</span>
        </Link>
      </div>

      <Link className="settings-link" to="/settings">
        <SpotIcon name="settings" size={18} />
        <span>Settings</span>
        <span className="settings-shortcut">⌘</span>
      </Link>
    </aside>
  );
}
