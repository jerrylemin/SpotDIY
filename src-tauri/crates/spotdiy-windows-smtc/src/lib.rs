#![deny(unsafe_op_in_unsafe_fn)]

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use thiserror::Error;
use windows::core::{factory, HSTRING};
use windows::Foundation::TypedEventHandler;
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
    SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::{
    ISystemMediaTransportControlsInterop, RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmtcPlaybackStatus {
    Closed,
    Playing,
    Paused,
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmtcUpdate {
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub status: SmtcPlaybackStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaCommand {
    Play,
    Pause,
    Next,
    Previous,
}

#[derive(Debug, Error)]
pub enum SmtcError {
    #[error("the Windows media-control bridge thread could not start")]
    ThreadStart,
    #[error("the Windows media-control bridge is unavailable: {0}")]
    Unavailable(String),
    #[error("the Windows media-control bridge command channel is closed")]
    ChannelClosed,
}

enum Command {
    Update(SmtcUpdate),
    SetEnabled(bool),
    Shutdown,
}

pub struct SmtcBridge {
    command_tx: Sender<Command>,
    join: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl SmtcBridge {
    pub fn start(hwnd: isize) -> Result<(Self, Receiver<MediaCommand>), SmtcError> {
        let (command_tx, command_rx) = mpsc::channel();
        let (media_tx, media_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("spotdiy-windows-smtc".to_owned())
            .spawn(move || {
                // SAFETY: this dedicated thread owns every WinRT object created below, and the
                // matching uninitialization runs on the same thread before it exits.
                if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
                    let _ = ready_tx.send(Err(error.to_string()));
                    return;
                }

                let controls = match create_controls(hwnd) {
                    Ok(controls) => {
                        let _ = ready_tx.send(Ok(()));
                        controls
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        // SAFETY: RoInitialize succeeded on this thread and no WinRT object is
                        // retained after the failed control acquisition.
                        unsafe { RoUninitialize() };
                        return;
                    }
                };
                run_controls(controls, command_rx, media_tx);
                // SAFETY: this balances the successful RoInitialize call above on the same
                // dedicated thread after all WinRT objects and event handlers are released.
                unsafe { RoUninitialize() };
            })
            .map_err(|_| SmtcError::ThreadStart)?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok((
                Self {
                    command_tx,
                    join: Arc::new(Mutex::new(Some(join))),
                },
                media_rx,
            )),
            Ok(Err(detail)) => {
                let _ = join.join();
                Err(SmtcError::Unavailable(detail))
            }
            Err(_) => {
                let _ = join.join();
                Err(SmtcError::ThreadStart)
            }
        }
    }

    pub fn update(&self, update: SmtcUpdate) -> Result<(), SmtcError> {
        self.command_tx
            .send(Command::Update(update))
            .map_err(|_| SmtcError::ChannelClosed)
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<(), SmtcError> {
        self.command_tx
            .send(Command::SetEnabled(enabled))
            .map_err(|_| SmtcError::ChannelClosed)
    }

    pub fn shutdown(&self) -> Result<(), SmtcError> {
        let _ = self.command_tx.send(Command::Shutdown);
        let mut join = self
            .join
            .lock()
            .map_err(|_| SmtcError::ThreadStart)?;
        if let Some(handle) = join.take() {
            let _ = handle.join();
        }
        Ok(())
    }
}

impl Drop for SmtcBridge {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn create_controls(hwnd: isize) -> windows::core::Result<SystemMediaTransportControls> {
    let interop: ISystemMediaTransportControlsInterop =
        factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
    // SAFETY: `hwnd` is copied from the live desktop Tauri main window and is
    // only consumed on this dedicated thread while the window is alive.
    let controls: SystemMediaTransportControls =
        unsafe { interop.GetForWindow(HWND(hwnd as _))? };
    controls.SetIsEnabled(true)?;
    controls.SetIsPlayEnabled(true)?;
    controls.SetIsPauseEnabled(true)?;
    controls.SetIsPreviousEnabled(true)?;
    controls.SetIsNextEnabled(true)?;
    controls.SetIsStopEnabled(false)?;
    controls.SetIsRecordEnabled(false)?;
    controls.SetIsFastForwardEnabled(false)?;
    controls.SetIsRewindEnabled(false)?;
    controls.SetIsChannelUpEnabled(false)?;
    controls.SetIsChannelDownEnabled(false)?;
    Ok(controls)
}

fn run_controls(
    controls: SystemMediaTransportControls,
    command_rx: Receiver<Command>,
    media_tx: Sender<MediaCommand>,
) {
    let media_tx_for_handler = media_tx.clone();
    let handler = TypedEventHandler::<
        SystemMediaTransportControls,
        SystemMediaTransportControlsButtonPressedEventArgs,
    >::new(move |_, args| {
        let Some(args) = args.as_ref() else {
            return Ok(());
        };
        let command = match args.Button()? {
            button if button == SystemMediaTransportControlsButton::Play => {
                Some(MediaCommand::Play)
            }
            button if button == SystemMediaTransportControlsButton::Pause => {
                Some(MediaCommand::Pause)
            }
            button if button == SystemMediaTransportControlsButton::Next => {
                Some(MediaCommand::Next)
            }
            button if button == SystemMediaTransportControlsButton::Previous => {
                Some(MediaCommand::Previous)
            }
            _ => None,
        };
        if let Some(command) = command {
            let _ = media_tx_for_handler.send(command);
        }
        Ok(())
    });
    let token = controls.ButtonPressed(&handler).ok();

    while let Ok(command) = command_rx.recv() {
        match command {
            Command::Update(update) => {
                let _ = apply_update(&controls, &update);
            }
            Command::SetEnabled(enabled) => {
                let _ = controls.SetIsEnabled(enabled);
            }
            Command::Shutdown => break,
        }
    }

    if let Some(token) = token {
        let _ = controls.RemoveButtonPressed(token);
    }
    let _ = controls.SetIsEnabled(false);
}

fn apply_update(
    controls: &SystemMediaTransportControls,
    update: &SmtcUpdate,
) -> windows::core::Result<()> {
    let status = match update.status {
        SmtcPlaybackStatus::Closed => MediaPlaybackStatus::Closed,
        SmtcPlaybackStatus::Playing => MediaPlaybackStatus::Playing,
        SmtcPlaybackStatus::Paused => MediaPlaybackStatus::Paused,
        SmtcPlaybackStatus::Stopped => MediaPlaybackStatus::Stopped,
    };
    controls.SetPlaybackStatus(status)?;
    let display = controls.DisplayUpdater()?;
    display.SetType(MediaPlaybackType::Music)?;
    let music = display.MusicProperties()?;
    music.SetTitle(&HSTRING::from(update.title.as_deref().unwrap_or("SpotDIY")))?;
    music.SetArtist(&HSTRING::from(update.artists.join(", ")))?;
    music.SetAlbumTitle(&HSTRING::from(update.album.as_deref().unwrap_or("SpotDIY")))?;
    display.Update()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_dto_never_contains_a_file_path_field() {
        let update = SmtcUpdate {
            title: Some("Track".to_owned()),
            artists: vec!["Artist".to_owned()],
            album: Some("Album".to_owned()),
            status: SmtcPlaybackStatus::Paused,
        };
        let debug = format!("{update:?}");
        assert!(!debug.contains("path"));
    }

    #[test]
    fn media_commands_are_stable_and_explicit() {
        assert_eq!(MediaCommand::Play, MediaCommand::Play);
        assert_ne!(MediaCommand::Play, MediaCommand::Pause);
    }
}
