// tests/deps_tests.rs

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use video_downloader::app::{App, CurrentScreen, SetupPhase, SetupState};
use video_downloader::deps::{
    RequiredTool, SetupEvent, ToolLocation, ToolSetupStatus, binary_candidates,
    find_file_recursive, get_download_specs, get_user_bin_dir, inject_bin_to_command,
    is_executable_file, resolve_binary,
};
use video_downloader::events::handle_key_event;
use video_downloader::ui;

static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);
static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn create_temp_test_dir(label: &str) -> PathBuf {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("vidown_deps_test_{}_{}_{}", label, std::process::id(), id));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn press_key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn press_char(c: char) -> KeyEvent {
    press_key(KeyCode::Char(c))
}

#[test]
fn test_get_user_bin_dir_custom_override() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let temp = create_temp_test_dir("custom_override");
    let custom_path = temp.join("my_custom_bin");
    let custom_str = custom_path.to_string_lossy().to_string();

    unsafe {
        std::env::set_var("VIDOWN_BIN_DIR", &custom_str);
    }

    let resolved = get_user_bin_dir();
    assert_eq!(resolved, custom_path);

    unsafe {
        std::env::remove_var("VIDOWN_BIN_DIR");
    }
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_binary_candidates_platform() {
    let candidates = binary_candidates("yt-dlp");
    assert!(!candidates.is_empty());

    #[cfg(target_os = "windows")]
    {
        assert!(candidates.contains(&"yt-dlp.exe".to_string()));
        assert!(candidates.contains(&"yt-dlp".to_string()));
    }

    #[cfg(not(target_os = "windows"))]
    {
        assert_eq!(candidates, vec!["yt-dlp".to_string()]);
    }
}

#[test]
fn test_is_executable_file_checks() {
    let temp = create_temp_test_dir("exec_checks");
    let dummy_file = temp.join("dummy.txt");
    File::create(&dummy_file).unwrap();

    // Check on regular non-executable file
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Without execute bit
        let mut perms = std::fs::metadata(&dummy_file).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&dummy_file, perms).unwrap();
        assert!(!is_executable_file(&dummy_file));

        // With execute bit
        let mut perms = std::fs::metadata(&dummy_file).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dummy_file, perms).unwrap();
        assert!(is_executable_file(&dummy_file));
    }

    #[cfg(not(unix))]
    {
        assert!(is_executable_file(&dummy_file));
    }

    // Non-existent file
    assert!(!is_executable_file(&temp.join("non_existent.bin")));
    // Directory
    assert!(!is_executable_file(&temp));

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_resolve_binary_local_vs_system() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let temp = create_temp_test_dir("resolve_bin");
    let local_bin = temp.join("bin");
    std::fs::create_dir_all(&local_bin).unwrap();

    unsafe {
        std::env::set_var("VIDOWN_BIN_DIR", local_bin.to_string_lossy().to_string());
    }

    // yt-dlp doesn't exist yet in local bin
    let resolved_default = resolve_binary("yt-dlp");
    assert_eq!(resolved_default, PathBuf::from("yt-dlp"));

    // Create binary in local bin
    #[cfg(target_os = "windows")]
    let bin_path = local_bin.join("yt-dlp.exe");
    #[cfg(not(target_os = "windows"))]
    let bin_path = local_bin.join("yt-dlp");

    File::create(&bin_path).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&bin_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin_path, perms).unwrap();
    }

    let resolved_local = resolve_binary("yt-dlp");
    assert_eq!(resolved_local, bin_path);

    unsafe {
        std::env::remove_var("VIDOWN_BIN_DIR");
    }
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_inject_bin_to_command_modifies_path() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let temp = create_temp_test_dir("inject_path");
    let local_bin = temp.join("test_bin");
    std::fs::create_dir_all(&local_bin).unwrap();

    unsafe {
        std::env::set_var("VIDOWN_BIN_DIR", local_bin.to_string_lossy().to_string());
    }

    let mut cmd = tokio::process::Command::new("dummy");
    inject_bin_to_command(&mut cmd);

    // Command should have PATH env set containing our local_bin
    let std_cmd = cmd.as_std();
    let env_path = std_cmd
        .get_envs()
        .find(|(k, _)| k.to_string_lossy().eq_ignore_ascii_case("PATH"))
        .and_then(|(_, v)| v);

    assert!(env_path.is_some());
    let path_str = env_path.unwrap().to_string_lossy();
    assert!(path_str.contains(&local_bin.to_string_lossy().to_string()));

    unsafe {
        std::env::remove_var("VIDOWN_BIN_DIR");
    }
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_get_download_specs_validity() {
    let specs = get_download_specs();
    assert_eq!(specs.len(), 3);

    let yt_spec = specs.iter().find(|s| s.tool == RequiredTool::YtDlp).unwrap();
    assert!(!yt_spec.urls.is_empty());
    assert!(yt_spec.urls[0].contains("github.com/yt-dlp/yt-dlp"));
    assert!(!yt_spec.is_archive);

    let ffmpeg_spec = specs.iter().find(|s| s.tool == RequiredTool::Ffmpeg).unwrap();
    assert!(!ffmpeg_spec.urls.is_empty());
    assert!(ffmpeg_spec.is_archive);
    assert!(ffmpeg_spec.target_binaries.contains(&"ffmpeg") || ffmpeg_spec.target_binaries.contains(&"ffmpeg.exe"));
    assert!(ffmpeg_spec.target_binaries.contains(&"ffprobe") || ffmpeg_spec.target_binaries.contains(&"ffprobe.exe"));

    let node_spec = specs.iter().find(|s| s.tool == RequiredTool::Node).unwrap();
    assert!(!node_spec.urls.is_empty());
    assert!(node_spec.urls[0].contains("nodejs.org"));
}

