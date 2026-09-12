// src/events.rs

use crate::app::{App, InputMode, InputScheme};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Strong typed events emitted by background download tasks.
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Progress { id: usize, track: u8, percent: f64 },
    Merging { id: usize },
    Success { id: usize },
    Error { id: usize, error: String },
    Log { id: usize, message: String },
    Filename { id: usize, name: String },
}

/// Unified event stream for the application loop.
#[allow(dead_code)]
#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    Download(DownloadEvent),
}

/// Dispatches key events to mutate the App state according to current mode and scheme.
/// Returns `Some((id, url))` if a download was initiated by submitting the URL.
pub fn handle_key_event(app: &mut App, key: KeyEvent) -> Option<(usize, String)> {
    // Only process Press events (ignore Release/Repeat to prevent duplicate inputs)
    if key.kind != KeyEventKind::Press {
        return None;
    }

    // Global quit with Ctrl+C
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return None;
    }

    // If help modal is currently open, handle help modal keys (works on both Setup and Main screens)
    if app.help_modal.is_some() {
        handle_help_modal_key(app, key);
        return None;
    }

    // If on Setup screen, handle setup-specific keys
    if app.current_screen == crate::app::CurrentScreen::Setup {
        handle_setup_screen_key(app, key);
        return None;
    }

    // Global toggle for persistent help guide modal (F1) - works in Normal and Editing modes
    if key.code == KeyCode::F(1) {
        app.toggle_help_modal();
        return None;
    }

    // Global toggle for history modal (F4) - works in Normal and Editing modes
    if key.code == KeyCode::F(4) {
        app.toggle_history_modal();
        return None;
    }

    // Global toggle for download directory modal (F3) - works in Normal and Editing modes
    if key.code == KeyCode::F(3) {
        app.toggle_path_modal();
        return None;
    }

    // If history modal is currently open, handle history modal keys
    if app.history_modal.is_some() {
        return handle_history_modal_key(app, key);
    }

    // If path modal dialog is currently open, handle modal editing/dismissal
    if app.path_modal.is_some() {
        handle_path_modal_key(app, key);
        return None;
    }

    // If modal dialog is currently open, handle modal dismissal or scrolling
    if app.detail_modal.is_some() {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                app.close_modal();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.modal_scroll_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.modal_scroll_down();
            }
            _ => {}
        }
        return None;
    }

    // Global toggle for keybinding scheme (F2)
    if key.code == KeyCode::F(2) {
        app.toggle_input_scheme();
        return None;
    }

    // Global paste shortcut: Ctrl+V, Ctrl+Shift+V, or Shift+Insert
    let is_paste = (key.modifiers.contains(KeyModifiers::CONTROL)
        && (key.code == KeyCode::Char('v') || key.code == KeyCode::Char('V')))
        || (key.modifiers.contains(KeyModifiers::SHIFT) && key.code == KeyCode::Insert);

    if is_paste {
        if let Some(text) = crate::clipboard::get_clipboard_text() {
            handle_paste_event(app, &text);
        }
        return None;
    }

    match app.input_mode {
        InputMode::Editing => handle_editing_key(app, key),
        InputMode::Normal => handle_normal_key(app, key),
    }
}

/// Handles keys when in Editing mode (typing in the URL input bar).
fn handle_editing_key(app: &mut App, key: KeyEvent) -> Option<(usize, String)> {
    match key.code {
        KeyCode::Enter => {
            // Submits URL and returns (id, url) to spawn background task
            app.submit_input()
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            None
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            let char_count = app.input_buffer.chars().count();
            if app.cursor_position > char_count {
                app.cursor_position = char_count;
            }
            let byte_idx = app
                .input_buffer
                .char_indices()
                .nth(app.cursor_position)
                .map(|(i, _)| i)
                .unwrap_or(app.input_buffer.len());
            app.input_buffer.insert(byte_idx, c);
            app.cursor_position += 1;
            None
        }
        KeyCode::Backspace => {
            if app.cursor_position > 0 {
                let prev_idx = app.cursor_position - 1;
                if let Some((byte_idx, ch)) = app.input_buffer.char_indices().nth(prev_idx) {
                    let end_byte = byte_idx + ch.len_utf8();
                    app.input_buffer.drain(byte_idx..end_byte);
                    app.cursor_position -= 1;
                }
            }
            None
        }
        KeyCode::Delete => {
            let count = app.input_buffer.chars().count();
            if app.cursor_position < count
                && let Some((byte_idx, ch)) =
                    app.input_buffer.char_indices().nth(app.cursor_position)
            {
                let end_byte = byte_idx + ch.len_utf8();
                app.input_buffer.drain(byte_idx..end_byte);
            }
            None
        }
        KeyCode::Left => {
            if app.cursor_position > 0 {
                app.cursor_position -= 1;
            }
            None
        }
        KeyCode::Right => {
            if app.cursor_position < app.input_buffer.len() {
                app.cursor_position += 1;
            }
            None
        }
        KeyCode::Home => {
            app.cursor_position = 0;
            None
        }
        KeyCode::End => {
            app.cursor_position = app.input_buffer.len();
            None
        }
        _ => None,
    }
}

