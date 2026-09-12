// src/bin/video_downloader.rs

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    video_downloader::run().await
}
