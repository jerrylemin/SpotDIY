use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::oneshot;

use crate::search::types::{ProviderSearchErrorCode, SearchCancellation};

pub const YT_DLP_STDOUT_LIMIT: usize = 4 * 1024 * 1024;
pub const YT_DLP_STDERR_LIMIT: usize = 256 * 1024;
pub const YT_DLP_PROCESS_TIMEOUT: Duration = Duration::from_secs(15);

const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YtDlpProcessOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
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

#[derive(Clone, Debug, Default)]
pub struct TokioYtDlpProcessRunner;

impl TokioYtDlpProcessRunner {
    pub const fn stdout_limit(&self) -> usize {
        YT_DLP_STDOUT_LIMIT
    }

    pub const fn stderr_limit(&self) -> usize {
        YT_DLP_STDERR_LIMIT
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
        Box::pin(run_yt_dlp(executable, args, cancellation))
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
) -> Result<YtDlpProcessOutput, YtDlpProcessError> {
    let mut cancellation_rx = cancellation.subscribe();
    if *cancellation_rx.borrow() {
        return Err(YtDlpProcessError::Cancelled);
    }

    let mut child = yt_dlp_command(executable, args)
        .spawn()
        .map_err(|_| YtDlpProcessError::Spawn)?;
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

fn yt_dlp_command(executable: &str, args: &[String]) -> Command {
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
        std::env::args().any(|argument| argument == "--exact")
            && std::env::args().any(|argument| argument == test_name)
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
        let runner = TokioYtDlpProcessRunner;
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
