# Vidown

<p align="center">
  <img src="assets/VIDOWN.jpeg" alt="Vidown Logo" width="500">
</p>

> **An Asynchronous Terminal-Based Video Downloader in Rust with Zero-Config Automated Setup**

Vidown is a fast, robust, and responsive Terminal User Interface (TUI) video downloader built in Rust. It pairs the power of [`ratatui`](https://github.com/ratatui/ratatui) and [`tokio`](https://tokio.rs/) with automated dependency management, media extraction via [`yt-dlp`](https://github.com/yt-dlp/yt-dlp), multiplexing via [`ffmpeg`](https://ffmpeg.org/), and JavaScript challenge handling via [`node`](https://nodejs.org/).

Vidown adheres strictly to the **Model-View-Update (MVU / Elm)** architecture, running non-blocking background downloads through asynchronous MPSC channels while keeping the terminal UI rendering at high frame rates with zero flickering or interface freezing.

---

## Key Features

- **Zero-Config Automated Dependency Setup**:
  - Automatically checks your system on startup for `yt-dlp`, `ffmpeg` (with `ffprobe`), and `node`.
  - **Hybrid Detection**: Reuses existing system binaries if they are in your `$PATH`. If missing, automatically downloads official releases and configures them into your local user data directory (`~/.local/share/vidown/bin` on Linux/WSL, `%LOCALAPPDATA%\Vidown\bin` on Windows).
  - Displays a dedicated first-time onboarding screen showing ASCII branding, live download/extraction progress, and a quick-start guide.
- **Asynchronous & Non-Blocking**: Network streams and media extraction run in decoupled background Tokio tasks communicating with the UI via immutable message channels.
- **Concurrent Multi-Download Management**: Queue and download multiple videos simultaneously; each download item tracks its own status, stream tracks, and progress independently.
- **Dual-Track Download & Muxing Visualization**:
  - `Track 1`: High-definition video stream download.
  - `Track 2`: High-quality audio stream download.
  - `Merging`: Real-time notification when FFmpeg multiplexes audio and video into a clean `.mp4` container.
- **Dual Keybinding Schemes**:
  - **Standard Modal**: Simple `i`/`Enter` to edit URLs, `Esc` for Normal mode, `q` to quit.
  - **Vim Mode**: Full Vim navigation (`0`, `$`, `x`, `a`, `i`, `j`, `k`).
  - Press <kbd>F2</kbd> anytime to toggle between schemes.
- **Persistent In-App Help Guide**:
  - Press <kbd>?</kbd> or <kbd>F1</kbd> at any time from the main screen to open an interactive, scrollable quick-start cheat sheet and keybinding guide.
- **Persistent Download History Inspector**:
  - Press <kbd>g</kbd> or <kbd>F4</kbd> to inspect past downloads.
  - Re-enqueue previous URLs with <kbd>r</kbd>, delete entries with <kbd>d</kbd>, or open the file directly in your desktop file manager with <kbd>o</kbd> (`explorer.exe`, `xdg-open`, or WSL Explorer bridge).
  - Persisted in JSON format with automatic FIFO limit pruning.
- **Configurable & Persistent Download Directory**:
  - Set a custom download destination directory via a dedicated modal dialog (<kbd>p</kbd> or <kbd>F3</kbd>).
  - Automatically creates destination directory upon saving.
  - Persists configured path across app restarts (`%APPDATA%\vidown\config.toml` on Windows, `~/.config/vidown/config.toml` on Linux/macOS).
- **Expandable Modal Log Viewer**:
  - Press <kbd>e</kbd> on any download in the list to inspect raw stdout/stderr logs, destination directory, and captured error traces.
- **Safe Terminal Lifecycle**:
  - Uses double-buffering and raw mode via `crossterm`.
  - Installs panic hooks via `color-eyre` ensuring your terminal is cleanly restored even in the event of an abnormal crash.
- **Rate-Limited Event Throttling**: Progress events are throttled to 150 ms intervals to prevent thread starvation and high CPU usage.

---

## Prerequisites

Vidown is designed to be **self-sufficient**. You only need the standard Rust toolchain to compile:

| Tool | Purpose | Automatic Setup |
|---|---|---|
| **Rust & Cargo** (1.80+) | Compilation & runtime execution | Required manually |
| **yt-dlp** | Core media extraction & API resolution | **Auto-downloaded if missing** |
| **FFmpeg & FFprobe** | Audio/video stream multiplexing | **Auto-downloaded if missing** |
| **Node.js** | JS runtime for YouTube n-sig extraction | **Auto-downloaded if missing** |

*Note: If you already have `yt-dlp`, `ffmpeg`, or `node` installed on your system (e.g. via `apt` in WSL/Linux, `brew` on macOS, or `winget` on Windows), Vidown will automatically detect and use them.*

---

## Installation & Setup

### 1. Clone the Repository
```bash
git clone https://github.com/TheNobodyBaruah/Vidown.git
cd Vidown
```

### 2. Build the Application
Compile the optimized release binary:
```bash
cargo build --release
```
The compiled binary will be located at `target/release/video_downloader`.

### 3. Run the Automated Tests
Verify that all 104 unit and integration tests pass:
```bash
cargo test
```

### 4. (Optional) Install System-Wide Command
To launch Vidown simply by typing `Vidown` in your terminal from any directory:

```bash
# On Linux / WSL:
sudo cp target/release/video_downloader /usr/local/bin/Vidown
sudo ln -sf /usr/local/bin/Vidown /usr/local/bin/vidown
```

Now you can start the application anytime with:
```bash
Vidown
```

---

## How to Use Vidown

### 1. First-Time Launch (Setup & Onboarding)
When launching Vidown for the first time without pre-installed tools:
```bash
cargo run --release
```
Vidown detects missing dependencies and automatically opens the **Onboarding Setup Screen**:
- Renders the ASCII banner and begins downloading missing tools to your user data folder.
- Displays an interactive **Quick Start & Keybinding Guide** so you can learn the shortcuts while files download.
- When setup finishes, press <kbd>Enter</kbd> to launch into the main application.
- If a download error occurs (e.g. network timeout), press <kbd>r</kbd> to retry, <kbd>c</kbd> to continue anyway, or <kbd>q</kbd> to exit.

### 2. Main Screen Overview
The main terminal interface is organized into 4 intuitive sections:
1. **Header**: Application title, active keybinding scheme, input mode (`[NORMAL]` vs `[EDITING]`), and current download directory.
2. **URL Input Field**: Where you paste and submit video links.
3. **Downloads & Progress Area**: Shows all active and past downloads with live gauges and track statuses.
4. **Help / Status Footer**: Context-aware keybindings and recent notifications.

---

### 3. Downloading a Video (Step-by-Step)

1. **Enter Editing Mode**:
   - Press <kbd>i</kbd> (or <kbd>Enter</kbd>).
   - The border of the **URL Input** box turns **green**, indicating you are in `[EDITING]` mode.

2. **Input the Video URL**:
   - Paste or type the target URL into the input field:
     - In Linux / Windows Terminal: press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> or right-click to paste.

3. **Submit the Download**:
   - Press <kbd>Enter</kbd>.
   - The URL is enqueued, the input box clears, and the download task starts immediately in the background.

4. **Track Download Progress**:
   - Vidown displays live progress percentages:
     - `[Track 1]`: Downloading primary video stream.
     - `[Track 2]`: Downloading primary audio stream.
     - `[Merging Audio/Video]`: FFmpeg multiplexes both tracks into an MP4 container.
     - `[Done]`: Completed and recorded into history!

5. **Download More Videos Concurrently**:
   - Press <kbd>i</kbd> again, enter another URL, and press <kbd>Enter</kbd>. Both downloads will proceed simultaneously.

---

### 4. Inspecting Logs & Details
If a download fails or you want to inspect raw output from `yt-dlp`:
1. Use <kbd>j</kbd> / <kbd>k</kbd> (or <kbd>↓</kbd> / <kbd>↑</kbd>) to select the download item in the list.
2. Press <kbd>e</kbd> to open the **Details & Logs Modal**.
3. Use <kbd>j</kbd> / <kbd>k</kbd> to scroll through captured stdout/stderr traces.
4. Press <kbd>Esc</kbd> or <kbd>Enter</kbd> to close the modal.

---

### 5. Download History Inspector
1. Press <kbd>g</kbd> or <kbd>F4</kbd> from Normal mode to open the **Download History Inspector**.
2. Navigate entries using <kbd>j</kbd> / <kbd>k</kbd> or arrow keys.
3. **Actions**:
   - <kbd>o</kbd> or <kbd>Enter</kbd>: Open the completed video file in your system file manager (`explorer.exe` or `xdg-open`).
   - <kbd>r</kbd>: Re-enqueue the selected URL into the active download queue.
   - <kbd>d</kbd>: Delete the entry from history.
   - <kbd>Esc</kbd> or <kbd>q</kbd>: Close the history inspector.

---

### 6. Switching Keybinding Schemes (Standard vs. Vim)
Press <kbd>F2</kbd> at any time in Normal mode to toggle schemes:
- **Standard Modal Scheme** (Default):
  - <kbd>i</kbd> or <kbd>Enter</kbd>: Focus URL input bar.
  - <kbd>Esc</kbd>: Exit URL input bar.
  - <kbd>j</kbd> / <kbd>k</kbd> or <kbd>↓</kbd> / <kbd>↑</kbd>: Navigate downloads.
  - <kbd>e</kbd>: Inspect logs.
  - <kbd>q</kbd>: Quit.
- **Vim-Style Scheme**:
  - <kbd>i</kbd>: Insert mode at current cursor position.
  - <kbd>a</kbd>: Append mode (cursor moves one right).
  - <kbd>0</kbd> / <kbd>$</kbd>: Jump cursor to beginning / end of buffer.
  - <kbd>x</kbd>: Delete character under cursor.
  - <kbd>j</kbd> / <kbd>k</kbd>: Navigate downloads list.
  - <kbd>q</kbd>: Quit.

---

## Keyboard Shortcuts Reference

| Key | Mode | Description |
|---|---|---|
| <kbd>i</kbd> or <kbd>Enter</kbd> | Normal | Enter URL editing mode |
| <kbd>Enter</kbd> | Editing | Submit URL and start download |
| <kbd>Esc</kbd> | Editing / Modal | Return to Normal mode / dismiss active modal |
| <kbd>←</kbd> / <kbd>→</kbd> | Editing | Move cursor in URL input bar |
| <kbd>Backspace</kbd> | Editing | Delete character before cursor |
| <kbd>Delete</kbd> | Editing | Delete character under cursor |
| <kbd>Home</kbd> / <kbd>End</kbd> | Editing | Jump cursor to start / end of URL |
| <kbd>?</kbd> or <kbd>F1</kbd> | Global | Open / toggle persistent Help & Keybinding Guide modal |
| <kbd>F2</kbd> | Normal | Toggle between Standard Modal and Vim schemes |
| <kbd>p</kbd> or <kbd>F3</kbd> | Global | Open / toggle download directory configuration modal |
| <kbd>g</kbd> or <kbd>F4</kbd> | Global | Open / toggle Download History Inspector modal |
| <kbd>j</kbd> / <kbd>↓</kbd> | Normal / Modal | Select next download item / scroll modal down |
| <kbd>k</kbd> / <kbd>↑</kbd> | Normal / Modal | Select previous download item / scroll modal up |
| <kbd>PageDown</kbd> / <kbd>PageUp</kbd> | Modal | Scroll modal by 5 lines |
| <kbd>e</kbd> | Normal | View logs and stderr traces for selected download |
| <kbd>0</kbd> / <kbd>$</kbd> | Normal (Vim) | Move cursor to beginning / end of URL input |
| <kbd>x</kbd> | Normal (Vim) | Delete character under cursor |
| <kbd>q</kbd> or <kbd>Ctrl</kbd> + <kbd>c</kbd> | Normal | Cleanly restore terminal and exit application |

---

## Output Directory & Configuration

By default, downloaded media files are saved in the **`downloads/`** subdirectory:
- Default Path: `./downloads/%(title)s.%(ext)s`

### Custom Download Path:
- Press <kbd>p</kbd> in Normal mode or <kbd>F3</kbd> anywhere to open the **Set Download Directory** modal.
- Type or paste your desired path (surrounding quotes are stripped; paths are treated literally without `~` expansion).
- Blank resets to `./downloads`.
- Pressing <kbd>Enter</kbd> automatically creates the directory if it does not exist.
- Persisted to standard user configuration:
  - **Windows**: `%APPDATA%\vidown\config.toml`
  - **Linux / macOS**: `~/.config/vidown/config.toml`

---

## Project Structure

```text
Vidown/
├── Cargo.toml               # Project dependencies and metadata
├── PLAN.md                  # Comprehensive architectural specification
├── README.md                # Documentation and user guide
├── PROJECT_STR.md           # Engineering architectural guide
├── walkthrough.md           # Implementation verification walkthrough
├── src/
│   ├── lib.rs               # Library root re-exporting modules
│   ├── main.rs              # Application entry point & Tokio select! loop
│   ├── app.rs               # Model: application state, items, modals, and input modes
│   ├── deps.rs              # Dependency bootstrapping, hybrid detection, and archive extraction
│   ├── history.rs           # History persistence: JSON serialization, FIFO limits, file manager launch
│   ├── config.rs            # Persistence: OS user config directory & TOML file manager
│   ├── ui.rs                # View: pure layout rendering, setup screen, and modal widgets
│   ├── events.rs            # Update: event stream, dispatch, and keybindings
│   ├── downloader.rs        # Domain Layer: yt-dlp & FFmpeg asynchronous execution
│   └── terminal.rs          # Safe terminal lifecycle and panic recovery
└── tests/
    ├── app_tests.rs         # Unit tests for state transitions, path modal, and key handling
    ├── deps_tests.rs        # Unit tests for dependency detection, setup screen, and help modal
    ├── history_tests.rs     # Unit tests for history persistence, FIFO limits, and file manager
    ├── config_tests.rs      # Unit tests for TOML serialization, parsing, and persistence
    ├── downloader_tests.rs  # Unit tests for progress regex & parsing logic
    └── ui_tests.rs          # Buffer-level, TestBackend, and 80x24 layout integration tests
```

---

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

*Disclaimer: This tool is intended for educational purposes and personal use with content you own or have explicit rights to download. Respect copyright laws and the terms of service of hosting platforms.*
