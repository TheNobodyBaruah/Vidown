# Vidown Codebase Architecture & Pedagogical Guide (`PROJECT_STR.md`)

Welcome to the internal engineering guide for **Vidown**. This document serves as an exhaustive architectural blueprint and code walkthrough. Its purpose is to teach you how the entire codebase is structured, how data flows through each module, the rationale behind critical engineering decisions, and how to navigate and extend the system with confidence.

---

## Table of Contents
1. [High-Level Architectural Mental Model](#1-high-level-architectural-mental-model)
2. [Codebase Navigation Map](#2-codebase-navigation-map)
3. [Deep Dive: Module-by-Module Walkthrough](#3-deep-dive-module-by-module-walkthrough)
   - [3.1 Entry Point: `src/main.rs`](#31-entry-point-srcmainrs)
   - [3.2 The Model: `src/app.rs`](#32-the-model-srcapprs)
   - [3.3 The View: `src/ui.rs`](#33-the-view-srcuirs)
   - [3.4 The Controller / Event Loop: `src/events.rs`](#34-the-controller--event-loop-srceventsrs)
   - [3.5 Domain Layer: `src/downloader.rs`](#35-domain-layer-srcdownloaderrs)
   - [3.6 Terminal Lifecycle: `src/terminal.rs`](#36-terminal-lifecycle-srcterminalrs)
   - [3.7 Public Crate Root: `src/lib.rs`](#37-public-crate-root-srclibrs)
4. [Testing Architecture & Strategies](#4-testing-architecture--strategies)
   - [4.1 State & Interaction Tests (`tests/app_tests.rs`)](#41-state--interaction-tests-testsapp_testsrs)
   - [4.2 Domain Regex Tests (`tests/downloader_tests.rs`)](#42-domain-regex-tests-testsdownloader_testsrs)
   - [4.3 Headless Buffer & TestBackend Tests (`tests/ui_tests.rs`)](#43-headless-buffer--testbackend-tests-testsui_testsrs)
5. [Key Design Decisions & Engineering Rationale](#5-key-design-decisions--engineering-rationale)
6. [Tracing a User Request (The Byte & Event Lifecycle)](#6-tracing-a-user-request-the-byte--event-lifecycle)
7. [Extension Guide: Adding New Features](#7-extension-guide-adding-new-features)

---

## 1. High-Level Architectural Mental Model

Vidown is architected using the **Model-View-Update (MVU)** pattern (popularized by the Elm programming language), combined with the **Actor Model** using the asynchronous **Tokio** runtime:

```
                  ┌──────────────────────────────────────────────┐
                  │                 User Inputs                  │
                  │   (Keystrokes, Terminal Resizes via Crossterm)│
                  └──────────────────────┬───────────────────────┘
                                         │
                                         ▼
                               ┌───────────────────┐
                               │  src/events.rs    │
                               │  Event Dispatch   │
                               └─────────┬─────────┘
                                         │
                 ┌───────────────────────┴───────────────────────┐
                 │                                               │
                 ▼ (Mutates)                                     ▼ (Spawns)
        ┌─────────────────┐                             ┌─────────────────┐
        │   src/app.rs    │                             │src/downloader.rs│
        │   (The Model)   │                             │ (Domain Layer)  │
        └────────┬────────┘                             └────────┬────────┘
                 │                                               │
                 │ Immutable &App                                │ MPSC DownloadEvents
                 ▼                                               ▼
        ┌─────────────────┐                             ┌─────────────────┐
        │    src/ui.rs    │                             │  Tokio Select!  │
        │   (The View)    │◄────────────────────────────┤  (src/main.rs)  │
        └────────┬────────┘      Triggers Redraw        └─────────────────┘
                 │
                 ▼
        ┌─────────────────┐
        │ Terminal Screen │ (Double-buffered diff rendered to stdout)
        └─────────────────┘
```

### The Three Pillars:
1. **The Model (`App` in `src/app.rs`)**: Single source of truth. Owns the text input buffer, concurrent download list, modal state, cursor position, and UI flags.
2. **The View (`ui::render` in `src/ui.rs`)**: A strictly pure function: `f(&mut Frame, &App)`. It takes an immutable reference to `App` and translates state directly into visual widgets. It contains **no business logic**, performs **no side effects**, and **never blocks**.
3. **The Update (`src/events.rs` & `src/main.rs`)**: The asynchronous circulatory system. It consumes keyboard events and background task channel messages, updates the Model, and triggers the View to render the next frame.

---

## 2. Codebase Navigation Map

```text
Vidown/
├── Cargo.toml               # Crate configuration and dependency specifications
├── .cargo/config.toml       # Compiler/linker configuration (clang linker target)
├── PLAN.md                  # Detailed architectural design and milestone roadmap
├── README.md                # User manual, setup instructions, and keybindings
├── PROJECT_STR.md           # This document (engineering deep dive)
├── src/
│   ├── lib.rs               # Library root re-exporting modules for binary & integration tests
│   ├── main.rs              # Tokio runtime entry point, terminal init, and event loop
│   ├── app.rs               # The Model: App struct, DownloadItem, PathModal, input modes
│   ├── config.rs            # Persistence: User config directory & TOML file serializer/parser
│   ├── ui.rs                # The View: Ratatui layout, Gauge rendering, and modal overlays
│   ├── events.rs            # The Update: Key event dispatcher, Modal & Vim keybinding schemes
│   ├── downloader.rs        # Domain Layer: Asynchronous yt-dlp & FFmpeg process manager
│   └── terminal.rs          # Low-level terminal setup, raw mode, and panic recovery hooks
└── tests/
    ├── app_tests.rs         # Unit tests for state transitions, path modal, and key handling
    ├── config_tests.rs      # Unit tests for TOML serialization, escaping, and persistence
    ├── downloader_tests.rs  # Unit tests for yt-dlp stdout progress parsing regex
    └── ui_tests.rs          # Headless Buffer rendering tests & TestBackend simulation
```

---

## 3. Deep Dive: Module-by-Module Walkthrough

---

### 3.1 Entry Point: `src/main.rs`

#### Purpose
`main.rs` is the application orchestrator. Its responsibility is strictly restricted to:
1. Initializing error tracking (`color_eyre`).
2. Setting up the terminal via `terminal::init()`.
3. Instantiating the shared `App` state and MPSC communication channel.
4. Running the asynchronous `tokio::select!` event loop.
5. Guaranteeing terminal restoration upon exit.

#### Key Code Structure & Logic
```rust
#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = terminal::init()?;
    let mut app = App::new();
    let (tx, mut rx) = mpsc::channel::<DownloadEvent>(128);
    let mut event_reader = EventStream::new();
    let mut tick_interval = tokio::time::interval(Duration::from_millis(16));
```
- **Tokio Multi-threaded Runtime**: `#[tokio::main]` initializes a work-stealing thread pool capable of multiplexing lightweight green tasks across all CPU cores.
- **`EventStream::new()`**: Converts raw terminal keystrokes and resize events from `crossterm` into a non-blocking asynchronous stream implementing `futures::Stream`.
- **`tick_interval`**: Enforces a 60 FPS clock tick (16 ms) ensuring that even when user input is idle, download animations and progress gauges render fluidly.

#### The `tokio::select!` Loop
```rust
tokio::select! {
    // Branch 1: User Terminal Inputs
    Some(crossterm_event) = event_reader.next() => { ... }

    // Branch 2: Background Download Task Updates
    Some(dl_event) = rx.recv() => { ... }

    // Branch 3: Clock Tick (Framerate enforcement)
    _ = tick_interval.tick() => {}
}
```
**Why `tokio::select!`?**
In a traditional synchronous loop, calling `rx.recv()` or `event::read()` would block the entire operating system thread. If no keystroke is pressed, a network progress update could not be drawn. If no network packet arrives, the user could not press `q` to quit. `tokio::select!` awaits multiple futures simultaneously; whichever branch completes first executes its block and immediately loops, delivering true real-time interactivity.

---

### 3.2 The Model: `src/app.rs`

#### Purpose
`app.rs` contains the **state representation** of the entire application. It contains no terminal rendering code and no process spawning code. It provides deterministic methods to mutate the state.

#### Core Structs & Enums

##### `InputScheme` & `InputMode`
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputScheme {
    StandardModal,
    Vim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}
```
- `InputScheme`: Allows users to switch between standard TUI navigation and Vim-style navigation (<kbd>F2</kbd>).
- `InputMode`: Controls whether keypresses are interpreted as text insertion into the URL bar (`Editing`) or navigation commands (`Normal`).

##### `DownloadItem` & `ItemStatus`
```rust
pub struct DownloadItem {
    pub id: usize,
    pub url: String,
    pub filename: Option<String>,
    pub track: u8,
    pub progress: f64,
    pub status: ItemStatus,
    pub logs: Vec<String>,
}

pub enum ItemStatus {
    Queued,
    Downloading,
    Merging,
    Completed,
    Failed(String),
}
```
- Each download is an independent entity tagged with an incrementing integer `id`.
- `track`: Stores whether track 1 (video) or track 2 (audio) is downloading.
- `logs`: A ring-buffer capped at 200 entries to prevent unbounded RAM consumption over hours of uptime.
- `filename`: Parsed dynamically from `yt-dlp` output when available, replacing the raw URL in the UI for cleaner presentation.

##### `DetailModal`
```rust
pub struct DetailModal {
    pub title: String,
    pub url: String,
    pub status: String,
    pub logs: Vec<String>,
    pub scroll_offset: usize,
}
```
When `app.detail_modal` is `Some(modal)`, the View renders a centered dialog overlay. `scroll_offset` tracks the vertical viewport scroll position for inspecting long stderr dumps.

---

### 3.3 The View: `src/ui.rs`

#### Purpose
`ui.rs` is responsible for taking `&App` and drawing to a `ratatui::Frame`.

#### The 4-Section Layout
The terminal screen is divided vertically using `ratatui::layout::Layout`:
```rust
let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(3), // 1. Header (Title, Scheme, Mode)
        Constraint::Length(3), // 2. URL Input Field
        Constraint::Min(8),    // 3. Downloads & Progress list (Expands dynamically)
        Constraint::Length(3), // 4. Status / Help Footer
    ])
    .split(f.area());
```

#### Widget Details:
1. **Header (`render_header`)**:
   - Displays application branding: `VIDOWN Video Downloader`.
   - Displays real-time status badges: `[Scheme: Modal | Mode: NORMAL]`.
   - Formatted to fit comfortably within standard 80-column terminal windows without text truncation.
2. **URL Input Field (`render_input`)**:
   - Changes border color dynamically: **Green** in `Editing` mode, **Dark Gray** in `Normal` mode.
   - When editing, calls `f.set_cursor_position((cursor_x, cursor_y))` so the hardware terminal cursor blinks at the exact character being typed.
3. **Downloads List (`render_downloads`)**:
   - Calculates visible item count based on remaining terminal height (`Constraint::Min(8)`).
   - Each item renders a `Gauge` widget showing live percentage:
     - Color-coded: Cyan for downloading, Magenta for FFmpeg merging, Green for completed, Red for failed.
     - Formats track indicators: `[Track 1]`, `[Track 2]`, `[Merging Audio/Video]`.
4. **Footer (`render_footer`)**:
   - Shows context-sensitive keyboard shortcuts tailored to the active `InputScheme` and `InputMode`.
5. **Modal Overlay (`render_modal`)**:
   - Uses `centered_rect(75, 70, f.area())` to create a centered popup occupying 75% width and 70% height.
   - **Crucial step**: Renders `ratatui::widgets::Clear` before rendering the modal block. In immediate-mode double-buffered TUIs, without `Clear`, the background text behind the modal would bleed through.
   - Applies `modal.scroll_offset` to allow navigating through hundreds of lines of error logs.

---

### 3.4 The Controller / Event Loop: `src/events.rs`

#### Purpose
`events.rs` maps raw input events from `crossterm` to state mutations in `App`.

#### `handle_key_event`
```rust
pub fn handle_key_event(app: &mut App, key: KeyEvent) -> Option<(usize, String)>
```
- **KeyEventKind Deduplication**: Windows and modern terminals emit both `KeyEventKind::Press` and `KeyEventKind::Release`. `events.rs` explicitly filters out everything except `KeyEventKind::Press` to eliminate double-character typing bugs.
- **Global Shortcuts**:
  - `Ctrl+C`: Instantly sets `app.should_quit = true`.
  - `F2`: Toggles between `StandardModal` and `Vim` keybinding schemes.
- **Modal Event Handling**: If `app.detail_modal.is_some()`, keys like `Esc`/`Enter` dismiss the modal, while `j`/`k` or `Down`/`Up` scroll the logs.
- **Editing Mode Handling**:
  - Typing characters inserts them at `app.cursor_position`.
  - `Backspace` removes the preceding character; `Delete` removes the current character.
  - `Left` / `Right` / `Home` / `End` adjust cursor indices with bounds checks.
  - `Enter` calls `app.submit_input()`, returning `Some((id, url))` which signals `main.rs` to spawn the download worker.
- **Vim Mode Handling**:
  - `i`: Enters editing mode.
  - `a`: Moves cursor forward by one and enters editing mode.
  - `0` / `$`: Jumps to beginning / end of buffer.
  - `x`: Deletes the character under the cursor without entering insert mode.

---

### 3.5 Domain Layer: `src/downloader.rs`

#### Purpose
`downloader.rs` handles the external world: spawning `yt-dlp` child processes, reading stdout/stderr streams, detecting tool dependencies, rate-limiting progress updates, and cleaning up processes on exit.

#### Key Features & Implementation:

##### 1. Runtime Detection (`is_node_available`)
```rust
fn is_node_available() -> bool {
    std::process::Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
```
Video platforms frequently require JavaScript interpretation for signature decryption. If Node.js is present on the system, `--js-runtime node` is passed; if absent, it gracefully falls back without failing to spawn.

##### 2. Asynchronous Process Execution
`tokio::process::Command` is used instead of `std::process::Command`. This ensures the child process does not block OS threads:
```rust
let mut child = cmd.spawn()?;
let stdout = child.stdout.take();
let stderr = child.stderr.take();
```

##### 3. Dedicated Stderr Capture Task
A detached task asynchronously buffers stderr lines and dispatches them as `DownloadEvent::Log`:
```rust
let stderr_handle = tokio::spawn(async move {
    let mut captured_errors = Vec::new();
    if let Some(err_stream) = stderr {
        let mut reader = BufReader::new(err_stream).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            captured_errors.push(line.clone());
            let _ = tx_stderr.send(DownloadEvent::Log { id, message: line }).await;
        }
    }
    captured_errors
});
```
**Why this matters**: If `yt-dlp` printed directly to stderr, the raw ANSI escape codes would write directly over the Ratatui double-buffer, permanently corrupting the terminal UI. Capturing stderr keeps the display clean while capturing full diagnostic backtraces.

##### 4. Precompiled Regex & Event Throttling
```rust
pub static PROGRESS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+(?:\.\d+)?)%").expect("Failed to compile progress regex")
});
```
- `PROGRESS_RE` is compiled once globally using `std::sync::LazyLock`, avoiding regex recompilation overhead on every stdout line.
- **Throttling Logic**: High-speed internet transfers emit dozens of stdout lines per millisecond. Emitting an MPSC channel message for each chunk saturates the Tokio executor. `downloader.rs` throttles updates so messages are only dispatched if:
  1. $\ge 150\text{ ms}$ have passed since the last update, OR
  2. The rounded percentage integer changed (e.g. $42.8\% \to 43.1\%$), OR
  3. The download reached $100\%$.

##### 5. Zombie Process Prevention
```rust
if tx.send(DownloadEvent::Progress { ... }).await.is_err() {
    let _ = child.kill().await;
    return;
}
```
If the user closes the application, the receiving end of the MPSC channel is dropped. The next time the background task tries to send an event, `send().is_err()` detects this and immediately issues `child.kill().await`, ensuring no background orphan processes continue downloading gigabytes of data in secret.

---

### 3.6 Terminal Lifecycle: `src/terminal.rs`

#### Purpose
`terminal.rs` abstracts entering and exiting terminal raw mode and managing alternate screens.

#### Raw Mode vs. Cooked Mode
- **Cooked Mode (Default)**: The OS buffers input until the user hits `Enter`, echoing every typed character directly to stdout.
- **Raw Mode**: Bypasses OS line buffering and local echo. Every keystroke is dispatched to the application immediately.

#### The Alternate Screen Buffer
When Vidown starts, it issues ANSI escape sequences (`EnterAlternateScreen`) telling the terminal emulator to switch to a secondary buffer. When Vidown exits (`LeaveAlternateScreen`), the terminal restores the user's previous shell history completely intact.

#### Panic Recovery Hook
```rust
pub fn init() -> Result<Tui> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore();
        original_hook(panic_info);
    }));
    ...
}
```
**Why this is critical**: If a Rust application panics while the terminal is in raw mode, the terminal remains broken—keystrokes won't echo, `Enter` won't create newlines, and the user must blindly type `reset`. By intercepting panics, `terminal.rs` guarantees `restore()` runs before the crash report is printed.

---

### 3.7 Public Crate Root: `src/lib.rs`

#### Purpose
```rust
pub mod app;
pub mod downloader;
pub mod events;
pub mod terminal;
pub mod ui;
```
Exposes all internal modules as a library crate (`video_downloader`). This enables tests in the `tests/` directory to import modules cleanly (`use video_downloader::app::App;`) rather than having to include them via awkward relative file paths.

---

## 4. Testing Architecture & Strategies

Vidown employs three tiers of automated tests, achieving 100% test passing rates with zero warnings:

### 4.1 State & Interaction Tests (`tests/app_tests.rs`)
- **`test_app_initial_state`**: Verifies default values for input scheme, mode, and empty buffers.
- **`test_input_mode_and_typing`**: Simulates typing `"http://test.com"`, pressing backspace, and exiting with `Esc`.
- **`test_submit_download_and_concurrent_management`**: Enqueues multiple downloads, simulates independent progress updates, track switching (video $\to$ audio), and FFmpeg merging transitions.
- **`test_vim_keybinding_scheme`**: Toggles to Vim mode with `F2`, exercises `0`, `$`, `x`, `a`, and `i`.
- **`test_detail_modal_open_scroll_and_dismiss`**: Generates mock log streams, opens the modal with `e`, verifies scroll offsets with `j`/`k`, and dismisses with `Esc`.

### 4.2 Domain Regex Tests (`tests/downloader_tests.rs`)
- **`test_progress_regex_matches_various_formats`**: Validates extraction across integer percentages (`100%`), decimals (`45.8%`), leading spaces (`  0.1%`), and gigabyte transfers (`9.5% of 1.20GiB`).
- **`test_progress_regex_ignores_non_progress_lines`**: Asserts that lines like `[download] Destination: ...` and `[Merger] ...` do not falsely match the progress regex.

### 4.3 Headless Buffer & TestBackend Tests (`tests/ui_tests.rs`)
- **`test_render_to_raw_buffer`**: Instantiates a headless `TestBackend(100, 24)` and renders `ui::render`. Converts the memory buffer cells into a multiline string and deterministically asserts that `"VIDOWN"`, `"NORMAL"`, `"Track 1"`, and `"64.0%"` are drawn at exact locations.
- **`test_render_editing_mode_and_modal_overlay`**: Asserts that `[EDITING]` mode changes the header and that opening `DetailModal` correctly draws the modal overlay and error text without crashing.
- **`test_interactive_session_with_test_backend`**: Simulates an entire interactive user journey (Launch $\to$ Press `i` $\to$ Type URL $\to$ Press `Enter` $\to$ Press `F2` $\to$ Press `q`) through `TestBackend` and asserts state mutations at every step.

---

## 5. Key Design Decisions & Engineering Rationale

| Architectural Decision | Chosen Strategy | Alternative Considered | Rationale |
|---|---|---|---|
| **Concurrency Communication** | Tokio MPSC Channels | `Arc<Mutex<App>>` | Shared state concurrency introduces lock contention and deadlocks between the 60 FPS UI thread and network workers. MPSC channels treat workers as isolated actors passing immutable values. |
| **Media Extraction** | External `yt-dlp` subprocess | Pure Rust extraction library | Streaming video platform APIs change weekly (cipher signatures, DASH manifests). Pure Rust crates quickly become obsolete; `yt-dlp` is the actively maintained industry standard. |
| **Error Reporting** | Dual Approach (Inline Badge + Modal Popup) | Simple CLI `eprintln!` | Printing directly to stderr corrupts the TUI screen. Stderr is captured into memory; the list shows a simple `Failed` badge while <kbd>e</kbd> opens full error logs. |
| **Progress Event Throttling** | 150 ms timer / integer step gate | Emit every chunk | High-bandwidth downloads can generate 5,000 progress events per second. Flooding the channel would starve the UI thread and degrade frame rates. |
| **Keybinding Schemes** | Dynamic scheme toggle (`F2`) | Hardcoded single scheme | Balances accessibility for casual terminal users (simple Modal edit) while catering to advanced power users accustomed to Vim navigation. |

---

## 6. Tracing a User Request (The Byte & Event Lifecycle)

Here is the exact step-by-step lifecycle of downloading a video in Vidown:

1. **User Types URL**: User presses <kbd>i</kbd>, enters `Editing` mode, and types `https://...`.
2. **Key Event Captured**: `crossterm` captures keypresses; `events::handle_key_event` appends chars to `app.input_buffer`.
3. **User Presses Enter**:
   - `events::handle_key_event` invokes `app.submit_input()`.
   - `App` creates a new `DownloadItem` with `ItemStatus::Queued` and assigns task ID `1`.
   - `handle_key_event` returns `Some((1, url))`.
4. **Tokio Task Spawned**: `main.rs` receives the tuple, clones the MPSC sender `tx`, and invokes `tokio::spawn(downloader::perform_download(1, url, ...))`.
5. **Subprocess Execution**: `downloader.rs` spawns `yt-dlp`. An asynchronous task monitors `stderr` while the main task monitors `stdout`.
6. **Progress Streaming**:
   - `yt-dlp` emits `[download]  25.4% of 100MiB...`.
   - `downloader.rs` parses `25.4` using `PROGRESS_RE`.
   - Since $>150\text{ ms}$ elapsed, it dispatches `DownloadEvent::Progress { id: 1, track: 1, percent: 25.4 }`.
7. **UI Update**:
   - `main.rs` select loop receives the message from `rx`.
   - Calls `app.update_progress(1, 1, 25.4)`.
8. **Double-Buffered Redraw**:
   - `terminal.draw()` calls `ui::render(&mut f, &app)`.
   - `render_download_item` reads `progress = 25.4` and renders a `Gauge` with a filled width proportional to $25.4\%$.
   - Ratatui calculates the minimal ANSI diff and flushes only the changed cells to stdout.
9. **FFmpeg Merger**:
   - Both tracks finish. `yt-dlp` outputs `[Merger]`.
   - `downloader.rs` emits `DownloadEvent::Merging { id: 1 }`.
   - UI reflects `[Merging Audio/Video]` in Magenta.
10. **Completion**:
    - Process exits with code 0. `downloader.rs` emits `DownloadEvent::Success { id: 1 }`.
    - Item status updates to `Completed`; gauge turns solid Green ($100\%$).

---

## 7. Extension Guide: Adding New Features

### Adding a New Keybinding
1. Open `src/events.rs`.
2. Locate `handle_normal_key` or `handle_editing_key`.
3. Match against `KeyCode::Char('your_key')`.
4. Call or create a helper method on `app` (e.g. `app.delete_selected_download()`).

### Adding a New Widget or View Section
1. Open `src/ui.rs`.
2. Adjust the constraints in `Layout::default().constraints([...])`.
3. Create your new drawing function `fn render_custom_widget(f: &mut Frame, app: &App, area: Rect)`.
4. Call it inside `pub fn render(f: &mut Frame, app: &App)`.

### Adding Custom yt-dlp Flags (e.g. Custom Quality or Format)
1. Open `src/downloader.rs`.
2. Locate the command builder in `perform_download`.
3. Add `.arg("--your-flag")`.
4. If you want this to be user-configurable, add the field to `DownloadItem` or `App` in `src/app.rs` and pass it into `perform_download`.
