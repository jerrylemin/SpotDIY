import { Link } from "@tanstack/react-router";

import { EmptyState } from "../components/common/EmptyState";
import { ProviderBadge } from "../components/common/ProviderBadge";
import { SpotIcon } from "../components/icons/SpotIcon";
import { useAppStatus } from "../hooks/useAppStatus";
import { useLibraryStatus } from "../hooks/useLibrary";
import { usePlayback } from "../hooks/usePlayback";
import { useQueue } from "../hooks/useQueue";
import { useUiStore } from "../stores/ui-store";
import type { AppStatus } from "../types/domain";

function DashboardHero({ indexedTracks, currentTitle }: { indexedTracks: number; currentTitle: string | null }) {
  return (
    <section className="hero-panel home-dashboard-hero">
      <div className="hero-copy">
        <span className="eyebrow accent-eyebrow">YOUR MUSIC, YOUR MACHINE</span>
        <h1>{currentTitle ? <>Keep listening to<br /><em>{currentTitle}.</em></> : <>Make room for<br /><em>listening.</em></>}</h1>
        <p>{currentTitle ? "Your current track, source, and queue stay visible while you move through the workspace." : "SpotDIY brings local files and the sources you already use into one calm, source-aware workspace."}</p>
        <div className="hero-actions">
          <Link className="button button-primary" to="/library"><SpotIcon name="library" size={17} /> Open your library <SpotIcon name="arrow" size={15} /></Link>
          <Link className="button button-quiet" to="/search"><SpotIcon name="search" size={16} /> Explore sources</Link>
        </div>
      </div>
      <div className="hero-mark" aria-hidden="true">
        <div className="orbit orbit-one" /><div className="orbit orbit-two" /><div className="orbit orbit-three" />
        <div className="hero-core"><SpotIcon name="play" size={34} /></div>
        <span className="orbit-label label-local">LOCAL</span><span className="orbit-label label-online">ONLINE</span><span className="orbit-label label-fusion">FUSION</span>
      </div>
      <span className="home-hero-count"><strong>{indexedTracks}</strong><span>indexed tracks</span></span>
    </section>
  );
}

function SetupHome({ indexedTracks, providers }: { indexedTracks: number; providers: AppStatus["providers"] }) {
  return (
    <>
      <DashboardHero currentTitle={null} indexedTracks={indexedTracks} />
      <section className="section-block">
        <div className="section-heading"><div><span className="eyebrow">START HERE</span><h2>A workspace that grows with you</h2></div><span className="section-note">{indexedTracks} tracks indexed</span></div>
        <div className="onboarding-grid">
          <Link className="onboarding-card onboarding-card-lime" to="/library"><span className="card-index">01</span><SpotIcon name="library" size={24} /><strong>Bring in your library</strong><p>Index selected folders, preserve tags, and keep quality visible.</p><span className="card-link">Choose a folder <SpotIcon name="arrow" size={14} /></span></Link>
          <Link className="onboarding-card onboarding-card-violet" to="/search"><span className="card-index">02</span><SpotIcon name="search" size={24} /><strong>Search without context switching</strong><p>See local, YouTube, SoundCloud, and Spotify catalog results together.</p><span className="card-link">Open search <SpotIcon name="arrow" size={14} /></span></Link>
          <Link className="onboarding-card onboarding-card-ink" to="/settings"><span className="card-index">03</span><SpotIcon name="settings" size={24} /><strong>Shape your setup</strong><p>Choose storage, connect optional sources, and tune the player.</p><span className="card-link">Open settings <SpotIcon name="arrow" size={14} /></span></Link>
        </div>
      </section>
      <SourceStatus providers={providers} />
      {indexedTracks === 0 ? <EmptyState icon="library" eyebrow="LOCAL LIBRARY" title="Nothing indexed yet" description="Choose a folder to turn your files into a searchable, source-aware library." /> : null}
    </>
  );
}

