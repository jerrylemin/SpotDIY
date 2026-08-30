import { Link } from "@tanstack/react-router";

import { EmptyState } from "../components/common/EmptyState";
import { SpotIcon } from "../components/icons/SpotIcon";

export function PlaylistsPage() {
  return <div className="page-stack"><section className="page-intro"><div><span className="eyebrow">PLAYLISTS</span><h1>Shape the <em>moment.</em></h1><p>Keep playlists simple, branchable, and close to how you actually listen.</p></div><button className="button button-primary" disabled type="button"><SpotIcon name="playlist" size={16} /> New playlist</button></section><EmptyState icon="playlist" eyebrow="NO PLAYLISTS YET" title="Your next context belongs here" description="Create playlists, branch a favorite, and keep a listening space ready for the next session." action={<Link className="button button-quiet" to="/search">Find something to add <SpotIcon name="arrow" size={14} /></Link>} /><section className="feature-strip"><span className="eyebrow">DESIGNED FOR DEPTH</span><div><strong>Queue, inbox, tags, and branches</strong><p>The playlist workspace will connect to the unified track model, so one track can carry local and online sources without duplicate entries.</p></div></section></div>;
}
