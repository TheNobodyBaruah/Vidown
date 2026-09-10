// src/config.rs

use std::path::{Path, PathBuf};

/// Returns the platform-specific configuration file path for vidown.
///
/// - Windows: `%APPDATA%\vidown\config.toml` (or fallback via `%USERPROFILE%\AppData\Roaming`)
/// - Unix/Linux/macOS: `$XDG_CONFIG_HOME/vidown/config.toml` or `~/.config/vidown/config.toml`
pub fn get_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            if !appdata.trim().is_empty() {
                return Some(PathBuf::from(appdata).join("vidown").join("config.toml"));
            }
        }
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            if !userprofile.trim().is_empty() {
                return Some(
                    PathBuf::from(userprofile)
                        .join("AppData")
                        .join("Roaming")
                        .join("vidown")
                        .join("config.toml"),
                );
            }
        }
        if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
            let combined = format!("{}{}", drive.trim(), path.trim());
            if !combined.is_empty() {
                return Some(
                    PathBuf::from(combined)
                        .join("AppData")
                        .join("Roaming")
                        .join("vidown")
                        .join("config.toml"),
                );
            }
        }
        None
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.trim().is_empty() {
                return Some(PathBuf::from(xdg).join("vidown").join("config.toml"));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            if !home.trim().is_empty() {
                return Some(
                    PathBuf::from(home)
                        .join(".config")
                        .join("vidown")
                        .join("config.toml"),
                );
            }
        }
        None
    }
}

/// Parses the output directory from the raw content of config.toml.
/// Handles inline comments, escape sequences, and surrounding quotes.
pub fn parse_config_output_dir(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some((key, val)) = trimmed.split_once('=') {
            if key.trim() == "output_dir" {
                let val = val.trim();
                if val.starts_with('"') {
                    let mut unescaped = String::new();
                    let mut chars = val[1..].chars().peekable();
                    let mut closed = false;
                    while let Some(c) = chars.next() {
                        if c == '\\' {
                            match chars.next() {
                                Some('\\') => unescaped.push('\\'),
                                Some('"') => unescaped.push('"'),
                                Some('n') => unescaped.push('\n'),
                                Some('r') => unescaped.push('\r'),
                                Some('t') => unescaped.push('\t'),
                                Some(other) => {
                                    unescaped.push('\\');
                                    unescaped.push(other);
                                }
                                None => unescaped.push('\\'),
                            }
                        } else if c == '"' {
                            closed = true;
                            break;
                        } else {
                            unescaped.push(c);
                        }
                    }
                    if closed && !unescaped.trim().is_empty() {
                        return Some(unescaped);
                    }
                } else if val.starts_with('\'') {
                    if let Some(end_idx) = val[1..].find('\'') {
                        let inner = &val[1..1 + end_idx];
                        if !inner.trim().is_empty() {
                            return Some(inner.to_string());
                        }
                    }
                } else {
                    let unquoted = if let Some((before_comment, _)) = val.split_once('#') {
                        before_comment.trim()
                    } else {
                        val.trim()
                    };
                    if !unquoted.is_empty() {
                        return Some(unquoted.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Generates a valid TOML configuration string containing `output_dir`.
pub fn format_config(output_dir: &str) -> String {
    let mut escaped = String::new();
    for c in output_dir.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(c),
        }
    }
    format!("# Vidown Configuration\noutput_dir = \"{}\"\n", escaped)
}

/// Loads the configured output directory from a specific path.
pub fn load_config_from_path(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    parse_config_output_dir(&content)
}

/// Saves the configured output directory to a specific path.
pub fn save_config_to_path(path: &Path, output_dir: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = format_config(output_dir);
    std::fs::write(path, content)
}

/// Loads the configured output directory from the standard user configuration file.
/// If missing, unreadable, or invalid, returns `None`.
pub fn load_config() -> Option<String> {
    let config_path = get_config_path()?;
    load_config_from_path(&config_path)
}

/// Saves the configured output directory to the standard user configuration file.
pub fn save_config(output_dir: &str) -> std::io::Result<()> {
    let config_path = get_config_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine system user config directory",
        )
    })?;
    save_config_to_path(&config_path, output_dir)
}
