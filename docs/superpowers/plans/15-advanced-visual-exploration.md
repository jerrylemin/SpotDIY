# Plan 15 — advanced visual exploration

## Goal

Implement Music Map, Library Galaxy, radial context menu, drag actions, hover/audio preview, dynamic theme, Theme Studio, and layout profiles with real data.

## Dependencies

Plans 08, 10, 11, and 14; performance budget.

## Exact files

`src/features/music-map/**`, `src/features/library-galaxy/**`, `src/components/radial-menu/**`, `src/features/preview/**`, `src/features/theme/**`, `src/features/layout/**`, and visual/interaction tests.

## Interfaces consumed

Library graph, listening analytics, track capabilities, theme schema, layout profile, and playback preview.

## Interfaces produced

Interactive visual routes with keyboard/context-menu fallbacks and bounded rendering work.

## Tests

Large-library rendering, keyboard alternatives, reduced motion, preview cancellation, theme import/export, layout persistence, and no-hover-only actions.

## Acceptance criteria

Every advanced surface has a real route, data contract, empty/error state, and relevant test; no placeholder card satisfies the feature.

## Commit boundary

`feat: add advanced visual exploration workspaces`

## Delivery record — 2026-09-03

Implemented through `1403955` (Plan-14 transition repair), `d73e755`,
`e442767`, `977176e`, `aee1dad`, and `2612804`. The implementation keeps
schema 9, adds no migration 10, and uses one bounded native visual dataset;
deterministic SVG/Canvas surfaces; capability-aware radial/drag actions; a
separate eight-second local preview; and Theme Studio with 15 validated
tokens, session preview, persistent activation, import/export, clone, dynamic
accent, and layout profiles.

Verification passed: 430 Rust unit tests plus real mpv, 88 Vitest tests, 70
Playwright tests, frontend/native quality gates, Tauri release/NSIS packaging,
and `scripts/packaged-playback-smoke.ps1 -Plan15VisualExploration`. The packaged
smoke verified the native dataset, real local preview, all three visual routes,
restart isolation, and owned-process cleanup.
