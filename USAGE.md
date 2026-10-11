# YTDL-TUI Usage Guide & Prerequisites

## Prerequisites

Before running **YTDL-TUI**, make sure you have the following system dependencies installed. 

### 1. FFmpeg (Required for Merging & Post-processing)
`yt-dlp` and this application require **FFmpeg** to merge separate video/audio streams together, embed chapters, and embed subtitles/thumbnails.
- **Linux (Ubuntu/Debian):**
  ```bash
  sudo apt update && sudo apt install -y ffmpeg
