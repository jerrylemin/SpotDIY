pub mod overlays;
pub mod shortcuts;
pub mod smtc;
pub mod tray;

use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use tauri::{tray::TrayIcon, AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::db::Database;
use crate::playback::{
    OutputProfile, OutputProfileApplyError, OutputProfileApplyErrorCode, PlaybackPhase,
    PlaybackService, PlaybackSnapshot,
};
use crate::settings::{
    default_global_shortcuts, validate_global_shortcuts, GlobalShortcutAction,
    GlobalShortcutBinding, SettingValue, SettingsRepository, WindowsIntegrationSettings,
};

pub use overlays::{OverlayKind, OverlaySnapshot, OverlayStatus};
pub use shortcuts::{ShortcutRegistrationStatus, ShortcutStatus};
pub use smtc::SmtcStatus;

pub const WINDOWS_STATE_EVENT: &str = "windows://state";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TrayStatus {
    Ready,
    #[default]
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GamingClickThroughErrorCode {
    RescueUnavailable,
    NativeCallFailed,
    OverlayUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamingClickThroughError {
    pub code: GamingClickThroughErrorCode,
    pub detail: String,
}

impl std::fmt::Display for GamingClickThroughError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.detail)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WindowsAction {
    PlayPause,
    Previous,
    Next,
    VolumeUp,
    VolumeDown,
    ShowHideMain,
    ToggleOverlay(OverlayKind),
    DisableGamingClickThrough,
    ApplyOutputProfile(String),
    Quit,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsIntegrationSnapshot {
    pub revision: u64,
    pub platform_supported: bool,
    pub tray_status: TrayStatus,
    pub tray_detail: Option<String>,
    pub smtc_status: SmtcStatus,
    pub smtc_detail: Option<String>,
    pub global_shortcuts_enabled: bool,
    pub shortcut_statuses: Vec<ShortcutStatus>,
    pub overlays: Vec<OverlaySnapshot>,
    pub gaming_click_through: bool,
    pub output_profiles: Vec<OutputProfile>,
}

#[derive(Clone)]
pub struct WindowsIntegrationService {
    inner: Arc<WindowsIntegrationInner>,
}

struct WindowsIntegrationInner {
    app: AppHandle,
    database: Database,
    playback: PlaybackService,
    overlays: overlays::OverlayManager,
    state: Mutex<WindowsIntegrationState>,
    tray: Mutex<Option<TrayIcon>>,
}

struct WindowsIntegrationState {
    settings: WindowsIntegrationSettings,
    bindings: Vec<GlobalShortcutBinding>,
    output_profiles: Vec<OutputProfile>,
    shortcuts: shortcuts::ShortcutController,
    smtc: smtc::SmtcController,
    tray_status: TrayStatus,
    tray_detail: Option<String>,
    gaming_click_through: bool,
    revision: u64,
    last_smtc_key: Option<SmtcKey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SmtcKey {
    title: Option<String>,
    artists: Vec<String>,
    album: Option<String>,
    phase: PlaybackPhase,
}

impl WindowsIntegrationService {
    pub fn new(app: AppHandle, database: Database, playback: PlaybackService) -> Self {
        let settings = SettingsRepository::new(&database)
            .get_snapshot()
            .unwrap_or_default();
        let overlays = overlays::OverlayManager::new(app.clone());
        Self {
            inner: Arc::new(WindowsIntegrationInner {
                app,
                database,
                playback,
                overlays,
                state: Mutex::new(WindowsIntegrationState {
                    settings: settings.windows_integration,
                    bindings: settings.global_shortcuts,
                    output_profiles: settings.output_profiles,
                    shortcuts: shortcuts::ShortcutController::default(),
                    smtc: smtc::SmtcController::default(),
                    tray_status: TrayStatus::Failed,
                    tray_detail: None,
                    gaming_click_through: false,
                    revision: 0,
                    last_smtc_key: None,
                }),
                tray: Mutex::new(None),
            }),
        }
    }

    pub fn initialize(&self) {
        self.initialize_tray();
        self.configure_shortcuts();
        let smtc_enabled = self.state_lock().settings.smtc_enabled;
        if smtc_enabled {
            self.start_smtc();
        } else {
            self.disable_smtc();
        }
        let snapshot = self.inner.playback.snapshot();
        self.on_playback_snapshot(&snapshot);
        self.publish_state();
    }

    pub fn snapshot(&self) -> WindowsIntegrationSnapshot {
        let state = self.state_lock();
        self.snapshot_from_state(&state)
    }

    pub fn handle_shortcut(&self, id: u32) {
        let action = {
            let state = self.state_lock();
            if state.shortcuts.is_rescue(id) {
                Some(WindowsAction::DisableGamingClickThrough)
            } else {
                state.shortcuts.action_for_id(id).map(action_for_shortcut)
            }
        };
        if let Some(action) = action {
            let _ = self.dispatch(action);
        }
    }

    pub fn handle_media_command(&self, command: smtc::MediaCommand) {
        let phase = self.inner.playback.snapshot().phase;
        match command {
            smtc::MediaCommand::Play
                if !matches!(phase, PlaybackPhase::Playing | PlaybackPhase::Seeking) =>
            {
                let _ = self.inner.playback.toggle_play_pause();
            }
            smtc::MediaCommand::Pause
                if matches!(phase, PlaybackPhase::Playing | PlaybackPhase::Seeking) =>
            {
                let _ = self.inner.playback.toggle_play_pause();
            }
            smtc::MediaCommand::Next => {
                let _ = self.inner.playback.next_track();
            }
            smtc::MediaCommand::Previous => {
                let _ = self.inner.playback.previous_track();
            }
            smtc::MediaCommand::Play | smtc::MediaCommand::Pause => {}
        }
    }

    pub fn on_playback_snapshot(&self, snapshot: &PlaybackSnapshot) {
        let key = SmtcKey {
            title: snapshot.title.clone(),
            artists: snapshot.artists.clone(),
            album: snapshot.album.clone(),
            phase: snapshot.phase,
        };
        let should_update = {
            let mut state = self.state_lock();
            if state.last_smtc_key.as_ref() == Some(&key) {
                false
            } else {
                state.last_smtc_key = Some(key);
                let _ = state.smtc.update(snapshot);
                true
            }
        };
        if should_update {
            self.publish_state();
        }
    }

    pub fn dispatch(&self, action: WindowsAction) -> Result<(), String> {
        match action {
            WindowsAction::PlayPause => self
                .inner
                .playback
                .toggle_play_pause()
                .map(|_| ())
                .map_err(|error| error.to_string()),
            WindowsAction::Previous => self
                .inner
                .playback
                .previous_track()
                .map(|_| ())
                .map_err(|error| error.to_string()),
            WindowsAction::Next => self
                .inner
                .playback
                .next_track()
                .map(|_| ())
                .map_err(|error| error.to_string()),
            WindowsAction::VolumeUp => {
                let volume = self
                    .inner
                    .playback
                    .snapshot()
                    .volume_percent
                    .saturating_add(5)
                    .min(100);
                self.inner
                    .playback
                    .set_playback_volume(volume)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
            WindowsAction::VolumeDown => {
                let volume = self
                    .inner
                    .playback
                    .snapshot()
                    .volume_percent
                    .saturating_sub(5);
                self.inner
                    .playback
                    .set_playback_volume(volume)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
            WindowsAction::ShowHideMain => self.toggle_main(),
            WindowsAction::ToggleOverlay(kind) => self.toggle_overlay(kind).map(|_| ()),
            WindowsAction::DisableGamingClickThrough => self
                .set_gaming_click_through(false)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            WindowsAction::ApplyOutputProfile(id) => self
                .apply_output_profile(&id)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            WindowsAction::Quit => {
                self.inner.app.exit(0);
                Ok(())
            }
        }
    }

    pub fn show_main(&self) -> Result<(), String> {
        let Some(window) = self.inner.app.get_webview_window("main") else {
            return Err("the main window is unavailable".to_owned());
        };
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())
    }

    pub fn open_overlay(&self, kind: OverlayKind) -> Result<WindowsIntegrationSnapshot, String> {
        self.inner.overlays.open(kind)?;
        self.publish_state();
        Ok(self.snapshot())
    }

    pub fn close_overlay(&self, kind: OverlayKind) -> Result<WindowsIntegrationSnapshot, String> {
        if kind == OverlayKind::Gaming {
            self.disable_gaming_click_through_no_publish();
        }
        self.inner.overlays.close(kind)?;
        self.publish_state();
        Ok(self.snapshot())
    }

    pub fn toggle_overlay(&self, kind: OverlayKind) -> Result<WindowsIntegrationSnapshot, String> {
        if kind == OverlayKind::Gaming && self.inner.overlays.is_open(kind) {
            self.disable_gaming_click_through_no_publish();
        }
        self.inner.overlays.toggle(kind)?;
        self.publish_state();
        Ok(self.snapshot())
    }

    pub fn set_gaming_click_through(
        &self,
        enabled: bool,
    ) -> Result<WindowsIntegrationSnapshot, GamingClickThroughError> {
        if enabled {
            if !self.inner.overlays.is_open(OverlayKind::Gaming) {
                return Err(gaming_error(
                    GamingClickThroughErrorCode::OverlayUnavailable,
                    "the Gaming overlay must be open before click-through can be enabled",
                ));
            }
            if self.state_lock().gaming_click_through {
                return Ok(self.snapshot());
            }
            if let Err(detail) = self.register_gaming_rescue() {
                return Err(gaming_error(
                    GamingClickThroughErrorCode::RescueUnavailable,
                    detail,
                ));
            }
            let Some(window) = self
                .inner
                .app
                .get_webview_window(OverlayKind::Gaming.label())
            else {
                self.disable_gaming_click_through_no_publish();
                return Err(gaming_error(
                    GamingClickThroughErrorCode::OverlayUnavailable,
                    "the Gaming overlay window is unavailable",
                ));
            };
            if let Err(error) = window.set_ignore_cursor_events(true) {
                self.disable_gaming_click_through_no_publish();
                return Err(gaming_error(
                    GamingClickThroughErrorCode::NativeCallFailed,
                    format!("could not enable Gaming click-through: {error}"),
                ));
            }
            self.state_lock().gaming_click_through = true;
            self.publish_state();
            return Ok(self.snapshot());
        }

        let native_error = self
            .inner
            .app
            .get_webview_window(OverlayKind::Gaming.label())
            .and_then(|window| window.set_ignore_cursor_events(false).err());
        self.disable_gaming_click_through_no_publish();
        self.publish_state();
        if let Some(error) = native_error {
            return Err(gaming_error(
                GamingClickThroughErrorCode::NativeCallFailed,
                format!("could not disable Gaming click-through: {error}"),
            ));
        }
        Ok(self.snapshot())
    }

    pub fn set_windows_integration_settings(
        &self,
        settings: WindowsIntegrationSettings,
    ) -> Result<WindowsIntegrationSnapshot, String> {
        let persisted = SettingsRepository::new(&self.inner.database)
            .set_setting(SettingValue::WindowsIntegration(settings))
            .map_err(|error| error.to_string())?;
        let previous = self.state_lock().settings;
        {
            let mut state = self.state_lock();
            state.settings = persisted.windows_integration;
            state.bindings = persisted.global_shortcuts.clone();
            state.output_profiles = persisted.output_profiles.clone();
        }
        if previous.smtc_enabled != settings.smtc_enabled {
            if settings.smtc_enabled {
                self.start_smtc();
            } else {
                self.disable_smtc();
            }
        }
        if previous.global_shortcuts_enabled != settings.global_shortcuts_enabled {
            self.configure_shortcuts();
        }
        self.publish_state();
        Ok(self.snapshot())
    }

    pub fn set_global_shortcuts_enabled(
        &self,
        enabled: bool,
    ) -> Result<WindowsIntegrationSnapshot, String> {
        let mut settings = self.state_lock().settings;
        settings.global_shortcuts_enabled = enabled;
        self.set_windows_integration_settings(settings)
    }

    pub fn update_global_shortcut(
        &self,
        binding: GlobalShortcutBinding,
    ) -> Result<WindowsIntegrationSnapshot, String> {
        let mut bindings = self.state_lock().bindings.clone();
        if let Some(existing) = bindings
            .iter_mut()
            .find(|item| item.action == binding.action)
        {
            *existing = binding.clone();
        } else {
            bindings.push(binding.clone());
        }
        validate_global_shortcuts(&bindings).map_err(|error| error.to_string())?;
        let persisted = SettingsRepository::new(&self.inner.database)
            .set_setting(SettingValue::GlobalShortcuts(bindings))
            .map_err(|error| error.to_string())?;
        {
            let mut state = self.state_lock();
            state.bindings = persisted.global_shortcuts.clone();
            let master_enabled = state.settings.global_shortcuts_enabled;
            let _ = state
                .shortcuts
                .update_binding(&self.inner.app, &binding, master_enabled);
        }
        self.publish_state();
        Ok(self.snapshot())
    }

    pub fn reset_global_shortcuts(&self) -> Result<WindowsIntegrationSnapshot, String> {
        let bindings = default_global_shortcuts();
        let persisted = SettingsRepository::new(&self.inner.database)
            .set_setting(SettingValue::GlobalShortcuts(bindings))
            .map_err(|error| error.to_string())?;
        {
            let mut state = self.state_lock();
            state.bindings = persisted.global_shortcuts;
        }
        self.configure_shortcuts();
        self.publish_state();
        Ok(self.snapshot())
    }

    pub fn list_output_profiles(&self) -> Vec<OutputProfile> {
        self.state_lock().output_profiles.clone()
    }

    pub fn create_output_profile(
        &self,
        name: String,
    ) -> Result<WindowsIntegrationSnapshot, String> {
        let playback = self.inner.playback.snapshot();
        let profile = OutputProfile {
            id: Uuid::new_v4().to_string(),
            name,
            audio_device_name: playback.selected_audio_device,
            volume_percent: playback.volume_percent,
            muted: playback.muted,
        }
        .normalized()
        .map_err(|error| error.to_string())?;
        let mut profiles = self.list_output_profiles();
        profiles.push(profile);
        self.persist_output_profiles(profiles)
    }

    pub fn update_output_profile(
        &self,
        profile: OutputProfile,
    ) -> Result<WindowsIntegrationSnapshot, String> {
        let profile = profile.normalized().map_err(|error| error.to_string())?;
        self.ensure_output_device_available(&profile)?;
        let mut profiles = self.list_output_profiles();
        let Some(existing) = profiles.iter_mut().find(|item| item.id == profile.id) else {
            return Err(format!("output profile {} was not found", profile.id));
        };
        *existing = profile;
        self.persist_output_profiles(profiles)
    }

    pub fn delete_output_profile(&self, id: &str) -> Result<WindowsIntegrationSnapshot, String> {
        let mut profiles = self.list_output_profiles();
        let original_len = profiles.len();
        profiles.retain(|profile| profile.id != id);
        if profiles.len() == original_len {
            return Err(format!("output profile {id} was not found"));
        }
        self.persist_output_profiles(profiles)
    }

    pub fn apply_output_profile(
        &self,
        id: &str,
    ) -> Result<PlaybackSnapshot, OutputProfileApplyError> {
        let profile = self
            .list_output_profiles()
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| OutputProfileApplyError {
                code: OutputProfileApplyErrorCode::InvalidProfile,
                detail: format!("output profile {id} was not found"),
                rollback_succeeded: true,
            })?;
        let result = self.inner.playback.apply_output_profile(profile);
        if let Ok(snapshot) = result.as_ref() {
            self.on_playback_snapshot(snapshot);
        }
        result
    }

    pub fn shutdown(&self) {
        self.disable_gaming_click_through_no_publish();
        {
            let mut state = self.state_lock();
            state.shortcuts.unregister_all(&self.inner.app);
        }
        self.disable_smtc();
        self.inner.overlays.close_all();
        let _ = self
            .inner
            .tray
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
    }

    fn initialize_tray(&self) {
        let profiles = self.list_output_profiles();
        match tray::build_tray(&self.inner.app, self.clone(), &profiles) {
            Ok(tray_icon) => {
                *self
                    .inner
                    .tray
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(tray_icon);
                let mut state = self.state_lock();
                state.tray_status = TrayStatus::Ready;
                state.tray_detail = None;
            }
            Err(error) => {
                let mut state = self.state_lock();
                state.tray_status = TrayStatus::Failed;
                state.tray_detail = Some(error.to_string());
            }
        }
    }

    fn refresh_tray(&self) {
        let profiles = self.list_output_profiles();
        let menu = match tray::build_menu(&self.inner.app, &profiles) {
            Ok(menu) => menu,
            Err(error) => {
                let mut state = self.state_lock();
                state.tray_status = TrayStatus::Failed;
                state.tray_detail = Some(error.to_string());
                return;
            }
        };
        let result = self
            .inner
            .tray
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|tray_icon| tray_icon.set_menu(Some(menu)));
        if let Some(Err(error)) = result {
            let mut state = self.state_lock();
            state.tray_status = TrayStatus::Failed;
            state.tray_detail = Some(error.to_string());
        }
    }

    fn persist_output_profiles(
        &self,
        profiles: Vec<OutputProfile>,
    ) -> Result<WindowsIntegrationSnapshot, String> {
        let persisted = SettingsRepository::new(&self.inner.database)
            .set_setting(SettingValue::OutputProfiles(profiles))
            .map_err(|error| error.to_string())?;
        {
            let mut state = self.state_lock();
            state.output_profiles = persisted.output_profiles;
        }
        self.refresh_tray();
        self.publish_state();
        Ok(self.snapshot())
    }

    fn ensure_output_device_available(&self, profile: &OutputProfile) -> Result<(), String> {
        if profile.audio_device_name.eq_ignore_ascii_case("auto") {
            return Ok(());
        }
        let devices = self
            .inner
            .playback
            .get_audio_devices()
            .map_err(|error| format!("could not enumerate audio devices: {error}"))?;
        if devices
            .iter()
            .any(|device| device.name == profile.audio_device_name)
        {
            Ok(())
        } else {
            Err(format!(
                "audio device {} is not currently available",
                profile.audio_device_name
            ))
        }
    }

    fn configure_shortcuts(&self) {
        let mut state = self.state_lock();
        let bindings = state.bindings.clone();
        let enabled = state.settings.global_shortcuts_enabled;
        let _ = state
            .shortcuts
            .configure(&self.inner.app, &bindings, enabled);
    }

    fn start_smtc(&self) {
        let main_window = self.inner.app.get_webview_window("main");
        let service = self.clone();
        let handler = Arc::new(move |command| service.handle_media_command(command));
        let mut controller = {
            let mut state = self.state_lock();
            std::mem::take(&mut state.smtc)
        };
        controller.disable();
        let result = controller.start(main_window.as_ref(), handler);
        if let Err(error) = &result {
            #[cfg(windows)]
            controller.fail(error.clone());
        }
        {
            let mut state = self.state_lock();
            state.smtc = controller;
            state.last_smtc_key = None;
        }
        self.publish_state();
    }

    fn disable_smtc(&self) {
        let mut state = self.state_lock();
        state.smtc.disable();
        state.last_smtc_key = None;
    }

    fn register_gaming_rescue(&self) -> Result<(), String> {
        let mut state = self.state_lock();
        state.shortcuts.register_rescue(&self.inner.app)
    }

    fn disable_gaming_click_through_no_publish(&self) {
        if let Some(window) = self
            .inner
            .app
            .get_webview_window(OverlayKind::Gaming.label())
        {
            let _ = window.set_ignore_cursor_events(false);
        }
        let mut state = self.state_lock();
        state.shortcuts.unregister_rescue(&self.inner.app);
        state.gaming_click_through = false;
    }

    fn toggle_main(&self) -> Result<(), String> {
        let Some(window) = self.inner.app.get_webview_window("main") else {
            return Err("the main window is unavailable".to_owned());
        };
        if window.is_visible().unwrap_or(false) {
            window.hide().map_err(|error| error.to_string())
        } else {
            self.show_main()
        }
    }

    fn publish_state(&self) {
        let snapshot = {
            let mut state = self.state_lock();
            state.revision = state.revision.saturating_add(1);
            self.snapshot_from_state(&state)
        };
        let _ = self.inner.app.emit(WINDOWS_STATE_EVENT, snapshot);
    }

    fn snapshot_from_state(&self, state: &WindowsIntegrationState) -> WindowsIntegrationSnapshot {
        WindowsIntegrationSnapshot {
            revision: state.revision,
            platform_supported: cfg!(windows),
            tray_status: state.tray_status,
            tray_detail: state.tray_detail.clone(),
            smtc_status: state.smtc.status(),
            smtc_detail: state.smtc.detail(),
            global_shortcuts_enabled: state.settings.global_shortcuts_enabled,
            shortcut_statuses: state
                .shortcuts
                .statuses(&state.bindings, state.settings.global_shortcuts_enabled),
            overlays: self.inner.overlays.snapshots(),
            gaming_click_through: state.gaming_click_through,
            output_profiles: state.output_profiles.clone(),
        }
    }

    fn state_lock(&self) -> MutexGuard<'_, WindowsIntegrationState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn action_for_shortcut(action: GlobalShortcutAction) -> WindowsAction {
    match action {
        GlobalShortcutAction::PlayPause => WindowsAction::PlayPause,
        GlobalShortcutAction::Next => WindowsAction::Next,
        GlobalShortcutAction::Previous => WindowsAction::Previous,
        GlobalShortcutAction::VolumeUp => WindowsAction::VolumeUp,
        GlobalShortcutAction::VolumeDown => WindowsAction::VolumeDown,
        GlobalShortcutAction::ShowHideMain => WindowsAction::ShowHideMain,
        GlobalShortcutAction::ToggleMiniOverlay => WindowsAction::ToggleOverlay(OverlayKind::Mini),
        GlobalShortcutAction::ToggleLyricsOverlay => {
            WindowsAction::ToggleOverlay(OverlayKind::Lyrics)
        }
        GlobalShortcutAction::ToggleGamingOverlay => {
            WindowsAction::ToggleOverlay(OverlayKind::Gaming)
        }
    }
}

fn gaming_error(
    code: GamingClickThroughErrorCode,
    detail: impl Into<String>,
) -> GamingClickThroughError {
    GamingClickThroughError {
        code,
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_actions_map_to_the_declared_native_actions() {
        assert_eq!(
            action_for_shortcut(GlobalShortcutAction::ToggleGamingOverlay),
            WindowsAction::ToggleOverlay(OverlayKind::Gaming)
        );
        assert_eq!(
            action_for_shortcut(GlobalShortcutAction::VolumeDown),
            WindowsAction::VolumeDown
        );
    }

    #[test]
    fn click_through_errors_are_stable_dtos() {
        let error = gaming_error(
            GamingClickThroughErrorCode::RescueUnavailable,
            "shortcut conflict",
        );
        assert_eq!(
            serde_json::to_value(error).unwrap()["code"],
            "rescueUnavailable"
        );
    }
}