/// Handles keys when in Normal navigation mode.
fn handle_normal_key(app: &mut App, key: KeyEvent) -> Option<(usize, String)> {
    match app.input_scheme {
        InputScheme::StandardModal => match key.code {
            KeyCode::Char('q') => {
                app.should_quit = true;
                None
            }
            KeyCode::Char('i') | KeyCode::Enter => {
                app.input_mode = InputMode::Editing;
                app.cursor_position = app.input_buffer.len();
                None
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.next_download();
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.previous_download();
                None
            }
            KeyCode::Char('e') => {
                app.open_selected_details();
                None
            }
            KeyCode::Char('p') => {
                app.open_path_modal();
                None
            }
            KeyCode::Char('g') | KeyCode::Char('G') => {
                app.open_history_modal();
                None
            }
            KeyCode::Char('?') => {
                app.open_help_modal();
                None
            }
            _ => None,
        },
        InputScheme::Vim => match key.code {
            KeyCode::Char('q') => {
                app.should_quit = true;
                None
            }
            KeyCode::Char('i') => {
                app.input_mode = InputMode::Editing;
                None
            }
            KeyCode::Char('a') => {
                app.input_mode = InputMode::Editing;
                if app.cursor_position < app.input_buffer.len() {
                    app.cursor_position += 1;
                }
                None
            }
            KeyCode::Char('x') => {
                if !app.input_buffer.is_empty() && app.cursor_position < app.input_buffer.len() {
                    app.input_buffer.remove(app.cursor_position);
                }
                None
            }
            KeyCode::Char('0') => {
                app.cursor_position = 0;
                None
            }
            KeyCode::Char('$') => {
                app.cursor_position = app.input_buffer.len();
                None
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.next_download();
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.previous_download();
                None
            }
            KeyCode::Char('e') => {
                app.open_selected_details();
                None
            }
            KeyCode::Char('p') => {
                app.open_path_modal();
                None
            }
            KeyCode::Char('g') | KeyCode::Char('G') => {
                app.open_history_modal();
                None
            }
            KeyCode::Char('?') => {
                app.open_help_modal();
                None
            }
            _ => None,
        },
    }
}

/// Handles keys when the download history modal is open.
fn handle_history_modal_key(app: &mut App, key: KeyEvent) -> Option<(usize, String)> {
    match key.code {
        // Esc, q, g, F4 closes the modal
        KeyCode::Esc
        | KeyCode::Char('q')
        | KeyCode::Char('Q')
        | KeyCode::Char('g')
        | KeyCode::Char('G')
        | KeyCode::F(4) => {
            app.close_history_modal();
            None
        }
        // Navigation: j/Down for next, k/Up for previous
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
            let total = app.history.len();
            if let Some(modal) = &mut app.history_modal {
                modal.next(total);
            }
            None
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
            let total = app.history.len();
            if let Some(modal) = &mut app.history_modal {
                modal.previous(total);
            }
            None
        }
        KeyCode::Home => {
            if let Some(modal) = &mut app.history_modal {
                modal.selected = 0;
            }
            None
        }
        KeyCode::End => {
            if let Some(modal) = &mut app.history_modal
                && !app.history.is_empty()
            {
                modal.selected = app.history.len() - 1;
            }
            None
        }
        // Retry download: 'r' or 'R'
        KeyCode::Char('r') | KeyCode::Char('R') => app.retry_selected_history(),
        // Open file / folder: 'o' or 'O' or Enter
        KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Enter => {
            app.open_selected_history_in_file_manager();
            None
        }
        // Delete history entry: 'd' or 'D'
        KeyCode::Char('d') | KeyCode::Char('D') => {
            app.delete_selected_history_entry();
            None
        }
        _ => None,
    }
}

