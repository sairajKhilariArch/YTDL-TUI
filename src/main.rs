
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Tabs},
};
use regex::Regex;
use serde::Deserialize;
use std::{
    fs,
    io::{self, BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

#[derive(PartialEq)]
enum AppState {
    SelectDirectory,
    InputQuery,
    FetchingMetadata,
    SelectVideos,
    SelectFolderStructure,
    InputCustomFolder,
    SelectPrefixOption,
    SelectFormat,
    SelectQuality,
    Downloading,
    Finished,
}

#[derive(Clone, Debug)]
struct VideoItem {
    id: String,
    title: String,
    uploader: String,
    duration: String,
    url: String,
    selected: bool,
}

#[derive(Deserialize, Debug)]
struct YtDlpJson {
    id: Option<String>,
    title: Option<String>,
    uploader: Option<String>,
    duration: Option<f64>,
    duration_string: Option<String>,
    webpage_url: Option<String>,
    url: Option<String>,
}

struct App {
    state: AppState,

    // Directory Picker
    current_dir: PathBuf,
    dir_entries: Vec<PathBuf>,
    dir_selected_index: usize,

    // Input & Shared Video Selection
    query_input: String,
    fetched_videos: Arc<Mutex<Vec<VideoItem>>>,
    video_selected_index: usize,

    // Folder Structure Options
    folder_structures: Vec<&'static str>,
    folder_structure_index: usize,
    custom_folder_input: String,

    // File Prefix Options
    prefix_options: Vec<&'static str>,
    prefix_option_index: usize,

    // Format & Quality Options
    format_index: usize,
    quality_index: usize,
    formats: Vec<&'static str>,
    qualities: Vec<&'static str>,

    // Progress
    logs: Arc<Mutex<Vec<String>>>,
    download_active: Arc<AtomicBool>,
}

impl App {
    fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut app = Self {
            state: AppState::SelectDirectory,
            current_dir,
            dir_entries: Vec::new(),
            dir_selected_index: 0,
            query_input: String::new(),
            fetched_videos: Arc::new(Mutex::new(Vec::new())),
            video_selected_index: 0,

            folder_structures: vec![
                "No Subfolder (Flat Destination)",
                "Organize by Playlist Title / Channel Name",
                "Organize by Artist / Uploader Name",
                "Organize by Upload Year",
                "Create Custom Named Subfolder",
            ],
            folder_structure_index: 0,
            custom_folder_input: String::new(),

            prefix_options: vec![
                "None (Title only)",
                "Add Track / Index Number (e.g., 01 - Title)",
                "Add Upload Date (e.g., 20240815_Title)",
            ],
            prefix_option_index: 0,

            format_index: 0,
            quality_index: 0,
            formats: vec!["MP4 (Video + Audio)", "MP3 (Audio Only)"],
            qualities: vec!["Best Available", "1080p", "720p", "480p", "360p"],

            logs: Arc::new(Mutex::new(Vec::new())),
            download_active: Arc::new(AtomicBool::new(false)),
        };
        app.refresh_directory();
        app
    }

    fn refresh_directory(&mut self) {
        self.dir_entries.clear();
        if let Ok(entries) = fs::read_dir(&self.current_dir) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        self.dir_entries.push(entry.path());
                    }
                }
            }
        }
        self.dir_entries.sort();
        self.dir_selected_index = 0;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if app.state == AppState::Downloading && !app.download_active.load(Ordering::Relaxed) {
            app.state = AppState::Finished;
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    KeyCode::Backspace => match app.state {
                        AppState::InputQuery => {
                            app.query_input.pop();
                        }
                        AppState::InputCustomFolder => {
                            app.custom_folder_input.pop();
                        }
                        _ => {}
                    },
                    KeyCode::Char(c) => match app.state {
                        AppState::InputQuery => {
                            app.query_input.push(c);
                        }
                        AppState::InputCustomFolder => {
                            app.custom_folder_input.push(c);
                        }
                        AppState::SelectVideos => {
                            if c == ' ' {
                                if let Ok(mut videos) = app.fetched_videos.lock() {
                                    if !videos.is_empty() {
                                        let idx = app.video_selected_index;
                                        if idx < videos.len() {
                                            videos[idx].selected = !videos[idx].selected;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    },
                    KeyCode::Left => {
                        if app.state == AppState::SelectDirectory {
                            if let Some(parent) = app.current_dir.parent().map(|p| p.to_path_buf())
                            {
                                app.current_dir = parent;
                                app.refresh_directory();
                            }
                        }
                    }
                    KeyCode::Right => {
                        if app.state == AppState::SelectDirectory && !app.dir_entries.is_empty() {
                            app.current_dir = app.dir_entries[app.dir_selected_index].clone();
                            app.refresh_directory();
                        }
                    }
                    KeyCode::Down => match app.state {
                        AppState::SelectDirectory => {
                            if !app.dir_entries.is_empty() {
                                app.dir_selected_index =
                                    (app.dir_selected_index + 1) % app.dir_entries.len();
                            }
                        }
                        AppState::SelectVideos => {
                            if let Ok(videos) = app.fetched_videos.lock() {
                                if !videos.is_empty() {
                                    app.video_selected_index =
                                        (app.video_selected_index + 1) % videos.len();
                                }
                            }
                        }
                        AppState::SelectFolderStructure => {
                            app.folder_structure_index =
                                (app.folder_structure_index + 1) % app.folder_structures.len();
                        }
                        AppState::SelectPrefixOption => {
                            app.prefix_option_index =
                                (app.prefix_option_index + 1) % app.prefix_options.len();
                        }
                        AppState::SelectFormat => {
                            app.format_index = (app.format_index + 1) % app.formats.len();
                        }
                        AppState::SelectQuality => {
                            app.quality_index = (app.quality_index + 1) % app.qualities.len();
                        }
                        _ => {}
                    },
                    KeyCode::Up => match app.state {
                        AppState::SelectDirectory => {
                            if app.dir_selected_index > 0 {
                                app.dir_selected_index -= 1;
                            }
                        }
                        AppState::SelectVideos => {
                            if app.video_selected_index > 0 {
                                app.video_selected_index -= 1;
                            }
                        }
                        AppState::SelectFolderStructure => {
                            if app.folder_structure_index > 0 {
                                app.folder_structure_index -= 1;
                            }
                        }
                        AppState::SelectPrefixOption => {
                            if app.prefix_option_index > 0 {
                                app.prefix_option_index -= 1;
                            }
                        }
                        AppState::SelectFormat => {
                            if app.format_index > 0 {
                                app.format_index -= 1;
                            }
                        }
                        AppState::SelectQuality => {
                            if app.quality_index > 0 {
                                app.quality_index -= 1;
                            }
                        }
                        _ => {}
                    },
                    KeyCode::Enter => match app.state {
                        AppState::SelectDirectory => {
                            app.state = AppState::InputQuery;
                        }
                        AppState::InputQuery => {
                            if !app.query_input.trim().is_empty() {
                                app.state = AppState::FetchingMetadata;
                                start_fetch_metadata_thread(app);
                            }
                        }
                        AppState::SelectVideos => {
                            if let Ok(videos) = app.fetched_videos.lock() {
                                if videos.iter().any(|v| v.selected) {
                                    app.state = AppState::SelectFolderStructure;
                                }
                            }
                        }
                        AppState::SelectFolderStructure => {
                            if app.folder_structure_index == 4 {
                                app.state = AppState::InputCustomFolder;
                            } else {
                                app.state = AppState::SelectPrefixOption;
                            }
                        }
                        AppState::InputCustomFolder => {
                            if !app.custom_folder_input.trim().is_empty() {
                                app.state = AppState::SelectPrefixOption;
                            }
                        }
                        AppState::SelectPrefixOption => {
                            app.state = AppState::SelectFormat;
                        }
                        AppState::SelectFormat => {
                            if app.format_index == 1 {
                                app.state = AppState::Downloading;
                                start_batch_download_thread(app);
                            } else {
                                app.state = AppState::SelectQuality;
                            }
                        }
                        AppState::SelectQuality => {
                            app.state = AppState::Downloading;
                            start_batch_download_thread(app);
                        }
                        AppState::Finished => return Ok(()),
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    }
}

fn start_fetch_metadata_thread(app: &mut App) {
    let input = app.query_input.trim().to_string();
    let logs = Arc::clone(&app.logs);
    let fetched_videos = Arc::clone(&app.fetched_videos);

    if let Ok(mut videos) = fetched_videos.lock() {
        videos.clear();
    }

    thread::spawn(move || {
        let mut targets = Vec::new();

        if !input.starts_with("http://") && !input.starts_with("https://") {
            targets.push(format!("ytsearch5:{}", input));
        } else {
            for part in input.split(',') {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    targets.push(trimmed.to_string());
                }
            }
        }

        let mut items = Vec::new();

        for target in targets {
            let output = Command::new("yt-dlp")
                .arg("-j")
                .arg("--flat-playlist")
                .arg(&target)
                .output();

            if let Ok(out) = output {
                let stdout_str = String::from_utf8_lossy(&out.stdout);
                for line in stdout_str.lines() {
                    if let Ok(parsed) = serde_json::from_str::<YtDlpJson>(line) {
                        let id = parsed.id.unwrap_or_default();
                        let title = parsed.title.unwrap_or_else(|| "Unknown Title".into());
                        let uploader = parsed.uploader.unwrap_or_else(|| "YouTube".into());

                        let duration = match parsed.duration_string {
                            Some(d) => d,
                            None => match parsed.duration {
                                Some(sec) => format!("{}s", sec as u64),
                                None => "--:--".into(),
                            },
                        };

                        let url = if let Some(wp) = parsed.webpage_url {
                            wp
                        } else if let Some(u) = parsed.url {
                            if u.starts_with("http") {
                                u
                            } else {
                                format!("https://www.youtube.com/watch?v={}", u)
                            }
                        } else if !id.is_empty() {
                            format!("https://www.youtube.com/watch?v={}", id)
                        } else {
                            target.clone()
                        };

                        items.push(VideoItem {
                            id,
                            title,
                            uploader,
                            duration,
                            url,
                            selected: true,
                        });
                    }
                }
            }
        }

        if let Ok(mut guard) = fetched_videos.lock() {
            *guard = items;
        }

        if let Ok(mut guard) = logs.lock() {
            guard.push("Metadata fetch completed.".to_string());
        }
    });
}

fn start_batch_download_thread(app: &mut App) {
    app.download_active.store(true, Ordering::Relaxed);
    let target_dir = app.current_dir.clone();

    let selected_videos: Vec<VideoItem> = if let Ok(videos) = app.fetched_videos.lock() {
        videos.iter().filter(|v| v.selected).cloned().collect()
    } else {
        Vec::new()
    };

    let is_mp3 = app.format_index == 1;
    let quality_choice = app.quality_index;
    let folder_struct_choice = app.folder_structure_index;
    let custom_folder_name = app.custom_folder_input.trim().to_string();
    let prefix_choice = app.prefix_option_index;

    let logs = Arc::clone(&app.logs);
    let download_active = Arc::clone(&app.download_active);

    thread::spawn(move || {
        let _ = fs::create_dir_all(&target_dir);

        for video in selected_videos {
            let (format_flag, output_ext) = if is_mp3 {
                ("bestaudio/best".to_string(), "mp3")
            } else {
                let res_filter = match quality_choice {
                    1 => "bestvideo[height<=1080]+bestaudio/best[height<=1080]",
                    2 => "bestvideo[height<=720]+bestaudio/best[height<=720]",
                    3 => "bestvideo[height<=480]+bestaudio/best[height<=480]",
                    4 => "bestvideo[height<=360]+bestaudio/best[height<=360]",
                    _ => "bestvideo+bestaudio/best",
                };
                (res_filter.to_string(), "mp4")
            };

            // Build File Name Prefix
            let file_prefix = match prefix_choice {
                1 => "%(playlist_index)s - ",
                2 => "%(upload_date)s_",
                _ => "",
            };

            // Build Subfolder Structure
            let custom_path;
            let folder_prefix: &str = match folder_struct_choice {
                1 => "%(playlist_title,uploader)s/",
                2 => "%(uploader)s/",
                3 => "%(upload_date>%Y)s/",
                4 => {
                    if !custom_folder_name.is_empty() {
                        custom_path = format!("{}/", custom_folder_name);
                        &custom_path
                    } else {
                        ""
                    }
                }
                _ => "",
            };

            let template = format!("{}{}%(title)s.{}", folder_prefix, file_prefix, output_ext);

            let mut cmd = Command::new("yt-dlp");
            cmd.current_dir(&target_dir);
            cmd.arg("-f").arg(&format_flag);
            cmd.arg("--no-overwrites");

            cmd.arg("--embed-thumbnail");
            cmd.arg("--add-metadata");
            cmd.arg("--convert-thumbnails").arg("jpg");

            if is_mp3 {
                cmd.arg("-x").arg("--audio-format").arg("mp3");
            } else {
                cmd.arg("--merge-output-format").arg("mp4");
            }

            cmd.arg("-o")
                .arg(&template)
                .arg("--newline")
                .arg("--progress")
                .arg(&video.url)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            if let Ok(mut child) = cmd.spawn() {
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().flatten() {
                        if let Ok(mut guard) = logs.lock() {
                            guard.push(line);
                        }
                    }
                }
                let _ = child.wait();
            }
        }

        download_active.store(false, Ordering::Relaxed);
    });
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.size());

    let titles = vec![
        "1. Destination",
        "2. Input",
        "3. Select",
        "4. Folder Structure",
        "5. File Prefix",
        "6. Format",
        "7. Quality",
        "8. Download",
    ];

    let tab_index = match app.state {
        AppState::SelectDirectory => 0,
        AppState::InputQuery | AppState::FetchingMetadata => 1,
        AppState::SelectVideos => 2,
        AppState::SelectFolderStructure | AppState::InputCustomFolder => 3,
        AppState::SelectPrefixOption => 4,
        AppState::SelectFormat => 5,
        AppState::SelectQuality => 6,
        AppState::Downloading | AppState::Finished => 7,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 🎬 YouTube Advanced TUI "),
        )
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .select(tab_index);
    f.render_widget(tabs, chunks[0]);

    match app.state {
        AppState::SelectDirectory => {
            let items: Vec<ListItem> = app
                .dir_entries
                .iter()
                .enumerate()
                .map(|(i, path)| {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if i == app.dir_selected_index {
                        ListItem::new(format!(" > 📁 {}/", name)).style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        ListItem::new(format!("   📁 {}/", name))
                            .style(Style::default().fg(Color::White))
                    }
                })
                .collect();

            let title = format!(
                " Select Save Directory (Current: {}) | Enter: Set Directory | →: Open Folder | ←: Go Up ",
                app.current_dir.display()
            );
            let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));
            f.render_widget(list, chunks[1]);
        }
        AppState::InputQuery => {
            let input_block =
                Paragraph::new(app.query_input.as_str())
                    .style(Style::default().fg(Color::Yellow))
                    .block(Block::default().borders(Borders::ALL).title(
                        " Search Term OR Playlist/Video URLs (comma separated) | Press Enter ",
                    ));
            f.render_widget(input_block, chunks[1]);
        }
        AppState::FetchingMetadata => {
            if let Ok(logs) = app.logs.lock() {
                if logs.iter().any(|l| l.contains("Metadata fetch completed")) {
                    app.state = AppState::SelectVideos;
                }
            }
            let info = Paragraph::new("Parsing playlist & video metadata... Please wait.")
                .style(Style::default().fg(Color::Cyan))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Metadata Lookup "),
                );
            f.render_widget(info, chunks[1]);
        }
        AppState::SelectVideos => {
            let items: Vec<ListItem> = if let Ok(videos) = app.fetched_videos.lock() {
                videos
                    .iter()
                    .enumerate()
                    .map(|(i, item)| {
                        let check = if item.selected { "[X]" } else { "[ ]" };
                        let line = format!(
                            "{} {} | {} ({})",
                            check, item.title, item.uploader, item.duration
                        );
                        if i == app.video_selected_index {
                            ListItem::new(format!(" > {}", line)).style(
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else {
                            ListItem::new(format!("   {}", line))
                                .style(Style::default().fg(Color::White))
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };

            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Select Videos to Download (Space: Toggle | Enter: Confirm) "),
            );
            f.render_widget(list, chunks[1]);
        }
        AppState::SelectFolderStructure => {
            render_list(
                f,
                chunks[1],
                " Select Subfolder Organization Option ",
                &app.folder_structures,
                app.folder_structure_index,
            );
        }
        AppState::InputCustomFolder => {
            let input_block = Paragraph::new(app.custom_folder_input.as_str())
                .style(Style::default().fg(Color::Yellow))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Type Custom Subfolder Name (Press Enter) "),
                );
            f.render_widget(input_block, chunks[1]);
        }
        AppState::SelectPrefixOption => {
            render_list(
                f,
                chunks[1],
                " Select Filename Prefix Option ",
                &app.prefix_options,
                app.prefix_option_index,
            );
        }
        AppState::SelectFormat => {
            render_list(
                f,
                chunks[1],
                " Select Format ",
                &app.formats,
                app.format_index,
            );
        }
        AppState::SelectQuality => {
            render_list(
                f,
                chunks[1],
                " Select Quality ",
                &app.qualities,
                app.quality_index,
            );
        }
        AppState::Downloading | AppState::Finished => {
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(5)])
                .split(chunks[1]);

            let logs_guard = app.logs.lock().unwrap();
            let mut current_percent = 0;
            let mut current_msg = "Downloading batch queue...".to_string();

            let re_percent = Regex::new(r"\[download\]\s+(\d+\.\d+)%\s+of\s+([^\s]+)").unwrap();
            for log in logs_guard.iter().rev() {
                if let Some(caps) = re_percent.captures(log) {
                    if let Ok(p) = caps[1].parse::<f64>() {
                        current_percent = p as u16;
                        current_msg = format!("{}% of {}", p, &caps[2]);
                        break;
                    }
                }
            }

            if app.state == AppState::Finished {
                current_percent = 100;
                current_msg = "All Downloads Complete! Press Enter/ESC to exit.".to_string();
            }

            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Download Progress "),
                )
                .gauge_style(
                    Style::default()
                        .fg(Color::Green)
                        .bg(Color::Black)
                        .add_modifier(Modifier::ITALIC),
                )
                .percent(current_percent)
                .label(current_msg);
            f.render_widget(gauge, main_chunks[0]);

            let log_items: Vec<ListItem> = logs_guard
                .iter()
                .rev()
                .take(15)
                .map(|l| ListItem::new(l.as_str()))
                .collect();

            let logs_list = List::new(log_items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Console Output Stream "),
            );
            f.render_widget(logs_list, main_chunks[1]);
        }
    }

    let footer_text = match app.state {
        AppState::SelectDirectory => "←/→: Change Folder | Enter: Set Directory | Esc: Quit",
        AppState::InputQuery => "Type Search or Playlist URL | Enter: Fetch Videos | Esc: Quit",
        AppState::FetchingMetadata => "Parsing playlist items...",
        AppState::SelectVideos => "Space: Toggle Checkbox | Enter: Confirm | Esc: Quit",
        AppState::SelectFolderStructure => {
            "↑/↓: Choose folder organization structure | Enter: Select"
        }
        AppState::InputCustomFolder => "Type custom subfolder name | Enter: Confirm",
        AppState::SelectPrefixOption => "↑/↓: Choose filename prefix style | Enter: Select",
        AppState::SelectFormat | AppState::SelectQuality => {
            "↑/↓: Navigate | Enter: Select | Esc: Quit"
        }
        AppState::Downloading => "Downloading items sequentially... | Esc: Quit",
        AppState::Finished => "Done! | Press Enter or Esc to Exit",
    };

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn render_list(f: &mut ratatui::Frame, area: Rect, title: &str, items: &[&str], selected: usize) {
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            if i == selected {
                ListItem::new(format!(" > {}", item)).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ListItem::new(format!("   {}", item)).style(Style::default().fg(Color::White))
            }
        })
        .collect();

    let list = List::new(list_items).block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(list, area);
}

