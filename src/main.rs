
use dialoguer::{Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let video_url = "https://youtu.be/kYNTrRVHNmk?si=f8tZcyTX3ZXjD5EE";

    println!("🎬 YouTube CLI Downloader\n");

    // --- 1. Select Format ---
    let format_options = vec!["MP4 (Video + Audio)", "MP3 (Audio Only)"];
    let format_choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select download format:")
        .default(0)
        .items(&format_options)
        .interact()?;

    let is_mp3 = format_choice == 1;

    // --- 2. Select Resolution / Quality ---
    let (format_flag, output_ext) = if is_mp3 {
        // MP3 extraction arguments
        ("bestaudio/best".to_string(), "mp3")
    } else {
        let quality_options = vec!["Best Available", "1080p", "720p", "480p", "360p"];

        let quality_choice = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select video resolution:")
            .default(0)
            .items(&quality_options)
            .interact()?;

        let res_filter = match quality_choice {
            1 => "bestvideo[height<=1080]+bestaudio/best[height<=1080]",
            2 => "bestvideo[height<=720]+bestaudio/best[height<=720]",
            3 => "bestvideo[height<=480]+bestaudio/best[height<=480]",
            4 => "bestvideo[height<=360]+bestaudio/best[height<=360]",
            _ => "bestvideo+bestaudio/best",
        };

        (res_filter.to_string(), "mp4")
    };

    println!("\n🚀 Starting Download...\n");

    // --- 3. Terminal GUI Progress Bar ---
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}% ({msg})")?
            .progress_chars("#>-"),
    );

    let re_percent =
        Regex::new(r"\[download\]\s+(\d+\.\d+)%\s+of\s+([^\s]+)\s+at\s+([^\s]+)\s+ETA\s+([^\s]+)")?;

    // --- 4. Build yt-dlp Command ---
    let mut cmd = Command::new("yt-dlp");
    cmd.arg("-f").arg(&format_flag);

    if is_mp3 {
        cmd.arg("-x").arg("--audio-format").arg("mp3");
    } else {
        cmd.arg("--merge-output-format").arg("mp4");
    }

    cmd.arg("-o")
        .arg(format!("downloaded_media.{}", output_ext))
        .arg("--newline")
        .arg("--progress")
        .arg(video_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = cmd.spawn()?;

    // --- 5. Stream Output to Progress Bar ---
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            if let Ok(line_text) = line {
                if let Some(caps) = re_percent.captures(&line_text) {
                    let percent: f64 = caps[1].parse().unwrap_or(0.0);
                    let size = &caps[2];
                    let speed = &caps[3];
                    let eta = &caps[4];

                    pb.set_position(percent as u64);
                    pb.set_message(format!("{} | {} | ETA: {}", size, speed, eta));
                }
            }
        }
    }

    child.wait()?;
    pb.finish_with_message("Done!");

    println!(
        "\n✅ Download complete! Saved as 'downloaded_media.{}'",
        output_ext
    );

    Ok(())
}