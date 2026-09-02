use std::collections::HashMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::settings::{is_valid_accelerator, GlobalShortcutAction, GlobalShortcutBinding};

pub const GAMING_RESCUE_ACCELERATOR: &str = "Ctrl+Alt+Shift+G";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutRegistrationStatus {
    #[default]
    Disabled,
    Registered,
    Conflict,
    Invalid,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub action: GlobalShortcutAction,
    pub accelerator: String,
    pub enabled: bool,
    pub status: ShortcutRegistrationStatus,
    pub detail: Option<String>,
}

struct RegisteredShortcut {
    shortcut: Shortcut,
    accelerator: String,
}

#[derive(Default)]
pub struct ShortcutController {
    registered: HashMap<GlobalShortcutAction, RegisteredShortcut>,
    actions_by_id: HashMap<u32, GlobalShortcutAction>,
    rescue: Option<Shortcut>,
    statuses: HashMap<GlobalShortcutAction, ShortcutStatus>,
}

impl ShortcutController {
    pub fn configure(
        &mut self,
        app: &AppHandle,
        bindings: &[GlobalShortcutBinding],
        enabled: bool,
    ) -> Vec<ShortcutStatus> {
        self.unregister_all(app);
        self.statuses.clear();
        for binding in bindings {
            let mut status = ShortcutStatus {
                action: binding.action,
                accelerator: binding.accelerator.clone(),
                enabled: binding.enabled,
                status: ShortcutRegistrationStatus::Disabled,
                detail: None,
            };
            if enabled && binding.enabled {
                if !is_valid_accelerator(&binding.accelerator) {
                    status.status = ShortcutRegistrationStatus::Invalid;
                    status.detail =
                        Some("the accelerator must contain a valid key and modifier".to_owned());
                } else {
                    match Shortcut::from_str(&binding.accelerator) {
                        Ok(shortcut) => match app.global_shortcut().register(shortcut) {
                            Ok(()) => {
                                self.actions_by_id.insert(shortcut.id(), binding.action);
                                self.registered.insert(
                                    binding.action,
                                    RegisteredShortcut {
                                        shortcut,
                                        accelerator: binding.accelerator.clone(),
                                    },
                                );
                                status.status = ShortcutRegistrationStatus::Registered;
                            }
                            Err(error) => {
                                status.status = classify_registration_error(&error.to_string());
                                status.detail = Some(error.to_string());
                            }
                        },
                        Err(error) => {
                            status.status = ShortcutRegistrationStatus::Invalid;
                            status.detail = Some(error.to_string());
                        }
                    }
                }
            }
            self.statuses.insert(binding.action, status);
        }
        bindings
            .iter()
            .filter_map(|binding| self.statuses.get(&binding.action).cloned())
            .collect()
    }

    pub fn update_binding(
        &mut self,
        app: &AppHandle,
        binding: &GlobalShortcutBinding,
        master_enabled: bool,
    ) -> ShortcutStatus {
        let mut status = ShortcutStatus {
            action: binding.action,
            accelerator: binding.accelerator.clone(),
            enabled: binding.enabled,
            status: ShortcutRegistrationStatus::Disabled,
            detail: None,
        };
        if !master_enabled || !binding.enabled {
            if let Some(old) = self.registered.remove(&binding.action) {
                self.actions_by_id.remove(&old.shortcut.id());
                let _ = app.global_shortcut().unregister(old.shortcut);
            }
            self.statuses.insert(binding.action, status.clone());
            return status;
        }
        if !is_valid_accelerator(&binding.accelerator) {
            status.status = ShortcutRegistrationStatus::Invalid;
            status.detail =
                Some("the accelerator must contain a valid key and modifier".to_owned());
            self.statuses.insert(binding.action, status.clone());
            return status;
        }

        let old = self.registered.remove(&binding.action);
        if let Some(old) = old.as_ref() {
            self.actions_by_id.remove(&old.shortcut.id());
            if let Err(error) = app.global_shortcut().unregister(old.shortcut) {
                self.actions_by_id.insert(old.shortcut.id(), binding.action);
                self.registered.insert(
                    binding.action,
                    RegisteredShortcut {
                        shortcut: old.shortcut,
                        accelerator: old.accelerator.clone(),
                    },
                );
                status.status = ShortcutRegistrationStatus::Failed;
                status.detail = Some(format!("could not release the previous binding: {error}"));
                self.statuses.insert(binding.action, status.clone());
                return status;
            }
        }

        match Shortcut::from_str(&binding.accelerator) {
            Ok(shortcut) => match app.global_shortcut().register(shortcut) {
                Ok(()) => {
                    self.actions_by_id.insert(shortcut.id(), binding.action);
                    self.registered.insert(
                        binding.action,
                        RegisteredShortcut {
                            shortcut,
                            accelerator: binding.accelerator.clone(),
                        },
                    );
                    status.status = ShortcutRegistrationStatus::Registered;
                }
                Err(error) => {
                    status.status = classify_registration_error(&error.to_string());
                    status.detail = Some(error.to_string());
                    if let Some(old) = old {
                        match app.global_shortcut().register(old.shortcut) {
                            Ok(()) => {
                                self.actions_by_id.insert(old.shortcut.id(), binding.action);
                                self.registered.insert(binding.action, old);
                                status.detail = Some(format!(
                                    "{}; previous binding was restored",
                                    status.detail.take().unwrap_or_default()
                                ));
                            }
                            Err(restore_error) => {
                                status.detail = Some(format!(
                                    "{}; previous binding could not be restored: {restore_error}",
                                    status.detail.take().unwrap_or_default()
                                ));
                            }
                        }
                    }
                }
            },
            Err(error) => {
                status.status = ShortcutRegistrationStatus::Invalid;
                status.detail = Some(error.to_string());
                if let Some(old) = old {
                    if app.global_shortcut().register(old.shortcut).is_ok() {
                        self.actions_by_id.insert(old.shortcut.id(), binding.action);
                        self.registered.insert(binding.action, old);
                    }
                }
            }
        }
        self.statuses.insert(binding.action, status.clone());
        status
    }

