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

### B. Dedicated Start & Setup Screen (`src/ui.rs`, `src/app.rs`)
- **Always-Open Start Screen**: Outside automated test runners, Vidown always opens on the Start Screen (`CurrentScreen::Setup`) on startup, even when all dependencies (`yt-dlp`, `ffmpeg`, `node`) are already present and installed.
- **ASCII Art Banner**: Renders the `VIDOWN` logo or an adaptive compact header on micro-terminals.
- **Dependency Status Panel**:
  - Displays color-coded checklist (`✔ [Ready: ...]`, `⟳ [Downloading: XX.X%]`, `⟳ [Extracting...]`, `✖ [Failed: ...]`) for all dependencies.
- **Interactive Quick-Start Guide**:
  - Embedded right on the start screen: explains URL insertion, list navigation, path configuration, history inspector, Vim toggle, and quitting.
  - Scrollable with `[↑]`/`[↓]`, `[j]`/`[k]`, and `PageUp`/`PageDown`.
- **Action Footer & Keybindings**:
  - Upon readiness, clearly states: `Press [Enter] to Start  •  [?/F1] Full Keybindings Guide  •  [q] Quit`.
  - Pressing `[Enter]` transitions to `CurrentScreen::Main`.
  - Pressing `[?]` or `[F1]` opens the full-screen persistent Keybindings Guide modal overlay (`HelpModal`).
  - Pressing `[q]` cleanly quits the application.
- **Interactive Failure Recovery**:
  - If a download or extraction fails, allows:
    - `[r]` Retry downloads
    - `[c]` Continue anyway (falling back to whatever tools exist)
    - `[q]` Quit

### C. Persistent In-App Guide Modal (`src/ui.rs`, `src/events.rs`)
- Pressing `?` or `F1` from anywhere (both the start screen and main application) opens a centered, scrollable **How to Use Vidown / Keybinding Guide** modal overlay.
- Supports smooth scrolling via `[j]`/`[k]`, arrow keys, `PageUp`/`PageDown`, and `Home`.
- Closes cleanly with `Esc`, `Enter`, `q`, or `?`/`F1`.

---

## 2. Verification Results

### A. Automated Test Suite (113 Tests)
Ran full test suite under WSL Kali Linux:
```bash
wsl bash -lc "cargo test"
```
**Result**: All 113 unit and integration tests passed cleanly:
- `tests/deps_tests.rs`: 33 tests (binary detection, download specs, recursive extraction, permissions, setup state machine, error recovery, help modal, start screen always open, enter transitions, help modal overlay on start screen, narrow footer responsiveness, dynamic subtitles).
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

---

## 3. Clipboard Pasting, Mouse Support & Windows Sandbox Hardening

### A. Universal Clipboard Paste Support (`src/clipboard.rs`, `src/events.rs`, `src/terminal.rs`)
- **Multi-Tier Clipboard Reader**:
  - Direct Win32 API access on Windows and X11/Wayland on Linux via `arboard`.
  - Fallbacks for headless WSL/SSH: queries Windows clipboard via `powershell.exe -NoProfile -Command Get-Clipboard`, `wl-paste`, `xclip`, or `xsel`.
  - Sanitizes pasted text: trims whitespace, strips outer single/double quotes, and keeps single-line URLs clean.
- **Terminal Bracketed Paste**:
  - Enabled via `crossterm::event::EnableBracketedPaste` in `terminal::init()` and restored on teardown/panic via `DisableBracketedPaste`.
  - Handles `Event::Paste(text)` seamlessly from terminal emulator paste actions.
- **Keyboard Paste Hotkeys**:
  - Intercepts `Ctrl + V`, `Ctrl + Shift + V`, and `Shift + Insert`.
  - In Normal mode, pressing `Ctrl + V` automatically transitions to Editing mode and pastes the link.
  - In `PathModal`, pastes destination folder paths directly into the modal input.
  - Fixed UTF-8 character insertion and deletion in the editing input buffer.

### B. Full Mouse Interaction (`src/events.rs`, `src/terminal.rs`, `src/main.rs`)
- **Mouse Capture**:
  - `EnableMouseCapture` and `DisableMouseCapture` active during the session and restored on teardown/panic.
