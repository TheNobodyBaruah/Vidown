// src/terminal.rs

use color_eyre::Result;
use crossterm::{
    cursor::Show,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{Stdout, stdout};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initializes the terminal: enables raw mode, enters alternate screen buffer,
/// and installs a panic hook to safely restore the terminal if a panic occurs.
pub fn init() -> Result<Tui> {
    // Install panic hook so panics don't leave the terminal in a broken state
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore();
        original_hook(panic_info);
    }));

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    terminal.hide_cursor()?;
    Ok(terminal)
}

/// Restores the terminal to its normal cooked mode and exits alternate screen.
pub fn restore() -> Result<()> {
    if crossterm::terminal::is_raw_mode_enabled().unwrap_or(false) {
        let mut out = stdout();
        let _ = execute!(out, Show, LeaveAlternateScreen, DisableMouseCapture);
        let _ = disable_raw_mode();
    }
    Ok(())
}
