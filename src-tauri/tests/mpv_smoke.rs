#![cfg(windows)]

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use spotdiy_lib::media_tools::MediaToolManager;
use spotdiy_lib::playback::backend::BackendEvent;
use spotdiy_lib::playback::mpv::MpvBackend;
use spotdiy_lib::playback::PlaybackBackend;

#[test]
fn real_mpv_synthetic_wav_smoke() {
    let smoke_enabled = std::env::var("SPOTDIY_REAL_MPV_SMOKE").as_deref() == Ok("1")
        || std::env::var("SPOTDIY_RUN_MPV_SMOKE").as_deref() == Ok("1");
    if !smoke_enabled {
        eprintln!("skipping real mpv smoke; set SPOTDIY_REAL_MPV_SMOKE=1 to run it");
        return;
    }

    let mpv_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the Tauri manifest has a repository parent")
        .join(".tools")
        .join("mpv")
        .join("v0.41.0")
        .join("mpv.exe");
    assert!(
        mpv_path.is_file(),
        "bundled development mpv is missing: {mpv_path:?}"
    );

    let media_dir = tempfile::tempdir().expect("synthetic media directory should exist");
    let media_path = media_dir.path().join("spotdiy-smoke.wav");
    std::fs::write(&media_path, silent_wav(4)).expect("synthetic WAV should be written");

    let manager = MediaToolManager::with_override(mpv_path);
    let status = manager.mpv_status();
    assert!(
        status.executable.is_some(),
        "mpv validation failed: {status:?}"
    );
    let mut backend = MpvBackend::new(manager);
    backend
        .start()
        .expect("mpv should start and complete its IPC health handshake");

    backend
        .load(&media_path)
        .expect("mpv should load the synthetic WAV");
    let loaded_events = backend.poll_events();
    assert!(loaded_events
        .iter()
        .any(|event| matches!(event, BackendEvent::FileLoaded)));
    assert!(loaded_events.iter().any(
        |event| matches!(event, BackendEvent::DurationChanged(Some(duration)) if *duration > 0)
    ));

    let position = wait_for_event(
        &mut backend,
        Duration::from_secs(3),
        |event| matches!(event, BackendEvent::PositionChanged(position) if *position > 0),
    );
    assert!(matches!(position, Some(BackendEvent::PositionChanged(value)) if value > 0));

    backend.pause().expect("pause should be accepted");
    assert!(
        wait_for_event(&mut backend, Duration::from_secs(2), |event| {
            matches!(event, BackendEvent::PauseChanged(true))
        })
        .is_some()
    );
    backend.resume().expect("resume should be accepted");
    assert!(
        wait_for_event(&mut backend, Duration::from_secs(2), |event| {
            matches!(event, BackendEvent::PauseChanged(false))
        })
        .is_some()
    );

    backend.seek(500).expect("absolute seek should be accepted");
    backend.set_volume(42).expect("volume should be accepted");
    backend.set_muted(true).expect("mute should be accepted");
    let devices = backend
        .list_audio_devices()
        .expect("mpv should answer the audio-device-list request");
    assert!(
        !devices.is_empty(),
        "mpv should expose at least its auto device"
    );
    backend.set_muted(false).expect("unmute should be accepted");

    backend
        .resume()
        .expect("the synthetic track should be playing for EOF");
    assert!(
        wait_for_event(&mut backend, Duration::from_secs(8), |event| {
            matches!(
                event,
                BackendEvent::EndFile(spotdiy_lib::playback::EndFileReason::Eof)
            )
        })
        .is_some()
    );

    backend.shutdown().expect("mpv should quit and be reaped");
}

fn wait_for_event(
    backend: &mut MpvBackend,
    timeout: Duration,
    predicate: impl Fn(&BackendEvent) -> bool,
) -> Option<BackendEvent> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(event) = backend
            .poll_events()
            .into_iter()
            .find(|event| predicate(event))
        {
            return Some(event);
        }
        thread::sleep(Duration::from_millis(25));
    }
    None
}

fn silent_wav(seconds: u32) -> Vec<u8> {
    let sample_rate = 8_000_u32;
    let channels = 1_u16;
    let bits_per_sample = 16_u16;
    let data_size = sample_rate * seconds * u32::from(channels) * u32::from(bits_per_sample / 8);
    let mut bytes = Vec::with_capacity(44 + data_size as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(
        &(sample_rate * u32::from(channels) * u32::from(bits_per_sample / 8)).to_le_bytes(),
    );
    bytes.extend_from_slice(&(channels * (bits_per_sample / 8)).to_le_bytes());
    bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    bytes.resize(44 + data_size as usize, 0);
    bytes
}
