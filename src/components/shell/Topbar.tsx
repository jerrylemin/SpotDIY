import { useNavigate } from "@tanstack/react-router";

import { SpotIcon } from "../icons/SpotIcon";
import { StatusDot } from "../common/StatusDot";
import type { AppStatus } from "../../types/domain";

interface TopbarProps {
  status?: AppStatus;
  statusError: boolean;
}

export function Topbar({ status, statusError }: TopbarProps) {
  const navigate = useNavigate();

  return (
    <header className="topbar">
      <div className="topbar-search-wrap">
        <button className="global-search-trigger" onClick={() => navigate({ to: "/search" })} type="button">
          <SpotIcon name="search" size={18} />
          <span>Search your library and sources</span>
          <kbd>CTRL K</kbd>
        </button>
      </div>
      <div className="topbar-actions">
        <StatusDot active={!statusError} label={statusError ? "Native bridge unavailable" : status?.runtime === "browser-preview" ? "Browser preview" : "Local-first"} />
        <button aria-label="Open settings" className="icon-button" onClick={() => navigate({ to: "/settings" })} title="Settings" type="button">
          <SpotIcon name="settings" size={18} />
        </button>
        <div className="avatar" aria-label="Local profile">L</div>
      </div>
    </header>
  );
}
