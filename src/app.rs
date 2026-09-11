// src/app.rs

/// Represents the active keybinding scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputScheme {
    StandardModal,
    Vim,
}

impl InputScheme {
    pub fn name(&self) -> &'static str {
        match self {
            InputScheme::StandardModal => "Modal (Standard)",
            InputScheme::Vim => "Vim",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            InputScheme::StandardModal => "Modal",
            InputScheme::Vim => "Vim",
        }
    }
}

/// Represents the current input mode of the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

/// Status of an individual download item.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemStatus {
    Queued,
    Downloading,
    Merging,
    Completed,
    Failed(String),
}

impl ItemStatus {
    pub fn label(&self) -> String {
        match self {
            ItemStatus::Queued => "Queued".to_string(),
            ItemStatus::Downloading => "Downloading".to_string(),
            ItemStatus::Merging => "Merging (FFmpeg)".to_string(),
            ItemStatus::Completed => "Completed".to_string(),
            ItemStatus::Failed(err) => format!("Failed: {}", err),
        }
    }
}

/// An individual download task in the concurrent manager.
#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub id: usize,
    pub url: String,
    pub filename: Option<String>,
    pub track: u8,
    pub progress: f64,
    pub status: ItemStatus,
    pub logs: Vec<String>,
    pub output_dir: String,
}

impl DownloadItem {
    pub fn new(id: usize, url: String, output_dir: String) -> Self {
        Self {
            id,
            url,
            filename: None,
            track: 1,
            progress: 0.0,
            status: ItemStatus::Queued,
            logs: Vec::new(),
            output_dir,
        }
    }
}

use crate::history::{HistoryEntry, open_in_file_manager};
use std::cell::Cell;

/// Information displayed in the full detail / error modal.
#[derive(Debug, Clone)]
pub struct DetailModal {
    pub title: String,
    pub url: String,
    pub status: String,
    pub output_dir: String,
    pub logs: Vec<String>,
    pub scroll_offset: usize,
}

/// Information and state for the download history inspector modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryModal {
    pub selected: usize,
    pub scroll_offset: Cell<usize>,
}

impl HistoryModal {
    pub fn new() -> Self {
        Self {
            selected: 0,
            scroll_offset: Cell::new(0),
        }
    }

    pub fn next(&mut self, total: usize) {
        if total > 0 {
            if self.selected + 1 < total {
                self.selected += 1;
            } else {
                self.selected = 0;
            }
        }
    }

    pub fn previous(&mut self, total: usize) {
        if total > 0 {
            if self.selected > 0 {
                self.selected -= 1;
            } else {
                self.selected = total - 1;
            }
        }
    }
}

impl Default for HistoryModal {
    fn default() -> Self {
        Self::new()
    }
}

/// Information and input state for the custom download path configuration modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathModal {
    pub input: String,
    pub cursor_position: usize,
    pub scroll_offset: usize,
}