#[test]
fn test_find_file_recursive_nested() {
    let temp = create_temp_test_dir("find_nested");
    let deep_dir = temp.join("a").join("b").join("bin");
    std::fs::create_dir_all(&deep_dir).unwrap();

    let target = deep_dir.join("ffmpeg");
    File::create(&target).unwrap();

    let found = find_file_recursive(&temp, "ffmpeg");
    assert!(found.is_some());
    assert_eq!(found.unwrap(), target);

    let not_found = find_file_recursive(&temp, "non_existent");
    assert!(not_found.is_none());

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_setup_state_machine_transitions() {
    let tools = vec![
        (RequiredTool::YtDlp, ToolLocation::Missing),
        (RequiredTool::Ffmpeg, ToolLocation::SystemPath(PathBuf::from("/usr/bin/ffmpeg"))),
        (RequiredTool::Node, ToolLocation::LocalBin(PathBuf::from("/home/user/.local/share/vidown/bin/node"))),
    ];

    let state = SetupState::new(tools);
    assert_eq!(state.phase, SetupPhase::Downloading);
    assert_eq!(state.tools.len(), 3);

    let yt_item = state.tools.iter().find(|t| t.id == "yt-dlp").unwrap();
    assert_eq!(yt_item.status, ToolSetupStatus::PendingDownload);

    let mut app = App::new_with_setup(state);
    assert_eq!(app.current_screen, CurrentScreen::Setup);

    // Progress update event
    app.handle_setup_event(SetupEvent::Progress {
        tool: "yt-dlp",
        percent: 45.5,
    });

    let item = app.setup_state.as_ref().unwrap().tools.iter().find(|t| t.id == "yt-dlp").unwrap();
    assert_eq!(item.status, ToolSetupStatus::Downloading { percent: 45.5 });

    // Installed event
    app.handle_setup_event(SetupEvent::ToolStatus {
        tool: "yt-dlp",
        status: ToolSetupStatus::Installed,
    });
    let item = app.setup_state.as_ref().unwrap().tools.iter().find(|t| t.id == "yt-dlp").unwrap();
    assert_eq!(item.status, ToolSetupStatus::Installed);

    // Complete event
    app.handle_setup_event(SetupEvent::Complete);
    assert_eq!(app.setup_state.as_ref().unwrap().phase, SetupPhase::Complete);

    // Press Enter to finish setup and launch main screen
    handle_key_event(&mut app, press_key(KeyCode::Enter));
    assert_eq!(app.current_screen, CurrentScreen::Main);
}

#[test]
fn test_setup_error_handling_retry_continue_quit() {
    let tools = vec![(RequiredTool::YtDlp, ToolLocation::Missing)];
    let state = SetupState::new(tools);
    let mut app = App::new_with_setup(state);

    // Simulate error
    app.handle_setup_event(SetupEvent::Error {
        tool: "yt-dlp",
        error: "Connection refused".to_string(),
    });

    let phase = &app.setup_state.as_ref().unwrap().phase;
    assert!(matches!(phase, SetupPhase::Error(_)));

    // Test 'r' for retry
    handle_key_event(&mut app, press_char('r'));
    assert!(app.take_setup_retry());
    assert_eq!(app.setup_state.as_ref().unwrap().phase, SetupPhase::Downloading);

    // Put into error again
    app.handle_setup_event(SetupEvent::Error {
        tool: "yt-dlp",
        error: "404 Not Found".to_string(),
    });

    // Test 'c' to continue anyway
    handle_key_event(&mut app, press_char('c'));
    assert_eq!(app.current_screen, CurrentScreen::Main);

    // Put into error again
    let mut app_quit = App::new_with_setup(SetupState::new(vec![(RequiredTool::YtDlp, ToolLocation::Missing)]));
    app_quit.handle_setup_event(SetupEvent::Error {
        tool: "yt-dlp",
        error: "Fatal error".to_string(),
    });

    // Test 'q' to quit
    handle_key_event(&mut app_quit, press_char('q'));
    assert!(app_quit.should_quit);
}

#[test]
fn test_setup_screen_guide_scrolling() {
    let tools = vec![(RequiredTool::YtDlp, ToolLocation::Missing)];
    let state = SetupState::new(tools);
    let mut app = App::new_with_setup(state);

    assert_eq!(app.setup_state.as_ref().unwrap().help_scroll, 0);

    // Scroll down via 'j'
    handle_key_event(&mut app, press_char('j'));
    assert_eq!(app.setup_state.as_ref().unwrap().help_scroll, 1);

    // Scroll down via Down arrow
    handle_key_event(&mut app, press_key(KeyCode::Down));
    assert_eq!(app.setup_state.as_ref().unwrap().help_scroll, 2);

    // Scroll up via 'k'
    handle_key_event(&mut app, press_char('k'));
    assert_eq!(app.setup_state.as_ref().unwrap().help_scroll, 1);

    // Scroll up via Up arrow
    handle_key_event(&mut app, press_key(KeyCode::Up));
    assert_eq!(app.setup_state.as_ref().unwrap().help_scroll, 0);

    // Cannot scroll below 0
    handle_key_event(&mut app, press_key(KeyCode::Up));
    assert_eq!(app.setup_state.as_ref().unwrap().help_scroll, 0);
}

#[test]
fn test_persistent_help_modal_shortcuts_and_mutual_exclusion() {
    let mut app = App::new_empty();
    assert!(app.help_modal.is_none());

    // Press '?' in normal mode opens help modal
    handle_key_event(&mut app, press_char('?'));
    assert!(app.help_modal.is_some());

    // Press '?' again closes help modal
    handle_key_event(&mut app, press_char('?'));
    assert!(app.help_modal.is_none());

    // Press F1 opens help modal
    handle_key_event(&mut app, press_key(KeyCode::F(1)));
    assert!(app.help_modal.is_some());

    // Press Esc closes help modal
    handle_key_event(&mut app, press_key(KeyCode::Esc));
    assert!(app.help_modal.is_none());

    // Open path modal, then press F1 -> path modal closes, help modal opens
    app.open_path_modal();
    assert!(app.path_modal.is_some());
    handle_key_event(&mut app, press_key(KeyCode::F(1)));
    assert!(app.path_modal.is_none());
    assert!(app.help_modal.is_some());

    // Open history modal -> help modal closes
    app.open_history_modal();
    assert!(app.history_modal.is_some());
    assert!(app.help_modal.is_none());
}

#[test]
fn test_persistent_help_in_editing_mode() {
    let mut app = App::new_empty();
    // Enter editing mode
    handle_key_event(&mut app, press_char('i'));
    assert_eq!(app.input_mode, video_downloader::app::InputMode::Editing);

    // Typing '?' in editing mode inserts '?' into buffer without opening help modal
    handle_key_event(&mut app, press_char('?'));
    assert_eq!(app.input_buffer, "?");
    assert!(app.help_modal.is_none());

    // Pressing F1 in editing mode opens help modal
    handle_key_event(&mut app, press_key(KeyCode::F(1)));
    assert!(app.help_modal.is_some());

    // Pressing Enter closes help modal
    handle_key_event(&mut app, press_key(KeyCode::Enter));
    assert!(app.help_modal.is_none());
}

#[test]
fn test_render_setup_screen_with_test_backend() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let tools = vec![
        (RequiredTool::YtDlp, ToolLocation::Missing),
        (RequiredTool::Ffmpeg, ToolLocation::SystemPath(PathBuf::from("/usr/bin/ffmpeg"))),
        (RequiredTool::Node, ToolLocation::LocalBin(PathBuf::from("node.exe"))),
    ];
    let mut state = SetupState::new(tools);
    state.phase = SetupPhase::Downloading;

    let app = App::new_with_setup(state);

    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let buffer = terminal.backend().buffer();

    let buffer_str: String = (0..buffer.area.height)
        .map(|y| {
            let line: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            format!("{}\n", line)
        })
        .collect();

    assert!(buffer_str.contains("VIDOWN") || buffer_str.contains(r"\ \"));
    assert!(buffer_str.contains("Required Tools"));
    assert!(buffer_str.contains("yt-dlp"));
    assert!(buffer_str.contains("ffmpeg"));
    assert!(buffer_str.contains("node"));
    assert!(buffer_str.contains("Quick Start"));
    assert!(buffer_str.contains("Downloading & verifying dependencies") || buffer_str.contains("WORKING"));
}

#[test]
fn test_render_setup_screen_complete_and_error_states() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    // 1. Complete state
    let mut state_complete = SetupState::new(vec![(RequiredTool::YtDlp, ToolLocation::SystemPath(PathBuf::from("yt-dlp")))]);
    state_complete.phase = SetupPhase::Complete;
    let app_complete = App::new_with_setup(state_complete);

    terminal.draw(|f| ui::render(f, &app_complete)).unwrap();
    let buffer = terminal.backend().buffer();
    let buffer_str: String = (0..buffer.area.height)
        .map(|y| {
            let line: String = (0..buffer.area.width).map(|x| buffer[(x, y)].symbol()).collect();
            format!("{}\n", line)
        })
        .collect();

    assert!(buffer_str.contains("Press [Enter] to launch Vidown"));

    // 2. Error state
    let mut state_err = SetupState::new(vec![(RequiredTool::YtDlp, ToolLocation::Missing)]);
    state_err.phase = SetupPhase::Error("Network timed out".to_string());
    let app_err = App::new_with_setup(state_err);

    terminal.draw(|f| ui::render(f, &app_err)).unwrap();
    let buffer2 = terminal.backend().buffer();
    let buffer2_str: String = (0..buffer2.area.height)
        .map(|y| {
            let line: String = (0..buffer2.area.width).map(|x| buffer2[(x, y)].symbol()).collect();
            format!("{}\n", line)
        })
        .collect();

    assert!(buffer2_str.contains("Network timed out"));
    assert!(buffer2_str.contains("[r]"));
    assert!(buffer2_str.contains("[c]"));
    assert!(buffer2_str.contains("[q]"));
}

#[test]
fn test_render_persistent_help_modal_on_main_screen() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new_empty();
    app.open_help_modal();

    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let buffer = terminal.backend().buffer();
    let buffer_str: String = (0..buffer.area.height)
        .map(|y| {
            let line: String = (0..buffer.area.width).map(|x| buffer[(x, y)].symbol()).collect();
            format!("{}\n", line)
        })
        .collect();

    assert!(buffer_str.contains("Keybinding Guide"));
    assert!(buffer_str.contains("URL Input & Downloading"));
    assert!(buffer_str.contains("Downloads List & Inspection"));
    assert!(buffer_str.contains("Configuration & History"));
    assert!(buffer_str.contains("Keybinding Schemes"));
    assert!(buffer_str.contains("Press [Esc], [Enter], [q], or [?/F1] to close"));
}

