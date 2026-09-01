use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::process::Stdio;
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

use crate::search::types::{ProviderSearchErrorCode, SearchCancellation};

pub const YT_DLP_STDOUT_LIMIT: usize = 4 * 1024 * 1024;
pub const YT_DLP_STDERR_LIMIT: usize = 256 * 1024;
pub const YT_DLP_PROCESS_TIMEOUT: Duration = Duration::from_secs(15);
pub const YT_DLP_DOWNLOAD_LINE_LIMIT: usize = 16 * 1024;
pub const YT_DLP_DOWNLOAD_STDERR_RING_LIMIT: usize = 256 * 1024;
pub const YT_DLP_DOWNLOAD_EVENT_CHANNEL_CAPACITY: usize = 128;

const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[cfg(test)]
const CONTROLLED_CHILD_MARKER_ENV: &str = "SPOTDIY_TASK2_CONTROLLED_CHILD";
#[cfg(test)]
const CONTROLLED_CHILD_MARKER_VALUE: &str = "yt-dlp-runner";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YtDlpProcessOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum YtDlpDownloadEvent {
    StdoutLine(String),
    StderrLine(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YtDlpDownloadProcessOutput {
    pub exit_code: Option<i32>,
    pub diagnostic: String,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum YtDlpDownloadProcessError {
    #[error("yt-dlp download could not be started")]
    Spawn,
    #[error("yt-dlp download output could not be read")]
    Read,
    #[error("yt-dlp download stdout line exceeded 16 KiB")]
    StdoutLineTooLong,
    #[error("yt-dlp download stderr line exceeded 16 KiB")]
    StderrLineTooLong,
    #[error("yt-dlp download was cancelled")]
    Cancelled,
    #[error("yt-dlp download exited unsuccessfully")]
    NonZeroExit {
        code: Option<i32>,
        diagnostic: String,
    },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum YtDlpProcessError {
    #[error("yt-dlp could not be started")]
    Spawn,
    #[error("yt-dlp output could not be read")]
    Read,
    #[error("yt-dlp stdout exceeded the size limit")]
    StdoutTooLarge,
    #[error("yt-dlp stderr exceeded the size limit")]
    StderrTooLarge,
    #[error("yt-dlp timed out")]
    Timeout,
    #[error("yt-dlp was cancelled")]
    Cancelled,
    #[error("yt-dlp exited unsuccessfully")]
    NonZeroExit { code: Option<i32>, stderr: String },
}

impl YtDlpProcessError {
    pub fn provider_error_code(&self) -> ProviderSearchErrorCode {
        match self {
            Self::Spawn => ProviderSearchErrorCode::Unavailable,
            Self::Timeout => ProviderSearchErrorCode::Timeout,
            Self::Cancelled => ProviderSearchErrorCode::Cancelled,
            Self::StdoutTooLarge | Self::StderrTooLarge => ProviderSearchErrorCode::InvalidResponse,
            Self::Read | Self::NonZeroExit { .. } => ProviderSearchErrorCode::Failed,
        }
    }
}

pub trait YtDlpProcessRunner: Send + Sync {
    fn run<'a>(
        &'a self,
        executable: &'a str,
        args: &'a [String],
        cancellation: SearchCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<YtDlpProcessOutput, YtDlpProcessError>> + Send + 'a>>;
}

pub trait YtDlpDownloadRunner: Send + Sync {
    fn run_download<'a>(
        &'a self,
        executable: &'a Path,
        args: &'a [String],
        cancellation: SearchCancellation,
        events: mpsc::Sender<YtDlpDownloadEvent>,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<YtDlpDownloadProcessOutput, YtDlpDownloadProcessError>>
                + Send
                + 'a,
        >,
    >;
}

#[derive(Clone, Debug, Default)]
pub struct TokioYtDlpProcessRunner {
    #[cfg(test)]
    controlled_child_marker: bool,
}

impl TokioYtDlpProcessRunner {
    pub const fn stdout_limit(&self) -> usize {
        YT_DLP_STDOUT_LIMIT
    }

    pub const fn stderr_limit(&self) -> usize {
        YT_DLP_STDERR_LIMIT
    }
}

#[cfg(test)]
impl TokioYtDlpProcessRunner {
    fn with_controlled_child_marker() -> Self {
        Self {
            controlled_child_marker: true,
        }
    }
}

impl YtDlpProcessRunner for TokioYtDlpProcessRunner {
    fn run<'a>(
        &'a self,
        executable: &'a str,
        args: &'a [String],
        cancellation: SearchCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<YtDlpProcessOutput, YtDlpProcessError>> + Send + 'a>>
    {
        Box::pin(run_yt_dlp(
            executable,
            args,
            cancellation,
            #[cfg(test)]
            self.controlled_child_marker,
        ))
    }
}

impl YtDlpDownloadRunner for TokioYtDlpProcessRunner {
    fn run_download<'a>(
        &'a self,
        executable: &'a Path,
        args: &'a [String],
        cancellation: SearchCancellation,
        events: mpsc::Sender<YtDlpDownloadEvent>,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<YtDlpDownloadProcessOutput, YtDlpDownloadProcessError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(run_yt_dlp_download(
            executable,
            args,
            cancellation,
            events,
            #[cfg(test)]
            self.controlled_child_marker,
        ))
    }
}

pub fn yt_dlp_search_args(query: &str) -> Vec<String> {
    [
        "--no-config".to_owned(),
        "--dump-single-json".to_owned(),
        "--flat-playlist".to_owned(),
        "--skip-download".to_owned(),
        "--no-warnings".to_owned(),
        "--socket-timeout".to_owned(),
        "10".to_owned(),
        format!("ytsearch25:{query}"),
    ]
    .to_vec()
}

async fn run_yt_dlp(
    executable: &str,
    args: &[String],
    cancellation: SearchCancellation,
    #[cfg(test)] controlled_child_marker: bool,
) -> Result<YtDlpProcessOutput, YtDlpProcessError> {
    let mut cancellation_rx = cancellation.subscribe();
    if *cancellation_rx.borrow() {
        return Err(YtDlpProcessError::Cancelled);
    }

    let mut command = yt_dlp_command(executable, args);
    #[cfg(test)]
    if controlled_child_marker {
        command.env(CONTROLLED_CHILD_MARKER_ENV, CONTROLLED_CHILD_MARKER_VALUE);
    }
    let mut child = command.spawn().map_err(|_| YtDlpProcessError::Spawn)?;
    let stdout = child.stdout.take().ok_or(YtDlpProcessError::Read)?;
    let stderr = child.stderr.take().ok_or(YtDlpProcessError::Read)?;
    let (stdout_tx, mut stdout_rx) = oneshot::channel();
    let (stderr_tx, mut stderr_rx) = oneshot::channel();
    tokio::spawn(async move {
        let _ = stdout_tx.send(read_bounded(stdout, YT_DLP_STDOUT_LIMIT, true).await);
    });
    tokio::spawn(async move {
        let _ = stderr_tx.send(read_bounded(stderr, YT_DLP_STDERR_LIMIT, false).await);
    });

    let mut stdout: Option<Vec<u8>> = None;
    let mut stderr: Option<Vec<u8>> = None;
    let mut exit_status = None;
    let deadline = tokio::time::sleep(YT_DLP_PROCESS_TIMEOUT);
    tokio::pin!(deadline);
    let mut poll = tokio::time::interval(PROCESS_POLL_INTERVAL);

    loop {
        if exit_status.is_none() {
            exit_status = match child.try_wait() {
                Ok(status) => status,
                Err(_) => {
                    terminate_owned_child(&mut child).await;
                    return Err(YtDlpProcessError::Read);
                }
            };
        }
        if exit_status.is_some() && stdout.is_some() && stderr.is_some() {
            let (status, stdout, stderr) = match (exit_status.take(), stdout.take(), stderr.take())
            {
                (Some(status), Some(stdout), Some(stderr)) => (status, stdout, stderr),
                _ => unreachable!("the process and both readers were checked"),
            };
            let output = YtDlpProcessOutput {
                stdout: String::from_utf8_lossy(&stdout).into_owned(),
                stderr: String::from_utf8_lossy(&stderr).into_owned(),
                exit_code: status.code(),
            };
            return if status.success() {
                Ok(output)
            } else {
                Err(YtDlpProcessError::NonZeroExit {
                    code: output.exit_code,
                    stderr: output.stderr,
                })
            };
        }

        tokio::select! {
            result = &mut stdout_rx, if stdout.is_none() => {
                stdout = match result {
                    Ok(Ok(stdout)) => Some(stdout),
                    Ok(Err(error)) => {
                        terminate_owned_child(&mut child).await;
                        return Err(error);
                    }
                    Err(_) => {
                        terminate_owned_child(&mut child).await;
                        return Err(YtDlpProcessError::Read);
                    }
                };
            }
            result = &mut stderr_rx, if stderr.is_none() => {
                stderr = match result {
                    Ok(Ok(stderr)) => Some(stderr),
                    Ok(Err(error)) => {
                        terminate_owned_child(&mut child).await;
                        return Err(error);
                    }
                    Err(_) => {
                        terminate_owned_child(&mut child).await;
                        return Err(YtDlpProcessError::Read);
                    }
                };
            }
            changed = cancellation_rx.changed() => {
                if changed.is_ok() && *cancellation_rx.borrow() {
                    terminate_owned_child(&mut child).await;
                    return Err(YtDlpProcessError::Cancelled);
                }
            }
            _ = &mut deadline => {
                terminate_owned_child(&mut child).await;
                return Err(YtDlpProcessError::Timeout);
            }
            _ = poll.tick() => {}
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DownloadStream {
    Stdout,
    Stderr,
}

enum DownloadReaderEvent {
    Line(DownloadStream, String),
    Finished(DownloadStream, Result<(), YtDlpDownloadProcessError>),
}

async fn run_yt_dlp_download(
    executable: &Path,
    args: &[String],
    cancellation: SearchCancellation,
    events: mpsc::Sender<YtDlpDownloadEvent>,
    #[cfg(test)] controlled_child_marker: bool,
) -> Result<YtDlpDownloadProcessOutput, YtDlpDownloadProcessError> {
    let mut cancellation_rx = cancellation.subscribe();
    if *cancellation_rx.borrow() {
        return Err(YtDlpDownloadProcessError::Cancelled);
    }

    let mut command = yt_dlp_download_command(executable, args);
    #[cfg(test)]
    if controlled_child_marker {
        command.env(CONTROLLED_CHILD_MARKER_ENV, CONTROLLED_CHILD_MARKER_VALUE);
    }
    let mut child = command
        .spawn()
        .map_err(|_| YtDlpDownloadProcessError::Spawn)?;
    let stdout = child.stdout.take().ok_or(YtDlpDownloadProcessError::Read)?;
    let stderr = child.stderr.take().ok_or(YtDlpDownloadProcessError::Read)?;
    let (reader_tx, mut reader_rx) = mpsc::channel(YT_DLP_DOWNLOAD_EVENT_CHANNEL_CAPACITY);
    let stdout_reader = tokio::spawn(read_download_lines(
        stdout,
        DownloadStream::Stdout,
        reader_tx.clone(),
    ));
    let stderr_reader = tokio::spawn(read_download_lines(
        stderr,
        DownloadStream::Stderr,
        reader_tx,
    ));

    let mut stdout_finished = false;
    let mut stderr_finished = false;
    let mut exit_status = None;
    let mut diagnostic = String::new();
    let mut poll = tokio::time::interval(PROCESS_POLL_INTERVAL);

    loop {
        if exit_status.is_none() {
            exit_status = match child.try_wait() {
                Ok(status) => status,
                Err(_) => {
                    terminate_owned_child(&mut child).await;
                    drop(reader_rx);
                    let _ = stdout_reader.await;
                    let _ = stderr_reader.await;
                    return Err(YtDlpDownloadProcessError::Read);
                }
            };
        }

        if exit_status.is_some() && stdout_finished && stderr_finished {
            let status = exit_status.take().expect("exit status was checked");
            let output = YtDlpDownloadProcessOutput {
                exit_code: status.code(),
                diagnostic,
            };
            return if status.success() {
                Ok(output)
            } else {
                Err(YtDlpDownloadProcessError::NonZeroExit {
                    code: output.exit_code,
                    diagnostic: output.diagnostic,
                })
            };
        }

        tokio::select! {
            reader_event = reader_rx.recv() => {
                let Some(reader_event) = reader_event else {
                    terminate_owned_child(&mut child).await;
                    let _ = stdout_reader.await;
                    let _ = stderr_reader.await;
                    return Err(YtDlpDownloadProcessError::Read);
                };
                match reader_event {
                    DownloadReaderEvent::Line(stream, line) => {
                        if stream == DownloadStream::Stderr {
                            append_diagnostic(&mut diagnostic, &line);
                        }
                        let event = match stream {
                            DownloadStream::Stdout => YtDlpDownloadEvent::StdoutLine(line),
                            DownloadStream::Stderr => YtDlpDownloadEvent::StderrLine(line),
                        };
                        if events.send(event).await.is_err() {
                            terminate_owned_child(&mut child).await;
                            drop(reader_rx);
                            let _ = stdout_reader.await;
                            let _ = stderr_reader.await;
                            return Err(YtDlpDownloadProcessError::Read);
                        }
                    }
                    DownloadReaderEvent::Finished(stream, result) => {
                        if let Err(error) = result {
                            terminate_owned_child(&mut child).await;
                            drop(reader_rx);
                            let _ = stdout_reader.await;
                            let _ = stderr_reader.await;
                            return Err(error);
                        }
                        match stream {
                            DownloadStream::Stdout => stdout_finished = true,
                            DownloadStream::Stderr => stderr_finished = true,
                        }
                    }
                }
            }
            changed = cancellation_rx.changed() => {
                if changed.is_ok() && *cancellation_rx.borrow() {
                    terminate_owned_child(&mut child).await;
                    drop(reader_rx);
                    let _ = stdout_reader.await;
                    let _ = stderr_reader.await;
                    return Err(YtDlpDownloadProcessError::Cancelled);
                }
            }
            _ = poll.tick() => {}
        }
    }
}

async fn read_download_lines<R>(
    mut reader: R,
    stream: DownloadStream,
    events: mpsc::Sender<DownloadReaderEvent>,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut line = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 4 * 1024];
    let result = 'read: loop {
        let read = match reader.read(&mut chunk).await {
            Ok(read) => read,
            Err(_) => break Err(YtDlpDownloadProcessError::Read),
        };
        if read == 0 {
            if !line.is_empty() {
                if line.last().copied() == Some(b'\r') {
                    line.pop();
                }
                let value = String::from_utf8_lossy(&line).into_owned();
                if events
                    .send(DownloadReaderEvent::Line(stream, value))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            break Ok(());
        }
        for byte in &chunk[..read] {
            if *byte == b'\n' {
                if line.last().copied() == Some(b'\r') {
                    line.pop();
                }
                let value = String::from_utf8_lossy(&line).into_owned();
                if events
                    .send(DownloadReaderEvent::Line(stream, value))
                    .await
                    .is_err()
                {
                    return;
                }
                line.clear();
            } else {
                if line.len() >= YT_DLP_DOWNLOAD_LINE_LIMIT {
                    break 'read Err(match stream {
                        DownloadStream::Stdout => YtDlpDownloadProcessError::StdoutLineTooLong,
                        DownloadStream::Stderr => YtDlpDownloadProcessError::StderrLineTooLong,
                    });
                }
                line.push(*byte);
            }
        }
    };
    let _ = events
        .send(DownloadReaderEvent::Finished(stream, result))
        .await;
}

fn append_diagnostic(diagnostic: &mut String, line: &str) {
    diagnostic.push_str(line);
    diagnostic.push('\n');
    if diagnostic.len() > YT_DLP_DOWNLOAD_STDERR_RING_LIMIT {
        let excess = diagnostic.len() - YT_DLP_DOWNLOAD_STDERR_RING_LIMIT;
        let split_at = diagnostic
            .char_indices()
            .find_map(|(index, _)| (index >= excess).then_some(index))
            .unwrap_or(diagnostic.len());
        diagnostic.drain(..split_at);
    }
}

fn yt_dlp_command(executable: &str, args: &[String]) -> Command {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

pub fn yt_dlp_download_command(executable: &Path, args: &[String]) -> Command {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

async fn terminate_owned_child(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

async fn read_bounded<R>(
    mut reader: R,
    limit: usize,
    stdout: bool,
) -> Result<Vec<u8>, YtDlpProcessError>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::with_capacity(limit.min(8 * 1024));
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let read = reader
            .read(&mut chunk)
            .await
            .map_err(|_| YtDlpProcessError::Read)?;
        if read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(read) > limit {
            return Err(if stdout {
                YtDlpProcessError::StdoutTooLarge
            } else {
                YtDlpProcessError::StderrTooLarge
            });
        }
        output.extend_from_slice(&chunk[..read]);
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::io::Write;
    use std::path::PathBuf;

    use super::*;
    use tokio::io::AsyncWriteExt;

    fn test_executable() -> PathBuf {
        std::env::current_exe().unwrap()
    }

    fn controlled_test_args(test_name: &'static str) -> Vec<String> {
        ["--exact", test_name, "--nocapture"]
            .into_iter()
            .map(str::to_owned)
            .collect()
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

    #[tokio::test]
    async fn yt_dlp_runner_records_exact_argv_without_shell() {
        let args = yt_dlp_search_args("query");
        let command = yt_dlp_command("C:/yt-dlp.exe", &args);
        assert_eq!(
            command.as_std().get_program(),
            std::ffi::OsStr::new("C:/yt-dlp.exe")
        );
        assert_eq!(
            command
                .as_std()
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "--no-config",
                "--dump-single-json",
                "--flat-playlist",
                "--skip-download",
                "--no-warnings",
                "--socket-timeout",
                "10",
                "ytsearch25:query",
            ]
        );
    }

    #[tokio::test]
    async fn metacharacters_remain_one_argument() {
        let args = yt_dlp_search_args("a & b | c");
        let command = yt_dlp_command("C:/yt-dlp.exe", &args);
        assert_eq!(
            command
                .as_std()
                .get_args()
                .last()
                .unwrap()
                .to_string_lossy(),
            "ytsearch25:a & b | c"
        );
    }

    #[test]
    fn controlled_runner_helpers_require_marker() {
        let test_name = "media_tools::yt_dlp::tests::controlled_runner_blocks";
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

    #[tokio::test]
    async fn runner_bounds_stdout_at_4_mib() {
        let (mut writer, reader) = tokio::io::duplex(8 * 1024);
        let writer_task = tokio::spawn(async move {
            let _ = writer.write_all(&vec![0; YT_DLP_STDOUT_LIMIT + 1]).await;
        });
        assert!(matches!(
            read_bounded(reader, YT_DLP_STDOUT_LIMIT, true).await,
            Err(YtDlpProcessError::StdoutTooLarge)
        ));
        let _ = writer_task.await;
    }

    #[tokio::test]
    async fn runner_bounds_stderr_at_256_kib() {
        let (mut writer, reader) = tokio::io::duplex(8 * 1024);
        let writer_task = tokio::spawn(async move {
            let _ = writer.write_all(&vec![0; YT_DLP_STDERR_LIMIT + 1]).await;
        });
        assert!(matches!(
            read_bounded(reader, YT_DLP_STDERR_LIMIT, false).await,
            Err(YtDlpProcessError::StderrTooLarge)
        ));
        let _ = writer_task.await;
    }

    #[tokio::test]
    async fn runner_cancellation_kills_and_reaps_owned_child() {
        let runner = TokioYtDlpProcessRunner::with_controlled_child_marker();
        let executable = test_executable().to_string_lossy().into_owned();
        let args = controlled_test_args("media_tools::yt_dlp::tests::controlled_runner_blocks");
        let cancellation = SearchCancellation::new();
        let runner_cancellation = cancellation.clone();
        let task =
            tokio::spawn(async move { runner.run(&executable, &args, runner_cancellation).await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        let started = tokio::time::Instant::now();
        cancellation.cancel();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap(),
            Err(YtDlpProcessError::Cancelled)
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn download_reader_rejects_an_oversized_line() {
        let (mut writer, reader) = tokio::io::duplex(YT_DLP_DOWNLOAD_LINE_LIMIT * 2);
        writer
            .write_all(&vec![b'x'; YT_DLP_DOWNLOAD_LINE_LIMIT + 1])
            .await
            .unwrap();
        drop(writer);
        let (events, mut received) = mpsc::channel(2);
        read_download_lines(reader, DownloadStream::Stdout, events).await;
        assert!(matches!(
            received.recv().await,
            Some(DownloadReaderEvent::Finished(
                DownloadStream::Stdout,
                Err(YtDlpDownloadProcessError::StdoutLineTooLong)
            ))
        ));
    }

    #[test]
    fn download_command_keeps_each_argument_structured() {
        let args = vec!["--output".to_owned(), "a & b".to_owned()];
        let command = yt_dlp_download_command(Path::new("C:/yt-dlp.exe"), &args);
        assert_eq!(
            command.as_std().get_program(),
            std::ffi::OsStr::new("C:/yt-dlp.exe")
        );
        assert_eq!(
            command
                .as_std()
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            args
        );
    }

    #[test]
    fn controlled_runner_blocks() {
        if !is_controlled_child("media_tools::yt_dlp::tests::controlled_runner_blocks") {
            return;
        }
        std::io::stdout().write_all(b"started\n").unwrap();
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
