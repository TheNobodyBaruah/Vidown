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

    if let Some(modal) = &app.detail_modal {
        render_modal(f, modal);
    }
}

/// Renders the top title and mode indicator header.
fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let mode_style = match app.input_mode {
        InputMode::Editing => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        InputMode::Normal => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    };

    let scheme_style = Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD);

    let title_line = Line::from(vec![
        Span::styled(" VIDOWN ", Style::default().fg(Color::White).bg(Color::Blue).add_modifier(Modifier::BOLD)),
        Span::raw(" Video Downloader "),
        Span::styled("[Scheme: ", Style::default().fg(Color::DarkGray)),
        Span::styled(app.input_scheme.name(), scheme_style),
        Span::styled(" | Mode: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            match app.input_mode {
                InputMode::Editing => "EDITING",
                InputMode::Normal => "NORMAL",
            },
            mode_style,
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
            InputMode::Normal => "[i] Edit  [Enter] Submit  [j/↓] Next  [k/↑] Prev  [e] View Logs  [F2] Vim  [q] Quit",
            InputMode::Editing => "[Enter] Submit URL  [Esc] Normal Mode  [←/→] Move Cursor  [Backspace] Delete",
        },
        crate::app::InputScheme::Vim => match app.input_mode {
            InputMode::Normal => "[i/a] Insert  [j/k] Select  [x] Del Char  [dd] Clear  [e] Logs  [F2] Modal  [q] Quit",
            InputMode::Editing => "[Esc] Normal Mode  [Enter] Submit URL  [←/→] Move Cursor",
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
            Constraint::Length(3), // Info: URL & Status
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
