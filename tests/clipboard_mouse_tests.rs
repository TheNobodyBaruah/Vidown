// tests/clipboard_mouse_tests.rs

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use video_downloader::app::{App, CurrentScreen, InputMode, PathModal, SetupPhase, SetupState};
use video_downloader::clipboard::sanitize_clipboard_text;
use video_downloader::config::{get_default_download_dir, get_safe_fallback_dir};
use video_downloader::events::{handle_key_event, handle_mouse_event, handle_paste_event};

fn press_ctrl(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn press_shift_insert() -> KeyEvent {
    KeyEvent {
        code: KeyCode::Insert,
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn make_mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn test_sanitize_clipboard_text_quotes_and_whitespace() {
    assert_eq!(
        sanitize_clipboard_text("  https://www.youtube.com/watch?v=12345  "),
        "https://www.youtube.com/watch?v=12345"
    );
    assert_eq!(
        sanitize_clipboard_text("\"https://www.youtube.com/watch?v=12345\""),
        "https://www.youtube.com/watch?v=12345"
    );
    assert_eq!(
        sanitize_clipboard_text("'https://www.youtube.com/watch?v=12345'"),
        "https://www.youtube.com/watch?v=12345"
    );
    assert_eq!(
        sanitize_clipboard_text("  \"  'https://site.com/video.mp4'  \"  "),
        "https://site.com/video.mp4"
    );
}

#[test]
fn test_sanitize_clipboard_text_multiline() {
    let multi = "   \n\r  https://site.com/first_video \n https://site.com/second_video \r\n";
    assert_eq!(
        sanitize_clipboard_text(multi),
        "https://site.com/first_video"
    );
}

#[test]
fn test_sanitize_clipboard_text_empty() {
    assert_eq!(sanitize_clipboard_text(""), "");
    assert_eq!(sanitize_clipboard_text("   \n\r\t  "), "");
    assert_eq!(sanitize_clipboard_text("\"\""), "");
    assert_eq!(sanitize_clipboard_text("''"), "");
}

#[test]
fn test_app_paste_text_to_input_from_normal_mode() {
    let mut app = App::new();
    assert_eq!(app.input_mode, InputMode::Normal);
    assert_eq!(app.input_buffer, "");
    assert_eq!(app.cursor_position, 0);

    app.paste_text_to_input("https://youtu.be/dQw4w9WgXcQ");
    assert_eq!(app.input_mode, InputMode::Editing);
    assert_eq!(app.input_buffer, "https://youtu.be/dQw4w9WgXcQ");
    assert_eq!(app.cursor_position, 28);
    assert!(app.status_message.as_ref().unwrap().contains("Pasted"));
}

#[test]
fn test_app_paste_text_to_input_middle_insertion_and_multibyte() {
    let mut app = App::new();
    app.input_mode = InputMode::Editing;
    app.input_buffer = "hello world".to_string();
    app.cursor_position = 5; // right after "hello"

    app.paste_text_to_input("🦀 beautiful");
    assert_eq!(app.input_buffer, "hello🦀 beautiful world");
    // "hello" (5) + "🦀 beautiful" (11 chars) = 16
    assert_eq!(app.cursor_position, 16);
}

#[test]
fn test_handle_paste_event_in_main_screen() {
    let mut app = App::new();
    assert_eq!(app.current_screen, CurrentScreen::Main);

    handle_paste_event(&mut app, "https://vimeo.com/987654");
    assert_eq!(app.input_mode, InputMode::Editing);
    assert_eq!(app.input_buffer, "https://vimeo.com/987654");
}

#[test]
fn test_handle_paste_event_in_path_modal() {
    let mut app = App::new();
    app.path_modal = Some(PathModal::new("/current/path"));
    assert_eq!(app.path_modal.as_ref().unwrap().input, "/current/path");

    handle_paste_event(&mut app, "/extra/subfolder");
    assert_eq!(
        app.path_modal.as_ref().unwrap().input,
        "/current/path/extra/subfolder"
    );
    // URL input buffer should not have changed
    assert_eq!(app.input_buffer, "");
}

#[test]
fn test_handle_paste_event_modal_exclusion() {
    let mut app = App::new();
    app.open_help_modal();
    assert!(app.help_modal.is_some());

    handle_paste_event(&mut app, "https://ignored.com");
    assert_eq!(app.input_buffer, "");
}

#[test]
fn test_ctrl_v_and_shift_insert_key_events() {
    let mut app = App::new();

    // Verify Ctrl+V does not insert literal 'v' into buffer
    app.input_mode = InputMode::Editing;
    app.input_buffer = "http://".to_string();
    app.cursor_position = 7;

    // Press Ctrl+V (clipboard might be empty in headless CI test, but it shouldn't type 'v')
    handle_key_event(&mut app, press_ctrl('v'));
    assert_ne!(app.input_buffer, "http://v");

    // Press Shift+Insert
    handle_key_event(&mut app, press_shift_insert());
    assert!(!app.input_buffer.ends_with('v'));
}

#[test]
fn test_mouse_left_click_focus_url_input() {
    let mut app = App::new();
    app.input_buffer = "https://example.com/video".to_string();
    app.cursor_position = 0;
    app.input_mode = InputMode::Normal;

    // Click on row 4, column 10 (inside URL input area rows 3..=5)
    let mouse = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 10, 4);
    handle_mouse_event(&mut app, mouse);

    assert_eq!(app.input_mode, InputMode::Editing);
    // Cursor position should be set to col - 1 = 9
    assert_eq!(app.cursor_position, 9);
}

#[test]
fn test_mouse_left_click_select_downloads_item() {
    let mut app = App::new();
    app.enqueue_download("https://site.com/1".to_string());
    app.enqueue_download("https://site.com/2".to_string());
    app.enqueue_download("https://site.com/3".to_string());
    assert_eq!(app.selected_download, 2);

    // Row 7 is item 0 (header=3, input=3, border=1 -> row 7 is list_row 0 -> item 0)
    let mouse = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 15, 7);
    handle_mouse_event(&mut app, mouse);
    assert_eq!(app.selected_download, 0);

    // Row 10 is item 1 (list_row 3 -> 3/3 = 1)
    let mouse = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 15, 10);
    handle_mouse_event(&mut app, mouse);
    assert_eq!(app.selected_download, 1);
}

