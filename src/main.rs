
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
use std::{
    io::{self, BufRead, BufReader},
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
    InputUrl,
    SelectMode,
    SelectFormat,
    SelectQuality,
    Downloading,
    Finished,
}

struct App {
    state: AppState,
    url_input: String,

    mode_index: usize,
    format_index: usize,
    quality_index: usize,

    modes: Vec<&'static str>,
    formats: Vec<&'static str>,
    qualities: Vec<&'static str>,

    logs: Arc<Mutex<Vec<String>>>,
    download_active: Arc<AtomicBool>,
}

impl App {
    fn new() -> Self {
        Self {
            state: AppState::InputUrl,
            url_input: String::new(),
            mode_index: 0,
            format_index: 0,
            quality_index: 0,
            modes: vec![
                "Auto-detect (Single video or Playlist)",
                "Force Playlist Download",
                "Force Single Video Only",
            ],
            formats: vec!["MP4 (Video + Audio)", "MP3 (Audio Only)"],
            qualities: vec!["Best Available", "1080p", "720p", "480p", "360p"],
            logs: Arc::new(Mutex::new(Vec::new())),
            download_active: Arc::new(AtomicBool::new(false)),
        }
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
                    KeyCode::Backspace => {
                        if app.state == AppState::InputUrl {
                            app.url_input.pop();
                        }
                    }
                    KeyCode::Char(c) => {
                        if app.state == AppState::InputUrl {
                            app.url_input.push(c);
                        }
                    }
                    KeyCode::Down => match app.state {
                        AppState::SelectMode => {
                            app.mode_index = (app.mode_index + 1) % app.modes.len();
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
                        AppState::SelectMode => {
                            if app.mode_index > 0 {
                                app.mode_index -= 1;
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
                        AppState::InputUrl => {
                            if !app.url_input.trim().is_empty() {
                                app.state = AppState::SelectMode;
                            }
                        }
                        AppState::SelectMode => app.state = AppState::SelectFormat,
                        AppState::SelectFormat => {
                            if app.format_index == 1 {
                                app.state = AppState::Downloading;
                                start_download_thread(app);
                            } else {
                                app.state = AppState::SelectQuality;
                            }
                        }
                        AppState::SelectQuality => {
                            app.state = AppState::Downloading;
                            start_download_thread(app);
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

fn start_download_thread(app: &mut App) {
    app.download_active.store(true, Ordering::Relaxed);
    let url = app.url_input.clone();
    let is_mp3 = app.format_index == 1;
    let mode_choice = app.mode_index;
    let quality_choice = app.quality_index;

    let logs = Arc::clone(&app.logs);
    let download_active = Arc::clone(&app.download_active);

    thread::spawn(move || {
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

        let mut cmd = Command::new("yt-dlp");
        cmd.arg("-f").arg(&format_flag);
        cmd.arg("--no-overwrites");
        cmd.arg("--download-archive").arg("archive.txt");

        if is_mp3 {
            cmd.arg("-x").arg("--audio-format").arg("mp3");
        } else {
            cmd.arg("--merge-output-format").arg("mp4");
        }

        let final_template = match mode_choice {
            1 => {
                cmd.arg("--yes-playlist");
                format!(
                    "%(playlist_title)s/%(playlist_index)s - %(title)s.{}",
                    output_ext
                )
            }
            2 => {
                cmd.arg("--no-playlist");
                format!("%(title)s.{}", output_ext)
            }
            _ => format!(
                "%(playlist_title)s/%(playlist_index)s - %(title)s.{}",
                output_ext
            ),
        };

        cmd.arg("-o")
            .arg(&final_template)
            .arg("--newline")
            .arg("--progress")
            .arg(&url)
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

        download_active.store(false, Ordering::Relaxed);
    });
}

fn ui(f: &mut ratatui::Frame, app: &App) {
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
        "1. URL Input",
        "2. Mode",
        "3. Format",
        "4. Quality",
        "5. Download",
    ];
    let tab_index = match app.state {
        AppState::InputUrl => 0,
        AppState::SelectMode => 1,
        AppState::SelectFormat => 2,
        AppState::SelectQuality => 3,
        AppState::Downloading | AppState::Finished => 4,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 🎬 YouTube TUI Downloader "),
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
        AppState::InputUrl => {
            let input_block = Paragraph::new(app.url_input.as_str())
                .style(Style::default().fg(Color::Yellow))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Paste Video/Playlist URL (Press Enter to confirm) "),
                );
            f.render_widget(input_block, chunks[1]);
        }
        AppState::SelectMode => {
            render_list(
                f,
                chunks[1],
                " Select Playlist Option ",
                &app.modes,
                app.mode_index,
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
            let mut current_msg = "Downloading...".to_string();

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
                current_msg = "Download Complete! Press Enter/ESC to exit.".to_string();
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
        AppState::InputUrl => "Type/Paste URL | Enter: Next | Esc/Ctrl+C: Quit",
        AppState::SelectMode | AppState::SelectFormat | AppState::SelectQuality => {
            "↑/↓: Navigate | Enter: Select | Esc: Quit"
        }
        AppState::Downloading => "Downloading in background... | Esc: Quit",
        AppState::Finished => "Process completed! | Enter or Esc to Exit",
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