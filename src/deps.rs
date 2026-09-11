// src/deps.rs

use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Regex pattern matching percentage strings in download progress output (e.g. " 45.2%").
pub static PROGRESS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)?)%").expect("Failed to compile progress regex"));

/// The set of external dependencies Vidown requires to function properly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequiredTool {
    YtDlp,
    Ffmpeg,
    Node,
}

impl RequiredTool {
    pub fn all() -> &'static [RequiredTool] {
        &[RequiredTool::YtDlp, RequiredTool::Ffmpeg, RequiredTool::Node]
    }

    pub fn id(&self) -> &'static str {
        match self {
            RequiredTool::YtDlp => "yt-dlp",
            RequiredTool::Ffmpeg => "ffmpeg",
            RequiredTool::Node => "node",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            RequiredTool::YtDlp => "yt-dlp (Media & metadata extractor)",
            RequiredTool::Ffmpeg => "ffmpeg & ffprobe (Audio/video multiplexer)",
            RequiredTool::Node => "node (JavaScript engine for YouTube n-sig)",
        }
    }
}

/// Discovered location for an external dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolLocation {
    SystemPath(PathBuf),
    LocalBin(PathBuf),
    Missing,
}

/// Status of an individual tool during onboarding/setup.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolSetupStatus {
    Checking,
    Found(String),
    PendingDownload,
    Downloading { percent: f64 },
    Extracting,
    Installed,
    Failed(String),
}

/// Strong-typed events emitted by the background dependency setup worker.
#[derive(Debug, Clone)]
pub enum SetupEvent {
    ToolStatus {
        tool: &'static str,
        status: ToolSetupStatus,
    },
    Progress {
        tool: &'static str,
        percent: f64,
    },
    Complete,
    Error {
        tool: &'static str,
        error: String,
    },
}

/// Returns the platform-specific standard user data bin directory for Vidown.
///
/// - Windows: `%LOCALAPPDATA%\Vidown\bin` (or `%USERPROFILE%\AppData\Local\Vidown\bin`)
/// - Unix/Linux/WSL: `$XDG_DATA_HOME/vidown/bin` or `~/.local/share/vidown/bin`
/// - Override: `VIDOWN_BIN_DIR` environment variable
pub fn get_user_bin_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("VIDOWN_BIN_DIR")
        && !custom.trim().is_empty()
    {
        return PathBuf::from(custom.trim());
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA")
            && !local.trim().is_empty()
        {
            return PathBuf::from(local).join("Vidown").join("bin");
        }
        if let Ok(profile) = std::env::var("USERPROFILE")
            && !profile.trim().is_empty()
        {
            return PathBuf::from(profile)
                .join("AppData")
                .join("Local")
                .join("Vidown")
                .join("bin");
        }
        PathBuf::from(r"C:\Vidown\bin")
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME")
            && !xdg_data.trim().is_empty()
        {
            return PathBuf::from(xdg_data).join("vidown").join("bin");
        }
        if let Ok(home) = std::env::var("HOME")
            && !home.trim().is_empty()
        {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("vidown")
                .join("bin");
        }
        PathBuf::from("/tmp/vidown/bin")
    }
}

/// Returns filename candidates for a binary on the current platform.
pub fn binary_candidates(bin_name: &str) -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        if bin_name.ends_with(".exe") {
            vec![bin_name.to_string()]
        } else {
            vec![
                format!("{}.exe", bin_name),
                bin_name.to_string(),
                format!("{}.cmd", bin_name),
                format!("{}.bat", bin_name),
            ]
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec![bin_name.to_string()]
    }
}

/// Checks if a given path points to an executable file.
pub fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = path.metadata() {
            return meta.permissions().mode() & 0o111 != 0;
        }
        false
    }

    #[cfg(not(unix))]
    {
        true
    }
}

/// Searches system PATH for an executable matching `bin_name`.
pub fn find_in_path(bin_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let candidates = binary_candidates(bin_name);
    for dir in std::env::split_paths(&path_var) {
        for candidate in &candidates {
            let full = dir.join(candidate);
            if is_executable_file(&full) {
                return Some(full);
            }
        }
    }
    None
}