/// Handles keys when the download path configuration modal is open.
fn handle_path_modal_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('v') | KeyCode::Char('V') => {
                if let Some(text) = crate::clipboard::get_clipboard_text() {
                    handle_paste_event(app, &text);
                }
                return;
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(modal) = &mut app.path_modal {
                    modal.reset_default();
                }
                return;
            }
            KeyCode::Char('u') | KeyCode::Char('U') => {
                if let Some(modal) = &mut app.path_modal {
                    modal.clear();
                }
                return;
            }
            _ => {}
        }
        return;
    }

    if key.modifiers.contains(KeyModifiers::SHIFT) && key.code == KeyCode::Insert {
        if let Some(text) = crate::clipboard::get_clipboard_text() {
            handle_paste_event(app, &text);
        }
        return;
    }

    match key.code {
        KeyCode::Enter => {
            app.commit_path_modal();
        }
        KeyCode::Esc => {
            app.cancel_path_modal();
        }
        KeyCode::Left => {
            if let Some(modal) = &mut app.path_modal {
                modal.move_left();
            }
        }
        KeyCode::Right => {
            if let Some(modal) = &mut app.path_modal {
                modal.move_right();
            }
        }
        KeyCode::Home => {
            if let Some(modal) = &mut app.path_modal {
                modal.move_home();
            }
        }
        KeyCode::End => {
            if let Some(modal) = &mut app.path_modal {
                modal.move_end();
            }
        }
        KeyCode::Backspace => {
            if let Some(modal) = &mut app.path_modal {
                modal.backspace();
            }
        }
        KeyCode::Delete => {
            if let Some(modal) = &mut app.path_modal {
                modal.delete();
            }
        }
        KeyCode::Char(c) => {
            if let Some(modal) = &mut app.path_modal {
                modal.insert_char(c);
            }
        }
        _ => {}
    }
}

/// Handles keys when the application is on the dependency setup / onboarding screen.
fn handle_setup_screen_key(app: &mut App, key: KeyEvent) {
    // Open full-screen persistent Keybindings Guide modal
    if key.code == KeyCode::Char('?') || key.code == KeyCode::F(1) {
        app.open_help_modal();
        return;
    }

    let phase = app.setup_state.as_ref().map(|s| s.phase.clone());
    match phase {
        Some(crate::app::SetupPhase::Complete) => {
            if key.code == KeyCode::Enter {
                app.finish_setup();
                return;
            }
            if key.code == KeyCode::Char('q') || key.code == KeyCode::Char('Q') {
                app.should_quit = true;
                return;
            }
        }
        Some(crate::app::SetupPhase::Error(_)) => match key.code {
            KeyCode::Char('r') | KeyCode::Char('R') => {
                app.retry_setup();
                return;
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                app.finish_setup();
                return;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                app.should_quit = true;
                return;
            }
            _ => {}
        },
        _ => {
            // While downloading/checking, user can still press 'q' to quit
            if key.code == KeyCode::Char('q') || key.code == KeyCode::Char('Q') {
                app.should_quit = true;
                return;
            }
        }
    }

    // Scroll the help/guide on the setup screen
    match key.code {
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
            app.setup_scroll_down();
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
            app.setup_scroll_up();
        }
        KeyCode::PageDown => {
            if let Some(s) = &mut app.setup_state {
                s.help_scroll = s.help_scroll.saturating_add(5);
            }
        }
        KeyCode::PageUp => {
            if let Some(s) = &mut app.setup_state {
                s.help_scroll = s.help_scroll.saturating_sub(5);
            }
        }
        KeyCode::Home => {
            if let Some(s) = &mut app.setup_state {
                s.help_scroll = 0;
            }
        }
        _ => {}
    }
}

