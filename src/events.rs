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

    // Global toggle for download directory modal (F3) - works in Normal and Editing modes
    if key.code == KeyCode::F(3) {
        app.toggle_path_modal();
        return None;
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
        KeyCode::Char(c) => {
            app.input_buffer.insert(app.cursor_position, c);
            app.cursor_position += 1;
            None
        }
        KeyCode::Backspace => {
            if app.cursor_position > 0 {
                app.cursor_position -= 1;
                app.input_buffer.remove(app.cursor_position);
            }
            None
        }
        KeyCode::Delete => {
            if app.cursor_position < app.input_buffer.len() {
                app.input_buffer.remove(app.cursor_position);
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
            _ => None,
        },
    }
}

/// Handles keys when the download path configuration modal is open.
fn handle_path_modal_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(modal) = &mut app.path_modal {
                    modal.reset_default();
                }
            }
            KeyCode::Char('u') | KeyCode::Char('U') => {
                if let Some(modal) = &mut app.path_modal {
                    modal.clear();
                }
            }
            _ => {}
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
