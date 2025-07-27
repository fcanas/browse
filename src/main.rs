use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{prelude::*, widgets::*};
use std::{collections::VecDeque, fs, io, path::PathBuf, time::Duration};
use ratatui::DefaultTerminal;
use chrono::{DateTime, Local};
use std::collections::HashMap;
use crossterm::event::KeyModifiers;
use std::time::Instant;
use std::io::Read;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
struct Settings {
    show_hidden_files: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab {
    Display,
    Keybindings,
}

impl SettingsTab {
    fn next(self) -> Self {
        match self {
            Self::Display => Self::Keybindings,
            Self::Keybindings => Self::Display,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Display => Self::Keybindings,
            Self::Keybindings => Self::Display,
        }
    }
}

struct SettingsState {
    active_tab: SettingsTab,
}

struct Command {
    key: &'static str,
    description: &'static str,
}

const COMMANDS: &[Command] = &[
    Command { key: "Ctrl+Q", description: "Quit the application" },
    Command { key: "?", description: "Show this help window" },
    Command { key: "Up/Down", description: "Navigate list" },
    Command { key: "Left", description: "Navigate to parent directory" },
    Command { key: "Right", description: "Enter directory / activate preview" },
    Command { key: ".", description: "Set selected directory as anchor" },
];

struct FileDetails {
    path: PathBuf,
    size: u64,
    created: Option<DateTime<Local>>,
    modified: Option<DateTime<Local>>,
    symlink_target: Option<PathBuf>,
    content_preview: String,
}

enum Preview {
    Directory(DirColumn),
    File(FileDetails),
}

struct App {
    columns: VecDeque<DirColumn>,
    preview: Option<Preview>,
    selection_cache: HashMap<PathBuf, usize>,
    settings: Option<SettingsState>,
    config: Settings,
    search_string: String,
    last_key_time: Instant,
    should_quit: bool,
}

impl App {
    fn new() -> io::Result<Self> {
        let path = std::env::current_dir()?;
        let config = load_settings().unwrap_or_default();
        let initial_column = DirColumn::new(path, 0, &config)?;
        let mut columns = VecDeque::new();
        columns.push_back(initial_column);
        let mut app = Self {
            columns,
            preview: None,
            selection_cache: HashMap::new(),
            settings: None,
            config,
            search_string: String::new(),
            last_key_time: Instant::now(),
            should_quit: false,
        };
        app.update_preview()?;
        Ok(app)
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        if let Some(settings_state) = &mut self.settings {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => self.settings = None,
                KeyCode::Up => settings_state.active_tab = settings_state.active_tab.prev(),
                KeyCode::Down => settings_state.active_tab = settings_state.active_tab.next(),
                KeyCode::Char(' ') | KeyCode::Enter => {
                    if settings_state.active_tab == SettingsTab::Display {
                        self.config.show_hidden_files = !self.config.show_hidden_files;
                        self.columns
                            .iter_mut()
                            .try_for_each(|c| c.reload(&self.config))?;
                        self.update_preview()?;
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => self.settings = Some(SettingsState { active_tab: SettingsTab::Display }),
            KeyCode::Up => {
                self.active_column_mut().select_previous();
                self.update_preview()?;
            }
            KeyCode::Down => {
                self.active_column_mut().select_next();
                self.update_preview()?;
            }
            KeyCode::Right => self.on_right()?,
            KeyCode::Left => self.on_left()?,
            KeyCode::Char('.') => self.set_anchor()?,
            KeyCode::Char(c) if key.modifiers.is_empty() => {
                let now = Instant::now();
                if now.duration_since(self.last_key_time) > Duration::from_secs(1) {
                    self.search_string.clear();
                }
                self.search_string.push(c);
                self.last_key_time = now;

                let search_string = self.search_string.to_lowercase();
                if let Some(col) = self.columns.back_mut() {
                    if let Some(pos) = col.entries.iter().position(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .to_lowercase()
                            .starts_with(&search_string)
                    }) {
                        col.selected.select(Some(pos));
                    }
                }
                self.update_preview()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn active_column(&self) -> &DirColumn {
        self.columns.back().unwrap()
    }

    fn active_column_mut(&mut self) -> &mut DirColumn {
        self.columns.back_mut().unwrap()
    }

    fn on_right(&mut self) -> io::Result<()> {
        if self
            .active_column()
            .selected_entry()
            .map_or(false, |e| e.path().is_dir())
        {
            self.cache_active_selection();
            if let Some(Preview::Directory(dir_col)) = self.preview.take() {
                self.columns.push_back(dir_col);
                self.update_preview()?;
            }
        }
        // If the selected item is a file, do nothing.
        Ok(())
    }

    fn on_left(&mut self) -> io::Result<()> {
        self.cache_active_selection();
        let child_path = self.active_column().path.clone();
        if self.columns.len() > 1 {
            self.columns.pop_back();
        } else if let Some(parent) = self.active_column().path.parent() {
            let parent_col = DirColumn::new(parent.to_path_buf(), 0, &self.config)?;
            self.columns.pop_back();
            self.columns.push_back(parent_col);
        } else {
            // at root, do nothing
            return Ok(());
        }

        if let Some(active_col) = self.columns.back_mut() {
            if let Some(idx) = active_col
                .entries
                .iter()
                .position(|e| e.path() == child_path)
            {
                active_col.selected.select(Some(idx));
            }
        }

        self.update_preview()?;
        Ok(())
    }

    fn set_anchor(&mut self) -> io::Result<()> {
        self.cache_active_selection();
        if let Some(selected_entry) = self.active_column().selected_entry() {
            if selected_entry.path().is_dir() {
                let new_anchor_path = selected_entry.path();
                self.columns.clear();
                self.columns
                    .push_back(DirColumn::new(new_anchor_path, 0, &self.config)?);
                self.update_preview()?;
            }
        }
        Ok(())
    }

    fn cache_active_selection(&mut self) {
        if let Some(active_col) = self.columns.back() {
            if let Some(selection) = active_col.selected.selected() {
                self.selection_cache
                    .insert(active_col.path.clone(), selection);
            }
        }
    }

    fn update_preview(&mut self) -> io::Result<()> {
        self.preview = if let Some(entry) = self.active_column().selected_entry() {
            if entry.path().is_dir() {
                let path = entry.path();
                let selection = self.selection_cache.get(&path).copied().unwrap_or(0);
                Some(Preview::Directory(DirColumn::new(path, selection, &self.config)?))
            } else {
                let path = entry.path();
                let metadata = fs::symlink_metadata(&path)?;
                let created = metadata.created().ok().map(DateTime::from);
                let modified = metadata.modified().ok().map(DateTime::from);

                let symlink_target = if metadata.file_type().is_symlink() {
                    fs::read_link(&path).ok()
                } else {
                    None
                };

                let content_preview = if metadata.is_file() {
                    match fs::File::open(&path) {
                        Ok(file) => {
                            let mut buffer = String::new();
                            if file.take(4096).read_to_string(&mut buffer).is_ok() {
                                buffer
                            } else {
                                "[Content not valid UTF-8]".to_string()
                            }
                        }
                        Err(_) => "[Could not open file]".to_string(),
                    }
                } else {
                    "[Not a regular file]".to_string()
                };

                let details = FileDetails {
                    path: path.clone(),
                    size: metadata.len(),
                    created,
                    modified,
                    symlink_target,
                    content_preview,
                };
                Some(Preview::File(details))
            }
        } else {
            None
        };
        Ok(())
    }
}

struct DirColumn {
    path: PathBuf,
    entries: Vec<fs::DirEntry>,
    selected: ListState,
}

impl DirColumn {
    fn new(path: PathBuf, initial_selection: usize, config: &Settings) -> io::Result<Self> {
        let entries = Self::read_dir(&path, config)?;
        let mut selected = ListState::default();
        if !entries.is_empty() {
            selected.select(Some(initial_selection.min(entries.len() - 1)));
        }
        Ok(Self {
            path,
            entries,
            selected,
        })
    }

    fn read_dir(path: &PathBuf, config: &Settings) -> io::Result<Vec<fs::DirEntry>> {
        let mut entries: Vec<_> = fs::read_dir(path)?
            .filter_map(Result::ok)
            .filter(|e| {
                config.show_hidden_files
                    || !e.file_name().to_string_lossy().starts_with('.')
            })
            .collect();
        entries.sort_by_key(|e| e.path());
        Ok(entries)
    }

    fn reload(&mut self, config: &Settings) -> io::Result<()> {
        self.entries = Self::read_dir(&self.path, config)?;
        if self.selected.selected().map_or(0, |i| i) >= self.entries.len() {
            self.selected.select(Some(self.entries.len().saturating_sub(1)));
        }
        Ok(())
    }

    fn selected_entry(&self) -> Option<&fs::DirEntry> {
        self.selected.selected().and_then(|i| self.entries.get(i))
    }

    fn select_previous(&mut self) {
        let i = match self.selected.selected() {
            Some(i) => {
                if i == 0 {
                    self.entries.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.selected.select(Some(i));
    }

    fn select_next(&mut self) {
        let i = match self.selected.selected() {
            Some(i) => {
                if i >= self.entries.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.selected.select(Some(i));
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let mut app = App::new()?;
    run(&mut terminal, &mut app)?;
    save_settings(&app.config)?;
    ratatui::restore();
    Ok(())
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|f| ui(f, app))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key)?;
            }
        }
    }
    Ok(())
}

fn ui(frame: &mut Frame, app: &mut App) {
    let num_cols = app.columns.len() + if app.preview.is_some() { 1 } else { 0 };
    let constraints = (0..num_cols)
        .map(|_| Constraint::Ratio(1, num_cols as u32))
        .collect::<Vec<_>>();
    let layout = Layout::horizontal(constraints).split(frame.area());

    let active_column_index = app.columns.len() - 1;
    for (i, column) in app.columns.iter_mut().enumerate() {
        let is_active = i == active_column_index;
        render_dir_column(frame, column, layout[i], is_active, false);
    }

    if let Some(preview) = &mut app.preview {
        let preview_area = layout[app.columns.len()];
        match preview {
            Preview::Directory(dir_column) => {
                render_dir_column(frame, dir_column, preview_area, false, true);
            }
            Preview::File(details) => {
                let chunks = Layout::vertical([Constraint::Max(6), Constraint::Min(0)])
                    .split(preview_area);
                let title = details
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let mut lines = vec![Line::from(vec![
                    Span::styled("Size: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!("{} bytes", details.size)),
                ])];
                if let Some(created) = details.created {
                    lines.push(Line::from(vec![
                        Span::styled("Created: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(created.format("%Y-%m-%d %H:%M:%S").to_string()),
                    ]));
                }
                if let Some(modified) = details.modified {
                    lines.push(Line::from(vec![
                        Span::styled("Modified: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(modified.format("%Y-%m-%d %H:%M:%S").to_string()),
                    ]));
                }
                if let Some(target) = &details.symlink_target {
                    lines.push(Line::from(vec![
                        Span::styled("Symlink -> ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(target.to_string_lossy().to_string()),
                    ]));
                }

                let metadata_widget = Paragraph::new(lines).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title.to_string())
                        .padding(Padding::uniform(1)),
                );

                let content_widget = Paragraph::new(details.content_preview.clone())
                    .block(Block::default().borders(Borders::ALL));

                frame.render_widget(metadata_widget, chunks[0]);
                frame.render_widget(content_widget, chunks[1]);
            }
        }
    }

    if let Some(settings) = &app.settings {
        render_settings_panel(frame, settings, &app.config);
    }
}

fn render_settings_panel(frame: &mut Frame, settings_state: &SettingsState, config: &Settings) {
    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);

    let chunks = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    let tabs = vec!["Display", "Keybindings"];
    let mut list_state = ListState::default();
    list_state.select(Some(settings_state.active_tab as usize));

    let list = List::new(tabs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Settings")
                .padding(Padding::uniform(1)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    match settings_state.active_tab {
        SettingsTab::Display => {
            let checkbox_text = if config.show_hidden_files {
                "[x] Show hidden files"
            } else {
                "[ ] Show hidden files"
            };
            let checkbox_widget = Paragraph::new(checkbox_text)
                .style(Style::default().fg(Color::Cyan))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Display")
                        .padding(Padding::uniform(1)),
                );
            frame.render_widget(checkbox_widget, chunks[1]);
        }
        SettingsTab::Keybindings => {
            let rows = COMMANDS.iter().map(|cmd| {
                Row::new(vec![Cell::from(cmd.key), Cell::from(cmd.description)])
            });
            let table = Table::new(rows, [Constraint::Percentage(30), Constraint::Percentage(70)])
                .block(
                    Block::default()
                        .title("Keybindings")
                        .borders(Borders::ALL)
                        .padding(Padding::uniform(1)),
                )
                .header(
                    Row::new(vec!["Key", "Description"])
                        .style(Style::new().add_modifier(Modifier::BOLD)),
                );
            frame.render_widget(table, chunks[1]);
        }
    }
}

fn render_dir_column(
    frame: &mut Frame,
    column: &mut DirColumn,
    area: Rect,
    is_active: bool,
    is_preview: bool,
) {
    let title = column
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let items: Vec<ListItem> = column
        .entries
        .iter()
        .map(|entry| {
            let path = entry.path();
            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let style = if path.is_dir() {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            ListItem::new(Span::styled(file_name, style))
        })
        .collect();

    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title.to_string());

    if is_preview {
        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    } else {
        let highlight_style = if is_active {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default().bg(Color::DarkGray)
        };
        let list = List::new(items)
            .block(block)
            .highlight_style(highlight_style)
            .highlight_symbol(">> ");
        frame.render_stateful_widget(list, area, &mut column.selected);
    }
}


fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

fn settings_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".browse")
}

fn load_settings() -> io::Result<Settings> {
    let path = settings_path();
    let file = fs::File::open(path)?;
    serde_json::from_reader(file).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

fn save_settings(settings: &Settings) -> io::Result<()> {
    let path = settings_path();
    let file = fs::File::create(path)?;
    serde_json::to_writer_pretty(file, settings).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
} 