impl PathModal {
    pub fn new(current_path: &str) -> Self {
        let cursor_position = current_path.chars().count();
        Self {
            input: current_path.to_string(),
            cursor_position,
            scroll_offset: 0,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let char_count = self.input.chars().count();
        if self.cursor_position > char_count {
            self.cursor_position = char_count;
        }
        let byte_idx = self
            .input
            .char_indices()
            .nth(self.cursor_position)
            .map(|(i, _)| i)
            .unwrap_or(self.input.len());
        self.input.insert(byte_idx, c);
        self.cursor_position += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor_position > 0 {
            let prev_idx = self.cursor_position - 1;
            if let Some((byte_idx, ch)) = self.input.char_indices().nth(prev_idx) {
                let end_byte = byte_idx + ch.len_utf8();
                self.input.drain(byte_idx..end_byte);
                self.cursor_position -= 1;
            }
        }
    }

    pub fn delete(&mut self) {
        let count = self.input.chars().count();
        if self.cursor_position < count
            && let Some((byte_idx, ch)) = self.input.char_indices().nth(self.cursor_position)
        {
            let end_byte = byte_idx + ch.len_utf8();
            self.input.drain(byte_idx..end_byte);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_position < self.input.chars().count() {
            self.cursor_position += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_position = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_position = self.input.chars().count();
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor_position = 0;
        self.scroll_offset = 0;
    }

    pub fn reset_default(&mut self) {
        self.input = "./downloads".to_string();
        self.cursor_position = self.input.chars().count();
        self.scroll_offset = 0;
    }
}

/// Sanitizes a path string:
/// - Strips accidental surrounding single or double quotes
/// - Trims leading and trailing whitespace
/// - Treats paths literally without tilde (~) expansion
/// - Resets empty inputs to "./downloads"
pub fn sanitize_path(input: &str) -> String {
    let mut trimmed = input.trim();
    while (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        trimmed = &trimmed[1..trimmed.len() - 1];
        trimmed = trimmed.trim();
    }
    if trimmed.is_empty() {
        "./downloads".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Resolves the exact absolute path for a download destination directory and optional filename.
pub fn resolve_exact_path(dir: &str, filename: Option<&str>) -> Option<String> {
    let base_path = match filename {
        Some(name) => std::path::Path::new(dir).join(name),
        None => std::path::PathBuf::from(dir),
    };

    if let Ok(canonical) = std::fs::canonicalize(&base_path) {
        let s = canonical.to_string_lossy().to_string();
        #[cfg(target_os = "windows")]
        let s = s.strip_prefix(r"\\?\").unwrap_or(&s).to_string();
        return Some(s);
    }

    // If the full base_path doesn't exist yet, but dir exists, canonicalize dir and join filename
    if let Ok(canonical_dir) = std::fs::canonicalize(dir) {
        let s = canonical_dir.to_string_lossy().to_string();
        #[cfg(target_os = "windows")]
        let s = s.strip_prefix(r"\\?\").unwrap_or(&s).to_string();
        let target = match filename {
            Some(name) => std::path::PathBuf::from(s).join(name),
            None => std::path::PathBuf::from(s),
        };
        return Some(target.to_string_lossy().to_string());
    }

    if base_path.is_absolute() {
        Some(base_path.to_string_lossy().to_string())
    } else if let Ok(cwd) = std::env::current_dir() {
        Some(cwd.join(&base_path).to_string_lossy().to_string())
    } else {
        Some(base_path.to_string_lossy().to_string())
    }
}

/// The single source of truth for application state.
pub struct App {
    /// The current URL text buffer.
    pub input_buffer: String,
    /// Cursor position (character index) within `input_buffer`.
    pub cursor_position: usize,
    /// Active input mode (Normal vs Editing).
    pub input_mode: InputMode,
    /// Active input keybinding scheme (Standard vs Vim).
    pub input_scheme: InputScheme,
    /// List of concurrent and completed downloads.
    pub downloads: Vec<DownloadItem>,
    /// Selected index in the downloads list.
    pub selected_download: usize,
    /// Counter for generating unique download IDs.
    pub next_download_id: usize,
    /// Target download destination directory.
    pub output_dir: String,
    /// Optional popup modal displaying error/log details.
    pub detail_modal: Option<DetailModal>,
    /// Optional popup modal configuring the download directory path.
    pub path_modal: Option<PathModal>,
    /// Optional popup modal displaying download history.
    pub history_modal: Option<HistoryModal>,
    /// Persisted download history list (FIFO ordered, newest at index 0).
    pub history: Vec<HistoryEntry>,
    /// Maximum number of items in history before FIFO pruning (at least 50).
    pub history_limit: usize,
    /// Whether history should be automatically saved to disk when modified.
    pub persist_history: bool,
    /// Optional custom path for the history file (for isolated testing or custom configuration).
    pub history_path: Option<std::path::PathBuf>,
    /// Flag signaling the main loop to terminate.
    pub should_quit: bool,
    /// Global notification message shown in the status bar (with timestamp or expiry).
    pub status_message: Option<String>,
}

impl App {
    pub fn new() -> Self {
        let initial_dir = crate::config::load_config()
            .map(|p| sanitize_path(&p))
            .unwrap_or_else(|| "./downloads".to_string());
        let history_limit = crate::config::load_history_limit();
        let persist_history = std::env::var("VIDOWN_NO_PERSIST").is_err();
        let mut history = if persist_history {
            crate::history::load_history()
        } else {
            Vec::new()
        };
        crate::history::prune_history(&mut history, history_limit);
        Self {
            input_buffer: String::new(),
            cursor_position: 0,
            input_mode: InputMode::Normal,
            input_scheme: InputScheme::StandardModal,
            downloads: Vec::new(),
            selected_download: 0,
            next_download_id: 1,
            output_dir: initial_dir,
            detail_modal: None,
            path_modal: None,
            history_modal: None,
            history,
            history_limit,
            persist_history,
            history_path: None,
            should_quit: false,
            status_message: Some("Ready. Press [i] to enter URL, [p/F3] to set directory, [g/F4] for history, [F2] for Vim, [q] to quit.".to_string()),
        }
    }

    /// Creates an isolated App instance without disk persistence and with empty history.
    pub fn new_empty() -> Self {
        Self {
            input_buffer: String::new(),
            cursor_position: 0,
            input_mode: InputMode::Normal,
            input_scheme: InputScheme::StandardModal,
            downloads: Vec::new(),
            selected_download: 0,
            next_download_id: 1,
            output_dir: "./downloads".to_string(),
            detail_modal: None,
            path_modal: None,
            history_modal: None,
            history: Vec::new(),
            history_limit: crate::config::DEFAULT_HISTORY_LIMIT,
            persist_history: false,
            history_path: None,
            should_quit: false,
            status_message: None,
        }
    }

    /// Persists the current history entries to disk if persistence is enabled.
    pub fn save_history(&self) -> std::io::Result<()> {
        if !self.persist_history {
            return Ok(());
        }
        if let Some(path) = &self.history_path {
            crate::history::save_history_to_path(path, &self.history)
        } else {
            crate::history::save_history(&self.history)
        }
    }

    /// Toggle between Standard Modal and Vim input schemes.
    pub fn toggle_input_scheme(&mut self) {
        self.input_scheme = match self.input_scheme {
            InputScheme::StandardModal => InputScheme::Vim,
            InputScheme::Vim => InputScheme::StandardModal,
        };
        self.status_message = Some(format!(
            "Switched keybinding scheme to: {}",
            self.input_scheme.name()
        ));
    }

    /// Move download list selection down.
    pub fn next_download(&mut self) {
        if !self.downloads.is_empty() {
            if self.selected_download + 1 < self.downloads.len() {
                self.selected_download += 1;
            } else {
                self.selected_download = 0;
            }
        }
    }

    /// Move download list selection up.
    pub fn previous_download(&mut self) {
        if !self.downloads.is_empty() {
            if self.selected_download > 0 {
                self.selected_download -= 1;
            } else {
                self.selected_download = self.downloads.len() - 1;
            }
        }
    }

    /// Enqueue a URL for download and return the allocated task ID.
    pub fn enqueue_download(&mut self, url: String) -> usize {
        let id = self.next_download_id;
        self.next_download_id += 1;
        let item = DownloadItem::new(id, url.clone(), self.output_dir.clone());
        self.downloads.push(item);
        self.selected_download = self.downloads.len() - 1;
        self.status_message = Some(format!("Started download #{}: {}", id, url));
        id
    }

    /// Submit the URL currently in the input buffer.
    pub fn submit_input(&mut self) -> Option<(usize, String)> {
        let trimmed = self.input_buffer.trim().to_string();
        if trimmed.is_empty() {
            return None;
        }
        self.input_buffer.clear();
        self.cursor_position = 0;
        self.input_mode = InputMode::Normal;
        let id = self.enqueue_download(trimmed.clone());
        Some((id, trimmed))
    }

    /// Update progress for a specific download item.
    pub fn update_progress(&mut self, id: usize, track: u8, percent: f64) {
        if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
            item.status = ItemStatus::Downloading;
            item.track = track;
            item.progress = percent.clamp(0.0, 100.0);
        }
    }

    /// Update merging status for a specific download item.
    pub fn update_merging(&mut self, id: usize) {
        if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
            item.status = ItemStatus::Merging;
            item.progress = 100.0;
        }
    }

    /// Mark a download as completed successfully.
    pub fn update_success(&mut self, id: usize) {
        let mut entry_to_record = None;
        if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
            item.status = ItemStatus::Completed;
            item.progress = 100.0;
            self.status_message = Some(format!("Download #{} completed successfully!", id));

            let file_path = resolve_exact_path(&item.output_dir, item.filename.as_deref());
            entry_to_record = Some(HistoryEntry::new_completed(
                item.id,
                item.url.clone(),
                item.filename.clone(),
                file_path,
            ));
        }
        if let Some(entry) = entry_to_record {
            self.add_history_entry(entry);
        }
    }

    /// Mark a download as failed with error details.
    pub fn update_error(&mut self, id: usize, error: String) {
        let mut entry_to_record = None;
        if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
            item.status = ItemStatus::Failed(error.clone());
            item.logs.push(format!("[ERROR] {}", error));
            self.status_message = Some(format!("Download #{} failed: {}", id, error));

            let file_path = item
                .filename
                .as_deref()
                .and_then(|name| resolve_exact_path(&item.output_dir, Some(name)));
            entry_to_record = Some(HistoryEntry::new_failed(
                item.id,
                item.url.clone(),
                item.filename.clone(),
                file_path,
                error,
            ));
        }
        if let Some(entry) = entry_to_record {
            self.add_history_entry(entry);
        }
    }

    /// Appends a new history entry at index 0 (newest first), prunes oldest if exceeding limit,
    /// and persists to disk.
    pub fn add_history_entry(&mut self, entry: HistoryEntry) {
        self.history.insert(0, entry);
        crate::history::prune_history(&mut self.history, self.history_limit);
        let _ = self.save_history();
    }

    /// Removes the currently selected history entry from the list and disk.
    pub fn delete_selected_history_entry(&mut self) {
        if self.history.is_empty() {
            self.status_message = Some("No history entry to delete.".to_string());
            return;
        }
        let selected = self.history_modal.as_ref().map(|m| m.selected).unwrap_or(0);
        if selected < self.history.len() {
            self.history.remove(selected);
            let _ = self.save_history();
            self.status_message = Some("Deleted entry from download history.".to_string());
            if let Some(modal) = &mut self.history_modal
                && modal.selected >= self.history.len()
                && !self.history.is_empty()
            {
                modal.selected = self.history.len() - 1;
            }
        }
    }

    /// Re-enqueues the URL from the selected history entry, closes the history modal,
    /// and returns the task ID and URL to spawn the worker.
    pub fn retry_selected_history(&mut self) -> Option<(usize, String)> {
        let modal = self.history_modal.as_ref()?;
        let entry = match self.history.get(modal.selected) {
            Some(e) => e,
            None => {
                self.status_message = Some("No history entry to retry.".to_string());
                return None;
            }
        };
        let url = entry.url.clone();
        self.close_history_modal();
        let id = self.enqueue_download(url.clone());
        self.status_message = Some(format!("Retrying download #{}: {}", id, url));
        Some((id, url))
    }

    /// Opens the file or containing folder of the selected history entry in system file explorer.
    pub fn open_selected_history_in_file_manager(&mut self) {
        if self.history.is_empty() {
            self.status_message = Some("No download history entry selected.".to_string());
            return;
        }
        let selected_path = if let Some(modal) = &self.history_modal {
            if let Some(entry) = self.history.get(modal.selected) {
                match &entry.file_path {
                    Some(p) => p.clone(),
                    None => {
                        self.status_message = Some(
                            "No file destination recorded for this history entry.".to_string(),
                        );
                        return;
                    }
                }
            } else {
                return;
            }
        } else {
            return;
        };

        match open_in_file_manager(&selected_path) {
            Ok(()) => {
                self.status_message = Some(format!("Opened file explorer for: {}", selected_path));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to open file explorer: {}", e));
            }
        }
    }

    /// Opens the download history modal, enforcing mutual exclusion with other modals.
    pub fn open_history_modal(&mut self) {
        self.detail_modal = None;
        self.path_modal = None;
        self.history_modal = Some(HistoryModal::new());
    }

    /// Closes the download history modal.
    pub fn close_history_modal(&mut self) {
        self.history_modal = None;
    }

    /// Toggles the download history modal open/closed.
    pub fn toggle_history_modal(&mut self) {
        if self.history_modal.is_some() {
            self.close_history_modal();
        } else {
            self.open_history_modal();
        }
    }

    /// Append a log or stderr line to an item.
    pub fn append_log(&mut self, id: usize, log: String) {
        if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
            // Keep at most 200 lines to prevent unbounded growth
            if item.logs.len() > 200 {
                item.logs.remove(0);
            }
            item.logs.push(log);
        }
    }

    /// Update filename discovered for a download item.
    pub fn update_filename(&mut self, id: usize, filename: String) {
        if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
            item.filename = Some(filename);
        }
    }

    /// Scroll down within the detail modal log viewer.
    pub fn modal_scroll_down(&mut self) {
        if let Some(modal) = &mut self.detail_modal
            && modal.scroll_offset + 1 < modal.logs.len()
        {
            modal.scroll_offset += 1;
        }
    }

    /// Scroll up within the detail modal log viewer.
    pub fn modal_scroll_up(&mut self) {
        if let Some(modal) = &mut self.detail_modal
            && modal.scroll_offset > 0
        {
            modal.scroll_offset -= 1;
        }
    }

    /// Open detailed log/error modal for the currently selected item.
    pub fn open_selected_details(&mut self) {
        if self.path_modal.is_some() || self.history_modal.is_some() {
            return;
        }
        if let Some(item) = self.downloads.get(self.selected_download) {
            self.detail_modal = Some(DetailModal {
                title: format!("Download #{}: Details & Logs", item.id),
                url: item.url.clone(),
                status: item.status.label(),
                output_dir: item.output_dir.clone(),
                logs: item.logs.clone(),
                scroll_offset: 0,
            });
        } else {
            self.status_message = Some("No download selected to inspect.".to_string());
        }
    }

    /// Opens the download path configuration modal, enforcing mutual exclusion with detail_modal and history_modal.
    pub fn open_path_modal(&mut self) {
        self.detail_modal = None;
        self.history_modal = None;
        self.path_modal = Some(PathModal::new(&self.output_dir));
    }

    /// Toggles the download path configuration modal.
    pub fn toggle_path_modal(&mut self) {
        if self.path_modal.is_some() {
            self.cancel_path_modal();
        } else {
            self.open_path_modal();
        }
    }

    /// Commits and saves the directory path from the modal.
    /// Automatically attempts to create the directory via `create_dir_all`.
    /// If creation fails, sets a warning in the status message.
    /// Persists the new path to user configuration.
    pub fn commit_path_modal(&mut self) {
        if let Some(modal) = self.path_modal.take() {
            let clean_path = sanitize_path(&modal.input);
            match std::fs::create_dir_all(&clean_path) {
                Ok(()) => {
                    self.status_message =
                        Some(format!("Download directory set to: {}", clean_path));
                }
                Err(e) => {
                    self.status_message = Some(format!(
                        "Warning: Failed to create directory '{}': {}",
                        clean_path, e
                    ));
                }
            }
            self.output_dir = clean_path.clone();
            let _ = crate::config::save_config(&clean_path);
        }
    }

    /// Cancels path configuration without saving changes.
    pub fn cancel_path_modal(&mut self) {
        self.path_modal = None;
    }

    /// Close any open modal dialog.
    pub fn close_modal(&mut self) {
        self.detail_modal = None;
        self.path_modal = None;
        self.history_modal = None;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
