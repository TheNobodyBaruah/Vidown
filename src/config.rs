// src/config.rs

use std::path::{Path, PathBuf};

/// Returns the platform-specific configuration file path for vidown.
///
/// - Windows: `%APPDATA%\vidown\config.toml` (or fallback via `%USERPROFILE%\AppData\Roaming`)
/// - Unix/Linux/macOS: `$XDG_CONFIG_HOME/vidown/config.toml` or `~/.config/vidown/config.toml`
pub fn get_config_path() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("VIDOWN_CONFIG_DIR")
        && !custom.trim().is_empty()
    {
        return Some(PathBuf::from(custom).join("config.toml"));
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA")
            && !appdata.trim().is_empty()
        {
            return Some(PathBuf::from(appdata).join("vidown").join("config.toml"));
        }
        if let Ok(userprofile) = std::env::var("USERPROFILE")
            && !userprofile.trim().is_empty()
        {
            return Some(
                PathBuf::from(userprofile)
                    .join("AppData")
                    .join("Roaming")
                    .join("vidown")
                    .join("config.toml"),
            );
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
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
            && !xdg.trim().is_empty()
        {
            return Some(PathBuf::from(xdg).join("vidown").join("config.toml"));
        }
        if let Ok(home) = std::env::var("HOME")
            && !home.trim().is_empty()
        {
            return Some(
                PathBuf::from(home)
                    .join(".config")
                    .join("vidown")
                    .join("config.toml"),
            );
        }
        None
    }
}

/// Default maximum number of download history entries kept in the persisted history list.
pub const DEFAULT_HISTORY_LIMIT: usize = 50;

/// Returns the platform-specific history JSON file path for vidown (`history.json`).
///
/// - Windows: `%APPDATA%\vidown\history.json` (or fallback via `%USERPROFILE%\AppData\Roaming`)
/// - Unix/Linux/macOS: `$XDG_CONFIG_HOME/vidown/history.json` or `~/.config/vidown/history.json`
pub fn get_history_path() -> Option<PathBuf> {
    let config_path = get_config_path()?;
    Some(config_path.with_file_name("history.json"))
}

/// Parses the output directory from the raw content of config.toml.
/// Handles inline comments, escape sequences, and surrounding quotes.
pub fn parse_config_output_dir(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some((key, val)) = trimmed.split_once('=')
            && key.trim() == "output_dir"
        {
            let val = val.trim();
            if let Some(rest) = val.strip_prefix('"') {
                let mut unescaped = String::new();
                let mut chars = rest.chars().peekable();
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
            } else if let Some(rest) = val.strip_prefix('\'') {
                if let Some(end_idx) = rest.find('\'') {
                    let inner = &rest[..end_idx];
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
    None
}

/// Parses the configured `history_limit` from the raw content of config.toml.
/// Ignores comments and handles inline comments or quotes.
pub fn parse_config_history_limit(content: &str) -> Option<usize> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some((key, val)) = trimmed.split_once('=')
            && key.trim() == "history_limit"
        {
            let unquoted = if let Some((before_comment, _)) = val.split_once('#') {
                before_comment.trim()
            } else {
                val.trim()
            };
            let clean = unquoted.trim_matches(|c| c == '"' || c == '\'').trim();
            if let Ok(limit) = clean.parse::<usize>() {
                return Some(limit);
            }
        }
    }
    None
}

/// Generates a valid TOML configuration string containing `output_dir` and `history_limit`.
pub fn format_config_full(output_dir: &str, history_limit: usize) -> String {
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
    format!(
        "# Vidown Configuration\noutput_dir = \"{}\"\nhistory_limit = {}\n",
        escaped, history_limit
    )
}

/// Generates a valid TOML configuration string containing `output_dir` and default `history_limit`.
pub fn format_config(output_dir: &str) -> String {
    format_config_full(output_dir, DEFAULT_HISTORY_LIMIT)
}

/// Loads the configured output directory from a specific path.
pub fn load_config_from_path(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    parse_config_output_dir(&content)
}

/// Loads the configured history limit from a specific path.
pub fn load_history_limit_from_path(path: &Path) -> Option<usize> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    parse_config_history_limit(&content)
}

/// Saves the configuration to a specific path preserving or setting history_limit.
pub fn save_config_full_to_path(
    path: &Path,
    output_dir: &str,
    history_limit: usize,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = format_config_full(output_dir, history_limit);
    std::fs::write(path, content)
}

/// Saves the configured output directory to a specific path, preserving existing `history_limit` if present.
pub fn save_config_to_path(path: &Path, output_dir: &str) -> std::io::Result<()> {
    let limit = load_history_limit_from_path(path).unwrap_or(DEFAULT_HISTORY_LIMIT);
    save_config_full_to_path(path, output_dir, limit)
}

/// Loads the configured output directory from the standard user configuration file.
/// If missing, unreadable, or invalid, returns `None`.
pub fn load_config() -> Option<String> {
    let config_path = get_config_path()?;
    load_config_from_path(&config_path)
}

/// Loads the configured history limit from the standard user configuration file,
/// enforcing a minimum of at least `DEFAULT_HISTORY_LIMIT` (50).
pub fn load_history_limit() -> usize {
    if let Some(config_path) = get_config_path() {
        load_history_limit_from_path(&config_path)
            .unwrap_or(DEFAULT_HISTORY_LIMIT)
            .max(DEFAULT_HISTORY_LIMIT)
    } else {
        DEFAULT_HISTORY_LIMIT
    }
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

/// Saves both output_dir and history_limit to the standard user configuration file.
pub fn save_config_full(output_dir: &str, history_limit: usize) -> std::io::Result<()> {
    let config_path = get_config_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine system user config directory",
        )
    })?;
    save_config_full_to_path(&config_path, output_dir, history_limit)
}
