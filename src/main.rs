use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::Marker,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Clear, Dataset, GraphType, List, ListItem, Paragraph},
};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;

/// Represents an entry in the file browser
struct FileEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

struct App {
    // File tree state
    current_directory: PathBuf,
    startup_directory: PathBuf, // Directory where app was launched
    entries: Vec<FileEntry>,

    // UI settings
    use_nerd_fonts: bool, // Use nerd font icons vs emoji fallback
    selected_index: usize,
    file_tree_scroll: usize, // Scroll offset for file tree

    // Text viewer state
    file_content: Vec<String>,
    scroll_offset: usize,
    visible_height: usize, // Track visible height for page navigation

    // Stats/info
    file_stats: String,
    current_file: Option<PathBuf>,
    file_size: u64,

    // UI state
    show_chart: bool,
    needs_resize: bool, // Trigger terminal resize to fix rendering
    show_recent_files: bool, // Show recent files popup

    // Chart data
    chart_data: Vec<(f64, f64)>,
    chart_bounds: ([f64; 2], [f64; 2]), // (x_bounds, y_bounds)

    // Recent files
    recent_files: Vec<PathBuf>,
    recent_files_selected: usize,
}

impl App {
    fn new() -> App {
        let startup_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let start_dir = Self::load_last_directory().unwrap_or_else(|| startup_dir.clone());

        let mut app = App {
            current_directory: start_dir,
            startup_directory: startup_dir,
            entries: Vec::new(),
            use_nerd_fonts: true, // Set to false for emoji fallback
            selected_index: 0,
            file_tree_scroll: 0,
            file_content: vec![
                "Welcome to Scientific File Viewer!".to_string(),
                "".to_string(),
                "Select a file and press Enter to view its contents.".to_string(),
                "".to_string(),
                "Supported formats: .txt, .dat, .cif, .xyz, .pdb".to_string(),
            ],
            scroll_offset: 0,
            visible_height: 20,
            file_stats: "No file selected".to_string(),
            current_file: None,
            file_size: 0,
            show_chart: true,
            needs_resize: false,
            show_recent_files: false,
            chart_data: Vec::new(),
            chart_bounds: ([0.0, 1.0], [0.0, 1.0]),
            recent_files: Vec::new(),
            recent_files_selected: 0,
        };
        app.refresh_directory();
        app
    }

