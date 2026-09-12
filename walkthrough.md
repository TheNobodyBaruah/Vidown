# Walkthrough: Clipboard Pasting, Mouse Interaction, Local Binary Compatibility & Windows Sandbox Hardening

We implemented cross-platform clipboard paste support, complete TUI mouse interaction, restored backward-compatible dual binaries for the local environment, resolved and hardened the download workflow for Windows Sandbox, and expanded our test suite to 141 passing tests.

---

## 1. Root Cause Investigations & Diagnostics

### A. Windows Sandbox Download Failure (`?v-` Typo & Directory Elevation)
- **Screenshot Diagnostic**:
  - In Windows Sandbox, the error log reported:
    ```text
    [HELP] Download #1 failed: ERROR: [youtube:truncated_id] dHs2rDospA: Incomplete YouTube ID dHs2rDospA. URL http://www.youtube.com/watch?v-dHs2rDospA looks truncated
    ```
  - Comparing the working local URL with the sandbox URL revealed that because `Ctrl + V` was previously unsupported, typing the URL manually resulted in `?v-` instead of `?v=` (and omitted the 11th character: 10 characters instead of 11). `yt-dlp` rejected the malformed ID as truncated.
- **Working Directory Elevation**:
  - When Windows PowerShell is launched as Administrator in Windows Sandbox, its current working directory defaults to `C:\Windows\System32`.
  - Previously, Vidown defaulted to `./downloads`, which attempted to create `C:\Windows\System32\downloads`. In pristine or security-restricted sandboxes, writing to `System32` can trigger permission failures.
- **NTFS Forbidden Characters & Timestamps**:
  - Video titles containing characters like `|`, `:`, `?`, `*`, `"`, `<`, `>` fail on Windows NTFS unless sanitized.
  - Virtualized filesystems in sandbox environments can fail when attempting to preserve original modification timestamps (`mtime`).

### B. Local Environment Binary Breakage (`video_downloader` vs `vidown`)
- **Diagnostic**:
  - In WSL Kali Linux, running `vidown` reported:
    ```text
    /home/riz_baruah/.local/bin/vidown: line 4: /mnt/c/My_Files/projects/rust/Vidown/target/release/video_downloader: No such file or directory
    ```
  - The previous milestone configured `[[bin]] name = "vidown"` in `Cargo.toml`. When compiled, Cargo generated `target/release/vidown`, while the user's pre-existing wrapper script at `/home/riz_baruah/.local/bin/vidown` was hardcoded to invoke `/mnt/c/My_Files/projects/rust/Vidown/target/release/video_downloader`.

---

## 2. Implemented Changes

### A. Universal Clipboard Paste Support (`src/clipboard.rs`, `src/events.rs`, `src/terminal.rs`, `src/app.rs`)
- **Multi-Tier Clipboard Engine**:
  - **Native Win32 / X11 / Wayland (`arboard`)**: First-tier cross-platform clipboard access without external shell dependencies.
  - **Secondary Windows Fallback**: If Win32 `OpenClipboard` is momentarily locked by another application, falls back to `powershell -NoProfile -Command Get-Clipboard` with `CREATE_NO_WINDOW` (`0x08000000`), completely preventing console window flashing.
  - **Secondary Linux / WSL Fallback**: Under WSL, queries the host Windows clipboard via `powershell.exe -NoProfile -Command Get-Clipboard` so text copied in Windows pastes instantly into WSL. Under native Linux, queries `wl-paste` (Wayland), `xclip`, or `xsel` (X11).
- **Terminal Bracketed Paste**:
  - Enabled via `crossterm::event::EnableBracketedPaste` in `terminal::init()` and restored via `DisableBracketedPaste` on teardown/panic.
  - Handles terminal emulator paste events (`crossterm::event::Event::Paste(text)`).
- **Keyboard Paste Hotkeys**:
  - Intercepts `Ctrl + V`, `Ctrl + Shift + V`, and `Shift + Insert`.
  - Pressing `Ctrl + V` from Normal mode automatically switches to Editing mode and inserts the clipboard text.
  - In `PathModal`, pastes directory paths directly into the destination input.
- **Smart Delimiter Stripping & Sanitization**:
  - `sanitize_clipboard_text()` safely strips enclosing parentheses `(...)`, square brackets `[...]`, angle brackets `<...>`, backticks (`` `...` ``), unicode curly quotes (`“...”`, `‘...’`), and single/double quotes.
  - Automatically selects the first non-empty line if multiline content is pasted.
  - `App::submit_input()` also applies sanitization, preventing URLs with copied outer brackets or quotes from being submitted to `yt-dlp`.
- **UTF-8 Character-Boundary Safe Editing**:
  - Cursor movement (`Home`, `End`, `Right`, `Left`), mode transitions (`i`, `a`, `$`), and Vim deletion (`x`) operate strictly on Unicode character boundaries via `.chars().count()` and `.char_indices()`, eliminating byte-boundary crashes on non-ASCII URLs.

### B. Full Mouse Interaction (`src/events.rs`, `src/terminal.rs`)
- **Mouse Capture**:
  - Enabled via `crossterm::event::EnableMouseCapture` and cleaned up on exit/panic with `DisableMouseCapture`.
