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