- **Left-Click Focus & Selection**:
  - Clicking on the URL input bar focuses `InputMode::Editing` and positions the cursor based on click column.
  - Clicking on an item in the downloads list selects it; re-clicking on the selected item opens the Details/Logs modal.
  - Clicking on the setup screen footer launches the application when onboarding is ready.
- **Right-Click Paste**:
  - Right-clicking anywhere on the main screen reads the system clipboard, switches to editing mode, and pastes the URL.
  - Right-clicking in `PathModal` pastes the clipboard text into the path configuration input.
- **Scroll Wheel Support**:
  - Scrolling up and down navigates the downloads list, help modal, history modal, detail logs, and setup screen guide.

### C. Restored Local Development Environment Command (`~/.local/bin/Vidown`)
- **Root Cause**: Renaming the binary target to `vidown` in `Cargo.toml` (`[[bin]] name = "vidown"`) caused `cargo build --release` to produce `target/release/vidown` rather than `target/release/video_downloader`. The user's launcher script `~/.local/bin/Vidown` had a hardcoded path to `video_downloader`.
- **Resolution**:
  - Updated `~/.local/bin/Vidown` to check for `target/release/vidown`, falling back gracefully to `video_downloader` or `target/debug/vidown`.
  - Created a compatibility symlink `target/release/video_downloader -> vidown`.

### D. Windows Sandbox Download Robustness (`src/downloader.rs`, `src/deps.rs`, `src/config.rs`, `src/app.rs`)
- **Safe Download Directory & Fallbacks**:
  - Resolved default download directory: defaults to user's personal Downloads folder (`%USERPROFILE%\Downloads` on Windows, `~/Downloads` on Linux) rather than `./downloads` (which previously tried to create `C:\Windows\System32\downloads` when launched from elevated administrator prompts in Windows Sandbox).
  - Runtime Fallback: If creating the target directory fails (e.g. PermissionDenied in System32), Vidown logs a warning and automatically falls back to a writable user directory (`Downloads` or `%LOCALAPPDATA%\Vidown\downloads`).
- **Child Working Directory**:
  - Sets `cmd.current_dir(&final_output_dir)` so `yt-dlp` runs strictly within the writable destination directory instead of inheriting System32.
- **Companion Tool Path Resolution**:
  - Explicitly passes `--ffmpeg-location <dir>` to `yt-dlp`, bypassing any Windows PATH inheritance/casing issues.
  - Explicitly passes `--js-runtimes node:<path>` pointing directly to the discovered `node.exe`.
  - Injected `bin` directory sets both `PATH` and `Path` environment variables for Windows case-insensitivity.
- **Format Selector Fallback**:
  - Upgraded format selector to `bestvideo[vcodec^=avc]+bestaudio[ext=m4a]/bestvideo+bestaudio/best[ext=mp4]/best` so videos without AVC or M4A formats still download and merge into MP4.
- **Detailed Error Diagnostics**:
  - Scans stdout logs for `ERROR:` lines or retains the last stdout lines when `stderr` is empty, ensuring users receive clear diagnostics instead of silent failures.

---

## 4. Verification Results

### A. Automated Test Suite (130 Tests)
```bash
wsl bash -lc "cargo test"
```
**Result**: All 130 tests passed cleanly:
- `tests/clipboard_mouse_tests.rs`: 17 tests (clipboard sanitization, multiline, quotes, middle insertion, multibyte UTF-8, paste events in main screen, path modal paste, modal exclusions, Ctrl+V and Shift+Insert, left-click focus, item selection, double-click details, right-click paste, scroll wheel navigation, safe fallback dirs).
- `tests/deps_tests.rs`: 33 tests.
- `tests/app_tests.rs`: 32 tests.
- `tests/ui_tests.rs`: 18 tests.
- `tests/history_tests.rs`: 14 tests.
- `tests/config_tests.rs`: 12 tests.
- `tests/downloader_tests.rs`: 4 tests.

### B. Linter Verification (Clean on Linux and Windows MSVC)
```bash
wsl bash -lc "cargo clippy --all-targets --all-features -- -D warnings"
wsl bash -lc "cargo clippy --target x86_64-pc-windows-msvc -- -D warnings"
```
**Result**: 0 errors and 0 warnings.