/// Searches the local user data bin directory for an executable matching `bin_name`.
pub fn find_in_user_bin(bin_name: &str) -> Option<PathBuf> {
    let bin_dir = get_user_bin_dir();
    let candidates = binary_candidates(bin_name);
    for candidate in candidates {
        let full = bin_dir.join(candidate);
        if is_executable_file(&full) {
            return Some(full);
        }
    }
    None
}

/// Checks availability of a specific required tool using the hybrid strategy
/// (System PATH first, then local user data bin).
pub fn check_tool(tool: RequiredTool) -> ToolLocation {
    match tool {
        RequiredTool::YtDlp => {
            if let Some(p) = find_in_path("yt-dlp") {
                ToolLocation::SystemPath(p)
            } else if let Some(p) = find_in_user_bin("yt-dlp") {
                ToolLocation::LocalBin(p)
            } else {
                ToolLocation::Missing
            }
        }
        RequiredTool::Ffmpeg => {
            // ffmpeg suite requires both ffmpeg and ffprobe
            let ffmpeg = find_in_path("ffmpeg").or_else(|| find_in_user_bin("ffmpeg"));
            let ffprobe = find_in_path("ffprobe").or_else(|| find_in_user_bin("ffprobe"));
            if let (Some(ff_path), Some(_)) = (ffmpeg, ffprobe) {
                if find_in_path("ffmpeg").is_some() {
                    return ToolLocation::SystemPath(ff_path);
                } else {
                    return ToolLocation::LocalBin(ff_path);
                }
            }

            ToolLocation::Missing
        }
        RequiredTool::Node => {
            if let Some(p) = find_in_path("node") {
                ToolLocation::SystemPath(p)
            } else if let Some(p) = find_in_user_bin("node") {
                ToolLocation::LocalBin(p)
            } else {
                ToolLocation::Missing
            }
        }
    }
}

/// Checks all required tools. Returns `(all_ok, list_of_tools_with_status)`.
pub fn check_all_dependencies() -> (bool, Vec<(RequiredTool, ToolLocation)>) {
    let mut results = Vec::new();
    let mut all_ok = true;

    for &tool in RequiredTool::all() {
        let loc = check_tool(tool);
        if loc == ToolLocation::Missing {
            all_ok = false;
        }
        results.push((tool, loc));
    }

    (all_ok, results)
}

/// Resolves the executable path to use for a binary. Prefers the local user bin
/// directory if present, otherwise returns the binary name for standard PATH lookup.
pub fn resolve_binary(bin_name: &str) -> PathBuf {
    if let Some(path) = find_in_user_bin(bin_name) {
        path
    } else {
        PathBuf::from(bin_name)
    }
}

/// Prepends Vidown's local user data bin directory to the `PATH` environment variable
/// of a `tokio::process::Command`. This ensures child processes (like `yt-dlp`) automatically
/// find companion tools (`ffmpeg`, `ffprobe`, `node`) located in the local user data directory.
pub fn inject_bin_to_command(cmd: &mut Command) {
    let bin_dir = get_user_bin_dir();
    if let Some(old_path) = std::env::var_os("PATH") {
        let mut paths = vec![bin_dir];
        paths.extend(std::env::split_paths(&old_path));
        if let Ok(new_path) = std::env::join_paths(paths) {
            cmd.env("PATH", new_path);
        }
    } else {
        cmd.env("PATH", bin_dir);
    }
}

/// Information needed to fetch and install a binary or archive for a tool.
#[derive(Debug, Clone)]
pub struct ToolDownloadSpec {
    pub tool: RequiredTool,
    pub urls: Vec<&'static str>,
    pub is_archive: bool,
    pub archive_format: Option<&'static str>, // "zip", "tar.xz", etc.
    pub target_binaries: Vec<&'static str>,
}

