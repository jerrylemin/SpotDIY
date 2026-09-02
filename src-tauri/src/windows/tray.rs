use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Runtime};

use crate::playback::OutputProfile;

use super::overlays::OverlayKind;
use super::{WindowsAction, WindowsIntegrationService};

pub const TRAY_ID: &str = "spotdiy-tray";

const SHOW_HIDE_ID: &str = "tray-show-hide-main";
const PLAY_PAUSE_ID: &str = "tray-play-pause";
const PREVIOUS_ID: &str = "tray-previous";
const NEXT_ID: &str = "tray-next";
const CLICK_THROUGH_ID: &str = "tray-disable-gaming-click-through";
const QUIT_ID: &str = "tray-quit";

pub fn build_tray<R: Runtime>(
    app: &AppHandle<R>,
    service: WindowsIntegrationService,
    profiles: &[OutputProfile],
) -> tauri::Result<TrayIcon<R>> {
    let menu = build_menu(app, profiles)?;

    let menu_service = service.clone();
    let tray_service = service.clone();
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("SpotDIY")
        .on_menu_event(move |_app, event| {
            if let Some(action) = action_for_menu_id(&event.id().0) {
                let _ = menu_service.dispatch(action);
            }
        })
        .on_tray_icon_event(move |_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = tray_service.show_main();
            }
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)
}

pub fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    profiles: &[OutputProfile],
) -> tauri::Result<tauri::menu::Menu<R>> {
    let overlays = SubmenuBuilder::new(app, "Overlays")
        .text("tray-overlay-mini", "Mini")
        .text("tray-overlay-edge", "Edge")
        .text("tray-overlay-lyrics", "Lyrics")
        .text("tray-overlay-gaming", "Gaming")
        .build()?;

    let mut profile_builder = SubmenuBuilder::new(app, "Output Profiles");
    for profile in profiles {
        profile_builder = profile_builder.text(profile_menu_id(&profile.id), profile.name.as_str());
    }
    let profiles_menu = profile_builder.build()?;

    MenuBuilder::new(app)
        .text(SHOW_HIDE_ID, "Show SpotDIY / Hide SpotDIY")
        .separator()
        .text(PLAY_PAUSE_ID, "Play / Pause")
        .text(PREVIOUS_ID, "Previous")
        .text(NEXT_ID, "Next")
        .separator()
        .item(&overlays)
        .text(CLICK_THROUGH_ID, "Disable Gaming click-through")
        .item(&profiles_menu)
        .separator()
        .text(QUIT_ID, "Quit")
        .build()
}

pub fn profile_menu_id(profile_id: &str) -> String {
    format!("tray-output-profile-{profile_id}")
}

pub fn action_for_menu_id(id: &str) -> Option<WindowsAction> {
    let action = match id {
        SHOW_HIDE_ID => WindowsAction::ShowHideMain,
        PLAY_PAUSE_ID => WindowsAction::PlayPause,
        PREVIOUS_ID => WindowsAction::Previous,
        NEXT_ID => WindowsAction::Next,
        CLICK_THROUGH_ID => WindowsAction::DisableGamingClickThrough,
        QUIT_ID => WindowsAction::Quit,
        "tray-overlay-mini" => WindowsAction::ToggleOverlay(OverlayKind::Mini),
        "tray-overlay-edge" => WindowsAction::ToggleOverlay(OverlayKind::Edge),
        "tray-overlay-lyrics" => WindowsAction::ToggleOverlay(OverlayKind::Lyrics),
        "tray-overlay-gaming" => WindowsAction::ToggleOverlay(OverlayKind::Gaming),
        value if value.starts_with("tray-output-profile-") => WindowsAction::ApplyOutputProfile(
            value.trim_start_matches("tray-output-profile-").to_owned(),
        ),
        _ => return None,
    };
    Some(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_ids_share_the_native_action_dispatcher() {
        assert_eq!(
            action_for_menu_id("tray-play-pause"),
            Some(WindowsAction::PlayPause)
        );
        assert_eq!(
            action_for_menu_id("tray-overlay-gaming"),
            Some(WindowsAction::ToggleOverlay(OverlayKind::Gaming))
        );
        assert_eq!(
            action_for_menu_id("tray-output-profile-desk"),
            Some(WindowsAction::ApplyOutputProfile("desk".to_owned()))
        );
    }
}
