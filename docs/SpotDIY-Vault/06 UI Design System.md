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
`layout_profile` and `custom_theme` persist as ordinary settings. Plan 11
adds migration 7 compatibility for shipped schema-6 databases while keeping
the Plan 10 settings contract unchanged.

The reusable component contract includes Button, IconButton, Surface,
StatusChip, Field, SegmentedControl, Tooltip, EmptyState, ProviderBadge, and
ContextActionMenu. ContextActionMenu supports right-click, More, ContextMenu,
and Shift+F10 entry, keyboard navigation, disabled reasons, and focus
restoration. InspectorPanel and IconGallery are development/design surfaces;
LibraryTrackRow is the representative Plan 10 adoption. Plan 10 itself did not
introduce permanent navigation or the full Track Inspector.

## Plan 11 shell surfaces

`AppShell` composes the real-data Home dashboard, route content, queue drawer,
command palette, inspectors, and the three in-shell player modes. Standard,
Mini, and Expanded Now Playing all use semantic tokens and the same
`usePlayback()` snapshot. Expanded Now Playing adds source switching and
compact measured quality/provenance facts; Mini remains a compact in-shell
footer rather than a native window.

The persisted Track Inspector uses `InspectorPanel` sections for OVERVIEW,
SOURCES, QUALITY, COLLECTION, and CAPABILITIES. It exposes actual DTO data,
keeps local paths out of the DTO, and makes unavailable actions explainable.
Online SearchResult inspection remains ephemeral and shows the explicit
metadata-only/unsupported playback limits. Search, Library, Playlists, and
Downloads reuse capability-aware action derivation, while Dark/Light/System/
Custom themes, density profiles, reduced motion, and focus behavior remain
under the Plan 10 semantic token contract.

## Plan 12 Windows integration surfaces

Settings now includes a Windows Integration section for the persisted SMTC
preference, the global shortcut master switch and nine editable bindings, four
native overlay toggles, session-only Gaming click-through, and output-profile
creation/edit/apply/delete controls. Every native-only action is hidden or
explained in browser preview; the preview adapter still exercises typed state
transitions without claiming native capabilities.

The overlay React surfaces are intentionally small and distinct: Mini is a
compact track/control card, Edge is a narrow right-edge now-playing card,
Lyrics is a synchronized cue surface, and Gaming is a minimal HUD. They share
the `OverlayFrame` shell and semantic tokens, expose an explicit close control,
and consume the same playback/lyrics snapshots as the main app. The native
window labels and exact dimensions are owned by Rust, not inferred from CSS.