#[test]
fn test_mouse_left_click_double_click_opens_details() {
    let mut app = App::new();
    let id = app.enqueue_download("https://site.com/1".to_string());
    app.append_log(id, "Error log line".to_string());
    app.selected_download = 0;

    // Re-clicking on the already selected item opens details modal
    let mouse = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 15, 7);
    handle_mouse_event(&mut app, mouse);
    assert!(app.detail_modal.is_some());
    assert_eq!(
        app.detail_modal.as_ref().unwrap().title,
        "Download #1: Details & Logs"
    );
}

#[test]
fn test_mouse_scroll_wheel_navigation() {
    let mut app = App::new();
    app.enqueue_download("https://site.com/1".to_string());
    app.enqueue_download("https://site.com/2".to_string());
    app.selected_download = 0;

    // Scroll down moves to next download
    let scroll_down = make_mouse_event(MouseEventKind::ScrollDown, 10, 10);
    handle_mouse_event(&mut app, scroll_down);
    assert_eq!(app.selected_download, 1);

    // Scroll up moves to previous download
    let scroll_up = make_mouse_event(MouseEventKind::ScrollUp, 10, 10);
    handle_mouse_event(&mut app, scroll_up);
    assert_eq!(app.selected_download, 0);
}

#[test]
fn test_mouse_scroll_wheel_in_help_modal() {
    let mut app = App::new();
    app.open_help_modal();
    assert_eq!(app.help_modal.as_ref().unwrap().scroll_offset, 0);

    let scroll_down = make_mouse_event(MouseEventKind::ScrollDown, 10, 10);
    handle_mouse_event(&mut app, scroll_down);
    assert_eq!(app.help_modal.as_ref().unwrap().scroll_offset, 1);

    let scroll_up = make_mouse_event(MouseEventKind::ScrollUp, 10, 10);
    handle_mouse_event(&mut app, scroll_up);
    assert_eq!(app.help_modal.as_ref().unwrap().scroll_offset, 0);
}

#[test]
fn test_get_default_download_dir_in_tests() {
    let default_dir = get_default_download_dir();
    assert_eq!(default_dir, "./downloads");
}

#[test]
fn test_get_safe_fallback_dir_is_valid() {
    let fallback = get_safe_fallback_dir();
    assert!(!fallback.as_os_str().is_empty());
}

