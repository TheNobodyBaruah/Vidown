use std::process::Command;

/// Retrieves text content from the system clipboard across platforms.
///
/// Strategy:
/// 1. Tries `arboard::Clipboard` (Win32 API on Windows, X11/Wayland on Linux).
/// 2. If `arboard` fails or is unavailable:
///    - On Windows: queries clipboard via PowerShell `powershell -NoProfile -Command Get-Clipboard`.
///    - On Linux/Unix: checks for WSL Windows interop (`powershell.exe -NoProfile -Command Get-Clipboard`).
///    - On Linux: checks for `wl-paste` (Wayland).
///    - On Linux: checks for `xclip` or `xsel` (X11 CLI tools).
pub fn get_clipboard_text() -> Option<String> {
    // Primary: arboard cross-platform clipboard
    if let Ok(mut clipboard) = arboard::Clipboard::new()
        && let Ok(text) = clipboard.get_text()
    {
        let sanitized = sanitize_clipboard_text(&text);
        if !sanitized.is_empty() {
            return Some(sanitized);
        }
    }

    // Secondary fallback for Windows
    #[cfg(target_os = "windows")]
    {
        if let Some(text) = get_clipboard_windows_fallback() {
            let sanitized = sanitize_clipboard_text(&text);
            if !sanitized.is_empty() {
                return Some(sanitized);
            }
        }
    }

    // Secondary fallback for Linux / WSL
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(text) = get_clipboard_wsl_or_cli() {
            let sanitized = sanitize_clipboard_text(&text);
            if !sanitized.is_empty() {
                return Some(sanitized);
            }
        }
    }

    None
}

/// Fallback for Windows when Win32 OpenClipboard is locked or restricted.
#[cfg(target_os = "windows")]
fn get_clipboard_windows_fallback() -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    if let Ok(output) = Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .arg("-NoProfile")
        .arg("-Command")
        .arg("Get-Clipboard")
        .output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

/// Fallback for Linux / WSL environments where arboard could not connect to a display.
#[cfg(not(target_os = "windows"))]
fn get_clipboard_wsl_or_cli() -> Option<String> {
    // 1. If running under WSL, query Windows clipboard via powershell.exe
    if is_wsl()
        && let Ok(output) = Command::new("powershell.exe")
            .arg("-NoProfile")
            .arg("-Command")
            .arg("Get-Clipboard")
            .output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        if !text.trim().is_empty() {
            return Some(text);
        }
    }

    // 2. Wayland: wl-paste
    if let Ok(output) = Command::new("wl-paste").arg("--no-newline").output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        if !text.trim().is_empty() {
            return Some(text);
        }
    }

    // 3. X11: xclip
    if let Ok(output) = Command::new("xclip")
        .arg("-selection")
        .arg("clipboard")
        .arg("-o")
        .output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        if !text.trim().is_empty() {
            return Some(text);
        }
    }

    // 4. X11: xsel
    if let Ok(output) = Command::new("xsel")
        .arg("--clipboard")
        .arg("--output")
        .output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        if !text.trim().is_empty() {
            return Some(text);
        }
    }

    None
}

/// Checks if the current Linux environment is running inside WSL (Windows Subsystem for Linux).
#[cfg(not(target_os = "windows"))]
fn is_wsl() -> bool {
    if std::env::var("WSL_DISTRO_NAME").is_ok() || std::env::var("WSL_INTEROP").is_ok() {
        return true;
    }
    if let Ok(version) = std::fs::read_to_string("/proc/version")
        && version.to_ascii_lowercase().contains("microsoft")
    {
        return true;
    }
    false
}

/// Sanitizes pasted clipboard content:
/// - Strips accidental enclosing single quotes, double quotes, backticks, angle brackets, or unicode curly quotes
/// - Trims leading and trailing whitespace / newlines
/// - Takes the first line if multi-line text is pasted (preventing accidental shell injection)
pub fn sanitize_clipboard_text(text: &str) -> String {
    let mut trimmed = text.trim();
    // If multi-line, take the first non-empty line
    if let Some(first_line) = trimmed.lines().find(|l| !l.trim().is_empty()) {
        trimmed = first_line.trim();
    }
    // Repeatedly strip enclosing quotes or delimiters (e.g. copied from markdown, terminal, chat)
    loop {
        let mut modified = false;
        if let Some(s) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            trimmed = s.trim();
            modified = true;
        } else if let Some(s) = trimmed.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
            trimmed = s.trim();
            modified = true;
        } else if let Some(s) = trimmed.strip_prefix('`').and_then(|s| s.strip_suffix('`')) {
            trimmed = s.trim();
            modified = true;
        } else if let Some(s) = trimmed.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
            trimmed = s.trim();
            modified = true;
        } else if let Some(s) = trimmed.strip_prefix('“').and_then(|s| s.strip_suffix('”')) {
            trimmed = s.trim();
            modified = true;
        } else if let Some(s) = trimmed.strip_prefix('‘').and_then(|s| s.strip_suffix('’')) {
            trimmed = s.trim();
            modified = true;
        } else if let Some(s) = trimmed.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            trimmed = s.trim();
            modified = true;
        } else if let Some(s) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            trimmed = s.trim();
            modified = true;
        }
        if !modified {
            break;
        }
    }
    trimmed.to_string()
}

/// Copies text content to the system clipboard across platforms.
pub fn set_clipboard_text(text: &str) -> bool {
    if let Ok(mut clipboard) = arboard::Clipboard::new()
        && clipboard.set_text(text.to_string()).is_ok()
    {
        return true;
    }

    #[cfg(target_os = "windows")]
    {
        use std::io::Write;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        if let Ok(mut child) = Command::new("powershell")
            .creation_flags(CREATE_NO_WINDOW)
            .arg("-NoProfile")
            .arg("-Command")
            .arg("Set-Clipboard")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait() {
                return status.success();
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if is_wsl() {
            use std::io::Write;
            if let Ok(mut child) = Command::new("powershell.exe")
                .arg("-NoProfile")
                .arg("-Command")
                .arg("Set-Clipboard")
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                if let Ok(status) = child.wait() {
                    return status.success();
                }
            }
        }
    }

    false
}
