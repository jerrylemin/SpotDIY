# ADR-0008: Local library identity and reconciliation

- Status: Accepted
- Date: 2026-08-30
- Scope: Plan 03 Local Library

## Context

SpotDIY needs to index user-selected Windows folders without treating a path
as a durable musical identity. Files can be renamed, temporarily unavailable,
restored from removable storage, or changed in place. Filesystem watcher events
can also be incomplete or ambiguous. The index must remain useful without
modifying user media or guessing when evidence is insufficient.

## Decision

`library_folders` owns canonical, case-insensitive root paths and one Notify
watcher is registered for each enabled root. Local `TrackId`, `SourceId`, and
provider item identities are opaque UUID-backed values; migration 2 rewrites
path-shaped Plan 02 local provider identities. A fingerprint is evidence for
change and rename matching, not a uniqueness key, so identical files at
different paths remain separate entries.

Ordinary scans use file size and modification time to skip unchanged available
files. Create, modify, and reliable rename notifications force a scan; uncertain
events request reconciliation, and watcher/channel failures re-register the
watcher before forcing a recovery scan. A complete scan marks only confirmed
missing files unavailable, promotes a single missing fingerprint candidate for
an unambiguous rename, and creates a new identity for ambiguous matches.

Reveal is source-ID based and revalidates enabled-folder ownership, canonical
path equality, and file existence before calling the scoped opener. Metadata,
fingerprints, and artwork extraction happen outside short SQLite write
transactions; aggregate writes are atomic. Partial scan failures remain in the
folder status while valid files continue indexing.

## Consequences

Renames and removable-drive restoration do not churn identities, while
ambiguous or unsafe cases remain visible instead of being silently guessed.
Legacy rows are preserved and can be promoted when a selected folder discovers
the same path. The design requires a later playback plan to decide how an
available local source is actually opened; Plan 03 exposes no playback action.
