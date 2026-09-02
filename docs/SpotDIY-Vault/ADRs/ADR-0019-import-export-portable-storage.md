# ADR-0019: deterministic import/export and portable storage

- Status: Accepted
- Date: 2026-09-02
- Plan: 13

## Context

SpotDIY needs a recoverable local backup format and a portable installation
mode without weakening the local-first database, library ownership, or secret
boundaries. SQLite may be open with WAL files, media paths may be outside the
application data root, and import archives are untrusted input. Startup must
not choose a database from a persisted mode value that disagrees with the
actual filesystem selector.

## Decision

Use the exact file `SpotDIY.portable` beside the running executable as the
authoritative startup selector. Without the marker, Standard mode uses the
platform local-data root. With the marker, Portable mode uses executable-
relative `Data`, `Music`, `Covers`, `Lyrics`, `Database`, `Cache`, and `Config`
roots. Required directories and the database path are validated without
following symlinks or reparse points; Portable startup fails explicitly rather
than falling back to AppData. The SQLite `storage_mode` setting remains an
ordinary persisted mirror updated after the selected database opens.

Use SQLite's online backup API for all live database snapshots. The
`.spotdiy` format is version 1 and contains stable JSON `manifest.json`, its
exact newline-terminated SHA-256 `manifest.sha256`,
`database/spotdiy.sqlite3`, and optional `media/`, `covers/`, and `lyrics/`
payloads. Archive paths are slash-separated and lexicographically ordered;
timestamps, permissions, compression settings, and JSON serialization are
fixed so equivalent exports have identical bytes. The manifest declares each
payload's kind, size, and SHA-256 digest plus source-ID media mappings.

Export includes only persisted library state, explicitly selected local audio
under a trusted configured folder, exact same-stem `.lrc` sidecars for included
audio, and files from the trusted artwork cache. It excludes credentials,
tokens, raw provider payloads, download/cache temp files, live SQLite WAL/SHM
files, arbitrary paths, and symlinks/reparse points.

Import is two phase. The archive is inspected and streamed into a private
staging root; names, compression, bounds, duplicates, case collisions,
manifest checksum, declared payloads, payload hashes, schema version, SQLite
integrity, foreign keys, and active storage mode are checked before preview.
Commit writes a pending descriptor and requires restart. Startup applies the
descriptor before normal database open, retaining one active rollback snapshot,
an online copy of the staged original database, and an exact list of created
media paths. Apply failure or crash restores the active database and removes
only paths created by the import; pre-existing files remain untouched.

Standard-mode audio restoration uses a native folder picker. Portable-mode
audio is restored below executable-relative `Music`; artwork is restored only
inside the active trusted cache. The frontend receives strict DTOs and never
supplies arbitrary filesystem paths to the archive service.

## Consequences

Backups are reproducible and inspectable, startup mode is deterministic, and
failed or interrupted imports have a recoverable boundary. Mode switches need
a restart and preserve the previous database until the target is validated.
The native dialog boundary is not browser-automatable, so packaged smoke
covers storage selection/restart while native tests cover archive and restore
behavior.

## Verification

Plan 13 verification covers deterministic archive bytes, manifest/hash
exactness, secure ZIP rejection, online WAL snapshots, staged import isolation,
schema/integrity/foreign-key validation, missing references, media/artwork
restore, crash recovery, database rollback, exact marker/layout selection, and
both mode-switch directions. The final packaged storage smoke proves
Standard -> Portable -> Standard restart behavior in an isolated release copy.
