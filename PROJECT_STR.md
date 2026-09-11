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
   - [3.6 Automated Dependency Management: `src/deps.rs`](#36-automated-dependency-management-srcdepsrs)
   - [3.7 Download History Management: `src/history.rs`](#37-download-history-management-srchistoryrs)
   - [3.8 User Configuration: `src/config.rs`](#38-user-configuration-srcconfigrs)
   - [3.9 Terminal Lifecycle: `src/terminal.rs`](#39-terminal-lifecycle-srcterminalrs)
   - [3.10 Public Crate Root: `src/lib.rs`](#310-public-crate-root-srclibrs)
4. [Testing Architecture & Strategies](#4-testing-architecture--strategies)
   - [4.1 State & Interaction Tests (`tests/app_tests.rs`)](#41-state--interaction-tests-testsapp_testsrs)
   - [4.2 Dependency Management Tests (`tests/deps_tests.rs`)](#42-dependency-management-tests-testsdeps_testsrs)
   - [4.3 Download History Tests (`tests/history_tests.rs`)](#43-download-history-tests-testshistory_testsrs)
   - [4.4 Configuration Persistence Tests (`tests/config_tests.rs`)](#44-configuration-persistence-tests-testsconfig_testsrs)
   - [4.5 Domain Regex & Process Tests (`tests/downloader_tests.rs`)](#45-domain-regex--process-tests-testsdownloader_testsrs)
   - [4.6 Headless Buffer & TestBackend Tests (`tests/ui_tests.rs`)](#46-headless-buffer--testbackend-tests-testsui_testsrs)
5. [Key Design Decisions & Engineering Rationale](#5-key-design-decisions--engineering-rationale)
6. [Tracing a User Request (The Byte & Event Lifecycle)](#6-tracing-a-user-request-the-byte--event-lifecycle)
7. [Extension Guide: Adding New Features](#7-extension-guide-adding-new-features)

---

## 1. High-Level Architectural Mental Model

Vidown is architected using the **Model-View-Update (MVU)** pattern (popularized by Elm), combined with the **Actor Model** using the asynchronous **Tokio** runtime:

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
        │   (The Model)   │                             │  src/deps.rs    │
        └────────┬────────┘                             └────────┬────────┘
                 │                                               │
                 │ Immutable &App                                │ MPSC Events
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
1. **The Model (`App` in `src/app.rs`)**: Single source of truth. Owns the text input buffer, concurrent download list, modal states (Path, History, Details, Help), setup bootstrapping state, cursor positions, and UI flags.
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
├── walkthrough.md           # Implementation verification walkthrough
├── src/
│   ├── lib.rs               # Library root re-exporting modules for binary & integration tests
│   ├── main.rs              # Tokio runtime entry point, terminal init, and select! event loop
│   ├── app.rs               # The Model: App struct, DownloadItem, modals, input modes, screen states
│   ├── deps.rs              # Dependency bootstrapping: hybrid PATH vs local bin detection & downloads
│   ├── history.rs           # History persistence: JSON serialization, FIFO limits, file manager launch
│   ├── config.rs            # Persistence: OS user config directory & TOML file serializer/parser
│   ├── ui.rs                # The View: Ratatui layout, Setup screen, Gauges, and modal overlays
│   ├── events.rs            # The Update: Key event dispatcher, Modal & Vim keybinding schemes
│   ├── downloader.rs        # Domain Layer: Asynchronous yt-dlp & FFmpeg process manager
│   └── terminal.rs          # Low-level terminal setup, raw mode, and panic recovery hooks
└── tests/
    ├── app_tests.rs         # Unit tests for state transitions, path modal, and key handling
    ├── deps_tests.rs        # Unit & integration tests for dependency detection & setup workflow
    ├── history_tests.rs     # Unit tests for history persistence, FIFO pruning, and file manager
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
3. Instantiating the shared `App` state and MPSC communication channels.
4. Spawning initial background tasks (e.g. `spawn_setup_task` if dependencies are missing).
5. Running the asynchronous `tokio::select!` event loop multiplexing keystrokes, setup events, and download events.
6. Guaranteeing terminal restoration upon exit.

#### Key Code Structure & Logic
```rust
#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = terminal::init()?;
    let mut app = App::new();

    let (tx, mut rx) = mpsc::channel::<DownloadEvent>(128);
    let (setup_tx, mut setup_rx) = mpsc::channel::<SetupEvent>(32);

    if app.current_screen == CurrentScreen::Setup {
        video_downloader::deps::spawn_setup_task(setup_tx.clone());
    }

    let mut event_reader = EventStream::new();
    let mut tick_interval = tokio::time::interval(Duration::from_millis(16));
```
- **Tokio Multi-threaded Runtime**: `#[tokio::main]` initializes a work-stealing thread pool capable of multiplexing lightweight green tasks across all CPU cores.
- **`EventStream::new()`**: Converts raw terminal keystrokes and resize events from `crossterm` into a non-blocking asynchronous stream implementing `futures::Stream`.
- **`tick_interval`**: Enforces a 60 FPS clock tick (16 ms) ensuring that even when user input is idle, download animations, setup progress gauges, and spin-renders execute fluidly.

#### The `tokio::select!` Loop
```rust
tokio::select! {
    // Branch 1: User Terminal Inputs
    Some(crossterm_event) = event_reader.next() => { ... }

    // Branch 2: Asynchronous Dependency Setup Worker Events
    Some(setup_event) = setup_rx.recv() => {
        app.handle_setup_event(setup_event);
    }

    // Branch 3: Background Download Task Updates
    Some(dl_event) = rx.recv() => { ... }

    // Branch 4: Clock Tick (Framerate enforcement)
    _ = tick_interval.tick() => {}
}
```

---

### 3.2 The Model: `src/app.rs`

#### Purpose
`app.rs` contains the **state representation** of the entire application. It contains no terminal rendering code and no process spawning code. It provides deterministic methods to mutate the state.

#### Core Structs & Enums

##### `CurrentScreen` & Screen State
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentScreen {
    Setup,
    Main,
}
```
- `CurrentScreen::Setup`: Always displayed on startup in normal runs; displays status of required dependencies (`yt-dlp`, `ffmpeg`, `node`), quick start guide, and action footer.
- `CurrentScreen::Main`: The standard downloader interface.

##### `SetupState` & `SetupPhase`
Tracks the status of each tool during onboarding:
```rust
pub struct SetupState {
    pub phase: SetupPhase,
    pub tools: Vec<ToolSetupItem>,
    pub status_message: String,
    pub help_scroll: usize,
}

pub enum SetupPhase {
    Checking,
    Downloading,
    Complete,
    Error(String),
}
```

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
    pub output_dir: String,
}
```

##### Modals
- `DetailModal`: Scrollable raw stdout/stderr logs and diagnostic backtraces for a download.
- `PathModal`: Interactive text editor for setting the destination download directory.
- `HistoryModal`: Interactive browser for persisted past downloads, with options to re-enqueue, open in OS file manager, or delete.
- `HelpModal`: Persistent in-app quick-start and keybinding guide overlay accessible at any time via <kbd>?</kbd> or <kbd>F1</kbd>.

---

### 3.3 The View: `src/ui.rs`

#### Purpose
`ui.rs` is responsible for taking `&App` and drawing to a `ratatui::Frame`.

#### Screen Switching Architecture
```rust
pub fn render(f: &mut Frame, app: &App) {
    match app.current_screen {
        CurrentScreen::Setup => render_setup_screen(f, app),
        CurrentScreen::Main => render_main_screen(f, app),
    }
}
```

#### 1. Setup Screen (`render_setup_screen`)
- **ASCII Logo Banner**: Dynamically adapts between full ASCII banner (`VIDOWN`) on standard terminals and a compact header on micro-terminals.
- **Dependency Status Panel**: Renders color-coded status badges for `yt-dlp`, `ffmpeg & ffprobe`, and `node`:
  - `✔ [Ready: ...]` (Green)
  - `⟳ [Downloading: XX.X%]` (Cyan)
  - `⟳ [Extracting archive...]` (Magenta)
  - `✖ [Failed: ...]` (Red)
- **Embedded Quick-Start Guide**: Scrollable guide explaining every keybinding and concept.
- **Action Footer**:
  - Displays progress notices during setup.
  - Prompts `Press [Enter] to Start  •  [?/F1] Full Keybindings Guide  •  [q] Quit` upon completion.
  - Prompts `[r] Retry [c] Continue [q] Quit` upon error.

#### 2. Main Screen (`render_main_screen`)
The terminal screen is divided vertically using `ratatui::layout::Layout`:
```rust
let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(3), // 1. Header (Title, Scheme, Mode, Output Dir)
        Constraint::Length(3), // 2. URL Input Field
        Constraint::Min(8),    // 3. Downloads & Progress list (Expands dynamically)
        Constraint::Length(3), // 4. Status / Help Footer
    ])
    .split(f.area());
```

#### Modal Overlays
Modals render with `ratatui::widgets::Clear` to prevent background bleed-through, with strict mutual exclusion:
1. `HelpModal` (<kbd>?</kbd> / <kbd>F1</kbd>)
2. `HistoryModal` (<kbd>g</kbd> / <kbd>F4</kbd>)
3. `PathModal` (<kbd>p</kbd> / <kbd>F3</kbd>)
4. `DetailModal` (<kbd>e</kbd> / <kbd>Enter</kbd> on item)

---

### 3.4 The Controller / Event Loop: `src/events.rs`

#### Purpose
`events.rs` maps raw input events from `crossterm` to state mutations in `App`.

#### `handle_key_event`
- **KeyEventKind Deduplication**: Explicitly filters out `KeyEventKind::Release` to prevent double-typing on Windows.
- **Setup Screen Mode**: If `app.current_screen == CurrentScreen::Setup`, delegates to `handle_setup_screen_key` (scroll guide, Enter to launch, r to retry, c to continue, q to quit).
- **Persistent Help Modal**: If `app.help_modal.is_some()`, handles scrolling and dismissal.
- **Global Shortcuts**:
  - `Ctrl+C`: Instantly sets `app.should_quit = true`.
  - `F2`: Toggles between `StandardModal` and `Vim` keybinding schemes.
  - `F3` / `p`: Toggles path configuration modal.
  - `F4` / `g`: Toggles history modal.
  - `?` / `F1`: Toggles help guide modal.

---

### 3.5 Domain Layer: `src/downloader.rs`

#### Purpose
`downloader.rs` handles the external world: spawning `yt-dlp` child processes, reading stdout/stderr streams, rate-limiting progress updates, extracting filenames portably, and cleaning up processes on exit.

#### Key Implementation Details
- **Binary Resolution**: Invokes `crate::deps::resolve_binary("yt-dlp")` to prefer Vidown's managed local binary if available.
- **PATH Injection**: Invokes `crate::deps::inject_bin_to_command(&mut cmd)` to prepend Vidown's local bin directory to the child environment.
- **Dedicated Stderr Task**: Buffers stderr lines asynchronously and emits `DownloadEvent::Log`.
- **Precompiled Regex & Event Throttling**: Progress regex is compiled once globally via `LazyLock<Regex>`. Emits progress updates at most every 150 ms unless an integer step change or 100% completion occurs.
- **Zombie Process Prevention**: If the receiver disconnects, `child.kill().await` is called immediately.

---

### 3.6 Automated Dependency Management: `src/deps.rs`

#### Purpose
`deps.rs` provides automated dependency detection, downloading, extraction, and execution environment configuration.

#### Hybrid Detection Strategy
Checks availability for each required tool (`RequiredTool::YtDlp`, `RequiredTool::Ffmpeg`, `RequiredTool::Node`):
1. **System PATH**: Searches `$PATH` for existing executable binaries. If present, uses them without downloading.
2. **Local User Bin**: Searches Vidown's standard local data directory:
   - Linux/WSL: `~/.local/share/vidown/bin/` (or `$XDG_DATA_HOME/vidown/bin/`)
   - Windows: `%LOCALAPPDATA%\Vidown\bin\` (or `%USERPROFILE%\AppData\Local\Vidown\bin\`)
   - Environment Override: `VIDOWN_BIN_DIR`

#### Download & Extraction Pipeline
- Downloads official binaries or static archives:
  - `yt-dlp`: Direct executable from GitHub releases.
  - `ffmpeg`: Static build archives (`.tar.xz` for Linux, `.zip` for Windows) from `yt-dlp/FFmpeg-Builds`.
  - `node`: Official binaries/archives from Node.js distribution mirrors.
- Download fallbacks: tries `curl` with real-time percentage parsing $\to$ `wget` on Linux $\to$ PowerShell on Windows.
- Extraction fallbacks: tries system `tar` $\to$ `unzip` $\to$ Python 3 standard library `tarfile`/`zipfile` (guaranteeing lzma/xz decompression without requiring an external `xz` package).
- Sets executable permissions (`chmod +x` / `0o755`) on Unix binaries.

---

### 3.7 Download History Management: `src/history.rs`

#### Purpose
`history.rs` manages the persisted log of completed and failed downloads.

#### Storage & Serialization
- Stored as JSON in the user data directory:
  - Linux/WSL: `~/.local/share/vidown/history.json`
  - Windows: `%LOCALAPPDATA%\Vidown\history.json`
- FIFO pruning: automatically keeps at most `history_limit` items (default: 100).
- Atomic saves: writes to `.tmp` file and atomically renames to prevent corruption on sudden termination.
- Desktop File Manager Launching: provides `open_in_file_manager` to highlight or open files using `explorer.exe` (Windows), `xdg-open` (Linux), or WSL Windows Explorer bridge (`wslview`/`explorer.exe`).

---

### 3.8 User Configuration: `src/config.rs`

#### Purpose
`config.rs` handles reading and writing the user's `config.toml`:
- Stored in `%APPDATA%\vidown\config.toml` (Windows) or `~/.config/vidown/config.toml` (Linux/macOS).
- Manages persistent settings:
  - `output_dir`: Default destination folder.
  - `history_limit`: Maximum number of history entries retained.

---

### 3.9 Terminal Lifecycle: `src/terminal.rs`

#### Purpose
Abstracts entering and exiting terminal raw mode, alternate screen buffers, and installs a panic recovery hook via `color_eyre` ensuring the terminal is restored to cooked mode even on sudden panics.

---

### 3.10 Public Crate Root: `src/lib.rs`

#### Purpose
Exposes internal modules (`app`, `config`, `deps`, `downloader`, `events`, `history`, `terminal`, `ui`) as a library crate so integration tests in `tests/` can import them cleanly.

---

## 4. Testing Architecture & Strategies

Vidown maintains a comprehensive test suite of **113 tests** across 6 specialized test suites:

### 4.1 State & Interaction Tests (`tests/app_tests.rs` - 32 tests)
- Initial state defaults, editing buffer, cursor boundaries, UTF-8 multibyte safety.
- Modal mutual exclusions, Vim navigation, and concurrent task management.

### 4.2 Dependency Management Tests (`tests/deps_tests.rs` - 33 tests)
- Hybrid detection (system PATH vs local user bin vs missing).
- Tool download specifications, recursive archive searching with depth bounding.
- Setup state machine transitions, retry on failure, continue anyway, and quit.
- Always-open start screen lifecycle, Enter transition to Main screen, F1/? keybinding modal overlay on start screen, and micro-terminal safety.
- Adaptive narrow footer layout and dynamic banner subtitles.

### 4.3 Download History Tests (`tests/history_tests.rs` - 14 tests)
- JSON serialization/deserialization, FIFO pruning, corrupted file recovery.
- Atomic file writes, path canonicalization, and file manager launching.

### 4.4 Configuration Persistence Tests (`tests/config_tests.rs` - 12 tests)
- TOML formatting, escaped quotes, comments handling, and round-trip saves.

### 4.5 Domain Regex & Process Tests (`tests/downloader_tests.rs` - 4 tests)
- Extraction progress percentage parsing regex across diverse formats and units.
- Portable filename extraction from destination and merger lines.

### 4.6 Headless Buffer & TestBackend Tests (`tests/ui_tests.rs` - 18 tests)
- Headless `TestBackend` rendering of main, setup, and modal views.
- 80x24 terminal constraint validation and viewport scrolling.

---

## 5. Key Design Decisions & Engineering Rationale

| Architectural Decision | Chosen Strategy | Alternative Considered | Rationale |
|---|---|---|---|
| **Dependency Management** | Automated bootstrapping into user data bin with hybrid PATH check | Manual user installation only | Eliminates the biggest barrier to entry (installing FFmpeg/Node/yt-dlp), delivering an "it-just-works" experience while respecting existing system packages. |
| **Setup UX** | Dedicated setup screen with logo + progress + quick-start guide | Silent download or CLI prompt | Turns download wait time into an interactive onboarding tutorial that educates users on keybindings. |
| **Concurrency Communication** | Tokio MPSC Channels | `Arc<Mutex<App>>` | Avoids lock contention between 60 FPS UI rendering and asynchronous network tasks. |
| **Media Extraction** | External `yt-dlp` subprocess | Pure Rust extraction library | Platform extraction algorithms change frequently; `yt-dlp` is actively maintained and handles challenges reliably. |
| **Error Reporting** | Dual Approach (Inline Badge + Modal Popup) | Simple CLI `eprintln!` | Captures full stderr backtraces into memory for inspection without corrupting TUI rendering. |
| **Progress Throttling** | 150 ms timer / integer step gate | Emit every chunk | Prevents channel flooding and UI thread starvation during high-bandwidth transfers. |
| **Keybinding Schemes** | Dynamic scheme toggle (<kbd>F2</kbd>) | Single hardcoded scheme | Welcomes casual users (Modal) while supporting power users (Vim). |

---

## 6. Tracing a User Request (The Byte & Event Lifecycle)

1. **User Types URL**: User presses <kbd>i</kbd>, enters `Editing` mode, and types `https://...`.
2. **Key Event Captured**: `crossterm` captures keypresses; `events::handle_key_event` appends chars to `app.input_buffer`.
3. **User Presses Enter**: `app.submit_input()` enqueues the download with `ItemStatus::Queued` and emits `(id, url)`.
4. **Tokio Task Spawned**: `main.rs` select loop spawns `downloader::perform_download`.
5. **Binary & Environment Injection**: `deps::resolve_binary` locates `yt-dlp`, and `deps::inject_bin_to_command` injects Vidown's local bin directory into `PATH`.
6. **Progress Streaming**: `yt-dlp` emits stdout lines; `downloader.rs` parses percentages and dispatches throttled `DownloadEvent::Progress`.
7. **Double-Buffered Redraw**: `terminal.draw()` calls `ui::render`, updating progress gauges at 60 FPS.
8. **FFmpeg Merger**: `yt-dlp` muxes streams; status updates to `[Merging Audio/Video]`.
9. **Completion**: Process exits cleanly; download records in `history.json` and status updates to `Completed`.

---

## 7. Extension Guide: Adding New Features

### Adding a New Keybinding
1. Open `src/events.rs`.
2. Match against `KeyCode::Char('your_key')` in `handle_normal_key` or `handle_editing_key`.
3. Call a helper method on `app`.

### Adding a New Tool to Manage
1. Open `src/deps.rs`.
2. Add a variant to `RequiredTool`.
3. Implement its detection in `check_tool` and download specs in `get_download_specs`.

### Adding a New Widget or Screen
1. Open `src/ui.rs`.
2. Create a drawing function `fn render_custom(...)`.
3. Call it inside `pub fn render(f: &mut Frame, app: &App)`.
