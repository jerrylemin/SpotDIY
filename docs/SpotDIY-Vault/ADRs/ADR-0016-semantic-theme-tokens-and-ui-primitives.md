# ADR-0016 Semantic theme tokens and accessible UI primitives

Status: Accepted
Date: 2026-09-02

## Context

Plan 10 needs a coherent visual foundation that can scale across the existing
Tauri/React shell, support user-selectable appearance, and keep accessibility
behavior consistent. Theme changes must be durable through the existing typed
settings boundary, while custom theme input must not become an unchecked CSS or
runtime injection path.

## Decision

Use semantic CSS variables as the UI contract and resolve them through a
frontend `ThemeController`. The controller applies root `data-theme` and
`data-layout` attributes for Dark, Light, System, or validated Custom themes
and Comfortable, Compact, or Dense layout profiles. System mode follows the
browser/OS media-query signal and invalid custom data falls back to dark with a
recoverable error.

Custom themes use schema version 1, a trimmed 1-to-80 Unicode-scalar name,
exactly 15 semantic color tokens, strict `#RRGGBB` values, a 64 KiB limit, and
WCAG contrast checks. The same contract is validated by frontend Zod and Rust.
Theme and layout values are ordinary settings keys in schema 6. No migration 7
is added by Plan 10.

Build shared Button, IconButton, Surface, StatusChip, Field, SegmentedControl,
Tooltip, EmptyState, ProviderBadge, and ContextActionMenu primitives. Context
actions are supplied by callers and support pointer, More, ContextMenu, and
Shift+F10 entry points with keyboard navigation, focus restoration, and
disabled explanations. Add `MotionConfig reducedMotion="user"` and a CSS
reduced-motion rule. Keep InspectorPanel/IconGallery as a development/design
surface without permanent navigation.

## Consequences

New surfaces consume stable semantic variables and primitives instead of
inventing local colors, spacing, or interaction semantics. Settings can import,
export, reset, and report custom themes without bypassing the persistence
boundary. The browser adapter remains an in-memory preview seam; native
packaged settings use `SettingsRepository`. Existing Track Inspector,
Theme Studio, mobile UI, and Plan 11 main-player refinement remain out of
scope.

## Verification

The implementation is covered by theme-schema/domain/design-system tests and a
three-project Playwright matrix at 1280, 1920, and 2560 pixels. The packaged
settings smoke proves default values, all theme/layout writes, custom-theme
persistence across restart, and reset behavior. Final frontend, Rust, Tauri
packaging, and clean-shutdown gates pass.
