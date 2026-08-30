use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

const MINIMUM_MPV_VERSION: MpvVersion = MpvVersion {
    major: 0,
    minor: 41,
    patch: 0,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaToolHealth {
    pub status: MediaToolStatus,
    pub version: Option<MpvVersion>,
    pub detail: Option<String>,
    pub recovery_action: Option<String>,
}

pub struct MediaToolManager {
    override_path: Option<PathBuf>,
    mpv_path: Option<PathBuf>,
    health: MediaToolHealth,
}

impl MediaToolManager {
    pub fn new() -> Self {
        let mut manager = Self {
            override_path: None,
            mpv_path: None,
            health: missing_health("mpv was not checked yet"),
        };
        manager.refresh();
        manager
    }

    pub fn with_override(path: PathBuf) -> Self {
        let mut manager = Self {
            override_path: Some(path),
            mpv_path: None,
            health: missing_health("mpv was not checked yet"),
        };
        manager.refresh();
        manager
    }

    pub fn refresh(&mut self) -> MediaToolHealth {
        let candidate = self
            .override_path
            .clone()
            .or_else(|| env::var_os("SPOTDIY_MPV_PATH").map(PathBuf::from))
            .or_else(find_mpv_on_path);

        let Some(path) = candidate else {
            self.mpv_path = None;
            self.health = missing_health("mpv was not found on PATH");
            return self.health.clone();
        };

        if !path.is_file() {
            self.mpv_path = None;
            self.health = MediaToolHealth {
                status: MediaToolStatus::Missing,
                version: None,
                detail: Some(format!(
                    "mpv executable was not found at {}",
                    path.display()
                )),
                recovery_action: Some(
                    "Install mpv or set SPOTDIY_MPV_PATH to an mpv executable".to_owned(),
                ),
            };
            return self.health.clone();
        }

        self.mpv_path = Some(path.clone());
        self.health = inspect_mpv(&path);
        self.health.clone()
    }

    pub fn health(&self) -> &MediaToolHealth {
        &self.health
    }

    pub fn mpv_path(&self) -> Option<&Path> {
        self.mpv_path.as_deref()
    }
}

impl Default for MediaToolManager {
    fn default() -> Self {
        Self::new()
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

fn inspect_mpv(path: &Path) -> MediaToolHealth {
    let output = match Command::new(path).arg("--version").output() {
        Ok(output) => output,
        Err(error) => {
            return MediaToolHealth {
                status: MediaToolStatus::Broken,
                version: None,
                detail: Some(format!("could not run mpv: {error}")),
                recovery_action: Some("Check the mpv executable and its permissions".to_owned()),
            };
        }
    };

    let output_text = String::from_utf8_lossy(&output.stdout);
    let version = parse_mpv_version(&output_text);
    if !output.status.success() {
        return MediaToolHealth {
            status: MediaToolStatus::Broken,
            version,
            detail: Some(command_failure_detail(&output)),
            recovery_action: Some("Install a working mpv release".to_owned()),
        };
    }

    let Some(version) = version else {
        return MediaToolHealth {
            status: MediaToolStatus::Broken,
            version: None,
            detail: Some("mpv --version did not report a recognizable version".to_owned()),
            recovery_action: Some("Install a supported mpv release".to_owned()),
        };
    };

    health_for_version(version)
}

fn command_failure_detail(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        format!("mpv --version exited with status {}", output.status)
    } else {
        format!("mpv --version failed: {stderr}")
    }
}

fn missing_health(detail: &str) -> MediaToolHealth {
    MediaToolHealth {
        status: MediaToolStatus::Missing,
        version: None,
        detail: Some(detail.to_owned()),
        recovery_action: Some("Install mpv or set SPOTDIY_MPV_PATH".to_owned()),
    }
}

pub(crate) fn parse_mpv_version(output: &str) -> Option<MpvVersion> {
    output.split_whitespace().find_map(parse_version_token)
}

fn health_for_version(version: MpvVersion) -> MediaToolHealth {
    if version < MINIMUM_MPV_VERSION {
        MediaToolHealth {
            status: MediaToolStatus::Unsupported,
            version: Some(version),
            detail: Some(format!(
                "mpv {}.{}.{} is older than the supported 0.41.0 release",
                version.major, version.minor, version.patch
            )),
            recovery_action: Some("Install mpv 0.41.0 or newer".to_owned()),
        }
    } else {
        MediaToolHealth {
            status: MediaToolStatus::Ready,
            version: Some(version),
            detail: None,
            recovery_action: None,
        }
    }
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
    fn injected_path_has_precedence_and_is_not_exposed_in_health() {
        let path = PathBuf::from(r"C:\SpotDIY\mpv.exe");
        let manager = MediaToolManager::with_override(path);

        assert_eq!(manager.health().status, MediaToolStatus::Missing);
        assert!(manager.health().detail.is_some());
        assert_eq!(manager.mpv_path(), None);
    }

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
    fn versions_below_minimum_are_unsupported() {
        let health = health_for_version(MpvVersion {
            major: 0,
            minor: 40,
            patch: 0,
        });

        assert_eq!(health.status, MediaToolStatus::Unsupported);
    }

    #[test]
    fn path_lookup_only_checks_path_entries() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join(executable_names()[0]);
        std::fs::File::create(&executable).unwrap();
        let path_entries = env::join_paths([directory.path()]).unwrap();

        assert_eq!(find_mpv_in_paths(&path_entries), Some(executable));
    }
}
