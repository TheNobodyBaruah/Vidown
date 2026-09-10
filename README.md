# Vidown

<p align="center">
  <img src="assets/VIDOWN.jpeg" alt="Vidown Logo width="500">
</p>

> **An Asynchronous Terminal-Based Video Downloader in Rust**

Vidown is a fast, robust, and responsive Terminal User Interface (TUI) video downloader built in Rust. It pairs the power of [`ratatui`](https://github.com/ratatui/ratatui) and [`tokio`](https://tokio.rs/) with the media extraction capabilities of [`yt-dlp`](https://github.com/yt-dlp/yt-dlp) and [`ffmpeg`](https://ffmpeg.org/).

Vidown adheres strictly to the **Model-View-Update (MVU / Elm)** architecture, running non-blocking background downloads through asynchronous MPSC channels while keeping the terminal UI rendering at high frame rates with zero flickering or interface freezing.

---

## Key Features

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
- **Dual Error & Diagnostic Reporting**:
  - Inline status badges in the download list for immediate visibility.
  - Expandable modal log viewer: press <kbd>e</kbd> on any download to inspect raw `yt-dlp` logs, destination directory, and captured stderr traces.
- **Configurable & Persistent Download Directory**:
  - Set a custom download destination directory via a dedicated modal dialog (<kbd>p</kbd> or <kbd>F3</kbd>).
  - Automatically creates destination directory upon saving.
  - Persists configured path across app restarts (`%APPDATA%\vidown\config.toml` on Windows, `~/.config/vidown/config.toml` on Linux/macOS).
  - Automatically tracks and displays per-item download directory in download details modal.
- **Safe Terminal Lifecycle**:
  - Uses double-buffering and raw mode via `crossterm`.
  - Installs panic hooks via `color-eyre` ensuring your terminal is cleanly restored even in the event of an abnormal crash.
- **Rate-Limited Event Throttling**: Progress events are throttled to 150 ms intervals to prevent thread starvation and memory leaks.

---

## Prerequisites

Ensure the following tools are installed and accessible on your system `$PATH`:

| Tool | Purpose | Status |
|---|---|---|
| **yt-dlp** | Core media extraction & API resolution | **Required** |
| **FFmpeg** | Video/audio stream multiplexing (muxing) | **Required** |
| **Rust & Cargo** (1.80+) | Compilation & runtime execution | **Required** |
| **C Compiler / Linker** (`clang` or `gcc`) | Native linking during build | **Required** |
| **Node.js** | JS runtime for signature decryption fallback | *Optional (Recommended)* |

---

## Installing & Updating Dependencies

### 1. `yt-dlp` (Media Extraction Engine)
`yt-dlp` is frequently updated to keep up with YouTube and hosting platform API changes.

- **How to Install**:
  ```bash
  # Using pipx (recommended for isolated environments):
  pipx install "yt-dlp[curl-cffi,default]"

  # Or using pip:
  pip install --upgrade yt-dlp
  ```
- **How to Update**:
  ```bash
  # If installed with pipx:
  pipx upgrade yt-dlp

  # If installed as a standalone binary or via pip:
  yt-dlp -U
  # or:
  pip install --upgrade yt-dlp
  ```
- **Verify Version**:
  ```bash
  yt-dlp --version
  ```

---

### 2. `FFmpeg` (Audio/Video Multiplexing)
Required to merge separate high-definition video and audio streams into playable `.mp4` containers.

- **How to Install**:
  - **Debian / Ubuntu / Kali Linux**:
    ```bash
    sudo apt update && sudo apt install -y ffmpeg
    ```
  - **Arch Linux**:
    ```bash
    sudo pacman -S ffmpeg
    ```
  - **macOS (Homebrew)**:
    ```bash
    brew install ffmpeg
    ```
  - **Windows (Winget / Chocolatey)**:
    ```powershell
    winget install Gyan.FFmpeg
    # or
    choco install ffmpeg
    ```
- **How to Update**:
  - **Linux (APT)**:
    ```bash
    sudo apt update && sudo apt --only-upgrade install -y ffmpeg
    ```
  - **macOS**:
    ```bash
    brew upgrade ffmpeg
    ```
  - **Windows**:
    ```powershell
    winget upgrade Gyan.FFmpeg
    ```
- **Verify Version**:
  ```bash
  ffmpeg -version
  ```

---

### 3. `Node.js` (JavaScript Runtime Fallback)
Provides JavaScript execution for deciphering encrypted video stream signatures.

- **How to Install**:
  - **Debian / Ubuntu / Kali Linux**:
    ```bash
    sudo apt update && sudo apt install -y nodejs npm
    ```
  - **macOS (Homebrew)**:
    ```bash
    brew install node
    ```
  - **Windows**:
    ```powershell
    winget install OpenJS.NodeJS
    ```
- **How to Update**:
  - **Linux (APT)**:
    ```bash
    sudo apt update && sudo apt --only-upgrade install -y nodejs
    ```
  - **macOS**:
    ```bash
    brew upgrade node
    ```
  - **Windows**:
    ```powershell
    winget upgrade OpenJS.NodeJS
    ```
- **Verify Version**:
  ```bash
  node --version
  ```

---

### 4. `Rust & Cargo` (Build Toolchain)
- **How to Install**:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source "$HOME/.cargo/env"
  ```
- **How to Update**:
  ```bash
  rustup update
  ```
- **Verify Version**:
  ```bash
  cargo --version
  rustc --version
  ```

---

### 5. `C Compiler Toolchain` (`clang` / `gcc`)
Required for native crate linking during compilation.

- **Debian / Ubuntu / Kali Linux**:
  ```bash
  sudo apt update && sudo apt install -y build-essential clang
  ```
- **Arch Linux**:
  ```bash
  sudo pacman -S base-devel clang
  ```
- **macOS**:
  ```bash
  xcode-select --install
  ```

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
Verify that all unit and integration tests pass:
```bash
cargo test
```

### 4. (Optional) Install System-Wide Command
To launch Vidown simply by typing `Vidown` in your terminal from any directory:

```bash
# Copy binary or create a symlink to /usr/local/bin
sudo cp target/release/video_downloader /usr/local/bin/Vidown
sudo ln -sf /usr/local/bin/Vidown /usr/local/bin/vidown
```

Now you can start the application anytime with:
```bash
Vidown
```

---

## How to Use Vidown

### 1. Starting the Application
From the repository directory:
```bash
cargo run --release
```
Or if installed system-wide:
```bash
Vidown
```

The terminal will switch into alternate screen raw mode and render the 4-section layout:
1. **Header**: Application title, active keybinding scheme, and current input mode (`[NORMAL]` vs `[EDITING]`).
2. **URL Input Field**: Where you paste and submit video links.
3. **Downloads & Progress Area**: Shows all active and past downloads with live gauges and track statuses.
4. **Help / Status Footer**: Context-aware keybindings and recent notifications.

---

### 2. Downloading a Video (Step-by-Step)

1. **Enter Editing Mode**:
   - Press <kbd>i</kbd> (or <kbd>Enter</kbd>).
   - The border of the **URL Input** box will turn **green**, indicating you are in `[EDITING]` mode.

2. **Input the Video URL**:
   - Paste or type the target URL into the input field:
     - In Linux / Windows Terminal: press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> or right-click to paste.

3. **Submit the Download**:
   - Press <kbd>Enter</kbd>.
   - The URL will be enqueued, the input box will clear, and the download task will start running in the background.

4. **Track Download Progress**:
   - Vidown displays live progress percentages:
     - `[Track 1]`: Downloading primary video stream.
     - `[Track 2]`: Downloading primary audio stream.
     - `[Merging Audio/Video]`: FFmpeg multiplexes both tracks.
     - `[Done]`: Completed and saved!

5. **Download More Videos Concurrently**:
   - Press <kbd>i</kbd> again, enter another URL, and press <kbd>Enter</kbd>. Both downloads will proceed simultaneously without interfering with each other.

---

### 3. Inspecting Logs and Troubleshooting Failures

If a download fails or you want to see detailed output from `yt-dlp`:

1. Use <kbd>j</kbd> / <kbd>k</kbd> (or <kbd>↓</kbd> / <kbd>↑</kbd>) to highlight the download item in the list (indicated by `>`).
2. Press <kbd>e</kbd> to open the **Details & Logs Modal**.
3. Use <kbd>j</kbd> / <kbd>k</kbd> (or <kbd>↓</kbd> / <kbd>↑</kbd>) to scroll through the captured stderr lines and download diagnostics.
4. Press <kbd>Esc</kbd> or <kbd>Enter</kbd> to dismiss the modal and return to the main view.

---

### 4. Switching Keybinding Schemes (Standard vs. Vim)

Press <kbd>F2</kbd> at any time in Normal mode to toggle between schemes:

- **Standard Modal Scheme** (Default):
  - <kbd>i</kbd> or <kbd>Enter</kbd>: Focus URL input bar.
  - <kbd>Esc</kbd>: Exit URL input bar.
  - <kbd>j</kbd> / <kbd>k</kbd> or <kbd>↓</kbd> / <kbd>↑</kbd>: Navigate downloads.
  - <kbd>e</kbd>: Inspect logs.
  - <kbd>q</kbd>: Quit.

- **Vim-Style Scheme**:
  - <kbd>i</kbd>: Insert mode at current cursor position.
  - <kbd>a</kbd>: Append mode (cursor moves one right).
  - <kbd>0</kbd>: Jump cursor to beginning of input buffer.
  - <kbd>$</kbd>: Jump cursor to end of input buffer.
  - <kbd>x</kbd>: Delete character under cursor.
  - <kbd>j</kbd> / <kbd>k</kbd>: Navigate downloads list.
  - <kbd>q</kbd>: Quit.

---

## Keyboard Shortcuts Reference

| Key | Mode | Description |
|---|---|---|
| <kbd>i</kbd> or <kbd>Enter</kbd> | Normal | Enter URL editing mode |
| <kbd>Enter</kbd> | Editing | Submit URL and start download |
| <kbd>Esc</kbd> | Editing / Modal | Return to Normal mode / dismiss modal dialog |
| <kbd>←</kbd> / <kbd>→</kbd> | Editing | Move cursor in URL input bar |
| <kbd>Backspace</kbd> | Editing | Delete character before cursor |
| <kbd>Delete</kbd> | Editing | Delete character under cursor |
| <kbd>Home</kbd> / <kbd>End</kbd> | Editing | Jump cursor to start / end of URL |
| <kbd>F2</kbd> | Normal | Toggle between Standard Modal and Vim schemes |
| <kbd>F3</kbd> | Global | Open / toggle download directory configuration modal |
| <kbd>p</kbd> | Normal | Open download directory configuration modal |
| <kbd>j</kbd> / <kbd>↓</kbd> | Normal / Modal | Select next download item / scroll modal down |
| <kbd>k</kbd> / <kbd>↑</kbd> | Normal / Modal | Select previous download item / scroll modal up |
| <kbd>e</kbd> | Normal | View logs, destination directory, and stderr traces for selected download |
| <kbd>0</kbd> / <kbd>$</kbd> | Normal (Vim) | Move cursor to beginning / end of URL input |
| <kbd>x</kbd> | Normal (Vim) | Delete character under cursor |
| <kbd>q</kbd> or <kbd>Ctrl</kbd> + <kbd>c</kbd> | Normal | Cleanly restore terminal and exit application |

### Path Configuration Modal Shortcuts:
| Key | Action |
|---|---|
| <kbd>Enter</kbd> | Save and apply destination path (automatically creates directory) |
| <kbd>Esc</kbd> | Cancel without saving changes |
| <kbd>Ctrl</kbd> + <kbd>d</kbd> | Reset input buffer to default (`./downloads`) |
| <kbd>Ctrl</kbd> + <kbd>u</kbd> | Clear input buffer |
| <kbd>←</kbd> / <kbd>→</kbd> / <kbd>Home</kbd> / <kbd>End</kbd> | Navigate cursor with safe multi-byte UTF-8 indexing |
| <kbd>Backspace</kbd> / <kbd>Delete</kbd> | Delete character before / under cursor |

---

## Output Directory & Configuration

By default, downloaded media files are saved in the **`downloads/`** subdirectory:
- Default Path: `./downloads/%(title)s.%(ext)s`

### Custom Download Path:
- Press <kbd>p</kbd> in Normal mode or press <kbd>F3</kbd> anywhere to open the **Set Download Directory** modal.
- Type or paste your desired path (surrounding quotes are automatically stripped; paths are treated literally without `~` expansion).
- Leaving the path blank resets to default (`./downloads`).
- When saving (<kbd>Enter</kbd>), Vidown automatically creates the directory via `create_dir_all`. If creation fails (e.g. permission error), a warning is displayed in the status bar.
- Your customized path is persisted to the standard user configuration file:
  - **Windows**: `%APPDATA%\vidown\config.toml`
  - **Linux / macOS**: `~/.config/vidown/config.toml`
- On startup, Vidown loads your saved path automatically.

---

## Project Structure

```text
Vidown/
├── Cargo.toml               # Project dependencies and metadata
├── PLAN.md                  # Comprehensive architectural specification
├── README.md                # Documentation and user guide
├── src/
│   ├── lib.rs               # Library root re-exporting modules
│   ├── main.rs              # Application entry point & Tokio select! loop
│   ├── app.rs               # Model: application state, items, and input modes
│   ├── config.rs            # Persistence: OS user config directory & TOML file manager
│   ├── ui.rs                # View: pure layout rendering & widgets
│   ├── events.rs            # Update: event stream, dispatch, and keybindings
│   ├── downloader.rs        # Domain Layer: yt-dlp & FFmpeg asynchronous execution
│   └── terminal.rs          # Safe terminal lifecycle and panic recovery
└── tests/
    ├── app_tests.rs         # Unit tests for state transitions, path modal, and key handling
    ├── config_tests.rs      # Unit tests for TOML serialization, parsing, and persistence
    ├── downloader_tests.rs  # Unit tests for progress regex & parsing logic
    └── ui_tests.rs          # Buffer-level, TestBackend, and 80x24 layout integration tests
```

---

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

*Disclaimer: This tool is intended for educational purposes and personal use with content you own or have explicit rights to download. Respect copyright laws and the terms of service of hosting platforms.*
