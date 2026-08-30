import { Link } from "@tanstack/react-router";

import { EmptyState } from "../components/common/EmptyState";
import { SpotIcon } from "../components/icons/SpotIcon";
import { useAppStatus } from "../hooks/useAppStatus";

export function LibraryPage() {
  const status = useAppStatus();
  const folders = status.data?.musicFolders ?? [];
  const tracks = status.data?.tracksIndexed ?? 0;

  return (
    <div className="page-stack">
      <section className="page-intro"><div><span className="eyebrow">LOCAL LIBRARY</span><h1>Your collection, <em>in focus.</em></h1><p>SpotDIY reads your files where they are and keeps the index close to the source.</p></div><div className="page-intro-stat"><strong>{tracks}</strong><span>tracks indexed</span></div></section>
      {folders.length > 0 ? <section className="folder-list"><div className="section-heading"><div><span className="eyebrow">MUSIC FOLDERS</span><h2>Connected locations</h2></div><button className="button button-primary" disabled title="Folder selection is part of the next native library slice" type="button"><SpotIcon name="folder" size={16} /> Add folder</button></div>{folders.map((folder) => <div className="folder-row" key={folder}><SpotIcon name="folder" size={20} /><span>{folder}</span><span className="folder-status">Indexed</span></div>)}</section> : <EmptyState icon="folder" eyebrow="NO MUSIC FOLDERS" title="Give your library a place to start" description="Add one or more music folders. SpotDIY will scan recursively, retain embedded metadata, and avoid rescanning unchanged files." action={<Link className="button button-primary" to="/settings"><SpotIcon name="settings" size={16} /> Open storage settings <SpotIcon name="arrow" size={14} /></Link>} />}
      <section className="library-principles"><div><span className="eyebrow">INDEXING PRINCIPLES</span><h2>Local by default.</h2></div><div className="principle-list"><div><span>01</span><strong>Incremental scans</strong><p>Only changed files need another look.</p></div><div><span>02</span><strong>Quality stays honest</strong><p>Codec, bitrate, sample rate, and provenance stay visible.</p></div><div><span>03</span><strong>Files stay yours</strong><p>SpotDIY keeps user music at the paths you choose.</p></div></div></section>
      <Link className="text-link" to="/settings">Storage and portable mode <SpotIcon name="arrow" size={14} /></Link>
    </div>
  );
}
