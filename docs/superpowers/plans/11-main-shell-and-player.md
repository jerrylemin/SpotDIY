# Plan 11 — main shell and player

## Goal

Connect real library/search/playback/queue data to Home, Search, Library, Playlists, Downloads, Settings, Now Playing, Mini Player, and Track Inspector surfaces.

## Dependencies

Plans 02–10 as relevant; stable UI contracts.

## Exact files

`src/app/**`, `src/routes.tsx`, `src/pages/**`, `src/components/shell/**`, `src/components/player/**`, `src/components/search/**`, `src/components/library/**`, `src/components/inspector/**`, and frontend interaction tests.

## Interfaces consumed

All service IPC DTOs and TanStack Query/Zustand boundaries.

## Interfaces produced

Real route loaders, partial search rendering, source switcher, quality/provenance display, queue controls, context menu actions, and responsive player modes.

## Tests

Route navigation, loading/error/partial states, transport interactions, context action capability rules, command palette, and accessibility.

## Acceptance criteria

Production UI contains no static provider result fixtures and communicates why unavailable actions are unavailable.

## Commit boundary

`feat: connect SpotDIY shell and main player`

## Delivery evidence

- Completed through `15031bf` with migration compatibility in `e5129a0`, real
  Track Inspector/Home data in `f5562e1`, player surfaces in `0012a43`, shell
  actions in `0026146`, interaction coverage in `dba1f24`, and follow-up
  coverage/strict-gate fixes through `d631a2a`, `d2199d5`, `e072fec`, and
  `15031bf`.
- Migration 7 restores the historical migration-1 settings allowlist and
  rebuilds/copies `settings_metadata` so old schema-6, Plan-10-shaped schema-6,
  and fresh databases reach schema 7 without value loss or foreign-key drift.
- Delivered real-data Home, persisted/ephemeral Track Inspectors,
  source/quality/provenance surfaces, capability-aware actions, command-palette
  and Escape coordination, and Standard/Mini/Expanded in-shell players with
  existing service ownership unchanged.
- Verification evidence: 347 Rust unit tests plus real-mpv smoke, 73 Vitest
  tests, 63 Playwright tests across 1280/1920/2560, strict quality gates,
  Tauri packaging, packaged Plan 09 persistence smoke, and packaged Plan 11
  migration/shell/restart smoke. Online playback, Spotify download, native
  overlays, and Plan 12 remain out of scope.
