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
