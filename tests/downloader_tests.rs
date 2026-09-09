// tests/downloader_tests.rs

use video_downloader::downloader::PROGRESS_RE;

#[test]
fn test_progress_regex_matches_various_formats() {
    let test_cases = vec![
        ("[download]   0.1% of 15.20MiB at 2.45MiB/s ETA 00:06", 0.1),
        ("[download]  45.8% of ~ 84.12MiB at  4.10MiB/s ETA 00:11", 45.8),
        ("[download] 100% of 50.00MiB in 00:05", 100.0),
        ("[download] 100.0% of 50.00MiB in 00:05", 100.0),
        ("[download]   9.5% of 1.20GiB at 15.20MiB/s ETA 01:15", 9.5),
    ];

    for (line, expected_percent) in test_cases {
        let captures = PROGRESS_RE.captures(line);
        assert!(captures.is_some(), "Expected regex match for line: {}", line);
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