    /// Get the config file path
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("sci-file-viewer").join("last_dir.txt"))
    }

    /// Load last directory from config
    fn load_last_directory() -> Option<PathBuf> {
        let config_path = Self::config_path()?;
        let file = File::open(&config_path).ok()?;
        let reader = BufReader::new(file);
        let line = reader.lines().next()?.ok()?;
        let path = PathBuf::from(line);
        if path.is_dir() { Some(path) } else { None }
    }

    /// Save current directory to config
    fn save_last_directory(&self) {
        if let Some(config_path) = Self::config_path() {
            if let Some(parent) = config_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(mut file) = File::create(&config_path) {
                let _ = writeln!(file, "{}", self.current_directory.display());
            }
        }
    }

    /// Navigate to home directory
    fn go_home(&mut self) {
        if let Some(home) = dirs::home_dir() {
            self.current_directory = home;
            self.refresh_directory();
        }
    }

    /// Navigate to startup directory
    fn go_startup(&mut self) {
        self.current_directory = self.startup_directory.clone();
        self.refresh_directory();
    }

    /// Read the current directory and populate entries
    fn refresh_directory(&mut self) {
        self.entries.clear();
        self.selected_index = 0;
        self.file_tree_scroll = 0;

        // Add parent directory entry (if not at root)
        if let Some(parent) = self.current_directory.parent() {
            self.entries.push(FileEntry {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                is_dir: true,
            });
        }

        // Read directory contents
        if let Ok(read_dir) = fs::read_dir(&self.current_directory) {
            let mut items: Vec<FileEntry> = read_dir
                .filter_map(|entry| entry.ok())
                .map(|entry| {
                    let path = entry.path();
                    let is_dir = path.is_dir();
                    let name = entry.file_name().to_string_lossy().to_string();
                    FileEntry { name, path, is_dir }
                })
                .collect();

            // Sort: directories first, then files, both alphabetically
            items.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });

            self.entries.extend(items);
        }
    }

    /// Navigate into a directory or open a file
    fn select_entry(&mut self) {
        if let Some(entry) = self.entries.get(self.selected_index) {
            if entry.is_dir {
                // Navigate into directory
                self.current_directory = entry.path.clone();
                self.refresh_directory();
            } else {
                // Open file
                self.open_file(&entry.path.clone());
            }
        }
    }

    /// Add a file to recent files list
    fn add_to_recent_files(&mut self, path: &PathBuf) {
        // Remove if already exists to move it to front
        self.recent_files.retain(|p| p != path);
        // Add to front
        self.recent_files.insert(0, path.clone());
        // Keep only 10 most recent
        self.recent_files.truncate(10);
    }

    /// Open and read a file
    fn open_file(&mut self, path: &PathBuf) {
        self.current_file = Some(path.clone());
        self.scroll_offset = 0;
        self.needs_resize = true; // Trigger resize to fix terminal rendering
        self.chart_data.clear();
        
        // Add to recent files
        self.add_to_recent_files(path);

        // Get file size and metadata (always available regardless of content)
        self.file_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let (created, modified) = self.get_file_dates(path);
        let file_metadata = format!(
            "Size: {}\nCreated: {}\nModified: {}",
            Self::format_size(self.file_size),
            created,
            modified
        );

        match fs::read_to_string(path) {
            Ok(content) => {
                self.file_content = content.lines().map(String::from).collect();
                if self.file_content.is_empty() {
                    self.file_content.push("(empty file)".to_string());
                }

                // Try to parse two-column numeric data
                self.parse_chart_data(&content);

                // Update stats with size, lines, dates, and chart info
                let line_count = self.file_content.len();
                let chart_info = if !self.chart_data.is_empty() {
                    format!("\nData points: {}", self.chart_data.len())
                } else {
                    String::new()
                };
                self.file_stats = format!(
                    "{}\nLines: {}{}",
                    file_metadata,
                    line_count,
                    chart_info
                );
            }
            Err(_e) => {
                self.file_content = vec![
                    "Binary file — no text content to display".to_string(),
                ];
                self.file_stats = file_metadata;
            }
        }
    }

    /// Format file size in human-readable format
    fn format_size(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Get file creation and modification dates
    fn get_file_dates(&self, path: &PathBuf) -> (String, String) {
        use std::time::{SystemTime, UNIX_EPOCH};

        let created = fs::metadata(path)
            .and_then(|m| m.created())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let modified = fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let format_datetime = |time: SystemTime| {
            let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
            let datetime = chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                .unwrap_or(chrono::DateTime::UNIX_EPOCH);
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        };

        (format_datetime(created), format_datetime(modified))
    }

    /// Parse two-column numeric data from file content
    fn parse_chart_data(&mut self, content: &str) {
        let mut data: Vec<(f64, f64)> = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            // Split by whitespace, comma, or tab
            let parts: Vec<&str> = line
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty())
                .collect();

            // We need exactly 2 numeric columns (or at least 2 parseable numbers)
            if parts.len() >= 2
                && let (Ok(x), Ok(y)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>())
                && x.is_finite()
                && y.is_finite()
            {
                data.push((x, y));
            }
        }

        // Only consider it valid chart data if we have at least 2 points
        if data.len() >= 2 {
            // Calculate bounds
            let x_min = data.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
            let x_max = data
                .iter()
                .map(|(x, _)| *x)
                .fold(f64::NEG_INFINITY, f64::max);
            let y_min = data.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
            let y_max = data
                .iter()
                .map(|(_, y)| *y)
                .fold(f64::NEG_INFINITY, f64::max);

            // Add small padding to bounds (5%)
            let x_padding = (x_max - x_min).abs() * 0.05;
            let y_padding = (y_max - y_min).abs() * 0.05;

            // Handle case where all values are the same
            let x_bounds = if x_max == x_min {
                [x_min - 1.0, x_max + 1.0]
            } else {
                [x_min - x_padding, x_max + x_padding]
            };

            let y_bounds = if y_max == y_min {
                [y_min - 1.0, y_max + 1.0]
            } else {
                [y_min - y_padding, y_max + y_padding]
            };

            self.chart_bounds = (x_bounds, y_bounds);
            self.chart_data = data;
        }
    }

    /// Downsample data while preserving peaks (local minima and maxima)
    fn downsample_with_peaks(data: &[(f64, f64)], target_points: usize) -> Vec<(f64, f64)> {
        if data.len() <= target_points {
            return data.to_vec();
        }

        let mut result: Vec<(f64, f64)> = Vec::with_capacity(target_points);

        // Always include first point
        result.push(data[0]);

        // Calculate bucket size
        let bucket_size = (data.len() - 2) as f64 / (target_points - 2) as f64;

        for i in 1..(target_points - 1) {
            let start = (1.0 + (i - 1) as f64 * bucket_size) as usize;
            let end = (1.0 + i as f64 * bucket_size) as usize;
            let end = end.min(data.len() - 1);

            if start >= end {
                continue;
            }

            // Find min and max Y in this bucket
            let mut min_idx = start;
            let mut max_idx = start;
            let mut min_y = data[start].1;
            let mut max_y = data[start].1;

            for (j, point) in data.iter().enumerate().take(end).skip(start) {
                if point.1 < min_y {
                    min_y = point.1;
                    min_idx = j;
                }
                if point.1 > max_y {
                    max_y = point.1;
                    max_idx = j;
                }
            }

            // Add min and max in X order to preserve shape
            if min_idx < max_idx {
                result.push(data[min_idx]);
                if min_idx != max_idx {
                    result.push(data[max_idx]);
                }
            } else {
                result.push(data[max_idx]);
                if min_idx != max_idx {
                    result.push(data[min_idx]);
                }
            }
        }

        // Always include last point
        result.push(data[data.len() - 1]);

        result
    }
}

