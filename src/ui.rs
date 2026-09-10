// src/ui.rs

use crate::app::{App, InputMode, ItemStatus};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Pure View function rendering the entire TUI from the App state.
pub fn render(f: &mut Frame, app: &App) {
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

    if let Some(path_modal) = &app.path_modal {
        render_path_modal(f, path_modal);
    } else if let Some(modal) = &app.detail_modal {
        render_modal(f, modal);
    }
}

/// Renders the top title, scheme/mode indicator, and active download directory header.
fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let mode_style = match app.input_mode {
        InputMode::Editing => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        InputMode::Normal => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    };

    let scheme_style = Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD);

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
        Span::styled(" VIDOWN ", Style::default().fg(Color::White).bg(Color::Blue).add_modifier(Modifier::BOLD)),
        Span::raw(subtitle),
        Span::styled("[Scheme: ", Style::default().fg(Color::DarkGray)),
        Span::styled(scheme_name, scheme_style),
        Span::styled(" | Mode: ", Style::default().fg(Color::DarkGray)),
        Span::styled(mode_str, mode_style),
        Span::styled("] ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Dir: ", Style::default().fg(Color::DarkGray)),
        Span::styled(dir_display, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
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
        InputMode::Editing => (Color::Green, " URL Input (Editing - Press [Enter] to download, [Esc] to cancel) "),
        InputMode::Normal => (Color::DarkGray, " URL Input (Normal - Press [i] or [Enter] to edit) "),
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
        let cursor_x = inner_area.x + (app.cursor_position as u16).min(inner_area.width.saturating_sub(1));
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
fn render_download_item(f: &mut Frame, item: &crate::app::DownloadItem, is_selected: bool, area: Rect) {
    let border_style = if is_selected {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("#{}: ", item.id), Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(display_name),
        Span::styled(track_info, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
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
            InputMode::Normal => "[i] Edit  [Enter] Submit  [j/↓] Next  [k/↑] Prev  [e] Logs  [p/F3] Path  [F2] Vim  [q] Quit",
            InputMode::Editing => "[Enter] Submit  [Esc] Normal  [F3] Path  [←/→] Cursor  [Backspace] Delete",
        },
        crate::app::InputScheme::Vim => match app.input_mode {
            InputMode::Normal => "[i/a] Insert  [j/k] Select  [x] Del  [p/F3] Path  [e] Logs  [F2] Modal  [q] Quit",
            InputMode::Editing => "[Esc] Normal  [Enter] Submit  [F3] Path  [←/→] Cursor",
        },
    };

    let status_line = app.status_message.as_deref().unwrap_or(keybindings_help);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" [HELP] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(status_line),
    ]))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));

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
        .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

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
            Span::styled("URL: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(&modal.url),
        ]),
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(&modal.status),
        ]),
        Line::from(vec![
            Span::styled("Directory: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
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

    let logs_list = List::new(log_items)
        .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::DarkGray)).title(" Logs / Stderr "));
    f.render_widget(logs_list, chunks[1]);

    let footer_text = Paragraph::new(Line::from(Span::styled(
        "Press [Esc] or [Enter] to dismiss this modal",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
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
        .border_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

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
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
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

        let visible_text: String = modal.input.chars().skip(scroll_offset).take(visible_width).collect();

        let input_widget = if modal.input.is_empty() {
            Paragraph::new(Span::styled(
                "./downloads (default)",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            ))
        } else {
            Paragraph::new(visible_text)
        };
        f.render_widget(input_widget, input_inner);

        // Position terminal hardware cursor
        let cursor_offset = cursor_pos.saturating_sub(scroll_offset);
        let cursor_screen_x = input_inner.x + (cursor_offset as u16).min(input_inner.width.saturating_sub(1));
        let cursor_screen_y = input_inner.y;
        f.set_cursor_position((cursor_screen_x, cursor_screen_y));
    }

    // 3. Hint
    let hint = Paragraph::new(Span::styled(
        "Empty resets to ./downloads. Paths are literal (no ~ expansion).",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
    ));
    f.render_widget(hint, chunks[2]);

    // 4. Action Shortcuts
    let shortcuts = Paragraph::new(Line::from(vec![
        Span::styled("[Enter] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("Save  "),
        Span::styled("[Esc] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw("Cancel  "),
        Span::styled("[Ctrl+D] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Default  "),
        Span::styled("[Ctrl+U] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
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
