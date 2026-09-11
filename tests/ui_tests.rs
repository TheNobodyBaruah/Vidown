// tests/ui_tests.rs

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
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
    assert!(
        screen_text.contains("Modal"),
        "Should display default scheme"
    );
    assert!(
        screen_text.contains("64.0%"),
        "Should display 64.0% progress gauge"
    );
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
    assert!(
        screen_text.contains("EDITING"),
        "Should indicate EDITING mode"
    );
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

#[test]
fn test_render_header_adaptive_truncation_under_95_cols() {
    let mut app = App::new();
    app.output_dir = "./downloads".to_string();

    // 1. Standard 80x24 terminal
    let backend_80 = TestBackend::new(80, 24);
    let mut terminal_80 = Terminal::new(backend_80).unwrap();
    terminal_80.draw(|f| ui::render(f, &app)).unwrap();
    let text_80 = buffer_to_string(terminal_80.backend().buffer());

    assert!(text_80.contains("VIDOWN"), "Header must have title");
    assert!(
        text_80.contains("[Dir: ./downloads]"),
        "80-col header should display active dir"
    );

    // 2. Long path on 80-column terminal triggers adaptive truncation with '…'
    app.output_dir = "C:\\Users\\Hp\\Very\\Long\\Path\\To\\Custom\\DownloadsFolder".to_string();
    let backend_80_long = TestBackend::new(80, 24);
    let mut terminal_80_long = Terminal::new(backend_80_long).unwrap();
    terminal_80_long.draw(|f| ui::render(f, &app)).unwrap();
    let text_80_long = buffer_to_string(terminal_80_long.backend().buffer());

    assert!(
        text_80_long.contains("[Dir: …"),
        "Long dir must be adaptively truncated with …"
    );
    assert!(
        text_80_long.contains("DownloadsFolder]"),
        "Truncation must retain the tail"
    );

    // 3. Narrow 70x24 terminal
    let backend_70 = TestBackend::new(70, 24);
    let mut terminal_70 = Terminal::new(backend_70).unwrap();
    terminal_70.draw(|f| ui::render(f, &app)).unwrap();
    let text_70 = buffer_to_string(terminal_70.backend().buffer());
    assert!(
        text_70.contains("VIDOWN"),
        "Narrow terminal must render without panicking"
    );
}

#[test]
fn test_render_path_modal_on_80x24_terminal() {
    let mut app = App::new();
    app.open_path_modal();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let screen = buffer_to_string(terminal.backend().buffer());

    // Verify modal elements are rendered without collapsing
    assert!(
        screen.contains("Set Download Directory"),
        "Must contain modal title"
    );
    assert!(
        screen.contains("Destination directory"),
        "Must contain instructions"
    );
    assert!(
        screen.contains("Directory Path"),
        "Must contain input box title"
    );
    assert!(screen.contains("./downloads"), "Must contain current path");
    assert!(
        screen.contains("Empty resets to ./downloads"),
        "Must contain hint"
    );
    assert!(
        screen.contains("[Enter] Save"),
        "Must contain Save shortcut"
    );
    assert!(
        screen.contains("[Esc] Cancel"),
        "Must contain Cancel shortcut"
    );
    assert!(
        screen.contains("[Ctrl+D] Default"),
        "Must contain Default shortcut"
    );
    assert!(
        screen.contains("[Ctrl+U] Clear"),
        "Must contain Clear shortcut"
    );
    assert!(
        screen.contains("[←/→] Cursor"),
        "Must contain Navigation help"
    );
}

#[test]
fn test_render_path_modal_horizontal_viewport_scrolling() {
    let mut app = App::new();
    app.open_path_modal();

    // Set a very long path
    let long_path = "C:\\VeryLongPath\\ThatExceedsTheWidthOfTheModalInputBox\\Subfolder\\Target";
    let modal = app.path_modal.as_mut().unwrap();
    modal.input = long_path.to_string();
    modal.cursor_position = modal.input.chars().count(); // cursor at end

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let screen = buffer_to_string(terminal.backend().buffer());
    // Since cursor is at the end, horizontal scroll must show the tail "Target"
    assert!(
        screen.contains("Target"),
        "Horizontal scroll must render tail when cursor is at end"
    );
}

#[test]
fn test_footer_displays_path_shortcuts() {
    let mut app = App::new();
    app.status_message = None; // clear greeting so keybindings help renders

    // Normal mode: StandardModal
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let screen = buffer_to_string(terminal.backend().buffer());
    assert!(
        screen.contains("[p/F3] Path"),
        "Normal footer must show [p/F3] Path hint"
    );

    // Editing mode
    app.input_mode = InputMode::Editing;
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let screen_edit = buffer_to_string(terminal.backend().buffer());
    assert!(
        screen_edit.contains("[F3] Path"),
        "Editing footer must show [F3] Path hint"
    );
}

