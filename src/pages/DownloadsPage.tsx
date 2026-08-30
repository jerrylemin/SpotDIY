import { Link } from "@tanstack/react-router";

import { EmptyState } from "../components/common/EmptyState";
import { SpotIcon } from "../components/icons/SpotIcon";

export function DownloadsPage() {
  return <div className="page-stack"><section className="page-intro"><div><span className="eyebrow">DOWNLOADS</span><h1>Offline, with <em>provenance.</em></h1><p>Every task will carry source, quality, progress, and destination context.</p></div><div className="page-intro-stat"><strong>0</strong><span>active tasks</span></div></section><EmptyState icon="download" eyebrow="DOWNLOAD QUEUE EMPTY" title="Downloaded tracks appear here" description="When the download engine is connected, tasks will persist across restarts and keep the original source quality honest." action={<Link className="button button-quiet" to="/search">Browse sources <SpotIcon name="arrow" size={14} /></Link>} /><section className="download-state-legend"><span className="eyebrow">TASK STATES</span><div><span className="state-chip">Queued</span><span className="state-chip">Resolving</span><span className="state-chip">Downloading</span><span className="state-chip">Post-processing</span><span className="state-chip">Completed</span></div></section></div>;
}