#[test]
fn test_setup_screen_mouse_click_starts_app_when_complete() {
    let state = SetupState {
        phase: SetupPhase::Complete,
        tools: Vec::new(),
        status_message: "Ready".to_string(),
        help_scroll: 0,
    };
    let mut app = App::new_with_setup(state);
    assert_eq!(app.current_screen, CurrentScreen::Setup);

    // Clicking in the bottom footer row triggers launch
    let mouse = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 20, 22);
    handle_mouse_event(&mut app, mouse);
    assert_eq!(app.current_screen, CurrentScreen::Main);
}

#[test]
fn test_sanitize_clipboard_text_brackets_backticks_and_unicode_quotes() {
    // Angle brackets
    assert_eq!(
        sanitize_clipboard_text("<https://www.youtube.com/watch?v=12345>"),
        "https://www.youtube.com/watch?v=12345"
    );
    // Backticks (e.g. from Markdown / chat)
    assert_eq!(
        sanitize_clipboard_text("`https://www.youtube.com/watch?v=12345`"),
        "https://www.youtube.com/watch?v=12345"
    );
    // Unicode curly double quotes
    assert_eq!(
        sanitize_clipboard_text("“https://www.youtube.com/watch?v=12345”"),
        "https://www.youtube.com/watch?v=12345"
    );
    // Unicode curly single quotes
    assert_eq!(
        sanitize_clipboard_text("‘https://www.youtube.com/watch?v=12345’"),
        "https://www.youtube.com/watch?v=12345"
    );
    // Nested delimiters (e.g. <"https://site.com">)
    assert_eq!(
        sanitize_clipboard_text("<\"https://site.com/video.mp4\">"),
        "https://site.com/video.mp4"
    );
}

#[test]
fn test_submit_input_sanitizes_enclosed_urls() {
    let mut app = App::new();
    app.input_mode = InputMode::Editing;
    app.input_buffer = "<https://youtu.be/dQw4w9WgXcQ>".to_string();

    let submitted = app.submit_input();
    assert!(submitted.is_some());
    let (id, url) = submitted.unwrap();
    assert_eq!(id, 1);
    assert_eq!(url, "https://youtu.be/dQw4w9WgXcQ");
    assert_eq!(app.downloads[0].url, "https://youtu.be/dQw4w9WgXcQ");
}

#[test]
fn test_mouse_left_click_outside_downloads_does_not_open_details() {
    let mut app = App::new();
    let id = app.enqueue_download("https://site.com/1".to_string());
    app.append_log(id, "Error log line".to_string());
    app.selected_download = 0;

    // Clicking row 6 (top border of downloads block) must not open details
    let border_click = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 15, 6);
    handle_mouse_event(&mut app, border_click);
    assert!(app.detail_modal.is_none());

    // Clicking row 22 (footer row / empty space far below the 1 item) must NOT open details
    let footer_click = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 15, 22);
    handle_mouse_event(&mut app, footer_click);
    assert!(app.detail_modal.is_none());
}

#[test]
fn test_mouse_scroll_wheel_does_not_scroll_downloads_when_path_modal_open() {
    let mut app = App::new();
    app.enqueue_download("https://site.com/1".to_string());
    app.enqueue_download("https://site.com/2".to_string());
    app.selected_download = 0;

    app.open_path_modal();
    assert!(app.path_modal.is_some());

    // Scrolling down while PathModal is open should NOT advance background downloads list
    let scroll_down = make_mouse_event(MouseEventKind::ScrollDown, 10, 10);
    handle_mouse_event(&mut app, scroll_down);
    assert_eq!(app.selected_download, 0);

    let scroll_up = make_mouse_event(MouseEventKind::ScrollUp, 10, 10);
    handle_mouse_event(&mut app, scroll_up);
    assert_eq!(app.selected_download, 0);
}

#[test]
fn test_paste_empty_or_whitespace_sets_status_message() {
    let mut app = App::new();
    app.paste_text_to_input("    \n\t  ");
    assert_eq!(app.input_buffer, "");
    assert_eq!(
        app.status_message.as_deref(),
        Some("Clipboard contains no valid text.")
    );
}

#[test]
fn test_paste_path_modal_sets_status_message() {
    let mut app = App::new();
    app.open_path_modal();

    handle_paste_event(&mut app, "/custom/downloads");
    assert_eq!(
        app.path_modal.as_ref().unwrap().input,
        "./downloads/custom/downloads"
    );
    assert_eq!(
        app.status_message.as_deref(),
        Some("Pasted path from clipboard.")
    );
}

