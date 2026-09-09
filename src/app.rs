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
}

impl DownloadItem {
    pub fn new(id: usize, url: String) -> Self {
        Self {
            id,
            url,
            filename: None,
            track: 1,
            progress: 0.0,
            status: ItemStatus::Queued,
            logs: Vec::new(),
        }
    }
}

/// Information displayed in the full detail / error modal.
#[derive(Debug, Clone)]
pub struct DetailModal {
    pub title: String,
    pub url: String,
    pub status: String,
    pub logs: Vec<String>,
    pub scroll_offset: usize,
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
    /// Flag signaling the main loop to terminate.
    pub should_quit: bool,
    /// Global notification message shown in the status bar (with timestamp or expiry).
    pub status_message: Option<String>,
}

impl App {
    pub fn new() -> Self {
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
            should_quit: false,
            status_message: Some("Ready. Press [i] or [Enter] to enter a URL, [F2] to toggle Vim mode, [q] to quit.".to_string()),
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
        let item = DownloadItem::new(id, url.clone());
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
        if let Some(item) = self.downloads.get(self.selected_download) {
            self.detail_modal = Some(DetailModal {
                title: format!("Download #{}: Details & Logs", item.id),
                url: item.url.clone(),
                status: item.status.label(),
                logs: item.logs.clone(),
                scroll_offset: 0,
            });
        } else {
            self.status_message = Some("No download selected to inspect.".to_string());
        }
    }

    /// Close any open modal dialog.
    pub fn close_modal(&mut self) {
        self.detail_modal = None;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