/// Handles keys when the persistent help guide modal is open.
fn handle_help_modal_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc
        | KeyCode::Enter
        | KeyCode::Char('q')
        | KeyCode::Char('Q')
        | KeyCode::Char('?')
        | KeyCode::F(1) => {
            app.close_help_modal();
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
            if let Some(m) = &mut app.help_modal {
                m.scroll_down(100);
            }
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
            if let Some(m) = &mut app.help_modal {
                m.scroll_up();
            }
        }
        KeyCode::PageDown => {
            if let Some(m) = &mut app.help_modal {
                m.page_down(100, 5);
            }
        }
        KeyCode::PageUp => {
            if let Some(m) = &mut app.help_modal {
                m.page_up(5);
            }
        }
        KeyCode::Home => {
            if let Some(m) = &mut app.help_modal {
                m.scroll_offset = 0;
            }
        }
        _ => {}
    }
}

/// Handles a paste event (from terminal bracketed paste or keyboard/mouse action).
pub fn handle_paste_event(app: &mut App, text: &str) {
    if let Some(modal) = &mut app.path_modal {
        let sanitized = crate::clipboard::sanitize_clipboard_text(text);
        modal.insert_str(&sanitized);
        return;
    }

    if app.help_modal.is_some() || app.history_modal.is_some() || app.detail_modal.is_some() {
        return;
    }

    if app.current_screen == crate::app::CurrentScreen::Main {
        app.paste_text_to_input(text);
    }
}

/// Handles mouse click, scroll, and drag interactions across the TUI.
pub fn handle_mouse_event(
    app: &mut App,
    mouse: crossterm::event::MouseEvent,
) -> Option<(usize, String)> {
    match mouse.kind {
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
            // Right-click pastes URL or directory from clipboard
            if let Some(text) = crate::clipboard::get_clipboard_text() {
                handle_paste_event(app, &text);
            }
            None
        }
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            // Left-click interaction based on current screen and modals
            if app.help_modal.is_some() || app.detail_modal.is_some() || app.history_modal.is_some()
            {
                return None;
            }

            if app.path_modal.is_some() {
                return None;
            }

            if app.current_screen == crate::app::CurrentScreen::Setup {
                // If on setup screen and setup is complete, clicking in the footer area starts the app
                if let Some(setup) = &app.setup_state
                    && setup.phase == crate::app::SetupPhase::Complete
                    && mouse.row >= 18
                {
                    app.finish_setup();
                }
                return None;
            }

            // Main screen interactions:
            // Row 3 to 5: URL Input area (header is rows 0..3, input is rows 3..6)
            if mouse.row >= 3 && mouse.row <= 5 {
                app.input_mode = InputMode::Editing;
                let col = (mouse.column as usize).saturating_sub(1);
                let char_count = app.input_buffer.chars().count();
                app.cursor_position = col.min(char_count);
                return None;
            }

            // Row >= 6: Downloads list area
            if mouse.row >= 6 {
                app.input_mode = InputMode::Normal;
                let list_row = (mouse.row as usize).saturating_sub(7);
                let clicked_idx = list_row / 3;
                if !app.downloads.is_empty() {
                    let new_selected = clicked_idx.min(app.downloads.len() - 1);
                    if app.selected_download == new_selected {
                        // Re-clicking on already selected item opens error/log details
                        app.open_selected_details();
                    } else {
                        app.selected_download = new_selected;
                    }
                }
            }

            None
        }
        crossterm::event::MouseEventKind::ScrollDown => {
            if let Some(m) = &mut app.help_modal {
                m.scroll_down(100);
            } else if let Some(m) = &mut app.history_modal {
                m.next(app.history.len());
            } else if app.detail_modal.is_some() {
                app.modal_scroll_down();
            } else if app.current_screen == crate::app::CurrentScreen::Setup {
                app.setup_scroll_down();
            } else if app.current_screen == crate::app::CurrentScreen::Main {
                app.next_download();
            }
            None
        }
        crossterm::event::MouseEventKind::ScrollUp => {
            if let Some(m) = &mut app.help_modal {
                m.scroll_up();
            } else if let Some(m) = &mut app.history_modal {
                m.previous(app.history.len());
            } else if app.detail_modal.is_some() {
                app.modal_scroll_up();
            } else if app.current_screen == crate::app::CurrentScreen::Setup {
                app.setup_scroll_up();
            } else if app.current_screen == crate::app::CurrentScreen::Main {
                app.previous_download();
            }
            None
        }
        _ => None,
    }
}
