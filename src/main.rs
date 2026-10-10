use dialoguer::{Input, Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎬 Interactive YouTube CLI Downloader\n");

    // --- 1. Prompt for YouTube URL ---
    let video_url: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Paste the YouTube video URL")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.trim().is_empty() {
                Err("URL cannot be empty")
            } else if !input.contains("youtube.com") && !input.contains("youtu.be") {
                Err("Please enter a valid YouTube URL (e.g., https://youtu.be/...)")
            } else {
                Ok(())
            }
        })
        .interact_text()?;

    println!();

    // --- 2. Select Format ---
    let format_options = vec!["MP4 (Video + Audio)", "MP3 (Audio Only)"];
    let format_choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select download format")
        .default(0)
        .items(&format_options)
        .interact()?;

    let is_mp3 = format_choice == 1;

    // --- 3. Select Resolution / Quality ---
    let (format_flag, output_ext) = if is_mp3 {
        ("bestaudio/best".to_string(), "mp3")
    } else {
        let quality_options = vec!["Best Available", "1080p", "720p", "480p", "360p"];

        let quality_choice = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select video resolution")
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

    // --- 4. Terminal Progress Bar Setup ---
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}% ({msg})")?
            .progress_chars("#>-"),
    );

    let re_percent =
        Regex::new(r"\[download\]\s+(\d+\.\d+)%\s+of\s+([^\s]+)\s+at\s+([^\s]+)\s+ETA\s+([^\s]+)")?;

    // --- 5. Run yt-dlp Subprocess ---
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
        .arg(&video_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = cmd.spawn()?;

    // --- 6. Stream Live Terminal GUI Output ---
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
        "\n✅ Download complete! Saved as 'downloaded_media.{}'\n",
        output_ext
    );

    Ok(())
}