fn main() -> Result<(), io::Error> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    // Run the app
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> io::Result<()> {
    loop {
        // Clear the whole terminal when switching files to fix artifacts
        if app.needs_resize {
            terminal.clear()?;
            app.needs_resize = false;
        }

        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            // Handle recent files popup first
            if app.show_recent_files {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => {
                        app.show_recent_files = false;
                        continue;
                    }
                    KeyCode::Up => {
                        if app.recent_files.is_empty() {
                            continue;
                        }
                        app.recent_files_selected = app.recent_files_selected.checked_sub(1)
                            .unwrap_or(app.recent_files.len() - 1);
                        continue;
                    }
                    KeyCode::Down => {
                        if app.recent_files.is_empty() {
                            continue;
                        }
                        app.recent_files_selected = (app.recent_files_selected + 1) % app.recent_files.len();
                        continue;
                    }
                    KeyCode::Enter => {
                        if !app.recent_files.is_empty() {
                            if let Some(path) = app.recent_files.get(app.recent_files_selected) {
                                let path = path.clone();
                                app.show_recent_files = false;
                                app.open_file(&path);
                            }
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            
            match key.code {
                KeyCode::Char('q') => {
                    app.save_last_directory();
                    return Ok(());
                }
                KeyCode::Up => {
                    if app.selected_index > 0 {
                        app.selected_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if app.selected_index < app.entries.len().saturating_sub(1) {
                        app.selected_index += 1;
                    }
                }
                KeyCode::Enter => {
                    app.select_entry();
                }
                KeyCode::Backspace => {
                    // Go to parent directory
                    if let Some(parent) = app.current_directory.parent() {
                        app.current_directory = parent.to_path_buf();
                        app.refresh_directory();
                    }
                }
                // Content scrolling
                KeyCode::Char('j') => {
                    let old_offset = app.scroll_offset;
                    app.scroll_offset = app.scroll_offset.saturating_add(1);
                    if app.scroll_offset != old_offset {
                        app.needs_resize = true;
                    }
                }
                KeyCode::Char('k') => {
                    let old_offset = app.scroll_offset;
                    app.scroll_offset = app.scroll_offset.saturating_sub(1);
                    if app.scroll_offset != old_offset {
                        app.needs_resize = true;
                    }
                }
                KeyCode::Char('u') | KeyCode::PageUp => {
                    // Page up in content
                    app.scroll_offset = app.scroll_offset.saturating_sub(app.visible_height);
                    app.needs_resize = true;
                }
                KeyCode::Char('d') | KeyCode::PageDown => {
                    // Page down in content
                    let max_scroll = app.file_content.len().saturating_sub(app.visible_height);
                    app.scroll_offset = (app.scroll_offset + app.visible_height).min(max_scroll);
                    app.needs_resize = true;
                }
                KeyCode::Home => {
                    // Go to start of file
                    app.scroll_offset = 0;
                    app.needs_resize = true;
                }
                KeyCode::End => {
                    // Go to end of file
                    app.scroll_offset = app.file_content.len().saturating_sub(app.visible_height);
                    app.needs_resize = true;
                }
                // Directory navigation
                KeyCode::Char('.') => {
                    // Return to startup directory
                    app.go_startup();
                }
                KeyCode::Char('~') => {
                    // Go to home directory
                    app.go_home();
                }
                KeyCode::Char('c') => {
                    app.show_chart = !app.show_chart;
                    app.needs_resize = true;
                }
                KeyCode::Char('n') => {
                    // Toggle nerd fonts vs emoji
                    app.use_nerd_fonts = !app.use_nerd_fonts;
                    app.needs_resize = true;
                }
                KeyCode::Char('r') => {
                    // Refresh directory
                    app.refresh_directory();
                }
                KeyCode::Char('h') => {
                    // Show recent files popup
                    app.show_recent_files = true;
                    app.recent_files_selected = 0;
                }
                _ => {}
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    // Clear the entire frame first to prevent any artifacts
    f.render_widget(Clear, f.area());

    // Create vertical layout: main content + path bar + status bar
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Main content takes remaining space
            Constraint::Length(1), // Path bar (1 line)
            Constraint::Length(1), // Status bar (1 line)
        ])
        .split(f.area());

    // Create the main horizontal layout: [File Tree | Content | Right Panel]
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20), // File tree
            Constraint::Percentage(50), // Content viewer
            Constraint::Percentage(30), // Right panel (chart + stats)
        ])
        .split(vertical_chunks[0]);

    // Render file tree (left panel)
    render_file_tree(f, app, main_chunks[0]);

    // Render content viewer (middle panel)
    render_content_viewer(f, app, main_chunks[1]);

    // Render right panel (chart + stats)
    render_right_panel(f, app, main_chunks[2]);

    // Render path bar
    render_path_bar(f, app, vertical_chunks[1]);

    // Render status bar
    render_status_bar(f, app, vertical_chunks[2]);

    // Render recent files popup if enabled
    if app.show_recent_files {
        render_recent_files_popup(f, app);
    }
}

fn render_file_tree(f: &mut Frame, app: &mut App, area: Rect) {
    let visible_height = area.height.saturating_sub(2) as usize; // Account for borders

    // Adjust scroll to keep selected item visible
    if app.selected_index < app.file_tree_scroll {
        app.file_tree_scroll = app.selected_index;
    } else if app.selected_index >= app.file_tree_scroll + visible_height {
        app.file_tree_scroll = app.selected_index.saturating_sub(visible_height - 1);
    }

    let items: Vec<ListItem> = app
        .entries
        .iter()
        .enumerate()
        .skip(app.file_tree_scroll)
        .take(visible_height)
        .map(|(i, entry)| {
            let (icon, color) = if entry.is_dir {
                if entry.name == ".." {
                    // Parent directory - nf-fa-arrow_up \uf062
                    if app.use_nerd_fonts {
                        ("\u{f062} ", Color::Rgb(97, 175, 239))
                    }
                    // Blue
                    else {
                        ("⬆️ ", Color::Rgb(97, 175, 239))
                    }
                } else {
                    // Regular directory - nf-fa-folder \uf07b
                    if app.use_nerd_fonts {
                        ("\u{f07b} ", Color::Rgb(229, 192, 123))
                    }
                    // Yellow
                    else {
                        ("📁 ", Color::Rgb(229, 192, 123))
                    }
                }
            } else {
                // Color based on file extension (Atom One Dark colors)
                let ext = entry
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                match ext {
                    "xyz" | "pdb" | "cif" => {
                        // nf-fa-flask \uf0c3
                        if app.use_nerd_fonts {
                            ("\u{f0c3} ", Color::Rgb(198, 120, 221))
                        }
                        // Purple
                        else {
                            ("🔬 ", Color::Rgb(198, 120, 221))
                        }
                    }
                    "dat" | "csv" => {
                        // nf-fa-table \uf0ce
                        if app.use_nerd_fonts {
                            ("\u{f0ce} ", Color::Rgb(152, 195, 121))
                        }
                        // Green
                        else {
                            ("📊 ", Color::Rgb(152, 195, 121))
                        }
                    }
                    "txt" | "log" => {
                        // nf-fa-file_text_o \uf0f6
                        if app.use_nerd_fonts {
                            ("\u{f0f6} ", Color::Rgb(171, 178, 191))
                        }
                        // Light gray
                        else {
                            ("📄 ", Color::Rgb(171, 178, 191))
                        }
                    }
                    "rs" | "py" | "js" | "ts" => {
                        // nf-fa-code \uf121
                        if app.use_nerd_fonts {
                            ("\u{f121} ", Color::Rgb(86, 182, 194))
                        }
                        // Cyan
                        else {
                            ("💻 ", Color::Rgb(86, 182, 194))
                        }
                    }
                    _ => {
                        // nf-fa-file_o \uf016
                        if app.use_nerd_fonts {
                            ("\u{f016} ", Color::Rgb(92, 99, 112))
                        }
                        // Dark gray
                        else {
                            ("📄 ", Color::Rgb(92, 99, 112))
                        }
                    }
                }
            };

            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Rgb(40, 44, 52)) // Dark background text
                    .bg(Color::Rgb(97, 175, 239)) // Blue highlight
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            };

            let display_name = format!("{}{}", icon, entry.name);
            ListItem::new(Line::from(Span::styled(display_name, style)))
        })
        .collect();

    // Show current directory in the title with scroll indicator
    let total = app.entries.len();
    let title = if total > visible_height {
        format!(
            "Files [{}-{}/{}]",
            app.file_tree_scroll + 1,
            (app.file_tree_scroll + visible_height).min(total),
            total
        )
    } else {
        "Files".to_string()
    };

    let list = List::new(items).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(86, 182, 194))), // Cyan
    );

    f.render_widget(list, area);
}

