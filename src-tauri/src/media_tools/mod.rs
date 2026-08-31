use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[path = "../sources/yt_dlp.rs"]
pub mod yt_dlp;

const MINIMUM_MPV_VERSION: MpvVersion = MpvVersion {
    major: 0,
    minor: 41,
    patch: 0,
};
const MPV_VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const MPV_VERSION_PROBE_POLL: Duration = Duration::from_millis(10);
const MPV_VERSION_PROBE_OUTPUT_LIMIT: usize = 64 * 1024;
const YT_DLP_VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const YT_DLP_VERSION_PROBE_OUTPUT_LIMIT: usize = 64 * 1024;
const MINIMUM_YT_DLP_VERSION: YtDlpVersion = YtDlpVersion {
    year: 2026,
    month: 8,
    day: 19,
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

/// Backend-only yt-dlp discovery result. The executable path is retained for
/// provider processes and never crosses the IPC boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YtDlpToolStatus {
    pub status: crate::search::types::ProviderRuntimeStatus,
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct YtDlpVersion {
    pub year: u64,
    pub month: u64,
    pub day: u64,
}

#[derive(Clone, Debug)]
struct ManagerState {
    mpv_status: MpvToolStatus,
    yt_dlp_status: YtDlpToolStatus,
    diagnostic: MediaToolDiagnostic,
}

#[derive(Clone)]
pub struct MediaToolManager {
    /// This seam is only used by deterministic Rust tests. Production
    /// discovery is environment override followed by PATH.
    override_path: Option<PathBuf>,
    /// This seam is only used by deterministic Rust tests. Production
    /// discovery is environment override followed by PATH.
    yt_dlp_override_path: Option<PathBuf>,
    state: Arc<Mutex<ManagerState>>,
}

impl MediaToolManager {
    pub fn new() -> Self {
        let manager = Self {
            override_path: None,
            yt_dlp_override_path: None,
            state: Arc::new(Mutex::new(initial_state())),
        };
        manager.refresh_mpv();
        manager.refresh_yt_dlp();
        manager
    }

    /// Construct a manager with an explicit test candidate. This does not
    /// represent a frontend-selectable executable path.
    pub fn with_override(path: PathBuf) -> Self {
        let manager = Self {
            override_path: Some(path),
            yt_dlp_override_path: None,
            state: Arc::new(Mutex::new(initial_state())),
        };
        manager.refresh_mpv();
        manager.refresh_yt_dlp();
        manager
    }

    /// Construct a manager with an explicit yt-dlp test candidate. This does
    /// not represent a frontend-selectable executable path.
    pub fn with_yt_dlp_override(path: PathBuf) -> Self {
        let manager = Self {
            override_path: None,
            yt_dlp_override_path: Some(path),
            state: Arc::new(Mutex::new(initial_state())),
        };
        manager.refresh_mpv();
        manager.refresh_yt_dlp();
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
            state.mpv_status = status.clone();
            state.diagnostic = diagnostic;
        }
        status
    }

    pub fn mpv_status(&self) -> MpvToolStatus {
        self.state
            .lock()
            .map(|state| state.mpv_status.clone())
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

    pub fn refresh_yt_dlp(&self) -> YtDlpToolStatus {
        let candidate = choose_yt_dlp_candidate(
            self.yt_dlp_override_path.clone(),
            env::var_os("SPOTDIY_YTDLP_PATH").map(PathBuf::from),
            find_yt_dlp_on_path(),
        );
        let status = match candidate.as_deref() {
            None => missing_yt_dlp_status("yt-dlp was not found on PATH"),
            Some(path) if !path.is_file() => {
                missing_yt_dlp_status("yt-dlp executable was not found")
            }
            Some(path) => inspect_yt_dlp(path),
        };
        if let Ok(mut state) = self.state.lock() {
            state.yt_dlp_status = status.clone();
        }
        status
    }

    pub fn yt_dlp_status(&self) -> YtDlpToolStatus {
        self.state
            .lock()
            .map(|state| state.yt_dlp_status.clone())
            .unwrap_or_else(|_| missing_yt_dlp_status("yt-dlp status is unavailable"))
    }

    pub fn require_yt_dlp(&self) -> Result<PathBuf, crate::search::types::ProviderSearchErrorCode> {
        let status = self.yt_dlp_status();
        match (status.status, status.executable) {
            (crate::search::types::ProviderRuntimeStatus::Ready, Some(path)) => Ok(path),
            (crate::search::types::ProviderRuntimeStatus::Missing, _) => {
                Err(crate::search::types::ProviderSearchErrorCode::Unavailable)
            }
            (crate::search::types::ProviderRuntimeStatus::Unsupported, _) => {
                Err(crate::search::types::ProviderSearchErrorCode::Unavailable)
            }
            _ => Err(crate::search::types::ProviderSearchErrorCode::Failed),
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
        mpv_status: status,
        yt_dlp_status: missing_yt_dlp_status("yt-dlp was not checked yet"),
    }
}

fn missing_yt_dlp_status(detail: &str) -> YtDlpToolStatus {
    YtDlpToolStatus {
        status: crate::search::types::ProviderRuntimeStatus::Missing,
        executable: None,
        version: None,
        detail: Some(detail.to_owned()),
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

fn find_yt_dlp_on_path() -> Option<PathBuf> {
    let path_entries = env::var_os("PATH")?;
    find_yt_dlp_in_paths(&path_entries)
}

fn find_yt_dlp_in_paths(path_entries: &std::ffi::OsStr) -> Option<PathBuf> {
    for directory in env::split_paths(path_entries) {
        for file_name in yt_dlp_executable_names() {
            let candidate = directory.join(file_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn choose_yt_dlp_candidate(
    test_override: Option<PathBuf>,
    environment_override: Option<PathBuf>,
    path_candidate: Option<PathBuf>,
) -> Option<PathBuf> {
    test_override.or(environment_override).or(path_candidate)
}

#[cfg(windows)]
fn executable_names() -> &'static [&'static str] {
    &["mpv.exe", "mpv"]
}

#[cfg(windows)]
fn yt_dlp_executable_names() -> &'static [&'static str] {
    &["yt-dlp.exe", "yt-dlp"]
}

#[cfg(not(windows))]
fn yt_dlp_executable_names() -> &'static [&'static str] {
    &["yt-dlp"]
}

#[cfg(not(windows))]
fn executable_names() -> &'static [&'static str] {
    &["mpv"]
}

fn inspect_mpv(path: &Path) -> MpvToolStatus {
    let output = match run_bounded_probe(
        path,
        &["--no-config", "--version"],
        MPV_VERSION_PROBE_TIMEOUT,
        MPV_VERSION_PROBE_OUTPUT_LIMIT,
        MPV_VERSION_PROBE_OUTPUT_LIMIT,
    ) {
        Ok(output) => output,
        Err(error) => return broken_mpv_status(probe_error_detail("mpv", error)),
    };
    let mut output_text = String::from_utf8_lossy(&output.stdout).into_owned();
    output_text.push('\n');
    output_text.push_str(&String::from_utf8_lossy(&output.stderr));
    let version = parse_mpv_version(&output_text);
    if !output.success {
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

fn inspect_yt_dlp(path: &Path) -> YtDlpToolStatus {
    match run_bounded_probe(
        path,
        &["--version"],
        YT_DLP_VERSION_PROBE_TIMEOUT,
        YT_DLP_VERSION_PROBE_OUTPUT_LIMIT,
        YT_DLP_VERSION_PROBE_OUTPUT_LIMIT,
    ) {
        Ok(output) => {
            inspect_yt_dlp_output_bytes(path, &output.stdout, &output.stderr, output.success)
        }
        Err(error) => YtDlpToolStatus {
            status: crate::search::types::ProviderRuntimeStatus::Broken,
            executable: None,
            version: None,
            detail: Some(probe_error_detail("yt-dlp", error)),
        },
    }
}

fn inspect_yt_dlp_output_bytes(
    path: &Path,
    stdout: &[u8],
    stderr: &[u8],
    success: bool,
) -> YtDlpToolStatus {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    inspect_yt_dlp_output(path, &stdout, &stderr, success)
}

fn inspect_yt_dlp_output(
    path: &Path,
    stdout: &str,
    stderr: &str,
    success: bool,
) -> YtDlpToolStatus {
    let mut output = stdout.to_owned();
    output.push('\n');
    output.push_str(stderr);
    let version = parse_yt_dlp_version(&output);
    if !success {
        return YtDlpToolStatus {
            status: crate::search::types::ProviderRuntimeStatus::Broken,
            executable: None,
            version: version.map(yt_dlp_version_string),
            detail: Some("yt-dlp --version failed".to_owned()),
        };
    }
    let Some(version) = version else {
        return YtDlpToolStatus {
            status: crate::search::types::ProviderRuntimeStatus::Broken,
            executable: None,
            version: None,
            detail: Some("yt-dlp --version did not report a recognizable version".to_owned()),
        };
    };
    let supported = version >= MINIMUM_YT_DLP_VERSION;
    YtDlpToolStatus {
        status: if supported {
            crate::search::types::ProviderRuntimeStatus::Ready
        } else {
            crate::search::types::ProviderRuntimeStatus::Unsupported
        },
        executable: supported.then(|| path.to_path_buf()),
        version: Some(yt_dlp_version_string(version)),
        detail: supported.then_some(()).map_or_else(
            || {
                Some(format!(
                    "yt-dlp {}.{}.{} is older than the supported 2026.08.19 release",
                    version.year, version.month, version.day
                ))
            },
            |_| None,
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundedProbeError {
    Spawn,
    Read,
    StdoutTooLarge,
    StderrTooLarge,
    Timeout,
    Wait,
}

struct BoundedProbeOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    success: bool,
}

fn run_bounded_probe(
    executable: &Path,
    args: &[&str],
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedProbeOutput, BoundedProbeError> {
    run_bounded_probe_with_environment(executable, args, timeout, stdout_limit, stderr_limit, None)
}

fn run_bounded_probe_with_environment(
    executable: &Path,
    args: &[&str],
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    child_environment: Option<(&str, &str)>,
) -> Result<BoundedProbeOutput, BoundedProbeError> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some((name, value)) = child_environment {
        command.env(name, value);
    }
    let mut child = command.spawn().map_err(|_| BoundedProbeError::Spawn)?;
    let stdout_rx = child
        .stdout
        .take()
        .map(|pipe| spawn_probe_output_reader(pipe, stdout_limit, true));
    let stderr_rx = child
        .stderr
        .take()
        .map(|pipe| spawn_probe_output_reader(pipe, stderr_limit, false));
    let deadline = Instant::now() + timeout;
    let mut stdout = None;
    let mut stderr = None;
    let mut status = None;

    loop {
        if status.is_none() {
            status = match child.try_wait() {
                Ok(status) => status,
                Err(_) => {
                    terminate_probe_child(&mut child);
                    return Err(BoundedProbeError::Wait);
                }
            };
        }
        if stdout.is_none() {
            stdout = match receive_probe_output(stdout_rx.as_ref()) {
                Ok(stdout) => stdout,
                Err(error) => {
                    terminate_probe_child(&mut child);
                    return Err(error);
                }
            };
        }
        if stderr.is_none() {
            stderr = match receive_probe_output(stderr_rx.as_ref()) {
                Ok(stderr) => stderr,
                Err(error) => {
                    terminate_probe_child(&mut child);
                    return Err(error);
                }
            };
        }
        if status.is_some() && stdout.is_some() && stderr.is_some() {
            let (status, stdout, stderr) = match (status.take(), stdout.take(), stderr.take()) {
                (Some(status), Some(stdout), Some(stderr)) => (status, stdout, stderr),
                _ => unreachable!("the process and both readers were checked"),
            };
            return Ok(BoundedProbeOutput {
                stdout,
                stderr,
                success: status.success(),
            });
        }
        if Instant::now() >= deadline {
            terminate_probe_child(&mut child);
            return Err(BoundedProbeError::Timeout);
        }
        thread::sleep(MPV_VERSION_PROBE_POLL);
    }
}

fn spawn_probe_output_reader<R: Read + Send + 'static>(
    mut pipe: R,
    limit: usize,
    stdout: bool,
) -> mpsc::Receiver<Result<Vec<u8>, BoundedProbeError>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut output = Vec::with_capacity(4 * 1024);
        let mut chunk = [0_u8; 8 * 1024];
        let result = loop {
            let read = match pipe.read(&mut chunk) {
                Ok(read) => read,
                Err(_) => break Err(BoundedProbeError::Read),
            };
            if read == 0 {
                break Ok(output);
            }
            if output.len().saturating_add(read) > limit {
                break Err(if stdout {
                    BoundedProbeError::StdoutTooLarge
                } else {
                    BoundedProbeError::StderrTooLarge
                });
            }
            output.extend_from_slice(&chunk[..read]);
        };
        let _ = sender.send(result);
    });
    receiver
}

fn receive_probe_output(
    receiver: Option<&mpsc::Receiver<Result<Vec<u8>, BoundedProbeError>>>,
) -> Result<Option<Vec<u8>>, BoundedProbeError> {
    let Some(receiver) = receiver else {
        return Ok(Some(Vec::new()));
    };
    match receiver.try_recv() {
        Ok(output) => output.map(Some),
        Err(mpsc::TryRecvError::Empty) => Ok(None),
        Err(mpsc::TryRecvError::Disconnected) => Err(BoundedProbeError::Read),
    }
}

fn probe_error_detail(tool: &str, error: BoundedProbeError) -> String {
    match error {
        BoundedProbeError::Spawn => format!("{tool} could not be started"),
        BoundedProbeError::Read => format!("{tool} --version output could not be read"),
        BoundedProbeError::StdoutTooLarge | BoundedProbeError::StderrTooLarge => {
            format!("{tool} --version output exceeded the size limit")
        }
        BoundedProbeError::Timeout => format!("{tool} --version timed out"),
        BoundedProbeError::Wait => format!("{tool} --version could not be checked"),
    }
}

fn terminate_probe_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn broken_mpv_status(detail: String) -> MpvToolStatus {
    MpvToolStatus {
        health: MediaToolHealth::Broken,
        executable: None,
        version: None,
        detail: Some(detail),
    }
}

fn version_string(version: MpvVersion) -> String {
    format!("{}.{}.{}", version.major, version.minor, version.patch)
}

fn yt_dlp_version_string(version: YtDlpVersion) -> String {
    format!("{}.{}.{:02}", version.year, version.month, version.day)
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

fn parse_yt_dlp_version(output: &str) -> Option<YtDlpVersion> {
    output
        .split_whitespace()
        .find_map(parse_yt_dlp_version_token)
}

fn parse_yt_dlp_version_token(token: &str) -> Option<YtDlpVersion> {
    let token = token.trim_start_matches(|character: char| !character.is_ascii_digit());
    let mut components = token.splitn(3, '.');
    let year = components.next()?.parse().ok()?;
    let month = components.next()?.parse().ok()?;
    let day = components
        .next()?
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()?;
    (month > 0 && month <= 12 && day > 0 && day <= 31).then_some(YtDlpVersion { year, month, day })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::io::Write;

    const CONTROLLED_PROBE_TIMEOUT: Duration = Duration::from_millis(100);
    const CONTROLLED_CHILD_MARKER_ENV: &str = "SPOTDIY_TASK2_CONTROLLED_CHILD";
    const CONTROLLED_CHILD_MARKER_VALUE: &str = "media-tools-probe";

    fn test_executable() -> PathBuf {
        std::env::current_exe().unwrap()
    }

    fn controlled_test_args(test_name: &'static str) -> [&'static str; 3] {
        ["--exact", test_name, "--nocapture"]
    }

    fn is_controlled_child(test_name: &str) -> bool {
        let args = std::env::args().collect::<Vec<_>>();
        controlled_child_is_authorized(
            &args,
            std::env::var_os(CONTROLLED_CHILD_MARKER_ENV).as_deref(),
            test_name,
        )
    }

    fn controlled_child_is_authorized(
        args: &[String],
        marker: Option<&OsStr>,
        test_name: &str,
    ) -> bool {
        marker == Some(OsStr::new(CONTROLLED_CHILD_MARKER_VALUE))
            && args.iter().any(|argument| argument == "--exact")
            && args.iter().any(|argument| argument == test_name)
    }

    fn run_controlled_probe(
        test_name: &'static str,
    ) -> Result<BoundedProbeOutput, BoundedProbeError> {
        let executable = test_executable();
        run_bounded_probe_with_environment(
            &executable,
            &controlled_test_args(test_name),
            CONTROLLED_PROBE_TIMEOUT,
            MPV_VERSION_PROBE_OUTPUT_LIMIT,
            MPV_VERSION_PROBE_OUTPUT_LIMIT,
            Some((CONTROLLED_CHILD_MARKER_ENV, CONTROLLED_CHILD_MARKER_VALUE)),
        )
    }

    #[test]
    fn yt_dlp_path_override_has_priority() {
        assert_eq!(
            choose_yt_dlp_candidate(
                Some(PathBuf::from("test-yt-dlp")),
                Some(PathBuf::from("environment-yt-dlp")),
                Some(PathBuf::from("path-yt-dlp")),
            ),
            Some(PathBuf::from("test-yt-dlp"))
        );
    }

    #[test]
    fn controlled_probe_helpers_require_marker() {
        let test_name = "media_tools::tests::controlled_probe_blocks";
        let args = vec![
            "--exact".to_owned(),
            test_name.to_owned(),
            "--nocapture".to_owned(),
        ];
        assert!(!controlled_child_is_authorized(&args, None, test_name));
        assert!(!controlled_child_is_authorized(
            &args,
            Some(OsStr::new("wrong-marker")),
            test_name
        ));
        assert!(controlled_child_is_authorized(
            &args,
            Some(OsStr::new(CONTROLLED_CHILD_MARKER_VALUE)),
            test_name
        ));
    }

    #[test]
    fn yt_dlp_missing_status_has_no_executable() {
        let manager = MediaToolManager::with_yt_dlp_override(PathBuf::from("missing-yt-dlp"));
        assert!(manager.yt_dlp_status().executable.is_none());
    }

    #[test]
    fn yt_dlp_version_below_minimum_is_unsupported() {
        let status = inspect_yt_dlp_output(Path::new("yt-dlp"), "2026.08.18", "", true);
        assert_eq!(
            status.status,
            crate::search::types::ProviderRuntimeStatus::Unsupported
        );
    }

    #[test]
    fn bounded_probe_rejects_oversized_stdout() {
        assert!(matches!(
            run_controlled_probe("media_tools::tests::controlled_probe_overflows_stdout"),
            Err(BoundedProbeError::StdoutTooLarge)
        ));
    }

    #[test]
    fn bounded_probe_rejects_oversized_stderr() {
        assert!(matches!(
            run_controlled_probe("media_tools::tests::controlled_probe_overflows_stderr"),
            Err(BoundedProbeError::StderrTooLarge)
        ));
    }

    #[test]
    fn bounded_probe_times_out_and_reaps() {
        let started = Instant::now();
        assert!(matches!(
            run_controlled_probe("media_tools::tests::controlled_probe_blocks"),
            Err(BoundedProbeError::Timeout)
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn bounded_probe_handles_invalid_utf8() {
        let output =
            run_controlled_probe("media_tools::tests::controlled_probe_writes_invalid_utf8")
                .unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains('\u{fffd}'));
        let status = inspect_yt_dlp_output_bytes(
            Path::new("yt-dlp"),
            &output.stdout,
            &output.stderr,
            output.success,
        );
        assert_eq!(
            status.status,
            crate::search::types::ProviderRuntimeStatus::Ready
        );
    }

    #[test]
    fn bounded_probe_rejects_malformed_version() {
        let output =
            run_controlled_probe("media_tools::tests::controlled_probe_writes_malformed_version")
                .unwrap();
        let status = inspect_yt_dlp_output_bytes(
            Path::new("yt-dlp"),
            &output.stdout,
            &output.stderr,
            output.success,
        );
        assert_eq!(
            status.status,
            crate::search::types::ProviderRuntimeStatus::Broken
        );
    }

    #[test]
    fn bounded_probe_rejects_nonzero_exit() {
        let output =
            run_controlled_probe("media_tools::tests::controlled_probe_exits_nonzero").unwrap();
        let status = inspect_yt_dlp_output_bytes(
            Path::new("yt-dlp"),
            &output.stdout,
            &output.stderr,
            output.success,
        );
        assert_eq!(
            status.status,
            crate::search::types::ProviderRuntimeStatus::Broken
        );
    }

    #[test]
    fn controlled_probe_overflows_stdout() {
        if !is_controlled_child("media_tools::tests::controlled_probe_overflows_stdout") {
            return;
        }
        let mut stdout = std::io::stdout().lock();
        for _ in 0..9 {
            stdout.write_all(&[b'x'; 8 * 1024]).unwrap();
            stdout.flush().unwrap();
        }
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    #[test]
    fn controlled_probe_overflows_stderr() {
        if !is_controlled_child("media_tools::tests::controlled_probe_overflows_stderr") {
            return;
        }
        let mut stderr = std::io::stderr().lock();
        for _ in 0..9 {
            stderr.write_all(&[b'x'; 8 * 1024]).unwrap();
            stderr.flush().unwrap();
        }
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    #[test]
    fn controlled_probe_blocks() {
        if !is_controlled_child("media_tools::tests::controlled_probe_blocks") {
            return;
        }
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    #[test]
    fn controlled_probe_writes_invalid_utf8() {
        if is_controlled_child("media_tools::tests::controlled_probe_writes_invalid_utf8") {
            std::io::stdout().write_all(b"2026.08.19\xff").unwrap();
        }
    }

    #[test]
    fn controlled_probe_writes_malformed_version() {
        if is_controlled_child("media_tools::tests::controlled_probe_writes_malformed_version") {
            std::io::stdout().write_all(b"not-a-version").unwrap();
        }
    }

    #[test]
    fn controlled_probe_exits_nonzero() {
        if is_controlled_child("media_tools::tests::controlled_probe_exits_nonzero") {
            std::io::stdout().write_all(b"2026.08.19").unwrap();
            std::process::exit(7);
        }
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
