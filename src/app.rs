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
        if self.cursor_position < count {
            if let Some((byte_idx, ch)) = self.input.char_indices().nth(self.cursor_position) {
                let end_byte = byte_idx + ch.len_utf8();
                self.input.drain(byte_idx..end_byte);
            }
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
            should_quit: false,
            status_message: Some("Ready. Press [i] to enter URL, [p/F3] to set directory, [F2] for Vim, [q] to quit.".to_string()),
        }
    }

    /// Toggle between Standard Modal and Vim input schemes.
    pub fn toggle_input_scheme(&mut self) {
        self.input_scheme = match self.input_scheme {
            InputScheme::StandardModal => InputScheme::Vim,
            InputScheme::Vim => InputScheme::StandardModal,
        };
        self.status_message = Some(format!("Switched keybinding scheme to: {}", self.input_scheme.name()));
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
        if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
            item.status = ItemStatus::Completed;
            item.progress = 100.0;
            self.status_message = Some(format!("Download #{} completed successfully!", id));
        }
    }

    /// Mark a download as failed with error details.
    pub fn update_error(&mut self, id: usize, error: String) {
        if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
            item.status = ItemStatus::Failed(error.clone());
            item.logs.push(format!("[ERROR] {}", error));
            self.status_message = Some(format!("Download #{} failed: {}", id, error));
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
        if self.path_modal.is_some() {
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

    /// Opens the download path configuration modal, enforcing mutual exclusion with detail_modal.
    pub fn open_path_modal(&mut self) {
        self.detail_modal = None;
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
                    self.status_message = Some(format!("Download directory set to: {}", clean_path));
                }
                Err(e) => {
                    self.status_message = Some(format!("Warning: Failed to create directory '{}': {}", clean_path, e));
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
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
