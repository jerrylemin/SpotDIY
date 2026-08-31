use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

const MINIMUM_MPV_VERSION: MpvVersion = MpvVersion {
    major: 0,
    minor: 41,
    patch: 0,
};

/// The public health classification used by backend startup and recovery.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaToolHealth {
    Ready,
    Missing,
    Broken,
}

/// Backend-only mpv discovery result. The executable path never crosses the
/// Tauri playback DTO boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MpvToolStatus {
    pub health: MediaToolHealth,
    pub executable: Option<PathBuf>,
    pub version: Option<String>,
    pub detail: Option<String>,
}

/// Compatibility diagnostics consumed by the existing mpv worker. This is
/// deliberately not serialized or exposed to the frontend.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaToolDiagnostic {
    pub status: MediaToolStatus,
    pub version: Option<MpvVersion>,
    pub detail: Option<String>,
    pub recovery_action: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaToolStatus {
    Ready,
    Missing,
    Unsupported,
    Broken,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MpvVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

#[derive(Clone, Debug)]
struct ManagerState {
    status: MpvToolStatus,
    diagnostic: MediaToolDiagnostic,
}

#[derive(Clone)]
pub struct MediaToolManager {
    /// This seam is only used by deterministic Rust tests. Production
    /// discovery is environment override followed by PATH.
    override_path: Option<PathBuf>,
    state: Arc<Mutex<ManagerState>>,
}

impl MediaToolManager {
    pub fn new() -> Self {
        let manager = Self {
            override_path: None,
            state: Arc::new(Mutex::new(initial_state())),
        };
        manager.refresh_mpv();
        manager
    }

    /// Construct a manager with an explicit test candidate. This does not
    /// represent a frontend-selectable executable path.
    pub fn with_override(path: PathBuf) -> Self {
        let manager = Self {
            override_path: Some(path),
            state: Arc::new(Mutex::new(initial_state())),
        };
        manager.refresh_mpv();
        manager
    }

    pub fn refresh_mpv(&self) -> MpvToolStatus {
        let candidate = self
            .override_path
            .clone()
            .or_else(|| env::var_os("SPOTDIY_MPV_PATH").map(PathBuf::from))
            .or_else(find_mpv_on_path);
        let status = match candidate.as_deref() {
            None => missing_status("mpv was not found on PATH"),
            Some(path) if !path.is_file() => missing_status("mpv executable was not found"),
            Some(path) => inspect_mpv(path),
        };
        let diagnostic = diagnostic_from_status(&status);
        if let Ok(mut state) = self.state.lock() {
            state.status = status.clone();
            state.diagnostic = diagnostic;
        }
        status
    }

    pub fn mpv_status(&self) -> MpvToolStatus {
        self.state
            .lock()
            .map(|state| state.status.clone())
            .unwrap_or_else(|_| missing_status("mpv status is unavailable"))
    }

    pub fn require_mpv(&self) -> Result<PathBuf, crate::playback::PlaybackError> {
        let status = self.mpv_status();
        match (status.health, status.executable) {
            (MediaToolHealth::Ready, Some(path)) => Ok(path),
            (MediaToolHealth::Missing, _) => Err(crate::playback::PlaybackError::new(
                crate::playback::PlaybackErrorCode::ToolMissing,
                status
                    .detail
                    .unwrap_or_else(|| "mpv is not installed".to_owned()),
                true,
            )),
            (MediaToolHealth::Broken, _) => Err(crate::playback::PlaybackError::new(
                crate::playback::PlaybackErrorCode::ToolBroken,
                status
                    .detail
                    .unwrap_or_else(|| "mpv is not usable".to_owned()),
                true,
            )),
            (MediaToolHealth::Ready, None) => Err(crate::playback::PlaybackError::new(
                crate::playback::PlaybackErrorCode::ToolBroken,
                "mpv was reported ready without an executable",
                true,
            )),
        }
    }

    /// Existing backend compatibility accessor. New code should use
    /// `mpv_status` instead.
    pub fn refresh(&mut self) -> MediaToolDiagnostic {
        self.refresh_mpv();
        self.health()
    }

    pub fn health(&self) -> MediaToolDiagnostic {
        self.state
            .lock()
            .map(|state| state.diagnostic.clone())
            .unwrap_or_else(|_| initial_state().diagnostic)
    }

    pub fn mpv_path(&self) -> Option<PathBuf> {
        self.mpv_status().executable
    }
}

impl Default for MediaToolManager {
    fn default() -> Self {
        Self::new()
    }
}

fn initial_state() -> ManagerState {
    let status = missing_status("mpv was not checked yet");
    ManagerState {
        diagnostic: diagnostic_from_status(&status),
        status,
    }
}

fn missing_status(detail: &str) -> MpvToolStatus {
    MpvToolStatus {
        health: MediaToolHealth::Missing,
        executable: None,
        version: None,
        detail: Some(detail.to_owned()),
    }
}

fn diagnostic_from_status(status: &MpvToolStatus) -> MediaToolDiagnostic {
    let (legacy_status, version) = match status.health {
        MediaToolHealth::Ready => (
            MediaToolStatus::Ready,
            status.version.as_deref().and_then(parse_mpv_version),
        ),
        MediaToolHealth::Missing => (MediaToolStatus::Missing, None),
        MediaToolHealth::Broken => {
            let version = status.version.as_deref().and_then(parse_mpv_version);
            let legacy_status = version
                .filter(|version| *version < MINIMUM_MPV_VERSION)
                .map_or(MediaToolStatus::Broken, |_| MediaToolStatus::Unsupported);
            (legacy_status, version)
        }
    };
    MediaToolDiagnostic {
        status: legacy_status,
        version,
        detail: status.detail.clone(),
        recovery_action: Some(match status.health {
            MediaToolHealth::Ready => "Retry the playback backend".to_owned(),
            MediaToolHealth::Missing => "Install mpv or set SPOTDIY_MPV_PATH".to_owned(),
            MediaToolHealth::Broken => "Install a working mpv release".to_owned(),
        }),
    }
}

fn find_mpv_on_path() -> Option<PathBuf> {
    let path_entries = env::var_os("PATH")?;
    find_mpv_in_paths(&path_entries)
}

fn find_mpv_in_paths(path_entries: &std::ffi::OsStr) -> Option<PathBuf> {
    for directory in env::split_paths(path_entries) {
        for file_name in executable_names() {
            let candidate = directory.join(file_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(windows)]
fn executable_names() -> &'static [&'static str] {
    &["mpv.exe", "mpv"]
}

#[cfg(not(windows))]
fn executable_names() -> &'static [&'static str] {
    &["mpv"]
}

fn inspect_mpv(path: &Path) -> MpvToolStatus {
    let output = match Command::new(path).arg("--version").output() {
        Ok(output) => output,
        Err(_) => {
            return MpvToolStatus {
                health: MediaToolHealth::Broken,
                executable: None,
                version: None,
                detail: Some("mpv could not be started".to_owned()),
            }
        }
    };
    let output_text = String::from_utf8_lossy(&output.stdout);
    let version = parse_mpv_version(&output_text);
    if !output.status.success() {
        return MpvToolStatus {
            health: MediaToolHealth::Broken,
            executable: None,
            version: version.map(version_string),
            detail: Some("mpv --version failed".to_owned()),
        };
    }
    let Some(version) = version else {
        return MpvToolStatus {
            health: MediaToolHealth::Broken,
            executable: None,
            version: None,
            detail: Some("mpv --version did not report a recognizable version".to_owned()),
        };
    };
    let supported = version >= MINIMUM_MPV_VERSION;
    MpvToolStatus {
        health: if supported {
            MediaToolHealth::Ready
        } else {
            MediaToolHealth::Broken
        },
        executable: supported.then(|| path.to_path_buf()),
        version: Some(version_string(version)),
        detail: supported.then_some(()).map_or_else(
            || {
                Some(format!(
                    "mpv {}.{}.{} is older than the supported 0.41.0 release",
                    version.major, version.minor, version.patch
                ))
            },
            |_| None,
        ),
    }
}

fn version_string(version: MpvVersion) -> String {
    format!("{}.{}.{}", version.major, version.minor, version.patch)
}

pub(crate) fn parse_mpv_version(output: &str) -> Option<MpvVersion> {
    output.split_whitespace().find_map(parse_version_token)
}

fn parse_version_token(token: &str) -> Option<MpvVersion> {
    let token =
        token.trim_start_matches(|character: char| !character.is_ascii_digit() && character != 'v');
    let token = token.strip_prefix('v').unwrap_or(token);
    let mut components = token.splitn(3, '.');
    let major = components.next()?.parse().ok()?;
    let minor = components.next()?.parse().ok()?;
    let patch = components
        .next()?
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()?;
    Some(MpvVersion {
        major,
        minor,
        patch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_release_version_with_development_suffix() {
        assert_eq!(
            parse_mpv_version("mpv v0.41.0-dev-g1234567 (C) 2000-2026"),
            Some(MpvVersion {
                major: 0,
                minor: 41,
                patch: 0,
            })
        );
    }

    #[test]
    fn versions_have_total_ordering() {
        assert!(
            MpvVersion {
                major: 0,
                minor: 41,
                patch: 0,
            } < MpvVersion {
                major: 1,
                minor: 0,
                patch: 0,
            }
        );
    }

    #[test]
    fn versions_below_minimum_are_broken() {
        let status = MpvToolStatus {
            health: MediaToolHealth::Broken,
            executable: None,
            version: Some("0.40.0".to_owned()),
            detail: Some("unsupported".to_owned()),
        };
        assert_eq!(status.health, MediaToolHealth::Broken);
        assert_eq!(
            diagnostic_from_status(&status).status,
            MediaToolStatus::Unsupported
        );
    }

    #[test]
    fn path_lookup_only_checks_path_entries() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join(executable_names()[0]);
        std::fs::File::create(&executable).unwrap();
        let path_entries = env::join_paths([directory.path()]).unwrap();

        assert_eq!(find_mpv_in_paths(&path_entries), Some(executable));
    }

    #[test]
    fn missing_tool_does_not_expose_an_executable() {
        let manager = MediaToolManager::with_override(PathBuf::from(r"C:\SpotDIY\mpv.exe"));
        let status = manager.mpv_status();
        assert_eq!(status.health, MediaToolHealth::Missing);
        assert!(status.executable.is_none());
        assert!(!status
            .detail
            .unwrap_or_default()
            .contains(r"C:\SpotDIY\mpv.exe"));
    }
}
