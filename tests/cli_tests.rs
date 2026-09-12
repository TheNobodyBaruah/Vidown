// tests/cli_tests.rs

use video_downloader::{handle_cli_args, CliAction};

#[test]
fn test_handle_cli_args_version_flags() {
    assert_eq!(handle_cli_args(["vidown", "--version"]), CliAction::Version);
    assert_eq!(handle_cli_args(["vidown", "-v"]), CliAction::Version);
    assert_eq!(handle_cli_args(["vidown", "-V"]), CliAction::Version);
}

#[test]
fn test_handle_cli_args_help_flags() {
    assert_eq!(handle_cli_args(["vidown", "--help"]), CliAction::Help);
    assert_eq!(handle_cli_args(["vidown", "-h"]), CliAction::Help);
}

#[test]
fn test_handle_cli_args_run_app_default() {
    assert_eq!(handle_cli_args(["vidown"]), CliAction::RunApp);
    assert_eq!(handle_cli_args(["vidown", "https://youtube.com/watch?v=123"]), CliAction::RunApp);
    assert_eq!(handle_cli_args(Vec::<String>::new()), CliAction::RunApp);
}
