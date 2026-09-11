// src/ui.rs

use crate::app::{App, InputMode, ItemStatus};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
};

/// Pure View function rendering the entire TUI from the App state.
pub fn render(f: &mut Frame, app: &App) {
    match app.current_screen {
        crate::app::CurrentScreen::Setup => {
            render_setup_screen(f, app);
        }
        crate::app::CurrentScreen::Main => {
            render_main_screen(f, app);
        }
    }
}

/// Renders the main downloader application view.
fn render_main_screen(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // URL Input
            Constraint::Min(8),    // Downloads & Progress list
            Constraint::Length(3), // Status / Help Footer
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_input(f, app, chunks[1]);
    render_downloads(f, app, chunks[2]);
    render_footer(f, app, chunks[3]);

    if let Some(help_modal) = &app.help_modal {
        render_help_modal(f, help_modal);
    } else if let Some(history_modal) = &app.history_modal {
        render_history_modal(f, app, history_modal);
    } else if let Some(path_modal) = &app.path_modal {
        render_path_modal(f, path_modal);
    } else if let Some(modal) = &app.detail_modal {
        render_modal(f, modal);
    }
}

/// Renders the top title, scheme/mode indicator, and active download directory header.
fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let mode_style = match app.input_mode {
        InputMode::Editing => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        InputMode::Normal => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    };

    let scheme_style = Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD);

    let mode_str = match app.input_mode {
        InputMode::Editing => "EDITING",
        InputMode::Normal => "NORMAL",
    };

    let (scheme_name, subtitle) = if area.width < 95 {
        (app.input_scheme.short_name(), " ")
    } else {
        (app.input_scheme.name(), " Video Downloader ")
    };

    // Calculate fixed character overhead:
    // " VIDOWN " (8) + subtitle + "[Scheme: " (9) + scheme_name + " | Mode: " (9) + mode_str + "] [Dir: " (8) + "]" (1)
    let fixed_len = 8 + subtitle.len() + 9 + scheme_name.len() + 9 + mode_str.len() + 8 + 1;
    let available_title_width = (area.width as usize).saturating_sub(4);
    let max_dir_len = available_title_width.saturating_sub(fixed_len);

    let dir_display = if app.output_dir.chars().count() > max_dir_len {
        truncate_path_tail(&app.output_dir, max_dir_len)
    } else {
        app.output_dir.clone()
    };

    let title_line = Line::from(vec![
        Span::styled(
            " VIDOWN ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(subtitle),
        Span::styled("[Scheme: ", Style::default().fg(Color::DarkGray)),
        Span::styled(scheme_name, scheme_style),
        Span::styled(" | Mode: ", Style::default().fg(Color::DarkGray)),
        Span::styled(mode_str, mode_style),
        Span::styled("] ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Dir: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            dir_display,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("]", Style::default().fg(Color::DarkGray)),
    ]);

    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title(title_line);

    f.render_widget(header_block, area);
}