/// Returns the download specifications for the current OS and architecture.
pub fn get_download_specs() -> Vec<ToolDownloadSpec> {
    let os = std::env::consts::OS;

    match os {
        "windows" => vec![
            ToolDownloadSpec {
                tool: RequiredTool::YtDlp,
                urls: vec![
                    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
                ],
                is_archive: false,
                archive_format: None,
                target_binaries: vec!["yt-dlp.exe"],
            },
            ToolDownloadSpec {
                tool: RequiredTool::Ffmpeg,
                urls: vec![
                    "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
                    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
                    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip",
                ],
                is_archive: true,
                archive_format: Some("zip"),
                target_binaries: vec!["ffmpeg.exe", "ffprobe.exe"],
            },
            ToolDownloadSpec {
                tool: RequiredTool::Node,
                urls: vec![
                    "https://nodejs.org/dist/v20.18.0/win-x64/node.exe",
                ],
                is_archive: false,
                archive_format: None,
                target_binaries: vec!["node.exe"],
            },
        ],
        _ => {
            // Linux and other Unix-like systems (WSL included)
            vec![
                ToolDownloadSpec {
                    tool: RequiredTool::YtDlp,
                    urls: vec![
                        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp",
                    ],
                    is_archive: false,
                    archive_format: None,
                    target_binaries: vec!["yt-dlp"],
                },
                ToolDownloadSpec {
                    tool: RequiredTool::Ffmpeg,
                    urls: vec![
                        "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz",
                        "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz",
                        "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz",
                    ],
                    is_archive: true,
                    archive_format: Some("tar.xz"),
                    target_binaries: vec!["ffmpeg", "ffprobe"],
                },
                ToolDownloadSpec {
                    tool: RequiredTool::Node,
                    urls: vec![
                        "https://nodejs.org/dist/v20.18.0/node-v20.18.0-linux-x64.tar.gz",
                        "https://nodejs.org/dist/v20.18.0/node-v20.18.0-linux-x64.tar.xz",
                    ],
                    is_archive: true,
                    archive_format: Some("tar.gz"),
                    target_binaries: vec!["node"],
                },
            ]
        }
    }
}

/// Recursively searches a directory for a file matching `target_name`.
pub fn find_file_recursive(dir: &Path, target_name: &str) -> Option<PathBuf> {
    find_file_recursive_bounded(dir, target_name, 0, 10)
}

fn find_file_recursive_bounded(
    dir: &Path,
    target_name: &str,
    current_depth: usize,
    max_depth: usize,
) -> Option<PathBuf> {
    if current_depth > max_depth || !dir.is_dir() {
        return None;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    let matches = if cfg!(target_os = "windows") {
                        name_str.eq_ignore_ascii_case(target_name)
                            || name_str.eq_ignore_ascii_case(&format!("{}.exe", target_name))
                    } else {
                        name_str == target_name || name_str == format!("{}.exe", target_name)
                    };
                    if matches {
                        return Some(path);
                    }
                }
            } else if path.is_dir() {
                subdirs.push(path);
            }
        }
        for subdir in subdirs {
            if let Some(found) =
                find_file_recursive_bounded(&subdir, target_name, current_depth + 1, max_depth)
            {
                return Some(found);
            }
        }
    }
    None
}