#[test]
fn test_render_setup_micro_terminals_no_panic() {
    let micro_dimensions = [(10, 5), (15, 8), (40, 10), (80, 24), (120, 40)];

    for (w, h) in micro_dimensions {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        let state = SetupState::new(vec![(RequiredTool::YtDlp, ToolLocation::Missing)]);
        let app = App::new_with_setup(state);

        let res = terminal.draw(|f| ui::render(f, &app));
        assert!(res.is_ok(), "Failed at dimension {}x{}", w, h);
    }
}

#[test]
fn test_setup_screen_page_down_and_up() {
    let tools = vec![(RequiredTool::YtDlp, ToolLocation::Missing)];
    let state = SetupState::new(tools);
    let mut app = App::new_with_setup(state);

    assert_eq!(app.setup_state.as_ref().unwrap().help_scroll, 0);

    // PageDown scrolls down by 5
    handle_key_event(&mut app, press_key(KeyCode::PageDown));
    assert_eq!(app.setup_state.as_ref().unwrap().help_scroll, 5);

    // PageUp scrolls up by 5
    handle_key_event(&mut app, press_key(KeyCode::PageUp));
    assert_eq!(app.setup_state.as_ref().unwrap().help_scroll, 0);
}

#[test]
fn test_help_modal_page_scrolling() {
    let mut app = App::new_empty();
    app.open_help_modal();
    assert!(app.help_modal.is_some());

    // PageDown advances scroll_offset by 5
    handle_key_event(&mut app, press_key(KeyCode::PageDown));
    assert_eq!(app.help_modal.as_ref().unwrap().scroll_offset, 5);

    // PageUp scrolls back by 5
    handle_key_event(&mut app, press_key(KeyCode::PageUp));
    assert_eq!(app.help_modal.as_ref().unwrap().scroll_offset, 0);
}

