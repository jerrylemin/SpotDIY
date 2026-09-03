//! Session-only listening modes and small, shared validation helpers.
//!
//! The mode state intentionally lives only in memory.  A fresh process starts
//! in the normal, non-private mode, so neither Private Session nor Temporary
//! Mode can leak into a later launch.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListeningModeState {
    pub private_session: bool,
    pub temporary: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ListeningModeReason {
    PrivateEnabled,
    PrivateDisabled,
    TemporaryEntered,
    TemporaryExited,
    PrivateLockedByTemporary,
}

#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[error("{detail}")]
pub struct ListeningModeError {
    pub code: ListeningModeErrorCode,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ListeningModeErrorCode {
    PrivateLocked,
    InvalidState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListeningModeChange {
    pub state: ListeningModeState,
    pub reason: ListeningModeReason,
}

#[derive(Clone, Default)]
pub struct ListeningModeService {
    inner: Arc<Mutex<ListeningModeState>>,
}

impl ListeningModeService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> ListeningModeState {
        *self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set_private(&self, enabled: bool) -> Result<ListeningModeChange, ListeningModeError> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.temporary && !enabled {
            return Err(ListeningModeError {
                code: ListeningModeErrorCode::PrivateLocked,
                detail: "Private Session cannot be disabled while Temporary Mode is active"
                    .to_owned(),
            });
        }
        state.private_session = enabled;
        Ok(ListeningModeChange {
            state: *state,
            reason: if enabled {
                ListeningModeReason::PrivateEnabled
            } else {
                ListeningModeReason::PrivateDisabled
            },
        })
    }

    pub fn enter_temporary(&self) -> Result<ListeningModeChange, ListeningModeError> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.temporary {
            return Err(ListeningModeError {
                code: ListeningModeErrorCode::InvalidState,
                detail: "Temporary Mode is already active".to_owned(),
            });
        }
        state.temporary = true;
        state.private_session = true;
        Ok(ListeningModeChange {
            state: *state,
            reason: ListeningModeReason::TemporaryEntered,
        })
    }

    pub fn exit_temporary(
        &self,
        private_before: bool,
    ) -> Result<ListeningModeChange, ListeningModeError> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.temporary {
            return Err(ListeningModeError {
                code: ListeningModeErrorCode::InvalidState,
                detail: "Temporary Mode is not active".to_owned(),
            });
        }
        state.temporary = false;
        state.private_session = private_before;
        Ok(ListeningModeChange {
            state: *state,
            reason: ListeningModeReason::TemporaryExited,
        })
    }
}

pub fn normalize_label(value: impl AsRef<str>, max_chars: usize) -> Option<String> {
    let normalized = value
        .as_ref()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let normalized = normalized.trim();
    if normalized.is_empty() || normalized.chars().count() > max_chars {
        None
    } else {
        Some(normalized.to_owned())
    }
}

pub fn local_parts(at: DateTime<Utc>) -> (String, u8, u8) {
    let local: DateTime<Local> = at.with_timezone(&Local);
    (
        local.format("%Y-%m-%d").to_string(),
        local.hour() as u8,
        local.weekday().num_days_from_sunday() as u8,
    )
}

use chrono::{Datelike, Timelike};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_locks_private_and_restores_the_previous_state() {
        let modes = ListeningModeService::new();
        modes.set_private(true).unwrap();
        modes.enter_temporary().unwrap();
        assert!(modes.set_private(false).is_err());
        modes.exit_temporary(true).unwrap();
        assert_eq!(
            modes.state(),
            ListeningModeState {
                private_session: true,
                temporary: false
            }
        );
    }

    #[test]
    fn labels_collapse_whitespace_and_enforce_unicode_length() {
        assert_eq!(
            normalize_label("  late\t night  ", 80).as_deref(),
            Some("late night")
        );
        assert!(normalize_label("é".repeat(81), 80).is_none());
    }
}
