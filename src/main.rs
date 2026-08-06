use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

/// This represents our "Domain Layer" isolated in a single function.
/// It takes a URL and downloads it. It knows nothing about UI or async.
fn download_video_sync(url: &str) {
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
    } else {
        println!("\nRust woke up! But the download failed (maybe a bad URL?)");
    }
}

fn main() {
    // A simple public domain / test video URL
    let target_url = "https://hls.strpst.com/records/228796749/2026/07/08/mrg_228796749_436_SBTeSRBUoG6Fyeay_1783492636.mp4";

    println!("---- Phase 1: Synchronous Download Test ---- ");

    // Call our domain logic 
    download_video_sync(target_url);

    println!("Program finished.");
}
