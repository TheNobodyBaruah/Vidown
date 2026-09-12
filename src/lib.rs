// src/lib.rs

use color_eyre::Result;
use crossterm::event::EventStream;
use futures::StreamExt;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

pub mod app;
pub mod clipboard;
pub mod config;
pub mod deps;
pub mod downloader;
pub mod events;
pub mod history;
pub mod terminal;
pub mod ui;

/// CLI argument action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliAction {
    Version,
    Help,
    RunApp,
}

/// Parses command-line arguments to determine whether to display version, help, or launch the TUI.
pub fn handle_cli_args<I, T>(args: I) -> CliAction
where
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    let args: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--version" | "-v" | "-V" => return CliAction::Version,
            "--help" | "-h" => return CliAction::Help,
            _ => {}
        }
    }
    CliAction::RunApp
}

/// Main application runner: initializes terminal, processes events, and manages download tasks.
pub async fn run() -> Result<()> {
    // Handle simple CLI flags before taking over the terminal
    let args: Vec<String> = std::env::args().collect();
    match handle_cli_args(&args) {
        CliAction::Version => {
            println!("vidown {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        CliAction::Help => {
            println!("Vidown - An Asynchronous Terminal-Based Video Downloader in Rust\n");
            println!("Usage: vidown [OPTIONS]\n");
            println!("Options:");
            println!("  -v, -V, --version    Print version information and exit");
            println!("  -h, --help           Print help information and exit");
            return Ok(());
        }
        CliAction::RunApp => {}
    }

    // Initialize color-eyre for clear, graceful error reports
    color_eyre::install()?;

    // Initialize raw mode, alternate screen, and panic hooks
    let mut terminal = terminal::init()?;
    let mut app = app::App::new();

    // MPSC channel for background download tasks to communicate with UI
    let (tx, mut rx) = mpsc::channel::<events::DownloadEvent>(128);

    // MPSC channel for background dependency setup worker to communicate with UI
    let (setup_tx, mut setup_rx) = mpsc::channel::<deps::SetupEvent>(32);

    // If initial dependencies need to be installed, spawn background setup task
    if let Some(setup) = &app.setup_state
        && setup.phase != app::SetupPhase::Complete
    {
        deps::spawn_setup_task(setup_tx.clone());
    }

    // Asynchronous crossterm event stream for user keyboard/terminal events
    let mut event_reader = EventStream::new();

    // Tick interval enforcing a consistent UI frame rate (approx 60 FPS)
    let mut tick_interval = tokio::time::interval(Duration::from_millis(16));

    // Main Update loop using tokio::select! to multiplex async sources without blocking
    loop {
        // Draw the current application state to the double-buffered terminal
        terminal.draw(|f| ui::render(f, &app))?;

        if app.should_quit {
            break;
        }

        tokio::select! {
            // Branch 1: User interactions (keyboard inputs, terminal resize)
            Some(crossterm_event) = event_reader.next() => {
                match crossterm_event {
                    Ok(crossterm::event::Event::Key(key_event)) => {
                        if let Some((id, url)) = events::handle_key_event(&mut app, key_event) {
                            // Spawn decoupled background task for the new download
                            let task_tx = tx.clone();
                            let output_dir = PathBuf::from(&app.output_dir);
                            tokio::spawn(downloader::perform_download(id, url, output_dir, task_tx));
                        }
                        if app.take_setup_retry() {
                            deps::spawn_setup_task(setup_tx.clone());
                        }
                    }
                    Ok(crossterm::event::Event::Paste(text)) => {
                        events::handle_paste_event(&mut app, &text);
                    }
                    Ok(crossterm::event::Event::Mouse(mouse_event)) => {
                        if let Some((id, url)) = events::handle_mouse_event(&mut app, mouse_event) {
                            let task_tx = tx.clone();
                            let output_dir = PathBuf::from(&app.output_dir);
                            tokio::spawn(downloader::perform_download(id, url, output_dir, task_tx));
                        }
                    }
                    Ok(crossterm::event::Event::Resize(..)) => {
                        // Terminal resize automatically redrawn on next tick/loop
                    }
                    _ => {}
                }
            }

            // Branch 2: Asynchronous events from background dependency setup worker
            Some(setup_event) = setup_rx.recv() => {
                app.handle_setup_event(setup_event);
            }

            // Branch 3: Asynchronous events from background download tasks
            Some(dl_event) = rx.recv() => {
                match dl_event {
                    events::DownloadEvent::Progress { id, track, percent } => {
                        app.update_progress(id, track, percent);
                    }
                    events::DownloadEvent::Merging { id } => {
                        app.update_merging(id);
                    }
                    events::DownloadEvent::Success { id } => {
                        app.update_success(id);
                    }
                    events::DownloadEvent::Error { id, error } => {
                        app.update_error(id, error);
                    }
                    events::DownloadEvent::Filename { id, name } => {
                        app.update_filename(id, name);
                    }
                    events::DownloadEvent::Log { id, message } => {
                        app.append_log(id, message);
                    }
                }
            }

            // Branch 4: Clock tick to ensure smooth redraws even when I/O is idle
            _ = tick_interval.tick() => {
                // Redraw on next loop iteration
            }
        }
    }

    // Safely restore terminal to cooked mode and exit alternate screen
    terminal::restore()?;
    Ok(())
}