#[test]
fn test_render_detail_modal_displays_output_dir() {
    let mut app = App::new();
    app.output_dir = "/custom/media/path".to_string();
    let id = app.enqueue_download("https://mysite.com/video.mp4".to_string());
    app.append_log(id, "Starting stream...".to_string());
    app.open_selected_details();

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let screen = buffer_to_string(terminal.backend().buffer());
    assert!(
        screen.contains("Directory: "),
        "Detail modal must have Directory label"
    );
    assert!(
        screen.contains("/custom/media/path"),
        "Detail modal must show output dir"
    );
}

#[test]
fn test_centered_rect_bounded_micro_terminals() {
    use ratatui::layout::Rect;
    use video_downloader::ui::centered_rect_bounded;

    // Zero / degenerate dimensions return default Rect (0x0)
    assert_eq!(
        centered_rect_bounded(64, 9, Rect::new(0, 0, 0, 0)),
        Rect::default()
    );
    assert_eq!(
        centered_rect_bounded(64, 9, Rect::new(0, 0, 2, 2)),
        Rect::default()
    );
    assert_eq!(
        centered_rect_bounded(64, 9, Rect::new(0, 0, 1, 10)),
        Rect::default()
    );

    // Small dimensions clamp bounded size safely
    let rect_small = centered_rect_bounded(64, 9, Rect::new(0, 0, 20, 10));
    assert!(rect_small.width <= 20);
    assert!(rect_small.height <= 10);

    // Standard 80x24 terminal gets exact 64x9
    let rect_std = centered_rect_bounded(64, 9, Rect::new(0, 0, 80, 24));
    assert_eq!(rect_std.width, 64);
    assert_eq!(rect_std.height, 9);
}

#[test]
fn test_render_path_modal_micro_terminal_no_panic() {
    let mut app = App::new();
    app.open_path_modal();

    // 10x4 micro terminal
    let backend_micro = TestBackend::new(10, 4);
    let mut terminal_micro = Terminal::new(backend_micro).unwrap();
    let result = terminal_micro.draw(|f| ui::render(f, &app));
    assert!(result.is_ok(), "Rendering on micro terminal must not panic");

    // 20x8 terminal
    let backend_small = TestBackend::new(20, 8);
    let mut terminal_small = Terminal::new(backend_small).unwrap();
    let result2 = terminal_small.draw(|f| ui::render(f, &app));
    assert!(
        result2.is_ok(),
        "Rendering on small terminal must not panic"
    );
}

#[test]
fn test_render_path_modal_empty_input_placeholder() {
    let mut app = App::new();
    app.open_path_modal();
    app.path_modal.as_mut().unwrap().input.clear();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let screen = buffer_to_string(terminal.backend().buffer());
    assert!(
        screen.contains("./downloads (default)"),
        "Empty input must display default placeholder"
    );
}

#[test]
fn test_render_history_modal_empty() {
    let mut app = App::new_empty();
    app.open_history_modal();

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let screen = buffer_to_string(terminal.backend().buffer());
    assert!(
        screen.contains("Download History (0)"),
        "Must render title with count 0"
    );
    assert!(
        screen.contains("Past Downloads"),
        "Must render Past Downloads list panel"
    );
    assert!(
        screen.contains("No download history recorded yet."),
        "Must render empty list placeholder"
    );
    assert!(
        screen.contains("Inspector Details"),
        "Must render inspector block"
    );
    assert!(
        screen.contains("Select an item from history"),
        "Must render empty inspector placeholder"
    );
    assert!(
        screen.contains("Retry"),
        "Must render Retry action shortcut"
    );
    assert!(
        screen.contains("Open File"),
        "Must render Open File shortcut"
    );
}

#[test]
fn test_render_history_modal_with_completed_and_failed_entries() {
    let mut app = App::new_empty();
    let id1 = app.enqueue_download("https://example.com/video1.mp4".to_string());
    app.update_filename(id1, "my_cool_video.mp4".to_string());
    app.update_success(id1);

    let id2 = app.enqueue_download("https://example.com/failed.mp4".to_string());
    app.update_error(id2, "HTTP 403 Forbidden error".to_string());

    app.open_history_modal();

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let screen = buffer_to_string(terminal.backend().buffer());
    assert!(
        screen.contains("Download History (2)"),
        "Title must show count 2"
    );
    assert!(screen.contains("[Failed]"), "Must render [Failed] badge");
    assert!(screen.contains("[Done]"), "Must render [Done] badge");
    // Newest is index 0 (failed)
    assert!(
        screen.contains("failed.mp4"),
        "Must display failed item name"
    );
    assert!(
        screen.contains("Error Details:"),
        "Inspector must show Error Details for failed item"
    );
    assert!(
        screen.contains("HTTP 403 Forbidden error"),
        "Inspector must display error message"
    );

    // Move to item 1 (completed download)
    app.history_modal.as_mut().unwrap().selected = 1;
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let screen2 = buffer_to_string(terminal.backend().buffer());
    assert!(
        screen2.contains("my_cool_video.mp4"),
        "Inspector must show completed video title"
    );
    assert!(
        screen2.contains("Completed"),
        "Inspector must show Completed status"
    );
    assert!(
        screen2.contains("Destination:"),
        "Inspector must display Destination label"
    );
}

