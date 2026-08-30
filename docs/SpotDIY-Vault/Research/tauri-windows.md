# Tauri 2 on Windows: SpotDIY research

Date: 2026-08-30

This is a read-only architecture/API survey for SpotDIY. The repository currently has no manifests or existing research-note convention, so version assumptions below are explicit rather than inferred from a lockfile.

## Primary sources (URLs)

- [Tauri ecosystem releases](https://v2.tauri.app/release/)
- [Tauri architecture](https://v2.tauri.app/concept/architecture/)
- [Tauri process model](https://v2.tauri.app/concept/process-model/)
- [Tauri JavaScript window API](https://v2.tauri.app/reference/javascript/api/namespacewindow/)
- [Tauri configuration reference](https://v2.tauri.app/reference/config/)
- [Tauri core permissions](https://v2.tauri.app/reference/acl/core-permissions/)
- [Tauri global-shortcut plugin](https://v2.tauri.app/plugin/global-shortcut/)
- [Official Tauri plugins workspace](https://github.com/tauri-apps/plugins-workspace)
- [Tauri system tray guide](https://v2.tauri.app/learn/system-tray/)
- [Tauri WebView2 versions](https://v2.tauri.app/reference/webview-versions/)
- [Tauri Windows installer modes](https://v2.tauri.app/distribute/windows-installer/)
- [Tauri `WebviewWindow` Rust API](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindow.html)
- [Tao window API](https://docs.rs/tao/latest/tao/window/struct.Window.html)
- [Microsoft WebView2 overview](https://learn.microsoft.com/en-us/microsoft-edge/webview2/)
- [Microsoft WebView2 browser-feature differences](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/browser-features)
- [Microsoft Win32 window features and z-order](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features)
- [Microsoft SMTC integration](https://learn.microsoft.com/en-us/windows/apps/develop/media-playback/integrate-with-systemmediatransportcontrols)
- [Microsoft desktop SMTC `GetForWindow` interop](https://learn.microsoft.com/en-us/windows/win32/api/systemmediatransportcontrolsinterop/nf-systemmediatransportcontrolsinterop-isystemmediatransportcontrolsinterop-getforwindow)
- [Microsoft global SMTC session manager](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager?view=winrt-26100)
- [W3C Media Session specification](https://www.w3.org/TR/2026/WD-mediasession-20260605/)

## Current API behavior

### Architecture

Tauri runs a Rust core process and one or more platform WebViews. On Windows the WebView is Microsoft Edge WebView2; TAO manages native windows and WRY hosts the WebView. Frontend code communicates with Rust through Tauri commands/events, so native window, tray, shortcut, and Windows-media integration belongs in the Rust core or a plugin rather than in WebView JavaScript alone.

`WebviewWindow` wraps a Tauri window and WebView. Its Windows Rust API exposes the native `HWND`, which is the useful bridge point for Windows APIs that Tauri does not wrap, including SMTC.

### Windows and overlay behavior

Use a dedicated `WebviewWindow` for the overlay. Creation options and runtime APIs cover the usual shell behavior:

- `alwaysOnTop`, `decorations`, `transparent`, `skipTaskbar`, `visible`, `focusable`, and positioning options are available.
- Runtime methods include `setAlwaysOnTop`, `setIgnoreCursorEvents`, `setPosition`, `setSkipTaskbar`, `setContentProtected`, and `isAlwaysOnTop`.
- `setIgnoreCursorEvents(true)` makes the overlay pass pointer events through to the window behind it; `false` restores interaction. Always-on-top does not itself make a window click-through.
- `parent` creates an owner relationship on Windows: the owned overlay stays above its owner and is hidden when the owner is minimized. Avoid that relationship if the overlay must remain visible independently.
- `visibleOnAllWorkspaces` is unsupported on Windows, so Tauri core does not provide virtual-desktop persistence.

Recommended baseline: an undecorated, transparent, taskbar-hidden window created hidden, positioned before showing, with always-on-top enabled. Toggle click-through explicitly for “display” versus “interactive” mode. Windows topmost is ordinary Win32 z-order; it should not be treated as a guarantee above every other topmost, system-controlled, or full-screen surface.

Transparency needs Windows-specific QA. Tauri documents alpha limitations in the native/window layers; `noRedirectionBitmap` can help avoid the initial white flash for transparent windows. Window effects require transparency and can interact with decorations and shadows. `setContentProtected` is available when the overlay should be excluded from other apps’ capture, but that behavior should be validated on the supported Windows versions.

Frontend calls that mutate windows are ACL-protected. Add only the required core permissions, such as `core:window:allow-set-always-on-top`, `allow-set-ignore-cursor-events`, `allow-set-position`, and `allow-set-skip-taskbar`, to the intended capability.

### Global shortcuts

The official `global-shortcut` plugin supports Windows. It provides registration, unregistration, `isRegistered`, and pressed/released state callbacks for combinations such as `CommandOrControl+Shift+C`. A shortcut already owned by another application will not invoke SpotDIY’s handler; `isRegistered` reports this app’s registration, not universal ownership.

Use this plugin for app-wide hotkeys, with unique and user-configurable defaults, registration-error handling, and explicit lifecycle cleanup. WebView key handlers are not a substitute: WebView2 browser accelerators and page focus can intercept or conflict with in-content shortcuts. Plugin commands also require the corresponding capability permissions.

### System tray

The tray is a Tauri core feature enabled with the `tray-icon` Cargo feature, not a separate official plugin. Rust can use `TrayIconBuilder`; JavaScript can use `TrayIcon`. Both support a menu, title, tooltip, and click/press/release events. The default menu behavior includes left and right click; disable the left-click menu when left click should show/focus the overlay instead.

Use the tray for show/focus, settings, pause/resume, and quit actions. It is a normal Windows notification-area surface and does not replace the overlay. Any JavaScript tray mutations still need the relevant core tray ACL permissions.

### Windows media controls

Tauri 2 has no Windows SMTC API in core, and the current official Tauri plugin catalog does not list a media-control plugin. The options are:

| Option | Behavior and decision |
| --- | --- |
| Web Media Session | Use `navigator.mediaSession` metadata and action handlers as the lowest-effort path. Feature-detect it. WebView2/Chromium behavior is not a Tauri contract, so verify Windows hardware keys, notification metadata, seek, and lock-screen behavior. |
| Native SMTC bridge | For dependable Windows metadata and play/pause/next/previous/seek integration, add a Windows-only Rust/WinRT module. Obtain SpotDIY’s top-level `HWND` from `WebviewWindow`, call `ISystemMediaTransportControlsInterop::GetForWindow`, and synchronize `DisplayUpdater`, `PlaybackStatus`, enabled buttons, button events, and timeline state with the actual player. |
| Native WinRT `MediaPlayer` | Microsoft documents automatic SMTC integration when playback is built on `MediaPlayer`. This is a larger playback-architecture choice and is not supplied by Tauri core. |
| Global SMTC sessions | `GlobalSystemMediaTransportControlsSessionManager` observes or controls other applications’ sessions. It is not the API for publishing SpotDIY’s own player and requires the documented `globalMediaControl` capability on Windows 10 1809+. |

Practical direction: start with Web Media Session if “best effort” controls are acceptable; plan a native SMTC adapter if system metadata/actions are an acceptance criterion. Keep the adapter behind `cfg(windows)` and treat it as a custom integration surface.

### WebView2 and packaging

Tauri uses the installed Microsoft WebView2 runtime rather than shipping a browser in the executable. The default Windows installer mode downloads the WebView2 bootstrapper when needed. Tauri also documents embedded, offline, fixed-runtime, and skip modes; `skip` can leave the app unusable when the runtime is absent, while fixed/offline modes substantially increase installer size and create runtime-update responsibility. `minimumWebview2Version` can enforce a minimum runtime.

WebView2 is Evergreen by default, so Chromium behavior and browser accelerator keys can change independently of SpotDIY. Microsoft documents gaps or differences for features such as Push Notifications, Web Payments, and Periodic Background Sync. Tauri’s `backgroundThrottling` setting is unsupported on Windows; hidden/minimized WebView timers must not be the sole source of time-sensitive playback state.

## Rejected alternatives

- **Legacy `MediaControl` APIs:** rejected; Microsoft recommends the newer System Media Transport Controls path.
- **Global SMTC session manager for SpotDIY’s own player:** rejected; it is for discovering/controlling sessions exposed by other apps and has an additional capability requirement.
- **A fixed WebView2 runtime as the default distribution:** rejected for a normal consumer install because of the large payload and manual security-update burden; reserve it for offline, kiosk, or compliance scenarios.
- **Raw Win32 topmost calls or keyboard hooks as the baseline:** rejected while Tauri/Tao expose the required window and shortcut semantics. Use the native `HWND` only where a real Tauri API gap exists, such as custom SMTC.
- **A WinUI/Windows App SDK rewrite:** rejected for the current scope; it would replace the existing Tauri/WebView architecture rather than solve a demonstrated limitation.

## Risks

- **Runtime/OS drift:** Evergreen WebView2 updates can change browser behavior. Tauri documentation still mentions legacy Windows 7 support, while Microsoft’s current WebView2 support list starts with modern Windows 10/11-era targets; clean-install, LTSC, and offline cases need explicit testing.
- **Overlay semantics:** topmost ordering, transparency, white-flash avoidance, shadows, DPI/scale-factor positioning, and full-screen interactions vary by Windows version and WebView2 runtime.
- **State throttling:** minimized/hidden WebViews may not be reliable media-state clocks, and Windows does not support Tauri’s background-throttling control.
- **Shortcut ownership:** common combinations may be unavailable or reserved. Registration failures must be visible and recoverable.
- **Capability exposure:** window, tray, and plugin commands are denied unless allowed by the capability ACL. Avoid broad permissions or privileged commands exposed to untrusted WebView origins.
- **Media synchronization:** a custom SMTC bridge can show stale metadata or issue duplicate actions unless it has one authoritative playback state and carefully handles teardown. WebView2 audio identity and system capture behavior should also be tested.

## Version assumptions

- Research date: **2026-08-30**.
- Current stable release information at research time: Tauri core `2.11.5`; the ecosystem release page lists `@tauri-apps/api` `2.11.1`, `@tauri-apps/cli` `2.11.4`, and official `global-shortcut` `2.3.2`.
- SpotDIY target assumption: Windows 10 1803+ and Windows 11 with Evergreen WebView2. Do not promise Windows 7, Server, or LTSC support without a packaging/runtime test matrix.
- Pin compatible Tauri 2 and plugin versions when implementation begins; the current repository has no manifests or lockfiles from which to infer a pinned version.