#[test]
fn test_find_file_recursive_depth_bound() {
    let temp = create_temp_test_dir("depth_bound");
    let mut curr = temp.clone();
    for i in 0..15 {
        curr = curr.join(format!("level_{}", i));
        std::fs::create_dir_all(&curr).unwrap();
    }
    let deep_file = curr.join("deep_target.bin");
    File::create(&deep_file).unwrap();

    // With max_depth = 10, searching from temp should not find a file 15 levels deep
    let not_found = find_file_recursive(&temp, "deep_target.bin");
    assert!(not_found.is_none());

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_ffmpeg_hybrid_split_detection() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let temp = create_temp_test_dir("ffmpeg_split");
    let local_bin = temp.join("bin");
    std::fs::create_dir_all(&local_bin).unwrap();

    unsafe {
        std::env::set_var("VIDOWN_BIN_DIR", local_bin.to_string_lossy().to_string());
    }

    // Place ffmpeg and ffprobe in local bin
    #[cfg(target_os = "windows")]
    {
        File::create(local_bin.join("ffmpeg.exe")).unwrap();
        File::create(local_bin.join("ffprobe.exe")).unwrap();
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let ff = local_bin.join("ffmpeg");
        let fp = local_bin.join("ffprobe");
        File::create(&ff).unwrap();
        File::create(&fp).unwrap();
        let mut perms = std::fs::metadata(&ff).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&ff, perms.clone()).unwrap();
        std::fs::set_permissions(&fp, perms).unwrap();
    }

    let loc = video_downloader::deps::check_tool(RequiredTool::Ffmpeg);
    assert_ne!(loc, ToolLocation::Missing);

    unsafe {
        std::env::remove_var("VIDOWN_BIN_DIR");
    }
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_download_specs_tar_gz_for_linux_node() {
    #[cfg(not(target_os = "windows"))]
    {
        let specs = get_download_specs();
        let node_spec = specs.iter().find(|s| s.tool == RequiredTool::Node).unwrap();
        assert_eq!(node_spec.archive_format, Some("tar.gz"));
        assert!(node_spec.urls[0].ends_with(".tar.gz"));
    }
}

