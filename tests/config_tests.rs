// tests/config_tests.rs

use video_downloader::config::{
    format_config, load_config_from_path, parse_config_output_dir, save_config_to_path,
};

#[test]
fn test_parse_config_output_dir_double_quotes() {
    let toml = r#"
# Vidown Config
output_dir = "C:\\Users\\Hp\\Downloads"
"#;
    assert_eq!(
        parse_config_output_dir(toml),
        Some("C:\\Users\\Hp\\Downloads".to_string())
    );
}

#[test]
fn test_parse_config_output_dir_single_quotes() {
    let toml = r#"
# Vidown Config
output_dir = '/home/user/downloads'
"#;
    assert_eq!(
        parse_config_output_dir(toml),
        Some("/home/user/downloads".to_string())
    );
}

#[test]
fn test_parse_config_output_dir_ignore_comments_and_empty() {
    let toml = r#"
# output_dir = "/ignored/comment"

output_dir = "./my_downloads"
"#;
    assert_eq!(
        parse_config_output_dir(toml),
        Some("./my_downloads".to_string())
    );
}

#[test]
fn test_format_and_parse_roundtrip_with_backslashes() {
    let original = "C:\\Program Files\\Vidown\\downloads";
    let formatted = format_config(original);
    let parsed = parse_config_output_dir(&formatted);
    assert_eq!(parsed, Some(original.to_string()));
}

#[test]
fn test_save_and_load_config_file_roundtrip() {
    let temp_dir = std::env::temp_dir().join("vidown_test_config_roundtrip");
    let config_file = temp_dir.join("config.toml");

    let test_path = "D:\\Media\\CustomDownloads";
    save_config_to_path(&config_file, test_path).expect("Failed to save config");

    let loaded = load_config_from_path(&config_file);
    assert_eq!(loaded, Some(test_path.to_string()));

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_load_config_non_existent() {
    let non_existent = std::env::temp_dir()
        .join("non_existent_vidown_dir")
        .join("config.toml");
    let loaded = load_config_from_path(&non_existent);
    assert_eq!(loaded, None);
}

#[test]
fn test_parse_config_output_dir_inline_comments_double_quotes() {
    let toml = r#"
output_dir = "D:\\Media\\Downloads" # trailing comment here
"#;
    assert_eq!(
        parse_config_output_dir(toml),
        Some("D:\\Media\\Downloads".to_string())
    );
}

#[test]
fn test_parse_config_output_dir_inline_comments_single_quotes() {
    let toml = r#"
output_dir = '/custom/path/downloads' # trailing note
"#;
    assert_eq!(
        parse_config_output_dir(toml),
        Some("/custom/path/downloads".to_string())
    );
}

#[test]
fn test_parse_config_output_dir_inline_comments_unquoted() {
    let toml = r#"
output_dir = ./downloads_custom # unquoted path
"#;
    assert_eq!(
        parse_config_output_dir(toml),
        Some("./downloads_custom".to_string())
    );
}

#[test]
fn test_parse_config_output_dir_escaped_double_quotes() {
    let toml = r#"
output_dir = "C:\\path\\\"quoted\"\\folder"
"#;
    assert_eq!(
        parse_config_output_dir(toml),
        Some("C:\\path\\\"quoted\"\\folder".to_string())
    );
}

#[test]
fn test_parse_config_output_dir_empty_or_whitespace_returns_none() {
    assert_eq!(parse_config_output_dir("output_dir = \"\""), None);
    assert_eq!(parse_config_output_dir("output_dir = \"   \""), None);
    assert_eq!(parse_config_output_dir("output_dir = ''"), None);
    assert_eq!(parse_config_output_dir("output_dir = "), None);
}

#[test]
fn test_get_config_path_structure() {
    if let Some(path) = video_downloader::config::get_config_path() {
        assert!(path.ends_with("config.toml"));
        assert!(path.to_string_lossy().contains("vidown"));
    }
}
