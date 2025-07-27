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
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
struct FileTypeRule {
    icon: String,
    preview: bool,
}

#[derive(Serialize, Deserialize)]
struct MimeTypeConfig {
    primary: HashMap<String, FileTypeRule>,
    subtypes: HashMap<String, FileTypeRule>,
}

#[derive(Serialize, Deserialize)]
struct Settings {
    show_hidden_files: bool,
    show_icons: bool,
    mime_types: MimeTypeConfig,
}

impl Default for Settings {
    fn default() -> Self {
        let mut primary = HashMap::new();
        primary.insert("text".to_string(), FileTypeRule { icon: "📄".to_string(), preview: true });
        primary.insert("image".to_string(), FileTypeRule { icon: "🖼️".to_string(), preview: false });
        primary.insert("video".to_string(), FileTypeRule { icon: "🎬".to_string(), preview: false });
        primary.insert("audio".to_string(), FileTypeRule { icon: "🎵".to_string(), preview: false });
        primary.insert("application".to_string(), FileTypeRule { icon: "📦".to_string(), preview: false });

        let mut subtypes = HashMap::new();
        subtypes.insert("text/markdown".to_string(), FileTypeRule { icon: "📝".to_string(), preview: true });
        subtypes.insert("text/x-rust".to_string(), FileTypeRule { icon: "🦀".to_string(), preview: true });
        subtypes.insert("application/toml".to_string(), FileTypeRule { icon: "🦀".to_string(), preview: true });
        subtypes.insert("application/x-sh".to_string(), FileTypeRule { icon: "🚀".to_string(), preview: true });
        subtypes.insert("symlink".to_string(), FileTypeRule { icon: "🔗".to_string(), preview: false });
        
        Self {
            show_hidden_files: false,
            show_icons: true,
            mime_types: MimeTypeConfig { primary, subtypes },
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab {
    Display,
    FileTypes,
    Keybindings,
}

impl SettingsTab {
    fn next(self) -> Self {
        match self {
            Self::Display => Self::FileTypes,
            Self::FileTypes => Self::Keybindings,
            Self::Keybindings => Self::Display,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Display => Self::Keybindings,
            Self::FileTypes => Self::Display,
            Self::Keybindings => Self::FileTypes,
        }
    }
}

#[derive(PartialEq)]
enum SettingsFocus {
    TabList,
    TabContent,
    AddFileTypePopup,
}

struct AddFileTypeState {
    mime_type: String,
    icon: String,
    preview: bool,
    focused_field: usize,
    is_editing: Option<String>,
}

struct SettingsState {
    active_tab: SettingsTab,
    display_selection: usize,
    file_type_selection: usize,
    file_type_column_selection: usize,
    focus: SettingsFocus,
    add_file_type_state: Option<AddFileTypeState>,
    file_type_table_state: TableState,
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
    mime_type: Option<String>,
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
            match settings_state.focus {
                SettingsFocus::TabList => match key.code {
                    KeyCode::Esc | KeyCode::Char('?') => self.settings = None,
                    KeyCode::Up => settings_state.active_tab = settings_state.active_tab.prev(),
                    KeyCode::Down => settings_state.active_tab = settings_state.active_tab.next(),
                    KeyCode::Right | KeyCode::Enter => {
                        settings_state.focus = SettingsFocus::TabContent
                    }
                    _ => {}
                },
                SettingsFocus::TabContent => match settings_state.active_tab {
                    SettingsTab::Display => match key.code {
                        KeyCode::Left | KeyCode::Esc => {
                            settings_state.focus = SettingsFocus::TabList
                        }
                        KeyCode::Up => {
                            settings_state.display_selection =
                                settings_state.display_selection.saturating_sub(1)
                        }
                        KeyCode::Down => {
                            settings_state.display_selection =
                                (settings_state.display_selection + 1).min(1)
                        }
                        KeyCode::Char(' ') | KeyCode::Enter => {
                            match settings_state.display_selection {
                                0 => {
                                    self.config.show_hidden_files =
                                        !self.config.show_hidden_files
                                }
                                1 => self.config.show_icons = !self.config.show_icons,
                                _ => {}
                            }
                            if settings_state.display_selection == 0 {
                                self.columns
                                    .iter_mut()
                                    .try_for_each(|c| c.reload(&self.config))?;
                                self.update_preview()?;
                            }
                        }
                        _ => {}
                    },
                    SettingsTab::FileTypes => {
                        if let Some(add_state) = &mut settings_state.add_file_type_state {
                            match key.code {
                                KeyCode::Esc => {
                                    settings_state.add_file_type_state = None;
                                    settings_state.focus = SettingsFocus::TabContent;
                                }
                                KeyCode::Tab => {
                                    add_state.focused_field = (add_state.focused_field + 1) % 3;
                                }
                                KeyCode::BackTab => {
                                    add_state.focused_field =
                                        (add_state.focused_field + 2) % 3;
                                }
                                KeyCode::Char(c) => match add_state.focused_field {
                                    0 => add_state.mime_type.push(c),
                                    1 => add_state.icon.push(c),
                                    _ => {}
                                },
                                KeyCode::Backspace => match add_state.focused_field {
                                    0 if add_state.is_editing.is_none() => {
                                        add_state.mime_type.pop();
                                    }
                                    1 => {
                                        add_state.icon.pop();
                                    }
                                    _ => {}
                                },
                                KeyCode::Enter => {
                                    if add_state.focused_field == 2 {
                                        add_state.preview = !add_state.preview;
                                    } else {
                                        let rule = FileTypeRule {
                                            icon: add_state.icon.clone(),
                                            preview: add_state.preview,
                                        };
                                        if let Some(original_key) = &add_state.is_editing {
                                            if original_key.contains('/') {
                                                self.config.mime_types.subtypes.remove(original_key);
                                            } else {
                                                self.config.mime_types.primary.remove(original_key);
                                            }
                                        }
                                        let key = add_state.mime_type.clone();
                                        if key.contains('/') {
                                            self.config.mime_types.subtypes.insert(key, rule);
                                        } else {
                                            self.config.mime_types.primary.insert(key, rule);
                                        }
                                        settings_state.add_file_type_state = None;
                                        settings_state.focus = SettingsFocus::TabContent;
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Left => {
                                    if settings_state.file_type_column_selection > 0 {
                                        settings_state.file_type_column_selection -= 1;
                                    } else {
                                        settings_state.focus = SettingsFocus::TabList;
                                    }
                                }
                                KeyCode::Right => {
                                    settings_state.file_type_column_selection =
                                        (settings_state.file_type_column_selection + 1).min(2);
                                }
                                KeyCode::Up => {
                                    settings_state.file_type_selection =
                                        settings_state.file_type_selection.saturating_sub(1);
                                    settings_state
                                        .file_type_table_state
                                        .select(Some(settings_state.file_type_selection));
                                }
                                KeyCode::Down => {
                                    let max_len = (self.config.mime_types.primary.len()
                                        + self.config.mime_types.subtypes.len())
                                        .saturating_sub(1);
                                    settings_state.file_type_selection =
                                        (settings_state.file_type_selection + 1).min(max_len);
                                    settings_state
                                        .file_type_table_state
                                        .select(Some(settings_state.file_type_selection));
                                }
                                KeyCode::Char('a') => {
                                    settings_state.add_file_type_state = Some(AddFileTypeState {
                                        mime_type: String::new(),
                                        icon: String::new(),
                                        preview: false,
                                        focused_field: 0,
                                        is_editing: None,
                                    });
                                    settings_state.focus = SettingsFocus::AddFileTypePopup;
                                }
                                KeyCode::Char('d') => {
                                    let mut sorted_keys: Vec<_> = self
                                        .config
                                        .mime_types
                                        .primary
                                        .keys()
                                        .chain(self.config.mime_types.subtypes.keys())
                                        .collect();
                                    sorted_keys.sort();

                                    if let Some(key_to_delete) =
                                        sorted_keys.get(settings_state.file_type_selection)
                                    {
                                        let key_str = key_to_delete.to_string();
                                        if self.config.mime_types.subtypes.remove(&key_str).is_none()
                                        {
                                            self.config.mime_types.primary.remove(&key_str);
                                        }

                                        let new_max_len = (self.config.mime_types.primary.len()
                                            + self.config.mime_types.subtypes.len())
                                            .saturating_sub(1);
                                        if settings_state.file_type_selection > new_max_len {
                                            settings_state.file_type_selection = new_max_len;
                                        }
                                        settings_state.file_type_table_state.select(Some(
                                            settings_state.file_type_selection,
                                        ));
                                    }
                                }
                                KeyCode::Char('e') => {
                                    let mut sorted_keys: Vec<_> = self
                                        .config
                                        .mime_types
                                        .primary
                                        .keys()
                                        .chain(self.config.mime_types.subtypes.keys())
                                        .collect();
                                    sorted_keys.sort();

                                    if let Some(key_to_edit) =
                                        sorted_keys.get(settings_state.file_type_selection)
                                    {
                                        if key_to_edit.contains('/') {
                                            if let Some(rule) = get_rule(&self.config, key_to_edit)
                                            {
                                                settings_state.add_file_type_state =
                                                    Some(AddFileTypeState {
                                                        mime_type: key_to_edit.to_string(),
                                                        icon: rule.icon.clone(),
                                                        preview: rule.preview,
                                                        focused_field: 0,
                                                        is_editing: Some(key_to_edit.to_string()),
                                                    });
                                                settings_state.focus =
                                                    SettingsFocus::AddFileTypePopup;
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char(' ') | KeyCode::Enter => {
                                    if settings_state.file_type_column_selection == 2 {
                                        let mut sorted_exts: Vec<_> = self
                                            .config
                                            .mime_types
                                            .primary
                                            .keys()
                                            .chain(self.config.mime_types.subtypes.keys())
                                            .collect();
                                        sorted_exts.sort();

                                        if let Some(key) =
                                            sorted_exts.get(settings_state.file_type_selection)
                                        {
                                            let key = key.to_string();
                                            let rule = self
                                                .config
                                                .mime_types
                                                .primary
                                                .get_mut(&key)
                                                .or_else(|| {
                                                    self.config.mime_types.subtypes.get_mut(&key)
                                                });
                                            if let Some(rule) = rule {
                                                rule.preview = !rule.preview;
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    SettingsTab::Keybindings => match key.code {
                        KeyCode::Left | KeyCode::Esc => {
                            settings_state.focus = SettingsFocus::TabList
                        }
                        _ => {}
                    },
                },
                SettingsFocus::AddFileTypePopup => {
                    if let Some(add_state) = &mut settings_state.add_file_type_state {
                        match key.code {
                            KeyCode::Esc => {
                                settings_state.add_file_type_state = None;
                                settings_state.focus = SettingsFocus::TabContent;
                            }
                            KeyCode::Tab => {
                                add_state.focused_field = (add_state.focused_field + 1) % 3;
                            }
                            KeyCode::BackTab => {
                                add_state.focused_field = (add_state.focused_field + 2) % 3;
                            }
                            KeyCode::Char(c) => match add_state.focused_field {
                                0 => add_state.mime_type.push(c),
                                1 => add_state.icon.push(c),
                                _ => {}
                            },
                            KeyCode::Backspace => match add_state.focused_field {
                                0 => {
                                    add_state.mime_type.pop();
                                }
                                1 => {
                                    add_state.icon.pop();
                                }
                                _ => {}
                            },
                            KeyCode::Enter => {
                                if add_state.focused_field == 2 {
                                    add_state.preview = !add_state.preview;
                                } else {
                                    let rule = FileTypeRule {
                                        icon: add_state.icon.clone(),
                                        preview: add_state.preview,
                                    };
                                    self.config
                                        .mime_types
                                        .subtypes
                                        .insert(add_state.mime_type.clone(), rule);
                                    settings_state.add_file_type_state = None;
                                    settings_state.focus = SettingsFocus::TabContent;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => {
                self.settings = Some(SettingsState {
                    active_tab: SettingsTab::Display,
                    display_selection: 0,
                    file_type_selection: 0,
                    file_type_column_selection: 0,
                    focus: SettingsFocus::TabList,
                    add_file_type_state: None,
                    file_type_table_state: TableState::default(),
                })
            }
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
                Some(Preview::Directory(DirColumn::new(
                    path,
                    selection,
                    &self.config,
                )?))
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

                let mut mime_type = None;
                let mut content_preview = "[Not a regular file]".to_string();

                if metadata.is_file() {
                    mime_type = get_mime_type(&path);

                    let can_preview = mime_type
                        .as_ref()
                        .and_then(|mime_str| get_rule(&self.config, mime_str))
                        .map_or(false, |rule| rule.preview);

                    content_preview = if can_preview {
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
                        "[Preview not available for this file type]".to_string()
                    };
                }

                let details = FileDetails {
                    path: path.clone(),
                    size: metadata.len(),
                    created,
                    modified,
                    symlink_target,
                    content_preview,
                    mime_type: mime_type.map(|t| t.to_string()),
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
        render_dir_column(frame, column, layout[i], is_active, false, &app.config);
    }

    if let Some(preview) = &mut app.preview {
        let preview_area = layout[app.columns.len()];
        match preview {
            Preview::Directory(dir_column) => {
                render_dir_column(frame, dir_column, preview_area, false, true, &app.config);
            }
            Preview::File(details) => {
                let chunks = Layout::vertical([Constraint::Max(8), Constraint::Min(0)])
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
                lines.push(Line::from(vec![
                    Span::styled("MIME Type: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(details.mime_type.as_deref().unwrap_or("unknown")),
                ]));

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

    if let Some(settings) = &mut app.settings {
        render_settings_panel(frame, settings, &app.config);
    }
}

fn render_settings_panel(frame: &mut Frame, settings_state: &mut SettingsState, config: &Settings) {
    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);

    let chunks = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    let tabs = vec!["Display", "File Types", "Keybindings"];
    let mut list_state = ListState::default();
    list_state.select(Some(settings_state.active_tab as usize));

    let list_border_style = if settings_state.focus == SettingsFocus::TabList {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let list = List::new(tabs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Settings")
                .padding(Padding::uniform(1))
                .border_style(list_border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    let content_border_style = if settings_state.focus == SettingsFocus::TabContent {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    match settings_state.active_tab {
        SettingsTab::Display => {
            let display_options = vec!["Show hidden files", "Show icons"];
            let mut list_state = ListState::default();
            list_state.select(Some(settings_state.display_selection));
            let items: Vec<ListItem> = display_options
                .iter()
                .enumerate()
                .map(|(i, &name)| {
                    let checked = match i {
                        0 => config.show_hidden_files,
                        1 => config.show_icons,
                        _ => false,
                    };
                    let prefix = if checked { "[x] " } else { "[ ] " };
                    ListItem::new(format!("{}{}", prefix, name))
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Display")
                        .padding(Padding::uniform(1))
                        .border_style(content_border_style),
                )
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            frame.render_stateful_widget(list, chunks[1], &mut list_state);
        }
        SettingsTab::FileTypes => {
            let file_types_chunks =
                Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(chunks[1]);

            let mut sorted_exts: Vec<_> = config
                .mime_types
                .primary
                .keys()
                .chain(config.mime_types.subtypes.keys())
                .collect();
            sorted_exts.sort();

            let rows = sorted_exts.iter().enumerate().map(|(row_index, ext)| {
                let rule = if let Some(rule) = config.mime_types.primary.get(*ext) {
                    rule
                } else {
                    config.mime_types.subtypes.get(*ext).unwrap()
                };
                let is_selected_row = row_index == settings_state.file_type_selection
                    && settings_state.focus == SettingsFocus::TabContent;

                let row_style = if is_selected_row {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };

                let cells_data = [
                    ext.to_string(),
                    rule.icon.clone(),
                    if rule.preview {
                        "✅".to_string()
                    } else {
                        "❌".to_string()
                    },
                ];

                let cells: Vec<Cell> = cells_data
                    .into_iter()
                    .enumerate()
                    .map(|(col_index, data)| {
                        let is_selected_cell = is_selected_row
                            && col_index == settings_state.file_type_column_selection;
                        let style = if is_selected_cell {
                            Style::default().add_modifier(Modifier::REVERSED)
                        } else {
                            row_style
                        };
                        Cell::from(data).style(style)
                    })
                    .collect();

                Row::new(cells)
            });

            let table = Table::new(
                rows,
                [
                    Constraint::Length(20),
                    Constraint::Length(4),
                    Constraint::Length(8),
                ],
            )
            .header(
                Row::new(vec!["Mime Type", "Icon", "Preview"])
                    .style(Style::new().add_modifier(Modifier::BOLD)),
            )
            .block(
                Block::default()
                    .title("File Type Handling")
                    .borders(Borders::ALL)
                    .padding(Padding::uniform(1))
                    .border_style(content_border_style),
            )
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_stateful_widget(
            table,
            file_types_chunks[0],
            &mut settings_state.file_type_table_state,
        );

        let footer =
            Paragraph::new("[A]dd, [D]elete, [E]dit").alignment(Alignment::Center);
        frame.render_widget(footer, file_types_chunks[1]);

        if let Some(add_state) = &settings_state.add_file_type_state {
            render_add_file_type_popup(frame, add_state);
        }
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
                        .padding(Padding::uniform(1))
                        .border_style(content_border_style),
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
    config: &Settings,
) {
    let title = column
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let items: Vec<ListItem> = column
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let path = entry.path();
            let is_selected = Some(i) == column.selected.selected();
            let icon = if is_active || is_preview {
                get_icon(entry, is_selected, config)
            } else {
                "".to_string()
            };
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
            ListItem::new(Span::styled(format!("{} {}", icon, file_name), style))
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
            .highlight_style(highlight_style);
        frame.render_stateful_widget(list, area, &mut column.selected);
    }
}

fn render_add_file_type_popup(frame: &mut Frame, state: &AddFileTypeState) {
    let area = centered_rect(60, 40, frame.area());
    frame.render_widget(Clear, area);
    let title = if state.is_editing.is_some() {
        "Edit File Type"
    } else {
        "Add New File Type"
    };
    let popup_block = Block::default().title(title).borders(Borders::ALL);

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .margin(1)
    .split(popup_block.inner(area));
    frame.render_widget(popup_block, area);

    let mime_input = Paragraph::new(state.mime_type.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title("MIME Type")
            .border_style(if state.focused_field == 0 {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    frame.render_widget(mime_input, chunks[0]);

    let icon_input = Paragraph::new(state.icon.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Icon")
            .border_style(if state.focused_field == 1 {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    frame.render_widget(icon_input, chunks[1]);

    let checkbox_text = if state.preview { "[x] Preview" } else { "[ ] Preview" };
    let checkbox = Paragraph::new(checkbox_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Preview")
            .border_style(if state.focused_field == 2 {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    frame.render_widget(checkbox, chunks[2]);
}

fn get_icon(entry: &fs::DirEntry, is_selected: bool, config: &Settings) -> String {
    if !config.show_icons {
        return "".to_string();
    }

    if let Ok(file_type) = entry.file_type() {
        if file_type.is_dir() {
            return if is_selected { "📂" } else { "📁" }.to_string();
        }
        if file_type.is_symlink() {
            return config
                .mime_types
                .subtypes
                .get("symlink")
                .map(|r| r.icon.clone())
                .unwrap_or("🔗".to_string());
        }
        if !file_type.is_file() {
            return "❓".to_string();
        }
    }

    let path = entry.path();
    #[cfg(unix)]
    if let Ok(metadata) = entry.metadata() {
        if metadata.permissions().mode() & 0o111 != 0 {
            return "🚀".to_string();
        }
    }

    if let Some(mime_type) = get_mime_type(&path) {
        if let Some(rule) = get_rule(config, &mime_type) {
            return rule.icon.clone();
        }
    }
    "📄".to_string()
}

fn get_mime_type(path: &Path) -> Option<String> {
    if let Some(kind) = infer::get_from_path(path).ok().flatten() {
        return Some(kind.mime_type().to_string());
    }

    // Fallback to extension
    let extension = path.extension()?.to_str()?.to_lowercase();
    match extension.as_str() {
        "txt" | "log" => Some("text/plain".to_string()),
        "md" => Some("text/markdown".to_string()),
        "rs" => Some("text/x-rust".to_string()),
        "toml" => Some("application/toml".to_string()),
        "json" => Some("application/json".to_string()),
        "yaml" | "yml" => Some("application/x-yaml".to_string()),
        "html" | "htm" => Some("text/html".to_string()),
        "xml" => Some("application/xml".to_string()),
        "svg" => Some("image/svg+xml".to_string()),
        "ics" => Some("text/calendar".to_string()),
        "css" => Some("text/css".to_string()),
        "csv" => Some("text/csv".to_string()),
        "js" | "mjs" => Some("application/javascript".to_string()),
        "ts" | "mts" => Some("application/typescript".to_string()),
        "py" | "pyw" => Some("text/x-python".to_string()),
        "sh" | "bash" => Some("application/x-sh".to_string()),
        "jpg" | "jpeg" => Some("image/jpeg".to_string()),
        "png" => Some("image/png".to_string()),
        "gif" => Some("image/gif".to_string()),
        "zip" => Some("application/zip".to_string()),
        "gz" => Some("application/gzip".to_string()),
        "c" | "cc" | "cpp" | "h" | "hpp" | "hh" | "hxx" | "cxx" => Some("text/x-c".to_string()),
        "java" => Some("text/x-java".to_string()),
        "php" => Some("text/x-php".to_string()),
        "rb" => Some("text/x-ruby".to_string()),
        "swift" => Some("text/x-swift".to_string()),
        "go" => Some("text/x-go".to_string()),
        "dart" => Some("text/x-dart".to_string()),
        "m" | "mm" | "mxx" => Some("text/x-objectivec".to_string()),
        "cs" => Some("text/x-csharp".to_string()),
        "pl" => Some("text/x-perl".to_string()),
        "lua" => Some("text/x-lua".to_string()),
        "sql" => Some("text/x-sql".to_string()),
        "kt" | "kts" => Some("text/x-kotlin".to_string()),
        _ => None,
    }
}

fn get_rule<'a>(config: &'a Settings, mime_type: &str) -> Option<&'a FileTypeRule> {
    if let Some(rule) = config.mime_types.subtypes.get(mime_type) {
        return Some(rule);
    }
    if let Some(primary_type) = mime_type.split('/').next() {
        if let Some(rule) = config.mime_types.primary.get(primary_type) {
            return Some(rule);
        }
    }
    None
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