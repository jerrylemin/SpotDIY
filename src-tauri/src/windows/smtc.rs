use std::sync::Arc;
#[cfg(windows)]
use std::thread::JoinHandle;

use serde::{Deserialize, Serialize};
use tauri::WebviewWindow;

#[cfg(windows)]
use crate::playback::PlaybackPhase;
use crate::playback::PlaybackSnapshot;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SmtcStatus {
    Ready,
    Disabled,
    #[default]
    Unsupported,
    Failed,
}

#[cfg(windows)]
struct Runtime {
    bridge: spotdiy_windows_smtc::SmtcBridge,
    media_worker: Option<JoinHandle<()>>,
}

pub struct SmtcController {
    status: SmtcStatus,
    detail: Option<String>,
    #[cfg(windows)]
    runtime: Option<Runtime>,
}

impl Default for SmtcController {
    fn default() -> Self {
        Self {
            status: SmtcStatus::Unsupported,
            detail: None,
            #[cfg(windows)]
            runtime: None,
        }
    }
}

impl SmtcController {
    pub fn status(&self) -> SmtcStatus {
        self.status
    }

    pub fn detail(&self) -> Option<String> {
        self.detail.clone()
    }

    pub fn fail(&mut self, detail: impl Into<String>) {
        self.status = SmtcStatus::Failed;
        self.detail = Some(detail.into());
    }

    pub fn start(
        &mut self,
        main_window: Option<&WebviewWindow>,
        media_handler: Arc<dyn Fn(MediaCommand) + Send + Sync>,
    ) -> Result<(), String> {
        #[cfg(windows)]
        {
            let window = main_window.ok_or_else(|| "the main window is unavailable".to_owned())?;
            let hwnd = window
                .hwnd()
                .map_err(|error| format!("could not read the main window handle: {error}"))?
                .0 as isize;
            let (bridge, media_rx) =
                spotdiy_windows_smtc::SmtcBridge::start(hwnd).map_err(|error| error.to_string())?;
            let worker = std::thread::Builder::new()
                .name("spotdiy-smtc-command-forwarder".to_owned())
                .spawn(move || {
                    while let Ok(command) = media_rx.recv() {
                        media_handler(map_media_command(command));
                    }
                })
                .map_err(|_| "the SMTC command forwarder could not start".to_owned())?;
            self.runtime = Some(Runtime {
                bridge,
                media_worker: Some(worker),
            });
            self.status = SmtcStatus::Ready;
            self.detail = None;
            Ok(())
        }

        #[cfg(not(windows))]
        {
            let _ = (main_window, media_handler);
            self.status = SmtcStatus::Unsupported;
            self.detail = Some(
                "Windows System Media Transport Controls are only available on Windows".to_owned(),
            );
            Err(self.detail.clone().unwrap_or_default())
        }
    }

    pub fn disable(&mut self) {
        #[cfg(windows)]
        if let Some(mut runtime) = self.runtime.take() {
            let _ = runtime.bridge.shutdown();
            if let Some(worker) = runtime.media_worker.take() {
                let _ = worker.join();
            }
        }
        self.status = SmtcStatus::Disabled;
        self.detail = None;
    }

    pub fn update(&mut self, snapshot: &PlaybackSnapshot) -> Result<(), String> {
        #[cfg(windows)]
        {
            let Some(runtime) = self.runtime.as_ref() else {
                return Ok(());
            };
            let update = spotdiy_windows_smtc::SmtcUpdate {
                title: snapshot.title.clone(),
                artists: snapshot.artists.clone(),
                album: snapshot.album.clone(),
                status: playback_status(snapshot),
            };
            if let Err(error) = runtime.bridge.update(update) {
                let detail = error.to_string();
                self.status = SmtcStatus::Failed;
                self.detail = Some(detail.clone());
                return Err(detail);
            }
        }
        let _ = snapshot;
        Ok(())
    }

    pub fn shutdown(&mut self) {
        #[cfg(windows)]
        if let Some(mut runtime) = self.runtime.take() {
            let _ = runtime.bridge.shutdown();
            if let Some(worker) = runtime.media_worker.take() {
                let _ = worker.join();
            }
        }
        self.status = SmtcStatus::Disabled;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaCommand {
    Play,
    Pause,
    Next,
    Previous,
}

#[cfg(windows)]
fn map_media_command(command: spotdiy_windows_smtc::MediaCommand) -> MediaCommand {
    match command {
        spotdiy_windows_smtc::MediaCommand::Play => MediaCommand::Play,
        spotdiy_windows_smtc::MediaCommand::Pause => MediaCommand::Pause,
        spotdiy_windows_smtc::MediaCommand::Next => MediaCommand::Next,
        spotdiy_windows_smtc::MediaCommand::Previous => MediaCommand::Previous,
    }
}

#[cfg(windows)]
fn playback_status(snapshot: &PlaybackSnapshot) -> spotdiy_windows_smtc::SmtcPlaybackStatus {
    match snapshot.phase {
        PlaybackPhase::Playing | PlaybackPhase::Seeking => {
            spotdiy_windows_smtc::SmtcPlaybackStatus::Playing
        }
        PlaybackPhase::Paused => spotdiy_windows_smtc::SmtcPlaybackStatus::Paused,
        PlaybackPhase::Ended => spotdiy_windows_smtc::SmtcPlaybackStatus::Stopped,
        PlaybackPhase::Idle
        | PlaybackPhase::Loading
        | PlaybackPhase::Recovering
        | PlaybackPhase::Failed
        | PlaybackPhase::ShuttingDown => spotdiy_windows_smtc::SmtcPlaybackStatus::Closed,
    }
}

pub fn media_command_name(command: MediaCommand) -> &'static str {
    match command {
        MediaCommand::Play => "play",
        MediaCommand::Pause => "pause",
        MediaCommand::Next => "next",
        MediaCommand::Previous => "previous",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_commands_are_named_without_platform_details() {
        assert_eq!(media_command_name(MediaCommand::Play), "play");
        assert_eq!(media_command_name(MediaCommand::Pause), "pause");
        assert_eq!(media_command_name(MediaCommand::Next), "next");
        assert_eq!(media_command_name(MediaCommand::Previous), "previous");
    }
}
