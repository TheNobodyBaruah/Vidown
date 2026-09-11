use color_eyre::Result;
use crossterm::event::EventStream;
use futures::StreamExt;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use video_downloader::{
    app::App,
    downloader,
    events::{DownloadEvent, handle_key_event},
    terminal, ui,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize color-eyre for clear, graceful error reports
    color_eyre::install()?;

    // Initialize raw mode, alternate screen, and panic hooks
    let mut terminal = terminal::init()?;
    let mut app = App::new();

    // MPSC channel for background download tasks to communicate with UI
    let (tx, mut rx) = mpsc::channel::<DownloadEvent>(128);

    // MPSC channel for background dependency setup worker to communicate with UI
    let (setup_tx, mut setup_rx) = mpsc::channel::<video_downloader::deps::SetupEvent>(32);

    // If initial dependencies need to be installed, spawn background setup task
    if let Some(setup) = &app.setup_state
        && setup.phase != video_downloader::app::SetupPhase::Complete
    {
        video_downloader::deps::spawn_setup_task(setup_tx.clone());
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
                        if let Some((id, url)) = handle_key_event(&mut app, key_event) {
                            // Spawn decoupled background task for the new download
                            let task_tx = tx.clone();
                            let output_dir = PathBuf::from(&app.output_dir);
                            tokio::spawn(downloader::perform_download(id, url, output_dir, task_tx));
                        }
                        if app.take_setup_retry() {
                            video_downloader::deps::spawn_setup_task(setup_tx.clone());
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
                    DownloadEvent::Progress { id, track, percent } => {
                        app.update_progress(id, track, percent);
                    }
                    DownloadEvent::Merging { id } => {
                        app.update_merging(id);
                    }
                    DownloadEvent::Success { id } => {
                        app.update_success(id);
                    }
                    DownloadEvent::Error { id, error } => {
                        app.update_error(id, error);
                    }
                    DownloadEvent::Filename { id, name } => {
                        app.update_filename(id, name);
                    }
                    DownloadEvent::Log { id, message } => {
                        app.append_log(id, message);
                    }
                }
            }

            // Branch 3: Clock tick to ensure smooth redraws even when I/O is idle
            _ = tick_interval.tick() => {
                // Redraw on next loop iteration
            }
        }
    }

    // Safely restore terminal to cooked mode and exit alternate screen
    terminal::restore()?;
    Ok(())
}
