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

`feat: add SpotDIY design system and theme foundations`
