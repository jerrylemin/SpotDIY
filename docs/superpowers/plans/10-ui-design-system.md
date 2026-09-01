# Plan 10 — UI design system

## Goal

Expand the bootstrap visual system into reusable accessible primitives, icon gallery, dynamic theme tokens, layout profiles, and interaction patterns.

## Dependencies

Plan 01 and approved UI design direction.

## Exact files

`src/styles/**`, `src/components/common/**`, `src/components/icons/**`, `src/components/inspector/**`, `src/features/theme/**`, `src/features/layout/**`, and visual tests/screens.

## Interfaces consumed

Frontend domain/provider capabilities and user settings.

## Interfaces produced

Tokens, provider badges, custom icon gallery, theme import/export validation, profile persistence, focus/reduced-motion behavior, and context-action visibility.

## Tests

Keyboard focus, labels, contrast snapshots, reduced motion, theme schema validation, responsive layout, and icon rendering.

## Acceptance criteria

Primary surfaces feel coherent at approved viewports; primary identity does not depend on external SVG/CDN icons.

## Commit boundary

Implementation is split into focused commits: `cc28ba1`, `f2a5995`,
`850bc82`, `8c62aed`, and `6eb231d`. Documentation closure is committed
separately after the final verification gates.

## Delivery evidence (2026-09-02)

Status: COMPLETE.

- Added semantic tokens, the schema-version-1 15-token custom-theme contract,
  Dark/Light/System/Custom resolution, persistent
  Comfortable/Compact/Dense layout profiles, reduced motion, accessible shared
  primitives, keyboard context actions, custom icons, and
  InspectorPanel/IconGallery foundations.
- Added Settings APPEARANCE controls for theme/layout selection,
  custom-theme import/export/reset/status/errors and representative
  `LibraryTrackRow` context actions. No migration 7 was added; storage remains
  schema 6 with ordinary `layout_profile` and `custom_theme` settings keys.
- `pnpm typecheck`, `pnpm lint`, `pnpm test` (70 tests), `pnpm build`, the
  three-project Playwright matrix (51 tests), Rust fmt/Clippy/tests (343 unit
  tests plus one synthetic mpv integration test), Tauri packaging, and
  `git diff --check` pass. Packaged settings persistence/reset and packaged
  Plan 09 playback/lyrics smoke pass with clean shutdown.
- CodeGraph and Graphify were each refreshed once. CodeGraph reports 138 files,
  4,655 nodes, and 17,004 edges; Graphify reports 4,023 nodes, 7,913 edges,
  and 245 communities. Screenshots and build artifacts remain outside the
  repository; `src-tauri\target` remains absent.

Plan 11 main-player refinement and full Track Inspector work are intentionally
deferred pending external ChatGPT GitHub review.