fn render_content_viewer(f: &mut Frame, app: &mut App, area: Rect) {
    // Calculate visible height for app state
    let visible_height = area.height.saturating_sub(2) as usize;
    app.visible_height = visible_height;

    // Calculate line number width based on total lines
    let total_lines = app.file_content.len();
    let line_num_width = if total_lines == 0 {
        1
    } else {
        (total_lines as f64).log10().floor() as usize + 1
    };

    // Build content lines with line numbers
    let mut lines: Vec<Line> = Vec::with_capacity(visible_height);
    let content_width = area.width.saturating_sub(2) as usize; // minus borders

    for i in 0..visible_height {
        let content_idx = app.scroll_offset + i;

        if content_idx < app.file_content.len() {
            let file_line = &app.file_content[content_idx];
            let line_num = content_idx + 1;

            // Format line number
            let prefix = format!("{:>width$} │ ", line_num, width = line_num_width);
            let prefix_len = prefix.len();

            // Truncate content if too long
            let available_width = content_width.saturating_sub(prefix_len);
            let display_content: String = file_line.chars().take(available_width).collect();

            // Pad with spaces to fill entire width
            let padding_needed = available_width.saturating_sub(display_content.chars().count());
            let padded_content = format!("{}{}", display_content, " ".repeat(padding_needed));

            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Rgb(92, 99, 112))), // Dark gray
                Span::raw(padded_content),
            ]));
        } else {
            // Empty line with just spaces to fill width
            lines.push(Line::from(" ".repeat(content_width)));
        }
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(get_scroll_info(app, area))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(152, 195, 121))), // Green
    );

    f.render_widget(paragraph, area);
}

