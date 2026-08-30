import { Link } from "@tanstack/react-router";

import { EmptyState } from "../components/common/EmptyState";
import { ProviderBadge } from "../components/common/ProviderBadge";
import { SpotIcon } from "../components/icons/SpotIcon";
import { useAppStatus } from "../hooks/useAppStatus";

export function HomePage() {
  const status = useAppStatus();

  return (
    <div className="page-stack home-page">
      <section className="hero-panel">
        <div className="hero-copy">
          <span className="eyebrow accent-eyebrow">YOUR MUSIC, YOUR MACHINE</span>
          <h1>Make room for<br /><em>listening.</em></h1>
          <p>SpotDIY brings local files and the sources you already use into one calm, source-aware workspace.</p>
          <div className="hero-actions">
            <Link className="button button-primary" to="/library"><SpotIcon name="folder" size={17} /> Add a music folder <SpotIcon name="arrow" size={15} /></Link>
            <Link className="button button-quiet" to="/search"><SpotIcon name="search" size={16} /> Explore sources</Link>
          </div>
        </div>
        <div className="hero-mark" aria-hidden="true">
          <div className="orbit orbit-one" /><div className="orbit orbit-two" /><div className="orbit orbit-three" />
          <div className="hero-core"><SpotIcon name="play" size={34} /></div>
          <span className="orbit-label label-local">LOCAL</span><span className="orbit-label label-online">ONLINE</span><span className="orbit-label label-fusion">FUSION</span>
        </div>
      </section>

      <section className="section-block">
        <div className="section-heading"><div><span className="eyebrow">START HERE</span><h2>A workspace that grows with you</h2></div><span className="section-note">{status.data?.tracksIndexed ?? 0} tracks indexed</span></div>
        <div className="onboarding-grid">
          <Link className="onboarding-card onboarding-card-lime" to="/library"><span className="card-index">01</span><SpotIcon name="library" size={24} /><strong>Bring in your library</strong><p>Index selected folders, preserve tags, and keep quality visible.</p><span className="card-link">Choose a folder <SpotIcon name="arrow" size={14} /></span></Link>
          <Link className="onboarding-card onboarding-card-violet" to="/search"><span className="card-index">02</span><SpotIcon name="search" size={24} /><strong>Search without context switching</strong><p>See local, YouTube, SoundCloud, and Spotify catalog results together.</p><span className="card-link">Open search <SpotIcon name="arrow" size={14} /></span></Link>
          <Link className="onboarding-card onboarding-card-ink" to="/settings"><span className="card-index">03</span><SpotIcon name="settings" size={24} /><strong>Shape your setup</strong><p>Choose storage, connect optional sources, and tune the player.</p><span className="card-link">Open settings <SpotIcon name="arrow" size={14} /></span></Link>
        </div>
      </section>

      <section className="section-block source-overview">
        <div className="section-heading"><div><span className="eyebrow">SOURCE MAP</span><h2>Know where a track comes from</h2></div><Link className="text-link" to="/settings">Manage sources <SpotIcon name="arrow" size={14} /></Link></div>
        <div className="source-status-grid">
          {(status.data?.providers ?? []).map((provider) => <div className="source-status-card" key={provider.kind}><div className="source-status-top"><ProviderBadge kind={provider.kind} /><span className={`availability ${provider.available ? "available" : "unavailable"}`}>{provider.available ? "Available" : "Setup needed"}</span></div><strong>{provider.label}</strong><p>{provider.detail}</p></div>)}
        </div>
      </section>

      {!status.data?.firstRun && status.data?.tracksIndexed === 0 ? <EmptyState icon="library" eyebrow="LOCAL LIBRARY" title="Nothing indexed yet" description="Choose a folder to turn your files into a searchable, source-aware library." /> : null}
    </div>
  );
}