#[test]
fn test_app_new_with_test_deps_flag() {
    let _lock = ENV_MUTEX.lock().unwrap();
    unsafe {
        std::env::set_var("VIDOWN_TEST_DEPS", "1");
    }

    let app = App::new();
    // App should have evaluated dependencies and either be in Setup or Main
    assert!(matches!(app.current_screen, CurrentScreen::Setup | CurrentScreen::Main));

    unsafe {
        std::env::remove_var("VIDOWN_TEST_DEPS");
    }
}

#[test]
fn test_setup_lifecycle_partial_failure() {
    let tools = vec![
        (RequiredTool::YtDlp, ToolLocation::Missing),
        (RequiredTool::Ffmpeg, ToolLocation::Missing),
    ];
    let state = SetupState::new(tools);
    let mut app = App::new_with_setup(state);

    // First tool fails: yt-dlp
    app.handle_setup_event(SetupEvent::ToolStatus {
        tool: "yt-dlp",
        status: ToolSetupStatus::Failed("Network timeout".to_string()),
    });

    // While setup is continuing, second tool succeeds: ffmpeg
    app.handle_setup_event(SetupEvent::ToolStatus {
        tool: "ffmpeg",
        status: ToolSetupStatus::Installed,
    });

    let setup = app.setup_state.as_ref().unwrap();
    let yt_item = setup.tools.iter().find(|t| t.id == "yt-dlp").unwrap();
    let ffmpeg_item = setup.tools.iter().find(|t| t.id == "ffmpeg").unwrap();
    assert!(matches!(yt_item.status, ToolSetupStatus::Failed(_)));
    assert_eq!(ffmpeg_item.status, ToolSetupStatus::Installed);

    // Worker emits final error event
    app.handle_setup_event(SetupEvent::Error {
        tool: "yt-dlp",
        error: "Network timeout".to_string(),
    });

    assert!(matches!(app.setup_state.as_ref().unwrap().phase, SetupPhase::Error(_)));

    // On retry, only failed tools reset to PendingDownload
    app.retry_setup();
    let setup_retry = app.setup_state.as_ref().unwrap();
    let yt_retry = setup_retry.tools.iter().find(|t| t.id == "yt-dlp").unwrap();
    let ffmpeg_retry = setup_retry.tools.iter().find(|t| t.id == "ffmpeg").unwrap();
    assert_eq!(yt_retry.status, ToolSetupStatus::PendingDownload);
    assert_eq!(ffmpeg_retry.status, ToolSetupStatus::Installed);
}

