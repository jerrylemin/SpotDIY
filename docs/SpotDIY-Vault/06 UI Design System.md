# UI design system

The visual direction is premium minimal with power-user depth: dark ink surfaces, deliberate whitespace, strong typography hierarchy, selective glass/blur, lime as the primary SpotDIY accent, and restrained provider colors. The icon language is custom rounded geometry with a shared waveform notch motif.

Accessibility requirements include visible focus, labels for icon-only controls, contrast validation, reduced motion, keyboard context-menu fallback, and no critical state communicated by color alone.

## Plan 03 Library patterns

The Library page uses the same dark-ink/lime visual language for folder rows,
scan banners, status chips, quality facts, and alert states. It renders only
backend-provided paged records. Folder rows show path, file/index counts,
last-scan time, queued/scanning/failed state, progress, and recovery actions.
Track rows show ordered artists, optional album, measured quality, Local
provenance, original artwork or a safe placeholder, unavailable/error detail,
and a disabled playback affordance whose Plan 04 explanation is visible.

Empty, loading, scanning, no-supported-files, partial-error, unavailable-row,
and stale/empty-page states are explicit. Add/remove/rescan actions are disabled
in browser preview, and removal confirmation states that music files remain
untouched. Artwork conversion uses only the backend-provided app-cache path;
the selected music roots are never exposed through the asset protocol.

## Plan 10 design-system foundation

Plan 10 establishes semantic tokens in `src/styles/tokens.css`, shared
foundations/primitives styles, and a `ThemeController` that resolves Dark,
Light, System, and Custom modes. The document root carries
`data-theme="dark|light|custom"` and `data-layout="comfortable|compact|dense"`;
System follows `matchMedia`, and invalid custom data falls back to dark while
surfacing a recoverable Settings error. Reduced motion is user-controlled and
also enforced by the CSS motion rule.

The custom theme format is schema version 1 with a trimmed 1-to-80
Unicode-scalar name, exactly 15 semantic color tokens, strict `#RRGGBB` values,
a 64 KiB maximum, and WCAG contrast checks. Dark and Light presets are built in.
`layout_profile` and `custom_theme` persist as ordinary settings in schema 6;
Plan 10 intentionally adds no migration 7.

The reusable component contract includes Button, IconButton, Surface,
StatusChip, Field, SegmentedControl, Tooltip, EmptyState, ProviderBadge, and
ContextActionMenu. ContextActionMenu supports right-click, More, ContextMenu,
and Shift+F10 entry, keyboard navigation, disabled reasons, and focus
restoration. InspectorPanel and IconGallery are development/design surfaces;
LibraryTrackRow is the representative existing-surface adoption. No permanent
navigation or full Plan 11 Track Inspector is introduced.