/// Renders the URL input text field with active cursor positioning.
fn render_input(f: &mut Frame, app: &App, area: Rect) {
    let (border_color, title_text) = match app.input_mode {
        InputMode::Editing => (
            Color::Green,
            " URL Input (Editing - Press [Enter] to download, [Esc] to cancel) ",
        ),
        InputMode::Normal => (
            Color::DarkGray,
            " URL Input (Normal - Press [i] or [Enter] to edit) ",
        ),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title_text);

    let input_text = if app.input_buffer.is_empty() && app.input_mode == InputMode::Normal {
        Paragraph::new(Span::styled(
            "Paste or type video URL here (press 'i' to edit)...",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Paragraph::new(app.input_buffer.as_str())
    };

    let inner_area = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(input_text, inner_area);

    // Set terminal cursor in editing mode
    if app.input_mode == InputMode::Editing {
        let cursor_x =
            inner_area.x + (app.cursor_position as u16).min(inner_area.width.saturating_sub(1));
        let cursor_y = inner_area.y;
        f.set_cursor_position((cursor_x, cursor_y));
    }
}

/// Renders the list of active and past downloads with live progress gauges.
fn render_downloads(f: &mut Frame, app: &App, area: Rect) {
    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White))
        .title(format!(
            " Downloads ({}) - Use [↑/↓] or [j/k] to select, [e] for logs/errors ",
            app.downloads.len()
        ));

    if app.downloads.is_empty() {
        let empty_msg = Paragraph::new("No active or past downloads. Enter a URL above to start!")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray))
            .block(main_block);
        f.render_widget(empty_msg, area);
        return;
    }

    let inner_area = main_block.inner(area);
    f.render_widget(main_block, area);

    // Allocate vertical space for each item (gauge + info line = ~3 lines per item)
    let visible_items_count = (inner_area.height / 3).max(1) as usize;
    let start_idx = if app.selected_download >= visible_items_count {
        app.selected_download + 1 - visible_items_count
    } else {
        0
    };
    let end_idx = (start_idx + visible_items_count).min(app.downloads.len());

    let item_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(3); end_idx - start_idx])
        .split(inner_area);

    for (idx, item_idx) in (start_idx..end_idx).enumerate() {
        if idx >= item_chunks.len() {
            break;
        }
        let item = &app.downloads[item_idx];
        let is_selected = item_idx == app.selected_download;
        render_download_item(f, item, is_selected, item_chunks[idx]);
    }
}

