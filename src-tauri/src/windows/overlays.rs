use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OverlayKind {
    Mini,
}

impl OverlayKind {
    pub const ALL: [Self; 1] = [Self::Mini];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Mini => "overlay-mini",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::Mini => "Mini",
        }
    }

    pub const fn dimensions(self) -> (f64, f64, bool) {
        match self {
            Self::Mini => (460.0, 88.0, false),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OverlayStatus {
    #[default]
    Closed,
    Open,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlaySnapshot {
    pub kind: OverlayKind,
    pub status: OverlayStatus,
    pub detail: Option<String>,
}

#[derive(Clone)]
pub struct OverlayManager {
    app: AppHandle,
    states: Arc<Mutex<BTreeMap<OverlayKind, OverlaySnapshot>>>,
}

impl OverlayManager {
    pub fn new(app: AppHandle) -> Self {
        let states = OverlayKind::ALL
            .into_iter()
            .map(|kind| {
                (
                    kind,
                    OverlaySnapshot {
                        kind,
                        status: OverlayStatus::Closed,
                        detail: None,
                    },
                )
            })
            .collect();
        Self {
            app,
            states: Arc::new(Mutex::new(states)),
        }
    }

    pub fn snapshots(&self) -> Vec<OverlaySnapshot> {
        let states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        OverlayKind::ALL
            .into_iter()
            .map(|kind| {
                states.get(&kind).cloned().unwrap_or(OverlaySnapshot {
                    kind,
                    status: OverlayStatus::Closed,
                    detail: None,
                })
            })
            .collect()
    }

    pub fn is_open(&self, kind: OverlayKind) -> bool {
        let label = kind.label();
        self.app.get_webview_window(label).is_some()
            || self
                .states
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .get(&kind)
                .is_some_and(|state| state.status == OverlayStatus::Open)
    }

    pub fn open(&self, kind: OverlayKind) -> Result<(), String> {
        if let Some(window) = self.app.get_webview_window(kind.label()) {
            window.show().map_err(|error| error.to_string())?;
            window.set_focus().map_err(|error| error.to_string())?;
            self.mark_open(kind, None);
            return Ok(());
        }

        let (width, height, resizable) = kind.dimensions();
        let builder = WebviewWindowBuilder::new(
            &self.app,
            kind.label(),
            WebviewUrl::App("index.html".into()),
        )
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(true)
        .resizable(resizable)
        .inner_size(width, height);
        let window = match builder.build() {
            Ok(window) => window,
            Err(error) => {
                let detail = format!("could not create {} overlay: {error}", kind.title());
                self.mark_error(kind, detail.clone());
                return Err(detail);
            }
        };
        let manager = self.clone();
        window.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                manager.mark_closed(kind);
            }
        });
        self.mark_open(kind, None);
        Ok(())
    }

    pub fn close(&self, kind: OverlayKind) -> Result<(), String> {
        if let Some(window) = self.app.get_webview_window(kind.label()) {
            window.close().map_err(|error| error.to_string())?;
        }
        self.mark_closed(kind);
        Ok(())
    }

    pub fn toggle(&self, kind: OverlayKind) -> Result<(), String> {
        if self.is_open(kind) {
            self.close(kind)
        } else {
            self.open(kind)
        }
    }

    pub fn close_all(&self) {
        for kind in OverlayKind::ALL {
            let _ = self.close(kind);
        }
    }

    fn mark_open(&self, kind: OverlayKind, detail: Option<String>) {
        let mut states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        states.insert(
            kind,
            OverlaySnapshot {
                kind,
                status: OverlayStatus::Open,
                detail,
            },
        );
    }

    fn mark_closed(&self, kind: OverlayKind) {
        let mut states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        states.insert(
            kind,
            OverlaySnapshot {
                kind,
                status: OverlayStatus::Closed,
                detail: None,
            },
        );
    }

    fn mark_error(&self, kind: OverlayKind, detail: String) {
        let mut states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        states.insert(
            kind,
            OverlaySnapshot {
                kind,
                status: OverlayStatus::Error,
                detail: Some(detail),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_labels_and_dimensions_are_frozen() {
        assert_eq!(OverlayKind::Mini.label(), "overlay-mini");
        assert_eq!(OverlayKind::ALL, [OverlayKind::Mini]);
        assert_eq!(OverlayKind::Mini.dimensions(), (460.0, 88.0, false));
    }
}
