// tests/downloader_tests.rs

use video_downloader::downloader::PROGRESS_RE;

#[test]
fn test_progress_regex_matches_various_formats() {
    let test_cases = vec![
        ("[download]   0.1% of 15.20MiB at 2.45MiB/s ETA 00:06", 0.1),
        (
            "[download]  45.8% of ~ 84.12MiB at  4.10MiB/s ETA 00:11",
            45.8,
        ),
        ("[download] 100% of 50.00MiB in 00:05", 100.0),
        ("[download] 100.0% of 50.00MiB in 00:05", 100.0),
        ("[download]   9.5% of 1.20GiB at 15.20MiB/s ETA 01:15", 9.5),
    ];

    for (line, expected_percent) in test_cases {
        let captures = PROGRESS_RE.captures(line);
        assert!(
            captures.is_some(),
            "Expected regex match for line: {}",
            line
        );
        let matched = captures.unwrap().get(1).unwrap().as_str();
        let parsed: f64 = matched.parse().expect("Failed to parse matched percentage");
        assert!(
            (parsed - expected_percent).abs() < f64::EPSILON,
            "Expected {}, got {}",
            expected_percent,
            parsed
        );
    }
}

#[test]
fn test_progress_regex_ignores_non_progress_lines() {
    let non_progress_lines = vec![
        "[download] Destination: my_video.f137.mp4",
        "[Merger] Merging formats into \"my_video.mp4\"",
        "[youtube] Extracting URL: https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "[info] Available formats for dQw4w9WgXcQ:",
        "Deleting original file my_video.f137.mp4 (pass -k to keep)",
    ];

    for line in non_progress_lines {
        let captures = PROGRESS_RE.captures(line);
        assert!(
            captures.is_none(),
            "Expected NO match for non-progress line: {}",
            line
        );
    }
}

#[test]
fn test_parse_ytdlp_filename_matches_all_variants() {
    use video_downloader::downloader::parse_ytdlp_filename;

    // 1. Destination line
    assert_eq!(
        parse_ytdlp_filename("[download] Destination: downloads/my_video.f137.mp4"),
        Some("my_video.f137.mp4".to_string())
    );

    // 2. Windows destination with backslashes
    assert_eq!(
        parse_ytdlp_filename(r"[download] Destination: C:\Downloads\my_video.f140.m4a"),
        Some("my_video.f140.m4a".to_string())
    );

    // 3. Merger line with double quotes
    assert_eq!(
        parse_ytdlp_filename(r#"[Merger] Merging formats into "downloads/finished_video.mp4""#),
        Some("finished_video.mp4".to_string())
    );

    // 4. Merger line with single quotes
    assert_eq!(
        parse_ytdlp_filename("[Merger] Merging formats into 'downloads/finished_video.mp4'"),
        Some("finished_video.mp4".to_string())
    );

    // 5. Already downloaded line
    assert_eq!(
        parse_ytdlp_filename("[download] ./downloads/cached_video.mp4 has already been downloaded"),
        Some("cached_video.mp4".to_string())
    );

    // 6. ExtractAudio destination
    assert_eq!(
        parse_ytdlp_filename("[ExtractAudio] Destination: /music/track.mp3"),
        Some("track.mp3".to_string())
    );

    // 7. Non-matching lines
    assert_eq!(
        parse_ytdlp_filename("[download]  45.8% of ~ 84.12MiB at  4.10MiB/s ETA 00:11"),
        None
    );
    assert_eq!(
        parse_ytdlp_filename("[youtube] Extracting URL: https://example.com/watch"),
        None
    );

    // 8. Windows backslashes in Merger line
    assert_eq!(
        parse_ytdlp_filename(
            r#"[Merger] Merging formats into "C:\Users\Admin\Downloads\finished.mp4""#
        ),
        Some("finished.mp4".to_string())
    );

    // 9. Windows backslashes in already downloaded line
    assert_eq!(
        parse_ytdlp_filename(
            r"[download] C:\Downloads\already_cached.mp4 has already been downloaded"
        ),
        Some("already_cached.mp4".to_string())
    );

    // 10. Already downloaded with quotes
    assert_eq!(
        parse_ytdlp_filename(
            r#"[download] "downloads/quoted_cached.mp4" has already been downloaded"#
        ),
        Some("quoted_cached.mp4".to_string())
    );

    // 11. Filename with no directory prefix
    assert_eq!(
        parse_ytdlp_filename("[download] Destination: standalone.mp4"),
        Some("standalone.mp4".to_string())
    );

    // 12. Windows drive prefix without slash
    assert_eq!(
        parse_ytdlp_filename(r"[download] Destination: D:drive_video.mkv"),
        Some("drive_video.mkv".to_string())
    );

    // 13. Empty or path-only destinations
    assert_eq!(parse_ytdlp_filename("[download] Destination: "), None);
    assert_eq!(parse_ytdlp_filename("[download] Destination: /"), None);
    assert_eq!(parse_ytdlp_filename(r"[download] Destination: C:\"), None);
    assert_eq!(parse_ytdlp_filename("[download] Destination: ."), None);
    assert_eq!(parse_ytdlp_filename("[download] Destination: .."), None);
    assert_eq!(parse_ytdlp_filename(r"[download] Destination: C:\."), None);
    assert_eq!(parse_ytdlp_filename(r"[download] Destination: C:\.."), None);

    // 14. Colons in subdirectory filenames
    assert_eq!(
        parse_ytdlp_filename("[download] Destination: /downloads/A: Part 1.mp4"),
        Some("A: Part 1.mp4".to_string())
    );
}

#[test]
fn test_extract_filename_portably_edge_cases() {
    use video_downloader::downloader::extract_filename_portably;

    assert_eq!(extract_filename_portably(""), None);
    assert_eq!(extract_filename_portably("   "), None);
    assert_eq!(extract_filename_portably("/"), None);
    assert_eq!(extract_filename_portably("\\"), None);
    assert_eq!(extract_filename_portably(r"C:\"), None);
    assert_eq!(extract_filename_portably("."), None);
    assert_eq!(extract_filename_portably(".."), None);
    assert_eq!(extract_filename_portably("/var/log/."), None);
    assert_eq!(extract_filename_portably(r"C:\Windows\.."), None);
    assert_eq!(
        extract_filename_portably("a.mp4"),
        Some("a.mp4".to_string())
    );
    assert_eq!(
        extract_filename_portably("/var/log/a.mp4"),
        Some("a.mp4".to_string())
    );
    assert_eq!(
        extract_filename_portably(r"C:\Windows\a.mp4"),
        Some("a.mp4".to_string())
    );
    assert_eq!(
        extract_filename_portably(r"C:a.mp4"),
        Some("a.mp4".to_string())
    );
    assert_eq!(
        extract_filename_portably("/downloads/A: Part 1.mp4"),
        Some("A: Part 1.mp4".to_string())
    );
    assert_eq!(
        extract_filename_portably(r"C:\downloads\A: Part 1.mp4"),
        Some("A: Part 1.mp4".to_string())
    );
    assert_eq!(
        extract_filename_portably(r#""/path/with space/vid.m4a""#),
        Some("vid.m4a".to_string())
    );
    assert_eq!(
        extract_filename_portably(r#"'C:\path with space\vid.m4a'"#),
        Some("vid.m4a".to_string())
    );
}
