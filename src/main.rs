use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;

// Import headless Chrome
use headless_chrome::{Browser, LaunchOptions, protocol::cdp::types::Event};

/// This represents our "Domain Layer" isolated in a single function.
/// It takes a URL and downloads it. It knows nothing about UI or async.
fn download_video_sync(url: &str) -> bool {
    println!("Telling the OS to start yt-dlp...");
    println!("The Rust program will now go to sleep (block) until the download is 100% finished.");

    // We build the command just like we would type in a terminal: 
    // yt-dlp <URL> -o "%(title)s.%(ext)s"
    /*let status = Command::new("yt-dlp")
        .arg(url)
        .arg("--impersonate")
        .arg("chrome")
        .arg("--merge-output-format")
        .arg("mp4")
        .arg("-o")
        .arg("%(title)s.%(ext)s") // Tells yt-dlp to name the file after the video title
        .status()                 // <--- THIS IS THE BLOCKING CALL
        .expect("Failed to execute yt-dlp. Is it installed and in your PATH?");*/

    // 1. We change from .status() to .spawn() and pipe the stdout
    let mut child = Command::new("yt-dlp")
        .arg("--js-runtime")
        .arg("node")
        .arg("--force-overwrites")
        .arg("-f")
        .arg("bestvideo[vcodec^=avc]+bestaudio[ext=m4a]/best[ext=mp4]/best")
        .arg("--merge-output-format")
        .arg("mp4")
        .arg("-P")
        .arg("/mnt/e/my_files")
        .arg(url)
        //.status()
        .stdout(Stdio::piped()) // Tell OS too send output to the program, not to screen
        .spawn() // .spawn() runs it without freezing Rust immediately
        .expect("Failed to execute yt-dlp. Is it installed and in your PATH?");

    // 2. We take ownership of the stdout stream from the child process
    if let Some(stdout) = child.stdout.take() {
        // 3. Wrap it in BufReader to read it line by line
        let reader = BufReader::new(stdout);

        // 4. Loop through every line as yt-dlp generates it
        for line in reader.lines() {
            let line = line.expect("Failed to read line from yt-dlp");

            // We prepend [yt-dlp] to prove Rust is the one printing this and not the child process
            println!("[yt-dlp] {}", line);
        }
    }

    // 5. Now we wait for the process to actually finish and get the exit code 
    let status = child.wait().expect("Failed to wait on child process");



    if status.success() {
        println!("\nRust woke up! The video downloaded successfully");
        true // Since this function returns bool now
    } else {
        println!("\nRust woke up! But the download failed (maybe a bad URL?)");
        false // Since this function returns bool now
    }
}

/// Spawns a headless browser, navigates to the page, and sniffs network
/// traffic to find a raw .mp4 or .m3u8 stream link.
fn extract_media_url_with_browser(page_url: &str) -> Option<String> {
    println!("Launching headless browser to inspect network traffic...");

    // Launch Chrome in headless mode 
    let browser = Browser::new(LaunchOptions::default_builder().headless(true).build().unwrap())
        .expect("Failed to launch headless browser");

    let tab = browser.new_tab().expect("Failed to open a new tab");

    // We'll store the found URL in a thread-safe Mutex
    let found_url = Arc::new(Mutex::new(None));
    let found_url_clone = Arc::clone(&found_url);

    // Listen to network events (like the F12 Network tab)
    tab.add_event_listener(Arc::new(move |event: &Event| {
        // Intercept network requests as they are about to be sent
        if let Event::NetworkRequestWillBeSent(network_event) = event {
            let req_url = &network_event.params.request.url;

            // Check if the page is trying to load a raw media file or playlist

            if req_url.contains(".m3u8") || req_url.contains(".mp4") {
                let mut locked_url = found_url_clone.lock().unwrap();
                if locked_url.is_none() {
                    println!("Found raw media streams: {}", req_url);
                    *locked_url = Some(req_url.clone());
                }

            }
        }
    })).expect("Failed to add event Listerner");

    // Navigate to the target page
    tab.navigate_to(page_url).expect("Failed to navigate to URL");

    // Let the page execute its JavaScript and load for up to 10 seconds
    for _ in 0..10 {
        thread::sleep(Duration::from_secs(1));
        let locked_url = found_url.lock().unwrap();
        if locked_url.is_some() {
            break; // We captured the stream link, we can stop waiting!
        }
    }

    // Extract the result from the Mutex and return it
    let result = found_url.lock().unwrap().clone();
    result
}


/// The main logic flow orchestrating yt-dlp and the headless browser fallback
fn process_video(target_url: &str) {
    println!("----- Attempting Primary Download -----");
    let success = download_video_sync(target_url);

    if !success {
        println!("----- yt-dlp failed! Triggering Browser Fallback -----");

        if let Some(raw_media_url) = extract_media_url_with_browser(target_url) {
            println!("Re-running yt-dlp with the extracted raw stream...");

            // Re-run the download logic with teh raw .m3u8 and .mp4 link
            let fallback_success = download_video_sync(&raw_media_url);

            if fallback_success {
                println!("Fallback download succeeded!");
            } else {
                println!("Fallback download failed too. The stream might be DRM protected");
            }
        } else {
            println!("Failed to find any .m3u8 or .mp4 streams on the webpage.");
        }
    }
}

fn main() {
    // A simple public domain / test video URL
    let target_url = "https://milfnut.com/bettie-bondage-tricking-your-best-friends-wife/";

    println!("---- Phase 1: Synchronous Download Test ---- ");

    // Call our domain logic 
    process_video(target_url);

    println!("Program finished.");
}
