use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OverlayKind {
    Mini,
    Edge,
    Lyrics,
    Gaming,
}

impl OverlayKind {
    pub const ALL: [Self; 4] = [Self::Mini, Self::Edge, Self::Lyrics, Self::Gaming];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Mini => "overlay-mini",
            Self::Edge => "overlay-edge",
            Self::Lyrics => "overlay-lyrics",
            Self::Gaming => "overlay-gaming",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::Mini => "Mini",
            Self::Edge => "Edge",
            Self::Lyrics => "Lyrics",
            Self::Gaming => "Gaming",
        }
    }

    pub const fn dimensions(self) -> (f64, f64, bool) {
        match self {
            Self::Mini => (420.0, 110.0, false),
            Self::Edge => (380.0, 72.0, false),
            Self::Lyrics => (640.0, 220.0, true),
            Self::Gaming => (420.0, 96.0, false),
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
        let mut builder = WebviewWindowBuilder::new(
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
        if kind == OverlayKind::Edge {
            if let Some((x, y)) = self.edge_position() {
                builder = builder.position(x, y);
            }
        }

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

    fn edge_position(&self) -> Option<(f64, f64)> {
        let main = self.app.get_webview_window("main");
        let monitor = main.as_ref().and_then(|window| {
            window.current_monitor().ok().flatten().or_else(|| {
                window
                    .available_monitors()
                    .ok()
                    .and_then(|monitors| monitors.into_iter().next())
            })
        })?;
        let work_area = monitor.work_area();
        Some((
            f64::from(work_area.position.x + work_area.size.width as i32 - 380 - 12),
            f64::from(work_area.position.y + 12),
        ))
    }
}

pub fn edge_position_for_work_area(
    work_area_x: i32,
    work_area_y: i32,
    work_area_width: u32,
) -> (i32, i32) {
    (
        work_area_x + work_area_width as i32 - 380 - 12,
        work_area_y + 12,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_labels_and_dimensions_are_frozen() {
        assert_eq!(OverlayKind::Mini.label(), "overlay-mini");
        assert_eq!(OverlayKind::Edge.label(), "overlay-edge");
        assert_eq!(OverlayKind::Lyrics.label(), "overlay-lyrics");
        assert_eq!(OverlayKind::Gaming.label(), "overlay-gaming");
        assert_eq!(OverlayKind::Mini.dimensions(), (420.0, 110.0, false));
        assert_eq!(OverlayKind::Lyrics.dimensions(), (640.0, 220.0, true));
    }

    #[test]
    fn edge_position_uses_work_area_and_twelve_pixel_margin() {
        assert_eq!(edge_position_for_work_area(100, 20, 1920), (1628, 32));
    }
}