/// Downloads a file from `url` to `dest_path` using curl while streaming progress percentages
/// through the provided channel sender.
pub async fn download_file_with_curl(
    url: &str,
    dest_path: &Path,
    tool_id: &'static str,
    tx: &mpsc::Sender<SetupEvent>,
) -> Result<(), String> {
    let mut cmd = Command::new("curl");
    cmd.arg("-f")
        .arg("-L")
        .arg("--progress-bar")
        .arg("-o")
        .arg(dest_path)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to spawn curl: {}", e)),
    };

    let stderr = child.stderr.take();
    let tx_progress = tx.clone();
    let mut error_lines = Vec::new();

    if let Some(mut stream) = stderr {
        let mut buffer = [0u8; 1024];
        let mut line_buf = String::new();
        while let Ok(n) = stream.read(&mut buffer).await {
            if n == 0 {
                break;
            }
            for &byte in &buffer[..n] {
                if byte == b'\r' || byte == b'\n' {
                    let trimmed = line_buf.trim();
                    if !trimmed.is_empty() {
                        if let Some(caps) = PROGRESS_RE.captures(trimmed)
                            && let Some(matched) = caps.get(1)
                            && let Ok(pct) = matched.as_str().parse::<f64>()
                        {
                            let _ = tx_progress
                                .send(SetupEvent::Progress {
                                    tool: tool_id,
                                    percent: pct.clamp(0.0, 100.0),
                                })
                                .await;
                        } else if !trimmed.starts_with('#') {
                            error_lines.push(trimmed.to_string());
                        }
                        line_buf.clear();
                    }
                } else {
                    line_buf.push(byte as char);
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Error waiting on curl: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        let detail = if let Some(last) = error_lines.last() {
            format!("curl error: {}", last)
        } else {
            format!("curl failed with exit status: {}", status)
        };
        Err(detail)
    }
}

/// Fallback downloader using wget on Unix-like systems.
#[cfg(not(target_os = "windows"))]
pub async fn download_file_with_wget(
    url: &str,
    dest_path: &Path,
    tool_id: &'static str,
    tx: &mpsc::Sender<SetupEvent>,
) -> Result<(), String> {
    let mut cmd = Command::new("wget");
    cmd.arg("-q")
        .arg("--show-progress")
        .arg("-O")
        .arg(dest_path)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn wget: {}", e))?;
    let stderr = child.stderr.take();
    let tx_progress = tx.clone();

    if let Some(mut stream) = stderr {
        let mut buffer = [0u8; 1024];
        let mut line_buf = String::new();
        while let Ok(n) = stream.read(&mut buffer).await {
            if n == 0 {
                break;
            }
            for &byte in &buffer[..n] {
                if byte == b'\r' || byte == b'\n' {
                    let trimmed = line_buf.trim();
                    if !trimmed.is_empty() {
                        if let Some(caps) = PROGRESS_RE.captures(trimmed)
                            && let Some(matched) = caps.get(1)
                            && let Ok(pct) = matched.as_str().parse::<f64>()
                        {
                            let _ = tx_progress
                                .send(SetupEvent::Progress {
                                    tool: tool_id,
                                    percent: pct.clamp(0.0, 100.0),
                                })
                                .await;
                        }
                        line_buf.clear();
                    }
                } else {
                    line_buf.push(byte as char);
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Error waiting on wget: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("wget failed with exit status: {}", status))
    }
}

/// Fallback downloader using PowerShell on Windows.
#[cfg(target_os = "windows")]
pub async fn download_file_with_powershell(
    url: &str,
    dest_path: &Path,
) -> Result<(), String> {
    let script = format!(
        "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; (New-Object System.Net.WebClient).DownloadFile('{}', '{}')",
        url.replace('\'', "''"),
        dest_path.to_string_lossy().replace('\'', "''")
    );
    let mut cmd = Command::new("powershell");
    cmd.arg("-NoProfile").arg("-Command").arg(&script);
    let status = cmd
        .status()
        .await
        .map_err(|e| format!("Failed to execute PowerShell download: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("PowerShell download failed with exit status: {}", status))
    }
}

/// Downloads a file from `url` to `dest_path` trying `curl` first, then system fallback.
pub async fn download_file(
    url: &str,
    dest_path: &Path,
    tool_id: &'static str,
    tx: &mpsc::Sender<SetupEvent>,
) -> Result<(), String> {
    let curl_res = download_file_with_curl(url, dest_path, tool_id, tx).await;
    if curl_res.is_ok() {
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(()) = download_file_with_wget(url, dest_path, tool_id, tx).await {
            return Ok(());
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(()) = download_file_with_powershell(url, dest_path).await {
            return Ok(());
        }
    }

    curl_res
}

/// Extracts an archive (`.zip`, `.tar.xz`, etc.) into `extract_dir`.
pub async fn extract_archive(
    archive_path: &Path,
    extract_dir: &Path,
    format: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(extract_dir)
        .map_err(|e| format!("Failed to create extract dir: {}", e))?;

    if format == "tar.xz" || format == "tar.gz" || format == "tar" {
        let mut cmd = Command::new("tar");
        cmd.arg("-xf").arg(archive_path).arg("-C").arg(extract_dir);
        if let Ok(status) = cmd.status().await
            && status.success()
        {
            return Ok(());
        }

        // Fallback to python3 with tarfile (standard on Linux/WSL, handles xz/gz/tar without external binaries)
        #[cfg(not(target_os = "windows"))]
        {
            let py_code = format!(
                "import tarfile; tf = tarfile.open('{}'); tf.extractall('{}'); tf.close()",
                archive_path.to_string_lossy().replace('\'', "\\'"),
                extract_dir.to_string_lossy().replace('\'', "\\'")
            );
            let mut py_cmd = Command::new("python3");
            py_cmd.arg("-c").arg(&py_code);
            if let Ok(status) = py_cmd.status().await
                && status.success()
            {
                return Ok(());
            }
        }

        return Err(format!("Failed to extract {} archive with tar or python3", format));
    }

    if format == "zip" {
        // First try system tar (which supports zip on Windows 10/11 and modern Linux)
        let mut cmd = Command::new("tar");
        cmd.arg("-xf").arg(archive_path).arg("-C").arg(extract_dir);
        if let Ok(status) = cmd.status().await
            && status.success()
        {
            return Ok(());
        }

        // Fallback to unzip on Linux
        #[cfg(not(target_os = "windows"))]
        {
            let mut unzip_cmd = Command::new("unzip");
            unzip_cmd.arg("-q").arg(archive_path).arg("-d").arg(extract_dir);
            if let Ok(status) = unzip_cmd.status().await
                && status.success()
            {
                return Ok(());
            }

            // Fallback to python3 zipfile on Linux
            let py_code = format!(
                "import zipfile; zf = zipfile.ZipFile('{}'); zf.extractall('{}'); zf.close()",
                archive_path.to_string_lossy().replace('\'', "\\'"),
                extract_dir.to_string_lossy().replace('\'', "\\'")
            );
            let mut py_cmd = Command::new("python3");
            py_cmd.arg("-c").arg(&py_code);
            if let Ok(status) = py_cmd.status().await
                && status.success()
            {
                return Ok(());
            }

            return Err("Failed to extract zip archive with tar, unzip, or python3".to_string());
        }

        // Fallback to PowerShell Expand-Archive on Windows
        #[cfg(target_os = "windows")]
        {
            let ps_script = format!(
                "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                archive_path.to_string_lossy().replace('\'', "''"),
                extract_dir.to_string_lossy().replace('\'', "''")
            );
            let mut ps_cmd = Command::new("powershell");
            ps_cmd.arg("-NoProfile").arg("-Command").arg(&ps_script);
            let status = ps_cmd
                .status()
                .await
                .map_err(|e| format!("Failed to execute PowerShell Expand-Archive: {}", e))?;
            if !status.success() {
                return Err(format!("Expand-Archive failed with status: {}", status));
            }
            return Ok(());
        }
    }

    Err(format!("Unsupported archive format: {}", format))
}

/// Sets executable permissions on Unix systems (chmod +x / 0o755).
pub fn make_executable(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = path.metadata()?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Downloads, extracts, and installs a single tool according to its specification.
pub async fn install_tool(
    spec: &ToolDownloadSpec,
    bin_dir: &Path,
    tx: &mpsc::Sender<SetupEvent>,
) -> Result<(), String> {
    let tool_id = spec.tool.id();

    let tmp_dir = bin_dir.join(format!(".tmp_setup_{}_{}", tool_id, std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("Failed to create temporary dir: {}", e))?;

    let mut last_err = String::new();
    let mut downloaded = false;

    let ext = if spec.is_archive {
        match spec.archive_format.unwrap_or("zip") {
            "tar.xz" => "tar.xz",
            "tar.gz" => "tar.gz",
            "tar" => "tar",
            _ => "zip",
        }
    } else {
        #[cfg(target_os = "windows")]
        {
            "exe"
        }
        #[cfg(not(target_os = "windows"))]
        {
            "bin"
        }
    };

    for url in &spec.urls {
        let download_path = tmp_dir.join(format!("download_{}.{}", tool_id, ext));

        let _ = tx
            .send(SetupEvent::ToolStatus {
                tool: tool_id,
                status: ToolSetupStatus::Downloading { percent: 0.0 },
            })
            .await;

        match download_file(url, &download_path, tool_id, tx).await {
            Ok(()) => {
                if spec.is_archive {
                    let format = spec.archive_format.unwrap_or("zip");
                    let _ = tx
                        .send(SetupEvent::ToolStatus {
                            tool: tool_id,
                            status: ToolSetupStatus::Extracting,
                        })
                        .await;

                    let extract_dest = tmp_dir.join(format!("extract_{}", tool_id));
                    if let Err(e) = extract_archive(&download_path, &extract_dest, format).await {
                        last_err = format!("Extraction error: {}", e);
                        let _ = std::fs::remove_dir_all(&extract_dest);
                        let _ = std::fs::remove_file(&download_path);
                        continue;
                    }

                    // Move each target binary from extracted folder to bin_dir
                    let mut all_moved = true;
                    for target_bin in &spec.target_binaries {
                        if let Some(src_bin) = find_file_recursive(&extract_dest, target_bin) {
                            let dest_bin = bin_dir.join(target_bin);
                            let _ = std::fs::remove_file(&dest_bin);
                            if let Err(e) = std::fs::copy(&src_bin, &dest_bin) {
                                last_err = format!("Failed to copy binary '{}': {}", target_bin, e);
                                all_moved = false;
                                break;
                            }
                            let _ = make_executable(&dest_bin);
                        } else {
                            last_err =
                                format!("Binary '{}' not found in extracted archive", target_bin);
                            all_moved = false;
                            break;
                        }
                    }

                    let _ = std::fs::remove_dir_all(&extract_dest);
                    let _ = std::fs::remove_file(&download_path);

                    if all_moved {
                        downloaded = true;
                        break;
                    }
                } else {
                    // Single binary executable
                    if let Some(target_bin) = spec.target_binaries.first() {
                        let final_bin = bin_dir.join(target_bin);
                        let _ = std::fs::remove_file(&final_bin);
                        if let Err(e) = std::fs::copy(&download_path, &final_bin) {
                            last_err = format!("Failed to place executable: {}", e);
                            let _ = std::fs::remove_file(&download_path);
                            continue;
                        }
                        let _ = make_executable(&final_bin);
                        let _ = std::fs::remove_file(&download_path);
                        downloaded = true;
                        break;
                    }
                }
            }
            Err(e) => {
                last_err = e;
                let _ = std::fs::remove_file(&download_path);
            }
        }
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);

    if downloaded {
        let _ = tx
            .send(SetupEvent::ToolStatus {
                tool: tool_id,
                status: ToolSetupStatus::Installed,
            })
            .await;
        Ok(())
    } else {
        let err_msg = if last_err.is_empty() {
            "All download attempts failed".to_string()
        } else {
            last_err
        };
        let _ = tx
            .send(SetupEvent::ToolStatus {
                tool: tool_id,
                status: ToolSetupStatus::Failed(err_msg.clone()),
            })
            .await;
        Err(err_msg)
    }
}

/// Spawns an asynchronous background task to detect and install missing dependencies.
pub fn spawn_setup_task(tx: mpsc::Sender<SetupEvent>) {
    tokio::spawn(async move {
        let bin_dir = get_user_bin_dir();
        if let Err(e) = tokio::fs::create_dir_all(&bin_dir).await {
            let _ = tx
                .send(SetupEvent::Error {
                    tool: "system",
                    error: format!(
                        "Failed to create local bin directory '{}': {}",
                        bin_dir.display(),
                        e
                    ),
                })
                .await;
            return;
        }

        let specs = get_download_specs();
        let mut first_error: Option<(&'static str, String)> = None;

        for spec in specs {
            let tool_loc = check_tool(spec.tool);
            let tool_id = spec.tool.id();

            match tool_loc {
                ToolLocation::SystemPath(p) => {
                    let _ = tx
                        .send(SetupEvent::ToolStatus {
                            tool: tool_id,
                            status: ToolSetupStatus::Found(format!(
                                "Found in PATH: {}",
                                p.display()
                            )),
                        })
                        .await;
                }
                ToolLocation::LocalBin(p) => {
                    let _ = tx
                        .send(SetupEvent::ToolStatus {
                            tool: tool_id,
                            status: ToolSetupStatus::Found(format!(
                                "Found in local bin: {}",
                                p.display()
                            )),
                        })
                        .await;
                }
                ToolLocation::Missing => {
                    let _ = tx
                        .send(SetupEvent::ToolStatus {
                            tool: tool_id,
                            status: ToolSetupStatus::PendingDownload,
                        })
                        .await;

                    if let Err(err_msg) = install_tool(&spec, &bin_dir, &tx).await
                        && first_error.is_none()
                    {
                        first_error = Some((tool_id, err_msg));
                    }
                }
            }
        }

        if let Some((tool_id, err_msg)) = first_error {
            let _ = tx
                .send(SetupEvent::Error {
                    tool: tool_id,
                    error: err_msg,
                })
                .await;
        } else {
            let _ = tx.send(SetupEvent::Complete).await;
        }
    });
}
