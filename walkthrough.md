# Walkthrough: Automated Dependency Management, Setup Screen & Help Guide

We implemented an automated dependency bootstrapping system, a dedicated first-time onboarding/setup screen with live download progress, and a persistent in-app help guide modal for **Vidown**.

---

## 1. Summary of Changes

### A. Automated Dependency Management (`src/deps.rs`)
- **Required Tools**: Detects and manages `yt-dlp`, `ffmpeg` (with `ffprobe`), and `node` (JavaScript runtime for YouTube n-sig extraction).
- **Hybrid Detection**:
  - Checks system `PATH` first for each tool. If already installed (e.g. via `apt` in WSL Kali Linux or on Windows), Vidown reuses the existing tool immediately without downloading.
  - If missing from `PATH`, checks Vidown's local user data bin directory:
    - **Linux / WSL**: `~/.local/share/vidown/bin` (or `$XDG_DATA_HOME/vidown/bin`)
    - **Windows**: `%LOCALAPPDATA%\Vidown\bin` (or `%USERPROFILE%\AppData\Local\Vidown\bin`)
    - **Custom Override**: Can be overridden via the `VIDOWN_BIN_DIR` environment variable.
- **Automated Downloads & Extraction**:
  - `yt-dlp`: Direct standalone binary download from official GitHub releases.
  - `ffmpeg`: Downloads official static release archives (`yt-dlp/FFmpeg-Builds` `.tar.xz` for Linux, `.zip` for Windows) and extracts `ffmpeg` and `ffprobe`.
  - `node`: Downloads standalone Node.js (`.tar.gz` for Linux, `.exe` for Windows).
  - Multi-tier extraction fallbacks: tries system `tar`, `unzip`, PowerShell `Expand-Archive`, and Python 3 `tarfile`/`zipfile` (built-in standard library with lzma/xz support).
  - Automatically sets executable permissions (`chmod +x` / `0o755`) on Unix systems.
- **Child Process Environment Injection**:
  - `inject_bin_to_command(&mut Command)` prepends Vidown's local bin directory to child processes' `PATH`, guaranteeing `yt-dlp` automatically locates `ffmpeg` and `node`.

### B. Dedicated Setup & Onboarding Screen (`src/ui.rs`, `src/app.rs`)
- **First-Boot Detection**: On startup, `App::new()` checks dependencies. If any tool is missing, the application automatically enters `CurrentScreen::Setup`.
- **ASCII Art Banner**: Renders the `VIDOWN` logo or an adaptive compact header on micro-terminals.
- **Live Progress & Status Panel**:
  - Shows real-time statuses (`✔ Ready`, `⟳ Downloading: XX.X%`, `⟳ Extracting...`, `✖ Failed`) for all dependencies.
- **Interactive Quick-Start Guide**:
  - Embedded right on the setup screen: explains URL insertion, list navigation, path configuration, history inspector, Vim toggle, and quitting.
  - Scrollable with `[↑]`/`[↓]`, `[j]`/`[k]`, and `PageUp`/`PageDown`.
- **Explicit Launch Prompt**:
  - Upon download completion, prompts: `[READY] All required tools are configured! Press [Enter] to launch Vidown >>`.
- **Interactive Failure Recovery**:
  - If a download or extraction fails, allows:
    - `[r]` Retry downloads
    - `[c]` Continue anyway (falling back to whatever tools exist)
    - `[q]` Quit

### C. Persistent In-App Guide Modal (`src/ui.rs`, `src/events.rs`)
- Pressing `?` or `F1` from anywhere in the main application opens a centered, scrollable **How to Use Vidown / Keybinding Guide** modal.
- Supports smooth scrolling via `[j]`/`[k]`, arrow keys, `PageUp`/`PageDown`, and `Home`.
- Closes cleanly with `Esc`, `Enter`, `q`, or `?`/`F1`.

---

## 2. Verification Results

### A. Automated Test Suite (104 Tests)
Ran full test suite under WSL Kali Linux:
```bash
wsl bash -lc "cargo test"
```
**Result**: All 104 unit and integration tests passed cleanly:
- `tests/deps_tests.rs`: 24 tests (binary detection, download specs, recursive extraction, permissions, setup state machine, error recovery, help modal).
- `tests/app_tests.rs`: 32 tests (model, modal exclusions, Vim scheme, sanitization).
- `tests/ui_tests.rs`: 18 tests (buffer rendering, micro-terminals, layout).
- `tests/history_tests.rs`: 14 tests (FIFO pruning, JSON persistence, file manager launching).
- `tests/config_tests.rs`: 12 tests (TOML config parsing and escaping).
- `tests/downloader_tests.rs`: 4 tests (regex and filename extraction).

### B. Linter Verification
```bash
wsl bash -lc "cargo clippy --all-targets --all-features -- -D warnings"
```
**Result**: Clean compilation with **0 errors and 0 warnings**.