#[test]
fn test_setup_lifecycle_multiple_consecutive_failures() {
    let tools = vec![
        (RequiredTool::YtDlp, ToolLocation::Missing),
        (RequiredTool::Ffmpeg, ToolLocation::Missing),
        (RequiredTool::Node, ToolLocation::Missing),
    ];
    let state = SetupState::new(tools);
    let mut app = App::new_with_setup(state);

    // First tool fails: yt-dlp
    app.handle_setup_event(SetupEvent::ToolStatus {
        tool: "yt-dlp",
        status: ToolSetupStatus::Failed("yt-dlp download timed out".to_string()),
    });

    // Second tool also fails: ffmpeg
    app.handle_setup_event(SetupEvent::ToolStatus {
        tool: "ffmpeg",
        status: ToolSetupStatus::Failed("ffmpeg extraction failed".to_string()),
    });

    // Third tool succeeds: node
    app.handle_setup_event(SetupEvent::ToolStatus {
        tool: "node",
        status: ToolSetupStatus::Installed,
    });

    // Worker emits final error event with first encountered error
    app.handle_setup_event(SetupEvent::Error {
        tool: "yt-dlp",
        error: "yt-dlp download timed out".to_string(),
    });

    let setup = app.setup_state.as_ref().unwrap();
    let yt_item = setup.tools.iter().find(|t| t.id == "yt-dlp").unwrap();
    let ffmpeg_item = setup.tools.iter().find(|t| t.id == "ffmpeg").unwrap();
    let node_item = setup.tools.iter().find(|t| t.id == "node").unwrap();

    assert!(matches!(yt_item.status, ToolSetupStatus::Failed(_)));
    assert!(matches!(ffmpeg_item.status, ToolSetupStatus::Failed(_)));
    assert_eq!(node_item.status, ToolSetupStatus::Installed);

    // Ensure first_error is preserved in the setup phase error
    match &setup.phase {
        SetupPhase::Error(msg) => {
            assert!(msg.contains("yt-dlp"));
            assert!(msg.contains("yt-dlp download timed out"));
            assert!(!msg.contains("ffmpeg"));
        }
        _ => panic!("Expected SetupPhase::Error"),
    }

    // On retry, both failed tools reset to PendingDownload, but Installed tool remains Installed
    app.retry_setup();
    assert!(app.take_setup_retry());

    let setup_retry = app.setup_state.as_ref().unwrap();
    let yt_retry = setup_retry.tools.iter().find(|t| t.id == "yt-dlp").unwrap();
    let ffmpeg_retry = setup_retry.tools.iter().find(|t| t.id == "ffmpeg").unwrap();
    let node_retry = setup_retry.tools.iter().find(|t| t.id == "node").unwrap();

    assert_eq!(yt_retry.status, ToolSetupStatus::PendingDownload);
    assert_eq!(ffmpeg_retry.status, ToolSetupStatus::PendingDownload);
    assert_eq!(node_retry.status, ToolSetupStatus::Installed);
}