/// Renders a single download entry with its status badge and progress gauge.
fn render_download_item(
    f: &mut Frame,
    item: &crate::app::DownloadItem,
    is_selected: bool,
    area: Rect,
) {
    let border_style = if is_selected {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let status_color = match &item.status {
        ItemStatus::Queued => Color::Yellow,
        ItemStatus::Downloading => Color::Cyan,
        ItemStatus::Merging => Color::Magenta,
        ItemStatus::Completed => Color::Green,
        ItemStatus::Failed(_) => Color::Red,
    };

    let track_info = match &item.status {
        ItemStatus::Downloading => format!(" [Track {}] ", item.track),
        ItemStatus::Merging => " [Merging Audio/Video] ".to_string(),
        ItemStatus::Completed => " [Done] ".to_string(),
        ItemStatus::Failed(_) => " [Failed] ".to_string(),
        ItemStatus::Queued => " [Queued] ".to_string(),
    };

    let display_name = if let Some(name) = &item.filename {
        format!("{:.45} ", name)
    } else {
        format!("{:.45} ", item.url)
    };

    let title = Line::from(vec![
        Span::styled(
            if is_selected { "> " } else { "  " },
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("#{}: ", item.id),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(display_name),
        Span::styled(
            track_info,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({})", item.status.label()),
            Style::default().fg(status_color),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);

    let inner = block.inner(area);
    f.render_widget(block, area);

    let gauge_color = match &item.status {
        ItemStatus::Completed => Color::Green,
        ItemStatus::Failed(_) => Color::Red,
        ItemStatus::Merging => Color::Magenta,
        _ => Color::Cyan,
    };

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(gauge_color).bg(Color::Black))
        .percent(item.progress as u16)
        .label(format!("{:.1}%", item.progress));

    f.render_widget(gauge, inner);
}

/// Renders the bottom footer with keybindings help and global status.
fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let keybindings_help = match app.input_scheme {
        crate::app::InputScheme::StandardModal => match app.input_mode {
            InputMode::Normal => {
                "[i] Edit  [Enter] Submit  [j/k] Move  [e] Logs  [p/F3] Path  [g/F4] History  [?/F1] Help  [F2] Vim  [q] Quit"
            }
            InputMode::Editing => {
                "[Enter] Submit  [Esc] Normal  [F1] Help  [F3] Path  [F4] History  [←/→] Cursor  [Backspace] Del"
            }
        },
        crate::app::InputScheme::Vim => match app.input_mode {
            InputMode::Normal => {
                "[i/a] Insert  [j/k] Select  [x] Del  [p/F3] Path  [g/F4] History  [?/F1] Help  [e] Logs  [F2] Modal  [q] Quit"
            }
            InputMode::Editing => {
                "[Esc] Normal  [Enter] Submit  [F1] Help  [F3] Path  [F4] History  [←/→] Cursor"
            }
        },
    };

    let status_line = app.status_message.as_deref().unwrap_or(keybindings_help);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            " [HELP] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(status_line),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(footer, area);
}

/// Renders a centered modal dialog displaying full error logs and details.
fn render_modal(f: &mut Frame, modal: &crate::app::DetailModal) {
    let area = centered_rect(75, 70, f.area());

    // Clear underneath the modal to avoid background bleed-through
    f.render_widget(Clear, area);

    let modal_block = Block::default()
        .title(format!(" {} ", modal.title))
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    let inner = modal_block.inner(area);
    f.render_widget(modal_block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Info: URL, Status, Output Dir
            Constraint::Min(4),    // Log content
            Constraint::Length(1), // Close instruction
        ])
        .split(inner);

    let info_text = vec![
        Line::from(vec![
            Span::styled(
                "URL: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&modal.url),
        ]),
        Line::from(vec![
            Span::styled(
                "Status: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&modal.status),
        ]),
        Line::from(vec![
            Span::styled(
                "Directory: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&modal.output_dir),
        ]),
    ];
    let info_paragraph = Paragraph::new(info_text).wrap(Wrap { trim: true });
    f.render_widget(info_paragraph, chunks[0]);

    let log_items: Vec<ListItem> = if modal.logs.is_empty() {
        vec![ListItem::new("No stderr/logs recorded for this download.")]
    } else {
        modal
            .logs
            .iter()
            .skip(modal.scroll_offset)
            .map(|log| {
                let style = if log.contains("[ERROR]") || log.contains("ERROR:") {
                    Style::default().fg(Color::Red)
                } else if log.contains("[Merger]") {
                    Style::default().fg(Color::Magenta)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(log.as_str()).style(style)
            })
            .collect()
    };

    let logs_list = List::new(log_items).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Logs / Stderr "),
    );
    f.render_widget(logs_list, chunks[1]);

    let footer_text = Paragraph::new(Line::from(Span::styled(
        "Press [Esc] or [Enter] to dismiss this modal",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )))
    .alignment(Alignment::Center);
    f.render_widget(footer_text, chunks[2]);
}

/// Renders the download directory configuration modal dialog.
/// Uses a fixed bounded height of at least 9 rows on standard 80x24 terminals.
fn render_path_modal(f: &mut Frame, modal: &crate::app::PathModal) {
    let modal_area = centered_rect_bounded(64, 9, f.area());
    if modal_area.width < 10 || modal_area.height < 5 {
        return;
    }

    // Clear underneath the modal to avoid background bleed-through
    f.render_widget(Clear, modal_area);

    let modal_block = Block::default()
        .title(" Set Download Directory ")
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let inner = modal_block.inner(modal_area);
    f.render_widget(modal_block, modal_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Instruction
            Constraint::Length(3), // Bordered text input field
            Constraint::Length(1), // Hint
            Constraint::Length(1), // Action Shortcuts
            Constraint::Length(1), // Navigation instructions
        ])
        .split(inner);

    // 1. Instruction line
    let instruction = Paragraph::new(Span::styled(
        "Destination directory for subsequent downloads:",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(instruction, chunks[0]);

    // 2. Input Box with Border and Horizontal Viewport Scrolling
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(" Directory Path ");

    let input_inner = input_block.inner(chunks[1]);
    f.render_widget(input_block, chunks[1]);

    let visible_width = input_inner.width as usize;
    if visible_width > 0 {
        let cursor_pos = modal.cursor_position;
        let mut scroll_offset = modal.scroll_offset;

        if cursor_pos < scroll_offset {
            scroll_offset = cursor_pos;
        } else if cursor_pos >= scroll_offset + visible_width {
            scroll_offset = cursor_pos.saturating_sub(visible_width.saturating_sub(1));
        }

        let visible_text: String = modal
            .input
            .chars()
            .skip(scroll_offset)
            .take(visible_width)
            .collect();

        let input_widget = if modal.input.is_empty() {
            Paragraph::new(Span::styled(
                "./downloads (default)",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ))
        } else {
            Paragraph::new(visible_text)
        };
        f.render_widget(input_widget, input_inner);

        // Position terminal hardware cursor
        let cursor_offset = cursor_pos.saturating_sub(scroll_offset);
        let cursor_screen_x =
            input_inner.x + (cursor_offset as u16).min(input_inner.width.saturating_sub(1));
        let cursor_screen_y = input_inner.y;
        f.set_cursor_position((cursor_screen_x, cursor_screen_y));
    }

    // 3. Hint
    let hint = Paragraph::new(Span::styled(
        "Empty resets to ./downloads. Paths are literal (no ~ expansion).",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    ));
    f.render_widget(hint, chunks[2]);

    // 4. Action Shortcuts
    let shortcuts = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Enter] ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Save  "),
        Span::styled(
            "[Esc] ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("Cancel  "),
        Span::styled(
            "[Ctrl+D] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Default  "),
        Span::styled(
            "[Ctrl+U] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Clear"),
    ]));
    f.render_widget(shortcuts, chunks[3]);

    // 5. Navigation Help
    let nav_help = Paragraph::new(Span::styled(
        "[←/→] Cursor  [Backspace/Del] Edit  [Home/End] Jump",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(nav_help, chunks[4]);
}

/// Truncates a file path string from the beginning, keeping the tail and prepending "…"
/// if it exceeds `max_chars`.
pub fn truncate_path_tail(path: &str, max_chars: usize) -> String {
    let char_count = path.chars().count();
    if char_count <= max_chars {
        path.to_string()
    } else if max_chars == 0 {
        String::new()
    } else if max_chars == 1 {
        "…".to_string()
    } else {
        let skip_count = char_count.saturating_sub(max_chars.saturating_sub(1));
        let tail: String = path.chars().skip(skip_count).collect();
        format!("…{}", tail)
    }
}

/// Helper function to generate a centered rect with fixed bounding dimensions,
/// clamped so it never exceeds the outer area.
pub fn centered_rect_bounded(width: u16, height: u16, r: Rect) -> Rect {
    if r.width <= 2 || r.height <= 2 {
        return Rect::default();
    }
    let actual_width = width.min(r.width.saturating_sub(2)).max(1);
    let actual_height = height.min(r.height.saturating_sub(2)).max(1);
    let x = r.x + (r.width.saturating_sub(actual_width)) / 2;
    let y = r.y + (r.height.saturating_sub(actual_height)) / 2;
    Rect {
        x,
        y,
        width: actual_width,
        height: actual_height,
    }
}

/// Helper function to generate a centered rect of given percentage dimensions.
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Renders the download history modal dialog with a two-panel layout:
/// - Left/Top: Scrollable list of past downloads with status indicators.
/// - Right/Bottom: Inspector panel detailing title, URL, exact file destination, status, timestamp, and errors.
fn render_history_modal(f: &mut Frame, app: &App, modal: &crate::app::HistoryModal) {
    let area = centered_rect(82, 75, f.area());
    if area.width < 24 || area.height < 8 {
        return;
    }

    // Clear underneath the modal to avoid background bleed-through
    f.render_widget(Clear, area);

    let modal_block = Block::default()
        .title(format!(" Download History ({}) ", app.history.len()))
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let inner = modal_block.inner(area);
    f.render_widget(modal_block, area);

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),    // Two-panel body area
            Constraint::Length(1), // Bottom action shortcuts
        ])
        .split(inner);

    let panels_area = main_chunks[0];
    let footer_area = main_chunks[1];

    // Bottom action shortcuts with responsive width adaptation
    let shortcuts_line = if footer_area.width >= 80 {
        Line::from(vec![
            Span::styled(
                "[↑/↓ or j/k] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Select  "),
            Span::styled(
                "[r] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Retry  "),
            Span::styled(
                "[o/Enter] ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Open File  "),
            Span::styled(
                "[d] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("Delete  "),
            Span::styled(
                "[q/Esc/g/F4] ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Close"),
        ])
    } else if footer_area.width >= 58 {
        Line::from(vec![
            Span::styled(
                "[j/k] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Select  "),
            Span::styled(
                "[r] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Retry  "),
            Span::styled(
                "[o/↵] ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Open  "),
            Span::styled(
                "[d] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("Del  "),
            Span::styled(
                "[q/Esc] ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Close"),
        ])
    } else {
        Line::from(vec![
            Span::styled("[j/k] ", Style::default().fg(Color::Yellow)),
            Span::raw("Sel "),
            Span::styled("[r] ", Style::default().fg(Color::Cyan)),
            Span::raw("Retry "),
            Span::styled("[o] ", Style::default().fg(Color::Green)),
            Span::raw("Open "),
            Span::styled("[d] ", Style::default().fg(Color::Red)),
            Span::raw("Del "),
            Span::styled("[q] ", Style::default().fg(Color::DarkGray)),
            Span::raw("Exit"),
        ])
    };

    let shortcuts = Paragraph::new(shortcuts_line).alignment(Alignment::Center);
    f.render_widget(shortcuts, footer_area);

    // Two-panel layout: horizontal if wide enough, vertical if narrow
    let (list_area, inspector_area) = if panels_area.width >= 70 {
        let horizontal_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(45), // List panel
                Constraint::Percentage(55), // Inspector panel
            ])
            .split(panels_area);
        (horizontal_split[0], horizontal_split[1])
    } else {
        let vertical_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // List panel
                Constraint::Percentage(50), // Inspector panel
            ])
            .split(panels_area);
        (vertical_split[0], vertical_split[1])
    };

    // 1. List Panel
    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White))
        .title(" Past Downloads ");

    if app.history.is_empty() {
        let empty_msg = Paragraph::new(
            "No download history recorded yet.\nCompleted and failed downloads will appear here.",
        )
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray))
        .wrap(Wrap { trim: true })
        .block(list_block);
        f.render_widget(empty_msg, list_area);
    } else {
        let list_inner = list_block.inner(list_area);
        f.render_widget(list_block, list_area);

        let visible_count = (list_inner.height as usize).max(1);
        let selected = modal.selected.min(app.history.len().saturating_sub(1));
        let mut scroll = modal.scroll_offset.get();

        if selected < scroll {
            scroll = selected;
        } else if selected >= scroll + visible_count {
            scroll = selected + 1 - visible_count;
        }
        if scroll + visible_count > app.history.len() {
            scroll = app.history.len().saturating_sub(visible_count);
        }
        modal.scroll_offset.set(scroll);

        let start_idx = scroll;
        let end_idx = (start_idx + visible_count).min(app.history.len());

        let max_text_width = (list_inner.width as usize).saturating_sub(12);

        let mut list_items = Vec::new();
        for item_idx in start_idx..end_idx {
            let entry = &app.history[item_idx];
            let is_selected = item_idx == selected;

            let (status_badge, status_color) = match entry.status {
                crate::history::HistoryStatus::Completed => ("[Done]", Color::Green),
                crate::history::HistoryStatus::Failed => ("[Failed]", Color::Red),
            };

            let prefix = if is_selected { "> " } else { "  " };
            let raw_display = entry.title.as_deref().unwrap_or(&entry.url);
            let display_name = truncate_str(raw_display, max_text_width);

            let item_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let line = Line::from(vec![
                Span::styled(
                    prefix,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<8} ", status_badge),
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(display_name, item_style),
            ]);
            list_items.push(ListItem::new(line));
        }

        let list_widget = List::new(list_items);
        f.render_widget(list_widget, list_inner);
    }

    // 2. Inspector Panel
    let inspector_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Inspector Details ");

    if app.history.is_empty() {
        let empty_inspector = Paragraph::new("Select an item from history to view full details.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: true })
            .block(inspector_block);
        f.render_widget(empty_inspector, inspector_area);
    } else {
        let selected = modal.selected.min(app.history.len().saturating_sub(1));
        let entry = &app.history[selected];

        let (status_text, status_color) = match entry.status {
            crate::history::HistoryStatus::Completed => ("Completed", Color::Green),
            crate::history::HistoryStatus::Failed => ("Failed", Color::Red),
        };

        let file_path_str = entry.file_path.as_deref().unwrap_or("N/A");
        let file_exists = entry
            .file_path
            .as_ref()
            .map(|p| std::path::Path::new(p).exists())
            .unwrap_or(false);

        let (exists_badge, exists_color) = if file_exists {
            ("Exists on disk", Color::Green)
        } else if entry.status == crate::history::HistoryStatus::Failed {
            ("Not created", Color::DarkGray)
        } else {
            ("Not found on disk", Color::DarkGray)
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    "Title: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(entry.title.as_deref().unwrap_or("N/A")),
            ]),
            Line::from(vec![
                Span::styled(
                    "URL: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&entry.url),
            ]),
            Line::from(vec![
                Span::styled(
                    "Destination: ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(file_path_str),
            ]),
            Line::from(vec![
                Span::styled(
                    "Status: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    status_text,
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  ("),
                Span::styled(exists_badge, Style::default().fg(exists_color)),
                Span::raw(")"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Timestamp: ",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&entry.timestamp),
            ]),
        ];

        if let Some(err) = &entry.error_message {
            lines.push(Line::from(vec![
                Span::styled(
                    "Error Details: ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(err),
            ]));
        }

        let inspector_paragraph = Paragraph::new(lines)
            .block(inspector_block)
            .wrap(Wrap { trim: true });
        f.render_widget(inspector_paragraph, inspector_area);
    }
}

/// Truncates a string to at most `max_chars`, appending an ellipsis if truncated.
fn truncate_str(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else if max_chars == 0 {
        String::new()
    } else if max_chars == 1 {
        "…".to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

/// Renders the dedicated Dependency Setup & Onboarding screen.
pub fn render_setup_screen(f: &mut Frame, app: &App) {
    let area = f.area();
    if area.width < 10 || area.height < 6 {
        return;
    }

    let is_compact = area.height < 28;
    let logo_height = if is_compact { 3 } else { 8 };
    let tools_height = if is_compact { 7 } else { 8 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(logo_height), // ASCII banner / Header
            Constraint::Length(tools_height), // Dependency Status Panel
            Constraint::Min(6),              // Quick Start & Keybindings Guide
            Constraint::Length(3),           // Footer instructions / actions
        ])
        .split(area);

    render_setup_banner(f, chunks[0], is_compact);
    render_setup_tools(f, app, chunks[1]);
    render_setup_guide(f, app, chunks[2]);
    render_setup_footer(f, app, chunks[3]);
}

/// Renders the ASCII logo banner or a compact header at the top of the setup screen.
fn render_setup_banner(f: &mut Frame, area: Rect, is_compact: bool) {
    if is_compact {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let p = Paragraph::new(Line::from(vec![
            Span::styled(
                " VIDOWN ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  Dependency Setup & Quick Start Guide",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Center)
        .block(block);
        f.render_widget(p, area);
    } else {
        let banner_lines = vec![
            Line::from(Span::styled(
                r#" __      ___     _                     "#,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                r#" \ \    / (_)   | |                    "#,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                r#"  \ \  / / _  __| | _____      ___ __  "#,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                r#"   \ \/ / | |/ _` |/ _ \ \ /\ / / '_ \ "#,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                r#"    \  /  | | (_| | (_) \ V  V /| | | |"#,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                r#"     \/   |_|\__,_|\___/ \_/\_/ |_| |_|"#,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled(
                    "Terminal Video Downloader",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  •  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "First-Time Setup & Onboarding",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
        ];
        let p = Paragraph::new(banner_lines).alignment(Alignment::Center);
        f.render_widget(p, area);
    }
}

/// Renders the status of required tools on the setup screen.
fn render_setup_tools(f: &mut Frame, app: &App, area: Rect) {
    let setup = match &app.setup_state {
        Some(s) => s,
        None => return,
    };

    let border_color = match setup.phase {
        crate::app::SetupPhase::Complete => Color::Green,
        crate::app::SetupPhase::Error(_) => Color::Red,
        _ => Color::Cyan,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" Required Tools & External Dependencies ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();
    for tool in &setup.tools {
        let (icon, icon_style, status_text, status_style) = match &tool.status {
            crate::deps::ToolSetupStatus::Found(loc) => (
                " ✔ ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                format!("[Ready: {}]", loc),
                Style::default().fg(Color::Green),
            ),
            crate::deps::ToolSetupStatus::Installed => (
                " ✔ ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                "[Installed in local data bin]".to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            crate::deps::ToolSetupStatus::Downloading { percent } => (
                " ⟳ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                format!("[Downloading: {:.1}%]", percent),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            crate::deps::ToolSetupStatus::Extracting => (
                " ⟳ ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
                "[Extracting archive...]".to_string(),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            crate::deps::ToolSetupStatus::PendingDownload => (
                " ○ ",
                Style::default().fg(Color::Yellow),
                "[Pending download]".to_string(),
                Style::default().fg(Color::Yellow),
            ),
            crate::deps::ToolSetupStatus::Failed(err) => (
                " ✖ ",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
                format!("[Failed: {}]", err),
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            crate::deps::ToolSetupStatus::Checking => (
                " … ",
                Style::default().fg(Color::DarkGray),
                "[Checking system...]".to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        };

        lines.push(Line::from(vec![
            Span::styled(icon, icon_style),
            Span::styled(
                format!("{:<38}", tool.display_name),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(status_text, status_style),
        ]));
    }

    let p = Paragraph::new(lines);
    f.render_widget(p, inner);
}

/// Renders the Quick Start / How to Use guide on the setup screen.
fn render_setup_guide(f: &mut Frame, app: &App, area: Rect) {
    let scroll_offset = app
        .setup_state
        .as_ref()
        .map(|s| s.help_scroll)
        .unwrap_or(0);

    let title = if scroll_offset > 0 {
        format!(
            " Quick Start & Keybinding Guide (Scrolled: {}) - [↑/↓ or j/k] to scroll ",
            scroll_offset
        )
    } else {
        " Quick Start & Keybinding Guide - [↑/↓ or j/k] to scroll ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White))
        .title(title);

    let inner = block.inner(area);
    f.render_widget(block, area);

    let guide_lines = vec![
        Line::from(vec![
            Span::styled(
                " • Downloading Videos: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                "Press [i] or [Enter] to enter URL editing mode. Type or paste your video link, then press [Enter] to start downloading.",
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " • List Navigation:    ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                "Use [j] or [↓] to move down, [k] or [↑] to move up through active and past downloads. Press [e] to view detailed logs and error traces.",
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " • Keybinding Schemes: ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                "Press [F2] anytime to toggle between Modal (Standard) mode and Vim navigation (h/j/k/l/x/0/$).",
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " • Download Location:  ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                "Press [p] or [F3] to open the destination folder modal. Changes are automatically saved to your system config file.",
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " • Download History:   ",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                "Press [g] or [F4] to open the persistent History Inspector. Press [r] to re-enqueue, [o] to open the file in explorer, [d] to delete.",
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " • Persistent Help:    ",
                Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Press [?] or [F1] from the main screen at any time to reopen this guide."),
        ]),
        Line::from(vec![
            Span::styled(
                " • Clean Exit:         ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                "Press [q] from normal mode or [Ctrl+C] from anywhere to safely restore the terminal and quit.",
            ),
        ]),
    ];

    let p = Paragraph::new(guide_lines)
        .wrap(Wrap { trim: true })
        .scroll((scroll_offset as u16, 0));

    f.render_widget(p, inner);
}

/// Renders the action footer of the setup screen.
fn render_setup_footer(f: &mut Frame, app: &App, area: Rect) {
    let setup = match &app.setup_state {
        Some(s) => s,
        None => return,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let content_line = match &setup.phase {
        crate::app::SetupPhase::Complete => Line::from(vec![
            Span::styled(
                " [READY] ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" All required tools are configured! "),
            Span::styled(
                "Press [Enter] to launch Vidown >>",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        crate::app::SetupPhase::Error(msg) => Line::from(vec![
            Span::styled(
                " [ERROR] ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {} ", msg), Style::default().fg(Color::Red)),
            Span::styled(
                "[r] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Retry  "),
            Span::styled(
                "[c] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Continue anyway  "),
            Span::styled(
                "[q] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("Quit"),
        ]),
        _ => Line::from(vec![
            Span::styled(
                " [WORKING] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                " Downloading & verifying dependencies in local data directory... Please wait.",
            ),
        ]),
    };

    let p = Paragraph::new(content_line).block(block);
    f.render_widget(p, area);
}

/// Renders the persistent help guide modal overlay on the main screen.
pub fn render_help_modal(f: &mut Frame, modal: &crate::app::HelpModal) {
    let area = centered_rect(82, 80, f.area());
    if area.width < 24 || area.height < 10 {
        return;
    }

    // Clear underneath the modal
    f.render_widget(Clear, area);

    let modal_block = Block::default()
        .title(" How to Use Vidown / Keybinding Guide ")
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let inner = modal_block.inner(area);
    f.render_widget(modal_block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),    // Scrollable help content
            Constraint::Length(1), // Footer instructions
        ])
        .split(inner);

    let help_lines = vec![
        Line::from(vec![Span::styled(
            "VIDOWN - Fast Asynchronous Terminal Video Downloader",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "1. URL Input & Downloading",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                "   • [i] or [Enter]: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Enter Editing mode. Paste or type your target video URL."),
        ]),
        Line::from(vec![
            Span::styled(
                "   • [Enter] (while editing): ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Submit URL to start background download task."),
        ]),
        Line::from(vec![
            Span::styled(
                "   • [Esc] (while editing): ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Cancel input and return to Normal navigation mode."),
        ]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "2. Downloads List & Inspection",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                "   • [j] / [↓]: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Select next download item in the list."),
        ]),
        Line::from(vec![
            Span::styled(
                "   • [k] / [↑]: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Select previous download item in the list."),
        ]),
        Line::from(vec![
            Span::styled(
                "   • [e]: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Open full Detail & Log modal for the selected download item."),
        ]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "3. Configuration & History",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                "   • [p] or [F3]: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                "Open Download Directory modal. Set custom folder or reset to default with [Ctrl+D].",
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "   • [g] or [F4]: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                "Open Download History. Browse completed & failed items, [r] retry, [o] open folder.",
            ),
        ]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "4. Keybinding Schemes",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                "   • [F2]: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Toggle between Standard Modal mode and Vim navigation scheme."),
        ]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "5. System & Help",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                "   • [?] or [F1]: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Toggle this How to Use / Keybinding Guide modal."),
        ]),
        Line::from(vec![
            Span::styled(
                "   • [q] or [Ctrl+C]: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Safely exit Vidown and restore terminal."),
        ]),
    ];

    let content = Paragraph::new(help_lines)
        .wrap(Wrap { trim: true })
        .scroll((modal.scroll_offset as u16, 0));
    f.render_widget(content, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![Span::styled(
        "Press [Esc], [Enter], [q], or [?/F1] to close  •  [↑/↓ or j/k] to scroll",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )]))
    .alignment(Alignment::Center);
    f.render_widget(footer, chunks[1]);
}