fn get_scroll_info(app: &App, area: Rect) -> String {
    let visible_height = area.height.saturating_sub(2) as usize;
    let total_lines = app.file_content.len();
    if total_lines > 0 {
        let end_line = (app.scroll_offset + visible_height).min(total_lines);
        format!(
            " Content [{}-{}/{}] ",
            app.scroll_offset + 1,
            end_line,
            total_lines
        )
    } else {
        " Content Viewer ".to_string()
    }
}

fn render_right_panel(f: &mut Frame, app: &App, area: Rect) {
    if app.show_chart {
        // Split right panel into chart (top) and stats (bottom)
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(60), // Chart
                Constraint::Percentage(40), // Stats/Info
            ])
            .split(area);

        // Render chart placeholder
        render_chart(f, app, right_chunks[0]);

        // Render stats/info
        render_stats(f, app, right_chunks[1]);
    } else {
        // Just show stats if chart is hidden
        render_stats(f, app, area);
    }
}

fn render_chart(f: &mut Frame, app: &App, area: Rect) {
    // Clear the chart area first to prevent Braille character artifacts
    f.render_widget(Clear, area);

    // Check if we have chart data
    if app.chart_data.is_empty() {
        // Show placeholder when no data
        let placeholder = Paragraph::new(vec![
            Line::from(""),
            Line::from("  No numeric data"),
            Line::from("  detected."),
            Line::from(""),
            Line::from("  Open a two-column"),
            Line::from("  data file to see"),
            Line::from("  a scatter plot."),
        ])
        .block(
            Block::default()
                .title(" Chart ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(198, 120, 221))), // Purple
        );
        f.render_widget(placeholder, area);
        return;
    }

    // Calculate available chart width for downsampling
    // Inner area is area minus borders (2 chars) minus y-axis labels (~8 chars)
    let chart_width = area.width.saturating_sub(12) as usize;

    // Downsample if we have too many points
    // Use 2 * width to allow for min/max preservation per bucket
    let target_points = (chart_width * 2).max(50);
    let display_data = App::downsample_with_peaks(&app.chart_data, target_points);

    // Format axis labels
    let (x_bounds, y_bounds) = app.chart_bounds;

    // Create nice axis labels
    let x_labels = vec![
        format_axis_value(x_bounds[0]).bold(),
        format_axis_value((x_bounds[0] + x_bounds[1]) / 2.0),
        format_axis_value(x_bounds[1]).bold(),
    ];

    let y_labels = vec![
        format_axis_value(y_bounds[0]).bold(),
        format_axis_value((y_bounds[0] + y_bounds[1]) / 2.0),
        format_axis_value(y_bounds[1]).bold(),
    ];

    // Create dataset
    let datasets = vec![
        Dataset::default()
            .name(format!("{} pts", app.chart_data.len()))
            .marker(Marker::Braille)
            .graph_type(GraphType::Scatter)
            .style(Style::default().fg(Color::Rgb(86, 182, 194))) // Cyan
            .data(&display_data),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(" Scatter Plot ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(198, 120, 221))), // Purple
        )
        .x_axis(
            Axis::default()
                .style(Style::default().fg(Color::Gray))
                .bounds(x_bounds)
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::Gray))
                .bounds(y_bounds)
                .labels(y_labels),
        );

    f.render_widget(chart, area);
}