    pub fn action_for_id(&self, id: u32) -> Option<GlobalShortcutAction> {
        self.actions_by_id.get(&id).copied()
    }

    pub fn register_rescue(&mut self, app: &AppHandle) -> Result<(), String> {
        if self.rescue.is_some() {
            return Ok(());
        }
        let shortcut = Shortcut::from_str(GAMING_RESCUE_ACCELERATOR)
            .map_err(|error| format!("invalid gaming rescue shortcut: {error}"))?;
        app.global_shortcut()
            .register(shortcut)
            .map_err(|error| error.to_string())?;
        self.rescue = Some(shortcut);
        Ok(())
    }

    pub fn unregister_rescue(&mut self, app: &AppHandle) {
        if let Some(shortcut) = self.rescue.take() {
            let _ = app.global_shortcut().unregister(shortcut);
        }
    }

    pub fn is_rescue(&self, id: u32) -> bool {
        self.rescue.is_some_and(|shortcut| shortcut.id() == id)
    }

    pub fn unregister_all(&mut self, app: &AppHandle) {
        for registered in self.registered.values() {
            let _ = app.global_shortcut().unregister(registered.shortcut);
        }
        self.registered.clear();
        self.actions_by_id.clear();
    }

    pub fn statuses(
        &self,
        bindings: &[GlobalShortcutBinding],
        master_enabled: bool,
    ) -> Vec<ShortcutStatus> {
        bindings
            .iter()
            .map(|binding| {
                self.statuses
                    .get(&binding.action)
                    .cloned()
                    .unwrap_or(ShortcutStatus {
                        action: binding.action,
                        accelerator: binding.accelerator.clone(),
                        enabled: binding.enabled,
                        status: if master_enabled && binding.enabled {
                            ShortcutRegistrationStatus::Failed
                        } else {
                            ShortcutRegistrationStatus::Disabled
                        },
                        detail: None,
                    })
            })
            .collect()
    }
}

fn classify_registration_error(detail: &str) -> ShortcutRegistrationStatus {
    let lower = detail.to_ascii_lowercase();
    if lower.contains("already") || lower.contains("conflict") || lower.contains("occupied") {
        ShortcutRegistrationStatus::Conflict
    } else {
        ShortcutRegistrationStatus::Failed
    }
}

pub fn action_label(action: GlobalShortcutAction) -> &'static str {
    match action {
        GlobalShortcutAction::PlayPause => "Play/Pause",
        GlobalShortcutAction::Next => "Next",
        GlobalShortcutAction::Previous => "Previous",
        GlobalShortcutAction::VolumeUp => "Volume +5%",
        GlobalShortcutAction::VolumeDown => "Volume -5%",
        GlobalShortcutAction::ShowHideMain => "Show/Hide main",
        GlobalShortcutAction::ToggleMiniOverlay => "Mini overlay",
        GlobalShortcutAction::ToggleLyricsOverlay => "Lyrics overlay",
        GlobalShortcutAction::ToggleGamingOverlay => "Gaming overlay",
    }
}

#[allow(dead_code)]
fn _state_is_pressed(state: ShortcutState) -> bool {
    state == ShortcutState::Pressed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::default_global_shortcuts;

    #[test]
    fn defaults_have_unique_valid_accelerators() {
        let bindings = default_global_shortcuts();
        assert_eq!(bindings.len(), 9);
        assert!(bindings
            .iter()
            .all(|binding| is_valid_accelerator(&binding.accelerator)));
        assert_eq!(
            bindings
                .iter()
                .map(|binding| binding.action)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            bindings.len()
        );
    }

    #[test]
    fn labels_cover_every_action() {
        for action in [
            GlobalShortcutAction::PlayPause,
            GlobalShortcutAction::Next,
            GlobalShortcutAction::Previous,
            GlobalShortcutAction::VolumeUp,
            GlobalShortcutAction::VolumeDown,
            GlobalShortcutAction::ShowHideMain,
            GlobalShortcutAction::ToggleMiniOverlay,
            GlobalShortcutAction::ToggleLyricsOverlay,
            GlobalShortcutAction::ToggleGamingOverlay,
        ] {
            assert!(!action_label(action).is_empty());
        }
    }
}
