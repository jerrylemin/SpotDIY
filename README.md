# SpotDIY

SpotDIY is a local-first Windows music operating environment. It combines a
managed local library with metadata search across Local, YouTube, SoundCloud,
and Spotify catalog sources, while keeping Spotify metadata-only and user data
on the machine.

## Implemented product

- Local indexing, metadata/artwork, playback through an owned external mpv
  process, source fusion, resolver policy, downloads, and provider search.
- Durable playlists, Inbox, likes, ratings, tags, persistent queue, lyrics,
  bookmarks, A/B loop controls, history, analytics, smart playlists, and
  deterministic Smart Shuffle.
- Windows tray, global shortcuts, SMTC, overlays, output profiles, backup and
  Standard/Portable storage modes.
- Music Map, Library Galaxy, local preview, Theme Studio, dynamic accent, and
  persisted layout profiles.

## Boundaries

SpotDIY has no account creation, mandatory cloud database, or application
telemetry. Provider calls are made only when a selected source needs them.
Spotify is catalog metadata only; SpotDIY does not provide Spotify audio or
circumvent its protection. Visual exploration is local/read-only, and preview
is limited to indexed local audio without queue, history, or analytics writes.

## Run locally

Exact Windows prerequisites and verification commands are in
[`setup_and_run.md`](setup_and_run.md).

```powershell
pnpm install --frozen-lockfile
pnpm dev       # browser preview
pnpm tauri dev # native Tauri window
```

## Release-candidate status

The Plan 16 release candidate is currently `PARTIAL`: the exact pinned CI
native/frontend/package jobs, NSIS artifact, clean install/uninstall, and
packaged feature smokes pass. The broad packaged process-tree performance
sample exceeds the requested playback budget, and the local host still lacks
MSVC headers needed for a native rebuild. Evidence is in
[`docs/SpotDIY-Vault/Sessions/final-verification.md`](docs/SpotDIY-Vault/Sessions/final-verification.md).

## Documentation

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — system boundaries and ownership.
- [`feature_progress.md`](feature_progress.md) — delivery status.
- [`setup_and_run.md`](setup_and_run.md) — Windows setup and release commands.
- [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) — dependency/license index.
- [`docs/superpowers/specs/2026-08-30-spotdiy-design.md`](docs/superpowers/specs/2026-08-30-spotdiy-design.md) — approved design.
- [`docs/SpotDIY-Vault/`](docs/SpotDIY-Vault/) — project knowledge vault.
- [`docs/execution/`](docs/execution/) — milestone, integration, and verification ledger.
