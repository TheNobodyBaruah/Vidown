/*use std::process::{Command, Stdio};
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
}*/


use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// The messages our background task will send to the main thread. 
pub enum DownloadState {
    Progress(String), // Later we'll change this to f64 percentage
    Success, 
    Error,
}

async fn perform_download(url: String, tx: mpsc::Sender<DownloadState>) {
    let mut child = Command::new("yt-dlp")
        .arg("--js-runtime")
        .arg("node")
        .arg("--force-overwrites")
        .arg("-f")
        .arg("bestvideo[vcodec^=avc]+bestaudio[ext=m4a]/best[ext=mp4]/best")
        .arg("-P")
        .arg("/mnt/e/my_files")
        .arg(&url) // Borrow it here for the command. Why not own? 
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to execute yt-dlp");

    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            // Send the raw text line to the UI
            let _ = tx.send(DownloadState::Progress(line)).await;
        }
    }

    let status = child.wait().await.expect("Failed to wait on child process");

    if status.success() {
        let _ = tx.send(DownloadState::Success).await;
    } else {
        let _ = tx.send(DownloadState::Error).await;
    }
}

#[tokio::main]
async fn main() {
    let target_url = "https://tulipvid.net/videos/Bettie_Bondage_Tricking-Your-Best-Friends-Wife_converted.m3u8";

    // 1. Create the channel
    let (tx, mut rx) = mpsc::channel::<DownloadState>(32); // Why the number?
                                                           // Why is rx mutable? 

    println!("----- Phase 2: Refactored Asynchronous Decoupling -----\n");

    // 2. Spawn the task
    // We clone 'tx' so the background task gets its own copy.
    // The main thread keep the original 'tx'.
    // We also convert the string literal to an owned `String`. Why did we do it? 
    tokio::spawn(perform_download(target_url.to_string(), tx.clone())); // why did we clone tx?

    // We could do a second download right here!
    // tokio::spawn(perform_download(target_url.to_string(), tx.clone())); // why did we clone tx?

    // 3. The Main Thread UI Loop
    while let Some(message) = rx.recv().await {
        match message {
            DownloadState::Progress(line) => println!("[BACKGROUND] {}", line), 
            DownloadState::Success => {
                println!("\n[MAIN] Download finished!");
                break;
            }
            DownloadState::Error => {
                println!("\n[MAIN] Download failed!");
                break;
            }
        }
    }
}
