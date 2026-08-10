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
    let target_url = "";

    println!("---- Phase 1: Synchronous Download Test ---- ");

    // Call our domain logic 
    download_video_sync(target_url);

    println!("Program finished.");
}*/


use regex::Regex;
use std::process::Stdio;
use std::sync::LazyLock;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;



static PROGRESS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+(?:\.\d+)?)%").expect("Failed to compile regex")
});

/// The messages our background task will send to the main thread. 
pub enum DownloadState {
    Downloading { track: u8, percent: f64 }, // Later we'll change this to f64 percentage
    Merging, // Tell the UI we are using FFmpeg now for muxing
    Success, 
    Error,
}

async fn perform_download(url: String, tx: mpsc::Sender<DownloadState>) {


    let mut child = Command::new("yt-dlp")
        .arg("--js-runtime")
        .arg("node")
        .arg("--force-overwrites")
        .arg("--newline")
        .arg("-f")
        .arg("bestvideo[vcodec^=avc]+bestaudio[ext=m4a]/best[ext=mp4]/best")
        .arg("-P")
        .arg("/mnt/e/my_files")
        .arg(&url) // Borrow it here for the command. Why not own? 
                   // Because, .arg() method expects a type that implements `AsRef<OsStr>`.
                   // Passing a reference is preferred since the OS needs to read the string to
                   // build the Os execution request. If you passed an owned `String`, would 
                   // have a memory overhead. Borrowing is cheaper. 
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to execute yt-dlp");

    // Compile the Regex ONCE before the loop
    // This pattern captures decimals.
    // Let's create a regex object
    // let re = Regex::new(r"(\d+(?:\.\d+)?)%").expect("Failed to compile regex");

    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout).lines();

        // Keep track of which file we are currently downloading
        let mut current_track = 0;

        while let Ok(Some(line)) = reader.next_line().await {

            // STATE CHECK A: Are we starting a new file?
            if line.contains("[download] Destination:") {
                current_track += 1;
                continue; // Skips regex parsing for this line
            }

            // STATE CHECK B: Are we merging?
            if line.contains("[Merger]") {
                if tx.send(DownloadState::Merging).await.is_err() {
                    let _ = child.kill().await;
                    return;
                }

                continue; // Skips regex parsing
            }

            // STATTE CHECK C: Update Progress
            // Search the line for the regex pattern
            // if let Some(captures) = re.captures(&line) {
            // Use the globally compiled regex
            if let Some(captures) = PROGRESS_RE.captures(&line) {
                // Extract  the first capture group
                if let Some(matched) = captures.get(1) {
                    // Attempt to parse the extracted string into an f64
                    if let Ok(percentage) = matched.as_str().parse::<f64>() {
                        // Send the clean f64 to the UI thread
                        // let _ = tx.send(DownloadState::Progress(percentage)).await;
                        
                        // Fallback in case we missed the Destination Line
                        let track = if current_track == 0 {
                            1
                        } else {
                            current_track
                        };
                        // Handle a dropped receiver (eg. TUI was closed)
                        if tx.send(DownloadState::Downloading { track, percent: percentage}).await.is_err() {
                            // The main UUI thread has closed the channel.
                            // Kill the child process so it doesn't become a zombie
                            // downloading gigabytes of data in the background
                            let _ = child.kill().await;
                            return; // Exit the background task completely
                        }
                    }
                }
            }
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
    let target_url = "https://www.youtube.com/watch?v=FmuPoaWmRQ8";

    // 1. Create the channel
    let (tx, mut rx) = mpsc::channel::<DownloadState>(32); // Why the number?
                                                           // It is the channel's(queue buffer) capacity.
                                                           // Why is rx mutable? 
                                                           // Because reading from a channel 
                                                           // changes its internal state. Every
                                                           // time `rx.recv().await` is called
                                                           // the receiver has to update its 
                                                           // internal pointers to pull the 
                                                           // message out of the queue and ensure
                                                           // you don't read the esame message
                                                           // twice. In Rust, any method that t
                                                           // modifies state requires `&mut self`.

    println!("----- Phase 2: Refactored Asynchronous Decoupling -----\n");

    // 2. Spawn the task
    // We clone 'tx' so the background task gets its own copy.
    // The main thread keep the original 'tx'.
    // We also convert the string literal to an owned `String`. Why did we do it? 
    // Because, tokio requires `'static` lifetime
    tokio::spawn(perform_download(target_url.to_string(), tx.clone())); // why did we clone tx?
                                                                        // We need multiple back-
                                                                        // ground task sending 
                                                                        // data.

    // We could do a second download right here!
    // tokio::spawn(perform_download(target_url.to_string(), tx.clone())); // why did we clone tx?

    // 3. The Main Thread UI Loop
    while let Some(message) = rx.recv().await {
        match message {
            DownloadState::Downloading{ track, percent } => println!("[BACKGROUND] Track {} - {}", track, percent), 

            DownloadState::Success => {
                println!("\n[MAIN] Download finished!");
                break;
            }

            DownloadState::Merging => {
                println!("\n[BACKGROUND] Merging Audio and Video with FFmpeg... Please wait.");
            }

            DownloadState::Error => {
                println!("\n[MAIN] Download failed!");
                break;
            }
        }
    }
}