/// Format a numeric value for axis labels (compact representation)
fn format_axis_value(val: f64) -> Span<'static> {
    let formatted = if val == 0.0 {
        "0".to_string()
    } else if val.abs() >= 1e6 || val.abs() < 1e-3 {
        // Scientific notation for very large or very small
        format!("{:.1e}", val)
    } else if val.abs() >= 1000.0 {
        format!("{:.0}", val)
    } else if val.abs() >= 1.0 {
        format!("{:.2}", val)
    } else {
        format!("{:.3}", val)
    };
    Span::raw(formatted)
}

fn render_stats(f: &mut Frame, app: &App, area: Rect) {
    // Build stats lines from file_stats (opened file)
    let mut stats_lines: Vec<Line> = vec![
        // Line::from(""),
        // Line::from(Span::styled(
        //    "File Statistics:",
        //    Style::default().fg(Color::Rgb(229, 192, 123)).add_modifier(Modifier::BOLD),  // Yellow
        //)),
        //Line::from(""),
    ];

    for line in app.file_stats.lines() {
        stats_lines.push(Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::Rgb(171, 178, 191)), // Light gray
        )));
    }

    let stats = Paragraph::new(stats_lines).block(
        Block::default()
            .title(" Info & Stats ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(229, 192, 123))), // Yellow
    );

    f.render_widget(stats, area);
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let nerd = app.use_nerd_fonts;

    // Create shortcut hints with Atom colors
    let shortcuts = vec![
        Span::styled(
            " ↑↓ ",
            Style::default()
                .fg(Color::Rgb(40, 44, 52))
                .bg(Color::Rgb(97, 175, 239)),
        ),
        Span::styled(" Nav ", Style::default().fg(Color::Rgb(171, 178, 191))),
        Span::raw(" "),
        Span::styled(
            " Enter ",
            Style::default()
                .fg(Color::Rgb(40, 44, 52))
                .bg(Color::Rgb(152, 195, 121)),
        ),
        Span::styled(" Open ", Style::default().fg(Color::Rgb(171, 178, 191))),
        Span::raw(" "),
        Span::styled(
            " Bksp ",
            Style::default()
                .fg(Color::Rgb(40, 44, 52))
                .bg(Color::Rgb(229, 192, 123)),
        ),
        Span::styled(" Parent ", Style::default().fg(Color::Rgb(171, 178, 191))),
        Span::raw(" "),
        Span::styled(
            " j/k ",
            Style::default()
                .fg(Color::Rgb(40, 44, 52))
                .bg(Color::Rgb(86, 182, 194)),
        ),
        Span::styled(" Scroll ", Style::default().fg(Color::Rgb(171, 178, 191))),
        Span::raw(" "),
        Span::styled(
            " u/d ",
            Style::default()
                .fg(Color::Rgb(40, 44, 52))
                .bg(Color::Rgb(86, 182, 194)),
        ),
        Span::styled(" Page ", Style::default().fg(Color::Rgb(171, 178, 191))),
        Span::raw(" "),
        Span::styled(
            " c ",
            Style::default()
                .fg(Color::Rgb(40, 44, 52))
                .bg(Color::Rgb(198, 120, 221)),
        ),
        Span::styled(" Chart ", Style::default().fg(Color::Rgb(171, 178, 191))),
        Span::raw(" "),
        Span::styled(
            " h ",
            Style::default()
                .fg(Color::Rgb(40, 44, 52))
                .bg(Color::Rgb(229, 192, 123)),
        ),
        Span::styled(" History ", Style::default().fg(Color::Rgb(171, 178, 191))),
        Span::raw(" "),
        Span::styled(
            " n ",
            Style::default()
                .fg(Color::Rgb(40, 44, 52))
                .bg(Color::Rgb(209, 154, 102)),
        ),
        Span::styled(
            if nerd { " Nerd✓ " } else { " Emoji " },
            Style::default().fg(Color::Rgb(171, 178, 191)),
        ),
        Span::raw(" "),
        Span::styled(
            " q ",
            Style::default()
                .fg(Color::Rgb(40, 44, 52))
                .bg(Color::Rgb(224, 108, 117)),
        ),
        Span::styled(" Quit ", Style::default().fg(Color::Rgb(171, 178, 191))),
    ];

    let status =
        Paragraph::new(Line::from(shortcuts)).style(Style::default().bg(Color::Rgb(33, 37, 43))); // Slightly lighter than main bg

    f.render_widget(status, area);
}

