import { useEffect, useState } from "react";
import * as ipc from "../services/ipc";

import { EmptyState } from "../components/common/EmptyState";
import { LibraryFolderRow } from "../components/library/LibraryFolderRow";
import { LibraryTrackRow } from "../components/library/LibraryTrackRow";
import { SpotIcon } from "../components/icons/SpotIcon";
import {
  IpcError,
  isTauriRuntime,
  pickLibraryFolders,
} from "../services/ipc";
import {
  useAddLibraryFolders,
  useLibraryPage,
  useLibraryProgress,
  useLibraryStatus,
  useRemoveLibraryFolder,
  useRescanAllLibraryFolders,
  useRescanLibraryFolder,
  useRevealLocalFile,
} from "../hooks/useLibrary";
import { usePlayback } from "../hooks/usePlayback";
import type {
  LibraryFolder,
  LibraryFolderId,
  LibrarySort,
  LibraryTrack,
  Playlist,
  Tag,
  TrackCollectionState,
} from "../types/domain";

const PAGE_SIZE = 50;
const EMPTY_FOLDERS: LibraryFolder[] = [];

function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof IpcError && error.message) {
    return error.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return fallback;
}

function actionError(...errors: unknown[]): string | null {
  const error = errors.find(Boolean);
  return error ? errorMessage(error, "The library action could not be completed.") : null;
}

function hasIpcExport(name: string): boolean {
  return Object.prototype.hasOwnProperty.call(ipc, name);
}

