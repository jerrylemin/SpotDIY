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
  useDeleteLocalFile,
  useLibraryPage,
  useLibraryProgress,
  useLibraryStatus,
  useRemoveLibraryFolder,
  useRenameLocalFile,
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
} from "../types/domain";

const PAGE_SIZE = 50;
const EMPTY_FOLDERS: LibraryFolder[] = [];
const PINNED_LIBRARY_TRACKS_KEY = "spotdiy.library.pinnedTracks";

function readPinnedTracks(): Set<string> {
  try {
    const value = JSON.parse(localStorage.getItem(PINNED_LIBRARY_TRACKS_KEY) ?? "[]") as unknown;
    return new Set(Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []);
  } catch {
    return new Set();
  }
}

function writePinnedTracks(value: Set<string>): void {
  try {
    localStorage.setItem(PINNED_LIBRARY_TRACKS_KEY, JSON.stringify([...value]));
  } catch {
    // Pinning is still useful for the current session when storage is unavailable.
  }
}

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
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [playlistPendingTrackId, setPlaylistPendingTrackId] = useState<string | null>(null);
  const [pinnedTrackIds, setPinnedTrackIds] = useState<Set<string>>(readPinnedTracks);

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
  const renameFile = useRenameLocalFile();
  const deleteFile = useDeleteLocalFile();

  useEffect(() => {
    if (folderFilter && !folders.some((folder) => folder.id === folderFilter)) {
      setFolderFilter(null);
      setPageNumber(0);
    }
  }, [folderFilter, folders]);

  useEffect(() => {
    setPageNumber(0);
  }, [descending, folderFilter, sort]);

  const busy = addFolders.isPending || removeFolder.isPending || rescanFolder.isPending || rescanAll.isPending || renameFile.isPending || deleteFile.isPending;
  const scanActive = status.data?.isScanning || progress?.status === "queued" || progress?.status === "scanning";
  const pageData = libraryPage.data;
  const pageItems = pageData
    ? pageData.items
      .filter((track) => track.indexStatus !== "missing")
      .sort((left, right) => Number(pinnedTrackIds.has(right.trackId)) - Number(pinnedTrackIds.has(left.trackId)))
    : [];
  const pageHasItems = pageItems.length > 0;
  const pageHasNoItems = pageData?.total === 0;
  const pageIsEmpty = pageData !== undefined && pageData.total > 0 && pageItems.length === 0;
  const hasIssues = folders.some((folder) => folder.status === "failed" || Boolean(folder.lastScanError))
    || Boolean(pageData?.items.some((track) => track.indexStatus === "error"));
  const visibleActionError = actionErrorMessage ?? actionError(
    addFolders.error,
    removeFolder.error,
    rescanFolder.error,
    rescanAll.error,
    revealFile.error,
    renameFile.error,
    deleteFile.error,
  );
  const playbackEnabled = nativeRuntime || e2ePlaybackPreview;
  const playbackErrorMessage = playback.bridgeError ?? playback.snapshot.error?.summary ?? null;

  useEffect(() => {
    if (!nativeRuntime) {
      setPlaylists([]);
      return;
    }
    let active = true;
    const playlistRequest = hasIpcExport("listPlaylists") ? ipc.listPlaylists() : Promise.resolve([] as Playlist[]);
    void playlistRequest
      .then((nextPlaylists) => {
        if (!active) {
          return;
        }
        setPlaylists(nextPlaylists.filter((playlist) => playlist.kind === "normal"));
      })
      .catch((playlistError) => {
        if (active) {
          setActionErrorMessage(errorMessage(playlistError, "SpotDIY could not read playlists."));
        }
      });
    return () => {
      active = false;
    };
  }, [nativeRuntime, pageData?.items.length]);

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

  const rename = (track: LibraryTrack, name: string) => {
    setActionErrorMessage(null);
    renameFile.mutate({ sourceId: track.sourceId, name });
  };

  const deleteTrack = (track: LibraryTrack) => {
    if (!nativeRuntime || deleteFile.isPending) {
      return;
    }
    const confirmed = window.confirm(`Delete “${track.title}” from its local folder? This permanently deletes the file.`);
    if (!confirmed) {
      return;
    }
    setActionErrorMessage(null);
    deleteFile.mutate(track.sourceId);
  };

  const togglePin = (track: LibraryTrack) => {
    setPinnedTrackIds((current) => {
      const next = new Set(current);
      if (next.has(track.trackId)) next.delete(track.trackId);
      else next.add(track.trackId);
      writePinnedTracks(next);
      return next;
    });
  };

  const addToPlaylist = (track: LibraryTrack, playlistId: Playlist["id"]) => {
    if (!hasIpcExport("addPlaylistItem")) {
      return;
    }
    setActionErrorMessage(null);
    setPlaylistPendingTrackId(track.trackId);
    void ipc.addPlaylistItem(playlistId, track.trackId, track.sourceId)
      .catch((playlistError) => setActionErrorMessage(errorMessage(playlistError, "SpotDIY could not add that track to the playlist.")))
      .finally(() => setPlaylistPendingTrackId(null));
  };

  const addFolderButton = (label: string) => (
    <button
      aria-label={label}
      className="button button-primary icon-only-button"
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
          action={<button aria-label="Try again" className="button button-primary icon-only-button" onClick={() => void status.refetch()} title="Try again" type="button"><SpotIcon name="refresh" size={15} /></button>}
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
                  aria-label="Rescan all"
                  className="button button-quiet icon-only-button"
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
              <span>Some files or folders need attention. Their measured status and error details are shown here.</span>
            </div>
          ) : null}

          <section aria-labelledby="indexed-tracks-heading" className="library-track-section">
            <div className="section-heading library-track-heading">
              <div>
                <span className="eyebrow">INDEXED TRACKS</span>
                <h2 id="indexed-tracks-heading">Your local library</h2>
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
                aria-label={descending ? "Descending" : "Ascending"}
                className="button button-quiet button-small icon-only-button library-sort-direction"
                onClick={() => setDescending((value) => !value)}
                title={descending ? "Sort ascending" : "Sort descending"}
                type="button"
              >
                <SpotIcon name="arrow" size={14} />
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
                  {pageItems.map((track) => (
                    <LibraryTrackRow
                      current={playback.snapshot.currentTrackId === track.trackId}
                      deletePending={deleteFile.isPending}
                      key={track.sourceId}
                      onAddToQueue={(row) => { void playback.addToQueue(row.trackId, row.sourceId); }}
                      onDelete={deleteTrack}
                      onPlayNext={(row) => { void playback.playNext(row.trackId, row.sourceId); }}
                      onPlayNow={(row) => { void playback.playNow(row.trackId, row.sourceId); }}
                      onReveal={reveal}
                      onRename={rename}
                      playbackEnabled={playbackEnabled}
                      playbackPending={playback.pending}
                      revealPending={revealFile.isPending}
                      track={track}
                      pinned={pinnedTrackIds.has(track.trackId)}
                      playlists={playlists}
                      playlistPending={playlistPendingTrackId === track.trackId}
                      onPin={togglePin}
                      onPlaylist={addToPlaylist}
                    />
                  ))}
                </div>
                {libraryPage.isFetching ? <span className="library-refreshing" role="status">Updating library results…</span> : null}
              </>
            ) : pageIsEmpty ? (
              <EmptyState icon="library" eyebrow="EMPTY PAGE" title="This library page is empty" description="The library changed while this page was open. Go back one page or refresh the library." action={<button aria-label="Previous page" className="button button-quiet icon-only-button" disabled={pageNumber === 0} onClick={() => setPageNumber((value) => Math.max(0, value - 1))} title="Previous page" type="button"><SpotIcon name="previous" size={14} /></button>} />
            ) : pageHasNoItems && scanActive ? (
              <EmptyState icon="spark" eyebrow="SCAN IN PROGRESS" title="Your tracks are being indexed" description="SpotDIY will keep the folder status and scan progress visible while it reads supported files." />
            ) : (
              <EmptyState icon="library" eyebrow="NO SUPPORTED TRACKS" title="No supported tracks found" description="The connected folders are ready, but no indexed MP3, FLAC, M4A, AAC, OGG, OPUS, or WAV files are available yet." />
            )}

            {pageData && pageData.total > 0 ? (
              <div className="library-pagination">
                <span>Showing {pageItems.length === 0 ? 0 : pageNumber * PAGE_SIZE + 1}–{Math.min((pageNumber * PAGE_SIZE) + pageItems.length, pageData.total)} of {pageData.total}</span>
                <div>
                  <button aria-label="Previous" className="button button-quiet button-small icon-only-button" disabled={pageNumber === 0 || libraryPage.isFetching} onClick={() => setPageNumber((value) => Math.max(0, value - 1))} title="Previous page" type="button"><SpotIcon name="previous" size={14} /></button>
                  <button aria-label="Next" className="button button-quiet button-small icon-only-button" disabled={!pageData.hasNext || libraryPage.isFetching} onClick={() => setPageNumber((value) => value + 1)} title="Next page" type="button"><SpotIcon name="next" size={14} /></button>
                </div>
              </div>
            ) : null}
          </section>
        </>
      )}

    </div>
  );
}
