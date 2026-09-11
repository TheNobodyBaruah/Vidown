// src/history.rs

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Status of a historical download attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryStatus {
    Completed,
    Failed,
}

impl HistoryStatus {
    pub fn label(&self) -> &'static str {
        match self {
            HistoryStatus::Completed => "Completed",
            HistoryStatus::Failed => "Failed",
        }
    }
}

/// A recorded historical download entry persisted across application sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: usize,
    pub url: String,
    pub title: Option<String>,
    pub file_path: Option<String>,
    pub status: HistoryStatus,
    pub timestamp: String,
    pub error_message: Option<String>,
}

impl HistoryEntry {
    /// Creates a new completed download history entry with the current local timestamp.
    pub fn new_completed(
        id: usize,
        url: String,
        title: Option<String>,
        file_path: Option<String>,
    ) -> Self {
        Self {
            id,
            url,
            title,
            file_path,
            status: HistoryStatus::Completed,
            timestamp: current_timestamp(),
            error_message: None,
        }
    }

    /// Creates a new failed download history entry with the current local timestamp.
    pub fn new_failed(
        id: usize,
        url: String,
        title: Option<String>,
        file_path: Option<String>,
        error_message: String,
    ) -> Self {
        Self {
            id,
            url,
            title,
            file_path,
            status: HistoryStatus::Failed,
            timestamp: current_timestamp(),
            error_message: Some(error_message),
        }
    }
}

/// Formats the current local time as "YYYY-MM-DD HH:MM:SS".
pub fn current_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Loads history entries from a specified JSON file path.
/// Returns an empty vector if the file does not exist or contains invalid JSON.
pub fn load_history_from_path(path: &Path) -> Vec<HistoryEntry> {
    if !path.exists() {
        return Vec::new();
    }
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Saves history entries to a specified JSON file path, formatting with 2-space indentation.
/// Automatically creates parent directories if they do not exist and writes atomically via a temp file.
pub fn save_history_to_path(path: &Path, entries: &[HistoryEntry]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // Atomic write via temporary file to prevent corruption if interrupted
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = path.with_extension(format!("tmp.{}.{}", std::process::id(), count));
    std::fs::write(&tmp_path, &json)?;

    #[cfg(target_os = "windows")]
    {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        if let Err(e) = std::fs::rename(&tmp_path, path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(e);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Err(e) = std::fs::rename(&tmp_path, path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(e);
        }
    }

    Ok(())
}

/// Loads history entries from the standard user configuration path (`history.json`).
pub fn load_history() -> Vec<HistoryEntry> {
    if let Some(path) = crate::config::get_history_path() {
        load_history_from_path(&path)
    } else {
        Vec::new()
    }
}

/// Saves history entries to the standard user configuration path (`history.json`).
pub fn save_history(entries: &[HistoryEntry]) -> std::io::Result<()> {
    let path = crate::config::get_history_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine system user config directory for history",
        )
    })?;
    save_history_to_path(&path, entries)
}

/// Prunes history entries to ensure the total count does not exceed `limit`.
/// Operates in FIFO order: newest entries are assumed to be at index 0, so oldest
/// entries at the tail are truncated.
pub fn prune_history(entries: &mut Vec<HistoryEntry>, limit: usize) {
    if entries.len() > limit {
        entries.truncate(limit);
    }
}

/// Opens a file or its containing folder in the system file explorer.
/// - Windows: `explorer.exe` (using `/select` with correct Windows backslashes and quotes)
/// - macOS: `open`
/// - Linux / Unix: `xdg-open`
pub fn open_in_file_manager(path_str: &str) -> std::io::Result<()> {
    let raw_path = Path::new(path_str);
    let abs_path = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(raw_path)
    } else {
        raw_path.to_path_buf()
    };

    let target = if abs_path.exists() {
        abs_path.clone()
    } else if let Some(parent) = abs_path.parent() {
        if parent.exists() {
            parent.to_path_buf()
        } else {
            PathBuf::from(".")
        }
    } else {
        PathBuf::from(".")
    };

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("explorer");
        if abs_path.is_file() {
            let win_path = abs_path.to_string_lossy().replace('/', "\\");
            cmd.raw_arg(format!("/select,\"{}\"", win_path));
        } else {
            let win_target = target.to_string_lossy().replace('/', "\\");
            cmd.raw_arg(format!("\"{}\"", win_target));
        }
        cmd.spawn()?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        if abs_path.is_file() {
            std::process::Command::new("open")
                .arg("-R")
                .arg(&abs_path)
                .spawn()?;
        } else {
            std::process::Command::new("open").arg(&target).spawn()?;
        }
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let to_open = if target.is_file() {
            target.parent().unwrap_or(&target)
        } else {
            &target
        };
        std::process::Command::new("xdg-open")
            .arg(to_open)
            .spawn()?;
        Ok(())
    }
}
