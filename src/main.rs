
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let video_url = "https://youtu.be/fwsprzz-pQY?si=cixr7cRqeiQDqzmq";

    println!("🚀 Starting YouTube Downloader...\n");

    // Initialize terminal progress bar
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}% ({msg})")?
            .progress_chars("#>-"),
    );

    // Regex to match yt-dlp percentage output e.g. "[download]  23.5% of 10.00MiB at 2.50MiB/s ETA 00:03"
    let re_percent =
        Regex::new(r"\[download\]\s+(\d+\.\d+)%\s+of\s+([^\s]+)\s+at\s+([^\s]+)\s+ETA\s+([^\s]+)")?;

    // Spawn yt-dlp with --newline and --progress flags
    let mut child = Command::new("yt-dlp")
        .arg("-o")
        .arg("downloaded_video.%(ext)s")
        .arg("--newline")
        .arg("--progress")
        .arg(video_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            if let Ok(line_text) = line {
                if let Some(caps) = re_percent.captures(&line_text) {
                    let percent: f64 = caps[1].parse().unwrap_or(0.0);
                    let size = &caps[2];
                    let speed = &caps[3];
                    let eta = &caps[4];

                    // Update terminal GUI progress bar
                    pb.set_position(percent as u64);
                    pb.set_message(format!("{} | {} | ETA: {}", size, speed, eta));
                }
            }
        }
    }

    child.wait()?;
    pb.finish_with_message("Done!");

    println!("\n✅ Download complete! Saved as 'downloaded_video.<ext>'");

    Ok(())
}