#[test]
fn test_sanitize_clipboard_text_parentheses_and_square_brackets() {
    // Round parentheses (e.g. from chat references or markdown (url))
    assert_eq!(
        sanitize_clipboard_text("(https://www.youtube.com/watch?v=12345)"),
        "https://www.youtube.com/watch?v=12345"
    );
    // Square brackets (e.g. [url])
    assert_eq!(
        sanitize_clipboard_text("[https://www.youtube.com/watch?v=12345]"),
        "https://www.youtube.com/watch?v=12345"
    );
    // Nested delimiters (e.g. [(url)] or <(url)>)
    assert_eq!(
        sanitize_clipboard_text("[(https://site.com/video.mp4)]"),
        "https://site.com/video.mp4"
    );
    assert_eq!(
        sanitize_clipboard_text("<(https://site.com/video.mp4)>"),
        "https://site.com/video.mp4"
    );
}

#[test]
fn test_mouse_left_click_scrolled_downloads_list() {
    let mut app = App::new();
    // Add 25 downloads so that the list is guaranteed to exceed visible viewport on any terminal
    for i in 0..25 {
        app.enqueue_download(format!("https://site.com/{}", i));
    }
    let term_height = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24);
    let downloads_height = term_height.saturating_sub(9);
    let inner_height = downloads_height.saturating_sub(2);
    let visible_items_count = (inner_height / 3).max(1) as usize;

    // Select last item (24), scrolling the list
    app.selected_download = 24;
    let expected_start_idx = 24 + 1 - visible_items_count;

    // Row 7 corresponds to slot 0 of visible viewport -> should select expected_start_idx
    let click_slot_0 = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 15, 7);
    handle_mouse_event(&mut app, click_slot_0);
    assert_eq!(app.selected_download, expected_start_idx);

    // Re-select 24 to preserve scroll:
    app.selected_download = 24;
    // Row 10 corresponds to slot 1 -> should select expected_start_idx + 1
    let click_slot_1 = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 15, 10);
    handle_mouse_event(&mut app, click_slot_1);
    assert_eq!(app.selected_download, expected_start_idx + 1);
}

#[test]
fn test_mouse_left_click_footer_with_many_downloads_does_not_trigger() {
    let mut app = App::new();
    for i in 0..25 {
        let id = app.enqueue_download(format!("https://site.com/{}", i));
        app.append_log(id, "Log line".to_string());
    }
    app.selected_download = 24;

    let term_height = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24);
    // Footer is always at (term_height - 3)..term_height (e.g. term_height - 2)
    let footer_row = term_height.saturating_sub(2);
    let footer_click = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 15, footer_row);
    handle_mouse_event(&mut app, footer_click);
    assert!(app.detail_modal.is_none());
    assert_eq!(app.selected_download, 24);
}

#[test]
fn test_multibyte_utf8_input_navigation_and_vim_x() {
    let mut app = App::new();
    app.input_mode = InputMode::Editing;
    // "https://сайт.рф" has 15 characters, but 22 bytes in UTF-8
    app.input_buffer = "https://сайт.рф".to_string();
    app.cursor_position = 0;

    // Press End key: cursor_position must be 15 (characters), NOT 22 (bytes)
    let end_key = KeyEvent {
        code: KeyCode::End,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    handle_key_event(&mut app, end_key);
    assert_eq!(app.cursor_position, 15);

    // Press Right key at end: stays at 15
    let right_key = KeyEvent {
        code: KeyCode::Right,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    handle_key_event(&mut app, right_key);
    assert_eq!(app.cursor_position, 15);

    // Switch to Vim scheme
    app.input_scheme = video_downloader::app::InputScheme::Vim;
    app.input_mode = InputMode::Normal;
    // Position cursor on character index 8 (the Cyrillic character 'с')
    app.cursor_position = 8;

    // Press 'x' in Vim mode: must delete character 'с' without UTF-8 boundary panic
    let x_key = KeyEvent {
        code: KeyCode::Char('x'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    handle_key_event(&mut app, x_key);
    assert_eq!(app.input_buffer, "https://айт.рф");
}

#[test]
fn test_right_click_ignored_when_modal_open() {
    let mut app = App::new();
    app.status_message = None;
    app.open_help_modal();
    assert!(app.help_modal.is_some());

    let right_click = make_mouse_event(MouseEventKind::Down(MouseButton::Right), 10, 10);
    handle_mouse_event(&mut app, right_click);
    // Should not set status message or mutate input
    assert_eq!(app.status_message, None);
    assert_eq!(app.input_buffer, "");
}