#[test]
fn test_render_history_modal_narrow_vertical_split() {
    let mut app = App::new_empty();
    let id = app.enqueue_download("https://example.com/video.mp4".to_string());
    app.update_success(id);
    app.open_history_modal();

    // Width 60 (< 70) exercises vertical layout branch
    let backend = TestBackend::new(60, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let res = terminal.draw(|f| ui::render(f, &app));
    assert!(
        res.is_ok(),
        "Vertical split layout must render without error"
    );

    let screen = buffer_to_string(terminal.backend().buffer());
    assert!(screen.contains("Download History (1)"));
    assert!(screen.contains("Past Downloads"));
    assert!(screen.contains("Inspector Details"));
}

#[test]
fn test_render_history_modal_micro_terminal_no_panic() {
    let mut app = App::new_empty();
    app.open_history_modal();

    // 15x5 micro terminal
    let backend_micro = TestBackend::new(15, 5);
    let mut terminal_micro = Terminal::new(backend_micro).unwrap();
    let result = terminal_micro.draw(|f| ui::render(f, &app));
    assert!(
        result.is_ok(),
        "Rendering history modal on micro terminal must not panic"
    );
}

#[test]
fn test_interactive_history_session_with_test_backend() {
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("Failed to create TestBackend");
    let mut app = App::new_empty();

    let key = |code: KeyCode| KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };

    // 1. Initial draw
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    // 2. Download a video and complete it
    let id = app.enqueue_download("https://example.com/video.mp4".to_string());
    app.update_filename(id, "finished_video.mp4".to_string());
    app.update_success(id);

    // 3. Press 'g' to open history modal
    handle_key_event(&mut app, key(KeyCode::Char('g')));
    assert!(app.history_modal.is_some());
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let screen_modal = buffer_to_string(terminal.backend().buffer());
    assert!(screen_modal.contains("Download History (1)"));
    assert!(screen_modal.contains("finished_video.mp4"));

    // 4. Press 'r' to retry the selected download
    let retry_res = handle_key_event(&mut app, key(KeyCode::Char('r')));
    assert!(retry_res.is_some());
    assert!(app.history_modal.is_none());

    // 5. Draw back on main screen with the retried download
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let screen_main = buffer_to_string(terminal.backend().buffer());
    assert!(screen_main.contains("Downloads (2)"));
}

#[test]
fn test_render_history_modal_80_col_footer_shortcuts() {
    let mut app = App::new_empty();
    let id = app.enqueue_download("https://example.com/video.mp4".to_string());
    app.update_filename(id, "vid.mp4".to_string());
    app.update_success(id);
    app.open_history_modal();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let screen = buffer_to_string(terminal.backend().buffer());
    assert!(screen.contains("Select"), "Must contain Select shortcut");
    assert!(screen.contains("Retry"), "Must contain Retry shortcut");
    assert!(screen.contains("Open"), "Must contain Open shortcut");
    assert!(screen.contains("Del"), "Must contain Del shortcut");
    assert!(screen.contains("Close"), "Must contain Close shortcut");
}

#[test]
fn test_render_history_modal_viewport_scrolling_bidirectional() {
    let mut app = App::new_empty();
    for i in 1..=12 {
        let entry = video_downloader::history::HistoryEntry::new_completed(
            i,
            format!("https://example.com/{}", i),
            Some(format!("item_{:02}.mp4", i)),
            Some(format!("/downloads/item_{:02}.mp4", i)),
        );
        app.add_history_entry(entry);
    }
    assert_eq!(app.history.len(), 12);
    app.open_history_modal();

    // 80x16 terminal with small visible list height (~6 lines)
    let backend = TestBackend::new(80, 16);
    let mut terminal = Terminal::new(backend).unwrap();

    // 1. Initial render at selected 0 (item_12 is newest)
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let screen0 = buffer_to_string(terminal.backend().buffer());
    assert!(screen0.contains("> [Done]   item_12.mp4"));

    // 2. Scroll down 7 times to selected 7
    for _ in 0..7 {
        handle_key_event(
            &mut app,
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
        );
        terminal.draw(|f| ui::render(f, &app)).unwrap();
    }
    assert_eq!(app.history_modal.as_ref().unwrap().selected, 7);
    let screen7 = buffer_to_string(terminal.backend().buffer());
    assert!(
        screen7.contains("> [Done]   item_05.mp4"),
        "Item 05 (index 7) must be selected"
    );

    // 3. Scroll back up 3 times to selected 4
    for _ in 0..3 {
        handle_key_event(
            &mut app,
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
        );
        terminal.draw(|f| ui::render(f, &app)).unwrap();
    }
    assert_eq!(app.history_modal.as_ref().unwrap().selected, 4);
    let screen4 = buffer_to_string(terminal.backend().buffer());
    assert!(
        screen4.contains("> [Done]   item_08.mp4"),
        "Item 08 (index 4) must be selected when scrolling back up"
    );
}
