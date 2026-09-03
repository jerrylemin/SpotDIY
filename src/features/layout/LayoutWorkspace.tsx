import { useTheme } from "../theme/theme-controller";
import { LAYOUT_PROFILE_LABELS, LAYOUT_PROFILES } from "./layout-profiles";
import type { LayoutProfile } from "../../types/domain";

export function LayoutWorkspace() {
  const theme = useTheme();
  const active = theme.settings?.layoutProfile ?? "comfortable";
  return (
    <section aria-labelledby="layout-workspace-heading" className="theme-studio-section">
      <div className="section-heading"><div><span className="eyebrow">LAYOUT</span><h2 id="layout-workspace-heading">Shape the workspace</h2></div><span className="section-note">Persisted density</span></div>
      <div className="layout-profile-grid">
        {LAYOUT_PROFILES.map((profile: LayoutProfile) => (
          <button aria-pressed={profile === active} className={`layout-profile-card${profile === active ? " layout-profile-card-active" : ""}`} key={profile} onClick={() => { void theme.setLayoutProfile(profile); }} type="button">
            <span className={`layout-profile-preview layout-preview-${profile}`}><i /><i /><i /><b /></span>
            <strong>{LAYOUT_PROFILE_LABELS[profile]}</strong>
            <small>{profile === "comfortable" ? "Roomy reading and browsing" : profile === "compact" ? "More tracks in view" : "Dense power-user layout"}</small>
          </button>
        ))}
      </div>
    </section>
  );
}
