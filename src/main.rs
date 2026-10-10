use youtube_dl::YoutubeDl;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let video_url = "https://youtu.be/JPN_gT37J-U?si=EJSXQ8w5cUKUDscl";

    println!("Fetching and downloading video...");

    // 1. Configure the downloader with output filename formatting
    let mut ytdl = YoutubeDl::new(video_url);
    ytdl.output_template("downloaded_video.%(ext)s");

    // 2. Download the video file to the current working directory "."
    ytdl.download_to(".")?;

    println!("Download complete!");
    println!("Check your directory for 'downloaded_video.mp4' (or .webm)");

    Ok(())
}