fn render_path_bar(f: &mut Frame, app: &App, area: Rect) {
    let path_text = if let Some(ref file_path) = app.current_file {
        format!(" {}", file_path.display())
    } else {
        format!(" {}", app.current_directory.display())
    };

    let path_bar = Paragraph::new(path_text).style(
        Style::default()
            .fg(Color::Rgb(171, 178, 191)) // Light gray text
            .bg(Color::Rgb(40, 44, 52)),
    ); // Dark background

    f.render_widget(path_bar, area);
}

fn render_recent_files_popup(f: &mut Frame, app: &App) {
    let area = f.area();
    
    // Calculate popup size (centered, 50% width, up to 14 lines height)
    let popup_width = (area.width as f32 * 0.5).min(60.0).max(30.0) as u16;
    let popup_height = if app.recent_files.is_empty() {
        5 // Minimum height for "no history" message
    } else {
        (app.recent_files.len() as u16 + 4).min(14)
    };
    
    let popup_x = (area.width - popup_width) / 2;
    let popup_y = (area.height - popup_height) / 2;
    
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);
    
    // Clear the popup area
    f.render_widget(Clear, popup_area);
    
    if app.recent_files.is_empty() {
        // Show "no history" message
        let message = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No history",
                Style::default().fg(Color::Rgb(92, 99, 112)), // Dark gray
            )),
        ])
        .block(
            Block::default()
                .title(" Recent Files ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(198, 120, 221))), // Purple
        );
        f.render_widget(message, popup_area);
        return;
    }
    
    // Create list items showing only the file name (end part of path)
    let items: Vec<ListItem> = app
        .recent_files
        .iter()
        .enumerate()
        .map(|(i, path)| {
            // Get the file name (end part of path)
            let display_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown");
            
            let style = if i == app.recent_files_selected {
                Style::default()
                    .fg(Color::Rgb(40, 44, 52)) // Dark background text
                    .bg(Color::Rgb(97, 175, 239)) // Blue highlight
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(171, 178, 191)) // Light gray
            };
            
            ListItem::new(Line::from(Span::styled(display_name.to_string(), style)))
        })
        .collect();
    
    let list = List::new(items).block(
        Block::default()
            .title(" Recent Files ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(198, 120, 221))), // Purple
    );
    
    f.render_widget(list, popup_area);
}
