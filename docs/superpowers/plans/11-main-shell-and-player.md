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
