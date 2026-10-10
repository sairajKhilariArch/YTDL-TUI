# YTDL-TUI

A terminal user interface (TUI) for downloading YouTube videos and playlists, built in Rust. Shared under a source-available, non-modifiable license for educational use.

## Features

- Interactive TUI built with Ratatui and Crossterm
- Batch select individual videos or download entire playlists
- Customizable folder structures (by playlist, uploader, upload year, or custom name)
- Filename prefix options (track numbers, upload dates)
- Subtitle and chapter embedding options
- Video (MP4) and Audio-only (MP3 with configurable bitrate) downloads
- Real-time download progress bar and live console log stream

## Releases

Final product builds for **Windows** and **Linux Ubuntu** are available on the repository's Releases page.

### Prerequisites

Make sure you have [yt-dlp](https://github.com/yt-dlp/yt-dlp) and `ffmpeg` installed and available in your system's PATH.

## License

Shared under a source-available, non-modifiable educational license. See [LICENSE](LICENSE) for details.
