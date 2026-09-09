// tests/ui_tests.rs

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
use video_downloader::app::{App, InputMode, InputScheme};
use video_downloader::events::handle_key_event;
use video_downloader::ui;

/// Helper function to convert a ratatui Buffer to a multiline string for deterministic assertions.
fn buffer_to_string(buffer: &Buffer) -> String {
    let mut s = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            s.push_str(cell.symbol());
        }
        s.push('\n');
    }
    s
}

#[test]
fn test_render_to_raw_buffer() {
    let mut app = App::new();
    let id = app.enqueue_download("https://example.com/test_video.mp4".to_string());
    app.update_progress(id, 1, 64.0);

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("Failed to create TestBackend");
    terminal
        .draw(|f| ui::render(f, &app))
        .expect("Failed to draw to TestBackend");

    let screen_text = buffer_to_string(terminal.backend().buffer());

    // Verify key UI elements are present in the rendered buffer
    assert!(screen_text.contains("VIDOWN"), "Header must contain VIDOWN");
    assert!(screen_text.contains("NORMAL"), "Should display NORMAL mode");
    assert!(screen_text.contains("Modal"), "Should display default scheme");
    assert!(screen_text.contains("64.0%"), "Should display 64.0% progress gauge");
    assert!(screen_text.contains("Track 1"), "Should display track 1");
}

#[test]
fn test_render_editing_mode_and_modal_overlay() {
    let mut app = App::new();
    app.input_mode = InputMode::Editing;
    app.input_buffer = "https://mysite.com/media.mp4".to_string();
    app.cursor_position = app.input_buffer.len();

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("Failed to create TestBackend");

    terminal
        .draw(|f| ui::render(f, &app))
        .expect("Failed to draw editing mode");

    let screen_text = buffer_to_string(terminal.backend().buffer());
    assert!(screen_text.contains("EDITING"), "Should indicate EDITING mode");
    assert!(
        screen_text.contains("https://mysite.com/media.mp4"),
        "Should display input buffer URL"
    );

    // Open detail modal and verify modal overlay rendering
    let id = app.enqueue_download("https://mysite.com/media.mp4".to_string());
    app.append_log(id, "[ERROR] HTTP 403 Forbidden".to_string());
    app.update_error(id, "Forbidden".to_string());
    app.open_selected_details();

    terminal
        .draw(|f| ui::render(f, &app))
        .expect("Failed to draw modal overlay");

    let modal_screen = buffer_to_string(terminal.backend().buffer());
    assert!(
        modal_screen.contains("Details & Logs"),
        "Should render modal title"
    );
    assert!(
        modal_screen.contains("HTTP 403 Forbidden"),
        "Should render log inside modal"
    );
}

#[test]
fn test_interactive_session_with_test_backend() {
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("Failed to create TestBackend");
    let mut app = App::new();

    let key = |code: KeyCode| KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };

    // 1. Initial draw
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    // 2. Press 'i' to enter editing
    handle_key_event(&mut app, key(KeyCode::Char('i')));
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    assert_eq!(app.input_mode, InputMode::Editing);

    // 3. Type "https://test.org/stream"
    for c in "https://test.org/stream".chars() {
        handle_key_event(&mut app, key(KeyCode::Char(c)));
    }
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    assert_eq!(app.input_buffer, "https://test.org/stream");

    // 4. Press Enter to submit
    let download = handle_key_event(&mut app, key(KeyCode::Enter));
    assert!(download.is_some());
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    assert_eq!(app.downloads.len(), 1);

    // 5. Toggle scheme to Vim with F2
    handle_key_event(&mut app, key(KeyCode::F(2)));
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    assert_eq!(app.input_scheme, InputScheme::Vim);

    // 6. Press 'q' to quit
    handle_key_event(&mut app, key(KeyCode::Char('q')));
    assert!(app.should_quit);
}
