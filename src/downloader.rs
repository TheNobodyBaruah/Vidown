// src/downloader.rs

use crate::events::DownloadEvent;
use regex::Regex;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// Regex pattern matching percentage strings in yt-dlp stdout (e.g., "  45.2%").
pub static PROGRESS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)?)%").expect("Failed to compile progress regex"));

/// Checks if Node.js runtime is available on the system PATH or local user data bin directory.
fn is_node_available() -> bool {
    if crate::deps::check_tool(crate::deps::RequiredTool::Node)
        != crate::deps::ToolLocation::Missing
    {
        return true;
    }
    let node_bin = crate::deps::resolve_binary("node");
    std::process::Command::new(node_bin)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Extracts the base file name from a path string portably across Windows and Linux.
///
/// Handles both forward slash (`/`) and backslash (`\`) separators, surrounding quotes,
/// and Windows drive prefixes (e.g. `C:\path\file.ext` or `C:file.ext`).
pub fn extract_filename_portably(path_str: &str) -> Option<String> {
    let trimmed = path_str.trim().trim_matches(|c| c == '"' || c == '\'');
    if trimmed.is_empty() {
        return None;
    }

    // Split on either unix '/' or windows '\' path separators
    let last_segment = trimmed.rsplit(['/', '\\']).next()?.trim();
    if last_segment.is_empty() {
        return None;
    }

    // A Windows drive prefix without slash (e.g. "C:video.mp4") only applies
    // when the drive prefix is at the start of the entire path string and there are no path separators.
    let has_leading_drive = trimmed.len() >= 2
        && trimmed.as_bytes()[1] == b':'
        && trimmed.as_bytes()[0].is_ascii_alphabetic();
    let has_no_slashes = !trimmed.contains('/') && !trimmed.contains('\\');

    let name = if has_leading_drive && has_no_slashes {
        trimmed[2..].trim()
    } else {
        last_segment
    };

    let name = name.trim_matches(|c| c == '"' || c == '\'').trim();
    if name.is_empty() || name == "." || name == ".." {
        None
    } else {
        Some(name.to_string())
    }
}

/// Extracts a discovered destination or merged filename from a yt-dlp log line.
pub fn parse_ytdlp_filename(line: &str) -> Option<String> {
    if line.contains("has already been downloaded")
        && let Some(prefix) = line.split("has already been downloaded").next()
    {
        let path_str = prefix.trim_start_matches("[download]").trim();
        if let Some(file_name) = extract_filename_portably(path_str) {
            return Some(file_name);
        }
    }

    if line.contains("[Merger]")
        && let Some(dest_part) = line.split("Merging formats into ").nth(1)
        && let Some(file_name) = extract_filename_portably(dest_part)
    {
        return Some(file_name);
    }

    if line.contains("Destination:")
        && let Some(dest) = line.split("Destination:").nth(1)
        && let Some(file_name) = extract_filename_portably(dest)
    {
        return Some(file_name);
    }

    None
}

/// Asynchronously downloads a video using `yt-dlp` while emitting throttled progress
/// events, capturing stderr for error reporting, and handling cancellation.
pub async fn perform_download(
    id: usize,
    url: String,
    output_dir: PathBuf,
    tx: mpsc::Sender<DownloadEvent>,
) {
    // Ensure target output directory exists
    if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
        let _ = tx
            .send(DownloadEvent::Error {
                id,
                error: format!("Failed to create output directory: {}", e),
            })
            .await;
        return;
    }

    let output_template = output_dir.join("%(title)s.%(ext)s");
    let output_str = output_template.to_string_lossy().to_string();

    let yt_dlp_bin = crate::deps::resolve_binary("yt-dlp");
    let mut cmd = Command::new(yt_dlp_bin);
    crate::deps::inject_bin_to_command(&mut cmd);

    // Conditionally include --js-runtime node with graceful fallback
    if is_node_available() {
        cmd.arg("--js-runtime").arg("node");
    }

    cmd.arg("--force-overwrites")
        .arg("--newline")
        .arg("-f")
        .arg("bestvideo[vcodec^=avc]+bestaudio[ext=m4a]/best[ext=mp4]/best")
        .arg("--merge-output-format")
        .arg("mp4")
        .arg("-o")
        .arg(&output_str)
        .arg(&url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            let _ = tx
                .send(DownloadEvent::Error {
                    id,
                    error: format!("Failed to spawn yt-dlp (is it installed?): {}", e),
                })
                .await;
            return;
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Spawn an asynchronous task to continuously capture stderr lines
    let tx_stderr = tx.clone();
    let stderr_handle = tokio::spawn(async move {
        let mut captured_errors = Vec::new();
        if let Some(err_stream) = stderr {
            let mut reader = BufReader::new(err_stream).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                captured_errors.push(line.clone());
                let _ = tx_stderr
                    .send(DownloadEvent::Log { id, message: line })
                    .await;
            }
        }
        captured_errors
    });

    if let Some(out_stream) = stdout {
        let mut reader = BufReader::new(out_stream).lines();
        let mut current_track: u8 = 0;
        let mut last_emitted_time = Instant::now() - Duration::from_millis(300);
        let mut last_emitted_percent: f64 = -1.0;

        while let Ok(Some(line)) = reader.next_line().await {
            // Forward stdout lines to logs
            let _ = tx
                .send(DownloadEvent::Log {
                    id,
                    message: line.clone(),
                })
                .await;

            // Extract filename from any destination, merger, or already-downloaded line
            if let Some(name) = parse_ytdlp_filename(&line) {
                let _ = tx.send(DownloadEvent::Filename { id, name }).await;
            }

            // Check if yt-dlp is starting a new stream/track (e.g. video track 1, audio track 2)
            if line.contains("[download] Destination:") {
                current_track = current_track.saturating_add(1);
                continue;
            }

            // Check if yt-dlp has reached the merger stage
            if line.contains("[Merger]") {
                if tx.send(DownloadEvent::Merging { id }).await.is_err() {
                    let _ = child.kill().await;
                    return;
                }
                continue;
            }

            // Extract download progress percentage using the precompiled regex
            if let Some(captures) = PROGRESS_RE.captures(&line)
                && let Some(matched) = captures.get(1)
                && let Ok(percentage) = matched.as_str().parse::<f64>()
            {
                let track = if current_track == 0 { 1 } else { current_track };

                // Throttling: Emit only if at least 150ms passed or integer changed
                let now = Instant::now();
                let time_elapsed =
                    now.duration_since(last_emitted_time) >= Duration::from_millis(150);
                let whole_percent_changed =
                    (percentage.floor() - last_emitted_percent.floor()).abs() >= 1.0;
                let is_complete = percentage >= 100.0;

                if time_elapsed || whole_percent_changed || is_complete {
                    last_emitted_time = now;
                    last_emitted_percent = percentage;

                    if tx
                        .send(DownloadEvent::Progress {
                            id,
                            track,
                            percent: percentage,
                        })
                        .await
                        .is_err()
                    {
                        // Channel closed (e.g. user quit application), kill child process
                        let _ = child.kill().await;
                        return;
                    }
                }
            }
        }
    }

    let status = match child.wait().await {
        Ok(s) => s,
        Err(e) => {
            let _ = tx
                .send(DownloadEvent::Error {
                    id,
                    error: format!("Error waiting on yt-dlp: {}", e),
                })
                .await;
            return;
        }
    };

    let stderr_lines = stderr_handle.await.unwrap_or_default();

    if status.success() {
        let _ = tx.send(DownloadEvent::Success { id }).await;
    } else {
        let error_summary = if !stderr_lines.is_empty() {
            stderr_lines.join(" | ")
        } else {
            format!("yt-dlp process exited with status: {}", status)
        };
        let _ = tx
            .send(DownloadEvent::Error {
                id,
                error: error_summary,
            })
            .await;
    }
}