export function LibraryPage() {
  const nativeRuntime = isTauriRuntime();
  const e2ePlaybackPreview = !nativeRuntime && import.meta.env.DEV && import.meta.env.VITE_SPOTDIY_E2E === "1";
  const status = useLibraryStatus();
  const progress = useLibraryProgress();
  const playback = usePlayback();
  const [folderFilter, setFolderFilter] = useState<LibraryFolderId | null>(null);
  const [sort, setSort] = useState<LibrarySort>("title");
  const [descending, setDescending] = useState(false);
  const [pageNumber, setPageNumber] = useState(0);
  const [actionErrorMessage, setActionErrorMessage] = useState<string | null>(null);
  const [collectionStates, setCollectionStates] = useState<Record<string, TrackCollectionState>>({});
  const [collectionPlaylists, setCollectionPlaylists] = useState<Playlist[]>([]);
  const [collectionTags, setCollectionTags] = useState<Tag[]>([]);
  const [collectionPendingTrackId, setCollectionPendingTrackId] = useState<string | null>(null);

  const folders = status.data?.folders ?? EMPTY_FOLDERS;
  const request = {
    page: pageNumber,
    pageSize: PAGE_SIZE,
    sort,
    descending,
    folderId: folderFilter,
  };
  const libraryPage = useLibraryPage(request);
  const addFolders = useAddLibraryFolders();
  const removeFolder = useRemoveLibraryFolder();
  const rescanFolder = useRescanLibraryFolder();
  const rescanAll = useRescanAllLibraryFolders();
  const revealFile = useRevealLocalFile();

  useEffect(() => {
    if (folderFilter && !folders.some((folder) => folder.id === folderFilter)) {
      setFolderFilter(null);
      setPageNumber(0);
    }
  }, [folderFilter, folders]);

  useEffect(() => {
    setPageNumber(0);
  }, [descending, folderFilter, sort]);

  const busy = addFolders.isPending || removeFolder.isPending || rescanFolder.isPending || rescanAll.isPending;
  const scanActive = status.data?.isScanning || progress?.status === "queued" || progress?.status === "scanning";
  const pageData = libraryPage.data;
  const pageHasItems = Boolean(pageData && pageData.items.length > 0);
  const pageHasNoItems = pageData?.total === 0;
  const pageIsEmpty = pageData !== undefined && pageData.total > 0 && pageData.items.length === 0;
  const hasIssues = folders.some((folder) => folder.status === "failed" || Boolean(folder.lastScanError))
    || Boolean(pageData?.items.some((track) => track.indexStatus === "error" || !track.available));
  const visibleActionError = actionErrorMessage ?? actionError(
    addFolders.error,
    removeFolder.error,
    rescanFolder.error,
    rescanAll.error,
    revealFile.error,
  );
  const playbackEnabled = nativeRuntime || e2ePlaybackPreview;
  const playbackErrorMessage = playback.bridgeError ?? playback.snapshot.error?.summary ?? null;

  useEffect(() => {
    const items = pageData?.items ?? [];
    if (!nativeRuntime || items.length === 0) {
      setCollectionStates({});
      setCollectionPlaylists([]);
      setCollectionTags([]);
      return;
    }
    let active = true;
    const trackIds = items.map((track) => track.trackId);
    const stateRequest = hasIpcExport("getTrackCollectionStates")
      ? ipc.getTrackCollectionStates(trackIds)
      : Promise.resolve([] as TrackCollectionState[]);
    const playlistRequest = hasIpcExport("listPlaylists") ? ipc.listPlaylists() : Promise.resolve([] as Playlist[]);
    const tagRequest = hasIpcExport("listTags") ? ipc.listTags() : Promise.resolve([] as Tag[]);
    void Promise.all([stateRequest, playlistRequest, tagRequest])
      .then(([states, playlists, tags]) => {
        if (!active) {
          return;
        }
        setCollectionStates(Object.fromEntries(states.map((state) => [state.trackId, state])));
        setCollectionPlaylists(playlists.filter((playlist) => playlist.kind !== "inbox" && playlist.branchStatus !== "merged"));
        setCollectionTags(tags);
      })
      .catch((collectionError) => {
        if (active) {
          setActionErrorMessage(errorMessage(collectionError, "SpotDIY could not read collection state."));
        }
      });
    return () => {
      active = false;
    };
  }, [nativeRuntime, pageData]);

  const addFolder = async () => {
    if (!nativeRuntime || busy) {
      return;
    }
    setActionErrorMessage(null);
    try {
      const paths = await pickLibraryFolders();
      if (paths.length > 0) {
        await addFolders.mutateAsync(paths);
      }
    } catch (error) {
      setActionErrorMessage(errorMessage(error, "SpotDIY could not add those library folders."));
    }
  };

  const removeLibraryFolder = (folder: LibraryFolder) => {
    const confirmed = window.confirm(
      `Remove ${folder.path} from SpotDIY? Its index will be removed, but your files will remain untouched.`,
    );
    if (!confirmed) {
      return;
    }
    setActionErrorMessage(null);
    removeFolder.mutate(folder.id);
  };

  const rescanOne = (folderId: LibraryFolderId) => {
    setActionErrorMessage(null);
    rescanFolder.mutate(folderId);
  };

  const rescanAllFolders = () => {
    setActionErrorMessage(null);
    rescanAll.mutate();
  };

  const reveal = (sourceId: Parameters<typeof revealFile.mutate>[0]) => {
    setActionErrorMessage(null);
    revealFile.mutate(sourceId);
  };

  const refreshCollectionState = async (track: LibraryTrack) => {
    if (!hasIpcExport("getTrackCollectionStates")) {
      return;
    }
    const states = await ipc.getTrackCollectionStates([track.trackId]);
    const next = states[0];
    if (next) {
      setCollectionStates((current) => ({ ...current, [track.trackId]: next }));
    }
  };

  const runCollectionAction = async (track: LibraryTrack, action: () => Promise<unknown>, fallback: string) => {
    setActionErrorMessage(null);
    setCollectionPendingTrackId(track.trackId);
    try {
      await action();
      await refreshCollectionState(track);
    } catch (collectionError) {
      setActionErrorMessage(errorMessage(collectionError, fallback));
    } finally {
      setCollectionPendingTrackId(null);
    }
  };

  const toggleLike = (track: LibraryTrack) => {
    const liked = collectionStates[track.trackId]?.liked ?? false;
    if (!hasIpcExport("setTrackLiked")) {
      return;
    }
    void runCollectionAction(track, () => ipc.setTrackLiked(track.trackId, !liked), "SpotDIY could not update that like.");
  };

  const updateRating = (track: LibraryTrack, rating: number | null) => {
    if (!hasIpcExport("setTrackRating")) {
      return;
    }
    void runCollectionAction(track, () => ipc.setTrackRating(track.trackId, rating), "SpotDIY could not update that rating.");
  };

  const addToInbox = (track: LibraryTrack) => {
    if (!hasIpcExport("addTrackToInbox")) {
      return;
    }
    void runCollectionAction(track, () => ipc.addTrackToInbox(track.trackId), "SpotDIY could not add that track to the Inbox.");
  };

  const addToPlaylist = (track: LibraryTrack, playlistId: Playlist["id"]) => {
    if (!hasIpcExport("addPlaylistItem")) {
      return;
    }
    void runCollectionAction(track, () => ipc.addPlaylistItem(playlistId, track.trackId, track.sourceId), "SpotDIY could not add that track to the playlist.");
  };

  const tagTrack = (track: LibraryTrack, tag: Tag | null, requestedName?: string) => {
    if (!hasIpcExport("addTrackTag")) {
      return;
    }
    void runCollectionAction(track, async () => {
      let selectedTag = tag;
      if (!selectedTag && requestedName && hasIpcExport("createTag")) {
        selectedTag = await ipc.createTag(requestedName);
        setCollectionTags((current) => [...current, selectedTag!]);
      }
      if (selectedTag) {
        await ipc.addTrackTag(track.trackId, selectedTag.id);
      }
    }, "SpotDIY could not apply that tag.");
  };

  const addFolderButton = (label: string) => (
    <button
      className="button button-primary"
      disabled={!nativeRuntime || busy}
      onClick={() => void addFolder()}
      title={nativeRuntime ? "Choose one or more music folders" : "Folder selection is available in the native SpotDIY app"}
      type="button"
    >
      <SpotIcon name="folder" size={16} />
      {label}
    </button>
  );

  return (
    <div className="page-stack">
      <section className="page-intro">
        <div>
          <span className="eyebrow">LOCAL LIBRARY</span>
          <h1>Your collection, <em>in focus.</em></h1>
          <p>SpotDIY reads your files where they are and keeps the index close to the source.</p>
        </div>
        <div className="page-intro-stat">
          <strong>{status.data?.indexedTrackCount ?? 0}</strong>
          <span>tracks indexed</span>
        </div>
      </section>

      {visibleActionError ? (
        <div className="library-alert library-alert-error" role="alert">
          <SpotIcon name="alert" size={16} />
          <span>{visibleActionError}</span>
        </div>
      ) : null}

      {playbackErrorMessage && pageHasItems ? (
        <div className="library-alert library-alert-warning" role="status">
          <SpotIcon name="alert" size={16} />
          <span>{playbackErrorMessage}</span>
        </div>
      ) : null}

      {status.isLoading ? (
        <EmptyState icon="library" eyebrow="LOCAL LIBRARY" title="Loading your library" description="Reading folder and index status from the local database…" />
      ) : status.isError ? (
        <EmptyState
          icon="alert"
          eyebrow="LIBRARY UNAVAILABLE"
          title="Could not read the local library"
          description={errorMessage(status.error, "The native library service did not return a valid status.")}
          action={<button className="button button-primary" onClick={() => void status.refetch()} type="button">Try again</button>}
        />
      ) : folders.length === 0 ? (
        <EmptyState
          icon="folder"
          eyebrow="NO MUSIC FOLDERS"
          title="No music folders connected"
          description={nativeRuntime
            ? "Add one or more folders and SpotDIY will scan them recursively for supported audio files."
            : "Browser preview cannot access your music folders. Open the native SpotDIY app to choose local folders."}
          action={addFolderButton("Add folder")}
        />
      ) : (
        <>
          <section aria-labelledby="music-folders-heading" className="folder-list">
            <div className="section-heading">
              <div>
                <span className="eyebrow">MUSIC FOLDERS</span>
                <h2 id="music-folders-heading">Connected locations</h2>
              </div>
              <div className="library-heading-actions">
                <button
                  className="button button-quiet"
                  disabled={!nativeRuntime || busy}
                  onClick={rescanAllFolders}
                  title="Scan every connected folder"
                  type="button"
                >
                  <SpotIcon name="refresh" size={15} />
                  Rescan all
                </button>
                {addFolderButton("Add folder")}
              </div>
            </div>
            <div className="library-folder-list">
              {folders.map((folder) => (
                <LibraryFolderRow
                  actionPending={busy}
                  folder={folder}
                  key={folder.id}
                  onRemove={removeLibraryFolder}
                  onRescan={rescanOne}
                  progress={progress}
                />
              ))}
            </div>
          </section>

          {scanActive ? (
            <section aria-live="polite" className="library-scan-banner" role="status">
              <div className="library-scan-icon"><SpotIcon name="spark" size={17} /></div>
              <div>
                <strong>Indexing local files</strong>
                <p>{progress?.currentFile ? `Reading ${progress.currentFile}` : "Preparing a recursive scan…"}</p>
              </div>
              <span>{progress && progress.candidates > 0 ? `${progress.processed} / ${progress.candidates}` : "Working"}</span>
            </section>
          ) : null}

          {hasIssues ? (
            <div className="library-alert library-alert-warning" role="status">
              <SpotIcon name="alert" size={16} />
              <span>Some files or folders need attention. They remain visible with their measured status and error details.</span>
            </div>
          ) : null}

          <section aria-labelledby="indexed-tracks-heading" className="library-track-section">
            <div className="section-heading library-track-heading">
              <div>
                <span className="eyebrow">INDEXED TRACKS</span>
                <h2 id="indexed-tracks-heading">Your local collection</h2>
              </div>
              <span className="section-note">
                {nativeRuntime
                  ? `${status.data?.availableTrackCount ?? 0} available to play now`
                  : e2ePlaybackPreview
                    ? "Synthetic playback fixtures are active for browser E2E coverage"
                    : "Playback remains available in the native SpotDIY app"}
              </span>
            </div>
            <div className="library-controls" aria-label="Library controls">
              <label>
                <span>Folder</span>
                <select
                  aria-label="Filter library folder"
                  onChange={(event) => setFolderFilter(event.target.value ? event.target.value as LibraryFolderId : null)}
                  value={folderFilter ?? ""}
                >
                  <option value="">All folders</option>
                  {folders.map((folder) => <option key={folder.id} value={folder.id}>{folder.path}</option>)}
                </select>
              </label>
              <label>
                <span>Sort by</span>
                <select aria-label="Sort library tracks" onChange={(event) => setSort(event.target.value as LibrarySort)} value={sort}>
                  <option value="title">Title</option>
                  <option value="artist">Artist</option>
                  <option value="dateAdded">Date added</option>
                  <option value="dateModified">Date modified</option>
                </select>
              </label>
              <button
                aria-pressed={descending}
                className="button button-quiet button-small library-sort-direction"
                onClick={() => setDescending((value) => !value)}
                type="button"
              >
                {descending ? "Descending" : "Ascending"}
              </button>
            </div>

            {libraryPage.isLoading && !pageData ? (
              <div className="library-pending-state" role="status"><SpotIcon name="spark" size={18} /> Loading indexed tracks…</div>
            ) : libraryPage.isError ? (
              <div className="library-alert library-alert-error" role="alert">
                <SpotIcon name="alert" size={16} />
                <span>{errorMessage(libraryPage.error, "Could not read the indexed tracks.")}</span>
              </div>
            ) : pageHasItems ? (
              <>
                <div className="library-track-list">
                  {pageData?.items.map((track) => (
                    <LibraryTrackRow
                      current={playback.snapshot.currentTrackId === track.trackId}
                      key={track.sourceId}
                      onAddToQueue={(row) => { void playback.addToQueue(row.trackId, row.sourceId); }}
                      onPlayNext={(row) => { void playback.playNext(row.trackId, row.sourceId); }}
                      onPlayNow={(row) => { void playback.playNow(row.trackId, row.sourceId); }}
                      onReveal={reveal}
                      playbackEnabled={playbackEnabled}
                      playbackPending={playback.pending}
                      revealPending={revealFile.isPending}
                      track={track}
                      collectionPending={collectionPendingTrackId === track.trackId}
                      collectionPlaylists={collectionPlaylists}
                      collectionState={collectionStates[track.trackId]}
                      collectionTags={collectionTags}
                      onInbox={addToInbox}
                      onLike={toggleLike}
                      onPlaylist={addToPlaylist}
                      onRating={updateRating}
                      onTag={tagTrack}
                    />
                  ))}
                </div>
                {libraryPage.isFetching ? <span className="library-refreshing" role="status">Updating library results…</span> : null}
              </>
            ) : pageIsEmpty ? (
              <EmptyState icon="library" eyebrow="EMPTY PAGE" title="This library page is empty" description="The collection changed while this page was open. Go back one page or refresh the library." action={<button className="button button-quiet" disabled={pageNumber === 0} onClick={() => setPageNumber((value) => Math.max(0, value - 1))} type="button">Previous page</button>} />
            ) : pageHasNoItems && scanActive ? (
              <EmptyState icon="spark" eyebrow="SCAN IN PROGRESS" title="Your tracks are being indexed" description="SpotDIY will keep the folder status and scan progress visible while it reads supported files." />
            ) : (
              <EmptyState icon="library" eyebrow="NO SUPPORTED TRACKS" title="No supported tracks found" description="The connected folders are ready, but no indexed MP3, FLAC, M4A, AAC, OGG, OPUS, or WAV files are available yet." />
            )}

            {pageData && pageData.total > 0 ? (
              <div className="library-pagination">
                <span>Showing {pageData.items.length === 0 ? 0 : pageNumber * PAGE_SIZE + 1}–{Math.min((pageNumber * PAGE_SIZE) + pageData.items.length, pageData.total)} of {pageData.total}</span>
                <div>
                  <button className="button button-quiet button-small" disabled={pageNumber === 0 || libraryPage.isFetching} onClick={() => setPageNumber((value) => Math.max(0, value - 1))} type="button">Previous</button>
                  <button className="button button-quiet button-small" disabled={!pageData.hasNext || libraryPage.isFetching} onClick={() => setPageNumber((value) => value + 1)} type="button">Next</button>
                </div>
              </div>
            ) : null}
          </section>
        </>
      )}

      <section className="library-principles">
        <div><span className="eyebrow">INDEXING PRINCIPLES</span><h2>Local by default.</h2></div>
        <div className="principle-list">
          <div><span>01</span><strong>Incremental scans</strong><p>Only changed files need another look.</p></div>
          <div><span>02</span><strong>Quality stays honest</strong><p>Codec, bitrate, sample rate, and provenance stay visible.</p></div>
          <div><span>03</span><strong>Files stay yours</strong><p>SpotDIY keeps user music at the paths you choose.</p></div>
        </div>
      </section>
    </div>
  );
}