- **Left-Click Focus & Selection**:
  - Clicking on the URL input area (rows 3–5) switches to `InputMode::Editing` and positions the cursor at the clicked column.
  - Clicking on a download item selects it. Clicking an already-selected item opens its error/log Details modal (`DetailModal`).
  - Scrolled viewport mapping: row calculation dynamically computes visible slots based on terminal height and scroll offset (`start_idx + slot`), ensuring accurate selection when scrolled through long lists.
  - Clicks on empty list space, borders, or the action footer are bounded and ignored.
  - Clicking the Setup screen action footer launches the main app when onboarding is ready.
- **Right-Click to Paste**:
  - Right-clicking anywhere on the main screen reads the system clipboard, activates editing mode, and pastes the URL.
  - Right-clicking in `PathModal` pastes the clipboard text into the path input.
  - Modal isolation: right-clicking while viewing `HelpModal`, `HistoryModal`, or `DetailModal` is safely ignored without background paste noise.
- **Scroll Wheel**:
  - Mouse wheel up/down scrolls through downloads, Help modal guide, History list, error logs, and the Setup screen guide.
  - Suppressed from scrolling background downloads when `PathModal` is active.

### C. Restored Local Development Command (`Cargo.toml`, `src/bin/video_downloader.rs`)
- Configured dual-binary targets in `Cargo.toml`:
  ```toml
  [[bin]]
  name = "vidown"
  path = "src/main.rs"

  [[bin]]
  name = "video_downloader"
  path = "src/bin/video_downloader.rs"
  ```
  Both forward directly to `video_downloader::run()` in `src/lib.rs`.
- Cargo natively compiles both release binaries (`target/release/vidown` and `target/release/video_downloader`) with zero duplicate build warnings.
- Updated `/home/riz_baruah/.local/bin/vidown` to search for `vidown`, `video_downloader`, and debug binaries, guaranteeing immediate and backward-compatible execution.

### D. Windows Sandbox Download Robustness (`src/downloader.rs`, `src/config.rs`)
- **User Downloads Default & Runtime Fallback**:
  - Default download directory resolves to `%USERPROFILE%\Downloads` on Windows and `~/Downloads` on Linux rather than `./downloads`, preventing permission errors when launched from `C:\Windows\System32`.
  - Added runtime fallback: if directory creation fails, Vidown warns and automatically falls back to a guaranteed writable folder (`%LOCALAPPDATA%\Vidown\downloads` or temp).
- **Child Working Directory**:
  - Sets `cmd.current_dir(&final_output_dir)` so `yt-dlp` runs within the destination folder instead of `System32`.
- **Filename & Timestamp Sanitization**:
  - Passes `--windows-filenames` to `yt-dlp` to restrict characters to valid Windows NTFS characters.
  - Passes `--no-mtime` to prevent virtualized timestamp permission issues.
- **Direct Companion Tool Injection**:
  - Explicitly passes `--ffmpeg-location <dir>` to `yt-dlp`.
  - Passes `--js-runtimes node` leveraging PATH injection, eliminating Windows drive letter colon parsing bugs (`node:C:\...`).
- **Format Fallback**:
  - Upgraded format selector to `bestvideo[vcodec^=avc]+bestaudio[ext=m4a]/bestvideo+bestaudio/best[ext=mp4]/best` for broader stream compatibility.

---

## 3. Verification Results

### A. Automated Test Suite (141 Tests)
Ran full test suite under WSL Kali Linux:
```bash
wsl bash -lc "cargo test"
```
**Result**: All 141 tests passed cleanly (0 failed, 0 ignored):
- `tests/clipboard_mouse_tests.rs`: 28 tests (clipboard sanitization, multiline, quotes, angle brackets, backticks, unicode curly quotes, parentheses & square brackets, URL submit sanitization, cursor navigation & multibyte UTF-8 Vim 'x', bracketed paste, path modal paste, modal exclusions, Ctrl+V, Shift+Insert, left-click focus, item selection, scrolled list item mapping, double-click details, bounds checking on footer/border clicks, right-click paste & modal isolation, scroll wheel navigation, path modal scroll isolation, safe fallback dirs).
- `tests/deps_tests.rs`: 33 tests.
- `tests/app_tests.rs`: 32 tests.
- `tests/ui_tests.rs`: 18 tests.
- `tests/history_tests.rs`: 14 tests.
- `tests/config_tests.rs`: 12 tests.
- `tests/downloader_tests.rs`: 4 tests.

### B. Linter & Cross-Platform Verification
- **Linux Check**:
  ```bash
  wsl bash -lc "cargo clippy --all-targets --all-features -- -D warnings"
  ```
  **Result**: 0 warnings, 0 errors.
- **Windows MSVC Cross-Check**:
  ```bash
  wsl bash -lc "cargo clippy --target x86_64-pc-windows-msvc -- -D warnings"
  ```
  **Result**: 0 warnings, 0 errors.
- **Release Build**:
  ```bash
  wsl bash -lc "cargo build --release"
  ```
  **Result**: Produced `target/release/vidown` (5.24 MB) and `target/release/video_downloader` (5.24 MB).