function SourceStatus({ providers }: { providers: AppStatus["providers"] }) {
  return (
    <section className="section-block source-overview">
      <div className="section-heading"><div><span className="eyebrow">SOURCE STATUS</span><h2>Know where a track comes from</h2></div><Link className="text-link" to="/settings">Manage sources <SpotIcon name="arrow" size={14} /></Link></div>
      <div className="source-status-grid">
        {providers.map((provider) => <div className="source-status-card" key={provider.kind}><div className="source-status-top"><ProviderBadge kind={provider.kind} /><span className={`availability ${provider.available ? "available" : "unavailable"}`}>{provider.available ? "Available" : "Setup needed"}</span></div><strong>{provider.label}</strong><p>{provider.detail}</p></div>)}
      </div>
    </section>
  );
}

function LiveDashboard({ indexedTracks, providers }: { indexedTracks: number; providers: AppStatus["providers"] }) {
  const playback = usePlayback();
  const queue = useQueue();
  const openTrackInspector = useUiStore((state) => state.openTrackInspector);
  const current = playback.snapshot.currentTrackId ? {
    trackId: playback.snapshot.currentTrackId,
    title: playback.snapshot.title ?? "Untitled track",
    artists: playback.snapshot.artists,
  } : null;
  const waiting = queue.workspace.upNext.length + queue.workspace.later.length;

  return (
    <>
      <DashboardHero currentTitle={current?.title ?? null} indexedTracks={indexedTracks} />
      <div className="home-dashboard-grid">
        <section className="home-dashboard-card home-now-playing-card">
          <div className="section-heading"><div><span className="eyebrow">NOW PLAYING</span><h2>{current ? "Resume where you left off" : "Nothing playing"}</h2></div><span className="section-note">{playback.snapshot.phase}</span></div>
          {current ? <div className="home-current-track"><div className="home-current-art"><SpotIcon name="play" size={22} /></div><div><strong>{current.title}</strong><span>{current.artists.join(" · ") || "Unknown artist"}</span></div><button className="button button-quiet button-small" onClick={() => openTrackInspector(current.trackId)} type="button"><SpotIcon name="info" size={14} /> Inspect</button></div> : <p className="home-dashboard-empty">Choose a local track to start playback. The in-shell player will keep its state visible as you browse.</p>}
          <div className="home-dashboard-actions"><Link className="text-link" to="/library">Open library <SpotIcon name="arrow" size={14} /></Link><Link className="text-link" to="/lyrics">Open lyrics <SpotIcon name="lyrics" size={14} /></Link></div>
        </section>
        <section className="home-dashboard-card">
          <div className="section-heading"><div><span className="eyebrow">LIBRARY</span><h2>Local index</h2></div><Link className="text-link" to="/library">Browse <SpotIcon name="arrow" size={14} /></Link></div>
          <div className="home-stat-pair"><div><strong>{indexedTracks}</strong><span>indexed</span></div><div><strong>{providers.filter((provider) => provider.available).length}</strong><span>available sources</span></div></div>
          <p className="home-dashboard-empty">Metadata, measured quality, and file provenance stay attached to the local source.</p>
        </section>
        <section className="home-dashboard-card">
          <div className="section-heading"><div><span className="eyebrow">QUEUE</span><h2>{waiting > 0 ? `${waiting} waiting` : "Queue is clear"}</h2></div><button className="text-link home-plain-button" onClick={() => useUiStore.getState().setQueueDrawerOpen(true)} type="button">Open queue <SpotIcon name="arrow" size={14} /></button></div>
          {queue.workspace.upNext.slice(0, 3).map((entry) => <div className="home-queue-row" key={entry.id}><strong>{entry.title ?? `Track ${entry.trackId}`}</strong><span>{entry.artists.join(" · ") || "Unknown artist"}</span></div>)}
          {waiting === 0 ? <p className="home-dashboard-empty">The persistent queue will appear here when you add the next listen.</p> : null}
        </section>
        <SourceStatus providers={providers} />
      </div>
    </>
  );
}

export function HomePage() {
  const status = useAppStatus();
  const library = useLibraryStatus();
  const indexedTracks = library.data?.indexedTrackCount ?? status.data?.tracksIndexed ?? 0;
  const providers = status.data?.providers ?? [];
  const setup = (status.data?.firstRun ?? true) || indexedTracks === 0;

  return <div className="page-stack home-page">{setup ? <SetupHome indexedTracks={indexedTracks} providers={providers} /> : <LiveDashboard indexedTracks={indexedTracks} providers={providers} />}</div>;
}
