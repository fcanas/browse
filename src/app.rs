use crate::config::{Settings, load_settings, SEARCH_TIMEOUT_SECONDS, MAX_COLUMNS_DISPLAY};
use crate::file_operations::{FileDetails, read_directory, is_safe_path};
use crate::ui::{render_ui, SettingsState, SettingsTab, SettingsFocus, AddFileTypeState};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Frame, widgets::ListState};
use std::collections::{HashMap, VecDeque};
use std::fs::DirEntry;
use std::io;
use std::path::PathBuf;
use std::time::Instant;

/// Preview content for the right panel
#[derive(Debug)]
pub enum Preview {
    Directory(DirColumn),
    File(FileDetails),
}

/// A column in the Miller columns interface
#[derive(Debug)]
pub struct DirColumn {
    pub path: PathBuf,
    pub entries: Vec<DirEntry>,
    pub selected: ListState,
}

impl DirColumn {
    /// Create a new directory column
    pub fn new(path: PathBuf, initial_selection: usize, config: &Settings) -> io::Result<Self> {
        if !is_safe_path(&path) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Path not allowed for security reasons",
            ));
        }
        
        let entries = read_directory(&path, config)?;
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
    
    /// Reload the directory contents
    pub fn reload(&mut self, config: &Settings) -> io::Result<()> {
        self.entries = read_directory(&self.path, config)?;
        
        // Adjust selection if it's out of bounds
        if let Some(current_selection) = self.selected.selected() {
            if current_selection >= self.entries.len() {
                let new_selection = self.entries.len().saturating_sub(1);
                self.selected.select(if self.entries.is_empty() { None } else { Some(new_selection) });
            }
        }
        
        Ok(())
    }
    
    /// Get the currently selected entry
    pub fn selected_entry(&self) -> Option<&DirEntry> {
        self.selected.selected().and_then(|i| self.entries.get(i))
    }
    
    /// Navigate to previous item
    pub fn select_previous(&mut self) {
        let new_index = match self.selected.selected() {
            Some(i) if i > 0 => i - 1,
            Some(_) => self.entries.len().saturating_sub(1),
            None => 0,
        };
        self.selected.select(if self.entries.is_empty() { None } else { Some(new_index) });
    }
    
    /// Navigate to next item
    pub fn select_next(&mut self) {
        let new_index = match self.selected.selected() {
            Some(i) if i < self.entries.len().saturating_sub(1) => i + 1,
            Some(_) => 0,
            None => 0,
        };
        self.selected.select(if self.entries.is_empty() { None } else { Some(new_index) });
    }
}

/// Main application state
pub struct App {
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
    /// Create a new application instance
    pub fn new() -> Result<Self> {
        let current_dir = std::env::current_dir()
            .map_err(|e| color_eyre::eyre::eyre!("Failed to get current directory: {}", e))?;
        
        let config = load_settings()
            .map_err(|e| color_eyre::eyre::eyre!("Failed to load settings: {}", e))?;
        
        let initial_column = DirColumn::new(current_dir, 0, &config)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to read initial directory: {}", e))?;
        
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
    
    /// Check if the application should quit
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }
    
    /// Get reference to the current configuration
    pub fn config(&self) -> &Settings {
        &self.config
    }
    
    /// Render the application UI
    pub fn render(&mut self, frame: &mut Frame) {
        render_ui(frame, self);
    }
    
    /// Handle keyboard input
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }
        
        // Handle settings panel if open
        if self.settings.is_some() {
            return self.handle_settings_key(key);
        }
        
        // Handle global shortcuts
        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return Ok(());
            }
            KeyCode::Char('?') => {
                self.open_settings();
                return Ok(());
            }
            KeyCode::Esc => {
                // Clear search string on Escape
                if !self.search_string.is_empty() {
                    self.search_string.clear();
                }
                return Ok(());
            }
            KeyCode::Home => {
                // Jump to first item
                if let Some(column) = self.columns.back_mut() {
                    if !column.entries.is_empty() {
                        column.selected.select(Some(0));
                        self.update_preview()?;
                    }
                }
                return Ok(());
            }
            KeyCode::End => {
                // Jump to last item
                if let Some(column) = self.columns.back_mut() {
                    if !column.entries.is_empty() {
                        column.selected.select(Some(column.entries.len() - 1));
                        self.update_preview()?;
                    }
                }
                return Ok(());
            }
            _ => {}
        }
        
        // Handle navigation
        self.handle_navigation_key(key)
    }
    
    /// Handle navigation keys
    fn handle_navigation_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up => {
                if let Some(column) = self.columns.back_mut() {
                    column.select_previous();
                    self.update_preview()?;
                }
            }
            KeyCode::Down => {
                if let Some(column) = self.columns.back_mut() {
                    column.select_next();
                    self.update_preview()?;
                }
            }
            KeyCode::Left => {
                self.navigate_left()?;
            }
            KeyCode::Right => {
                self.navigate_right()?;
            }
            KeyCode::Char('.') => {
                self.set_anchor()?;
            }
            KeyCode::PageUp => {
                // Jump up by 10 items
                if let Some(column) = self.columns.back_mut() {
                    if let Some(current) = column.selected.selected() {
                        let new_index = current.saturating_sub(10);
                        column.selected.select(Some(new_index));
                        self.update_preview()?;
                    }
                }
            }
            KeyCode::PageDown => {
                // Jump down by 10 items
                if let Some(column) = self.columns.back_mut() {
                    if let Some(current) = column.selected.selected() {
                        let new_index = (current + 10).min(column.entries.len().saturating_sub(1));
                        column.selected.select(Some(new_index));
                        self.update_preview()?;
                    }
                }
            }
            KeyCode::Char(c) => {
                self.handle_search_char(c)?;
            }
            _ => {}
        }
        Ok(())
    }
    
    /// Navigate left (parent directory)
    fn navigate_left(&mut self) -> Result<()> {
        if self.columns.is_empty() {
            return Ok(());
        }
        
        // Cache current selection
        if let Some(column) = self.columns.back() {
            if let Some(selected_idx) = column.selected.selected() {
                self.selection_cache.insert(column.path.clone(), selected_idx);
            }
        }
        
        // If we have more than one column, just remove the rightmost
        if self.columns.len() > 1 {
            self.columns.pop_back();
        } else {
            // Navigate to parent directory
            if let Some(parent) = self.columns.back().unwrap().path.parent() {
                let parent_path = parent.to_path_buf();
                let current_name = self.columns.back().unwrap().path.file_name();
                
                // Find the index of the current directory in the parent
                let initial_selection = if let Some(name) = current_name {
                    let parent_column = DirColumn::new(parent_path.clone(), 0, &self.config)?;
                    parent_column.entries.iter().position(|entry| {
                        entry.file_name() == name
                    }).unwrap_or(0)
                } else {
                    0
                };
                
                let parent_column = DirColumn::new(parent_path, initial_selection, &self.config)?;
                self.columns.clear();
                self.columns.push_back(parent_column);
            }
        }
        
        self.update_preview()?;
        Ok(())
    }
    
    /// Navigate right (enter directory)
    fn navigate_right(&mut self) -> Result<()> {
        if let Some(entry) = self.active_column().selected_entry() {
            let path = entry.path();
            
            if path.is_dir() {
                // Cache current selection
                if let Some(selected_idx) = self.active_column().selected.selected() {
                    self.selection_cache.insert(self.active_column().path.clone(), selected_idx);
                }
                
                let cached_selection = self.selection_cache.get(&path).copied().unwrap_or(0);
                
                // Try to create new column, but don't fail the whole operation if it fails
                match DirColumn::new(path, cached_selection, &self.config) {
                    Ok(new_column) => {
                        // Limit the number of columns to prevent UI clutter
                        if self.columns.len() >= MAX_COLUMNS_DISPLAY {
                            self.columns.pop_front();
                        }
                        
                        self.columns.push_back(new_column);
                        self.update_preview()?;
                    }
                    Err(_) => {
                        // Directory couldn't be read (permission denied, etc.)
                        // Just ignore and stay in current directory
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Set the current directory as anchor (clear all columns to the left)
    fn set_anchor(&mut self) -> Result<()> {
        if let Some(current_column) = self.columns.back() {
            let path = current_column.path.clone();
            let selection = current_column.selected.selected().unwrap_or(0);
            
            self.columns.clear();
            let new_column = DirColumn::new(path, selection, &self.config)?;
            self.columns.push_back(new_column);
            self.update_preview()?;
        }
        Ok(())
    }
    
    /// Handle search character input
    fn handle_search_char(&mut self, c: char) -> Result<()> {
        let now = Instant::now();
        
        // Reset search string if too much time has passed
        if now.duration_since(self.last_key_time).as_secs() > SEARCH_TIMEOUT_SECONDS {
            self.search_string.clear();
        }
        
        self.search_string.push(c);
        self.last_key_time = now;
        
        // Find matching entry
        if let Some(column) = self.columns.back_mut() {
            for (i, entry) in column.entries.iter().enumerate() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.to_lowercase().starts_with(&self.search_string.to_lowercase()) {
                        column.selected.select(Some(i));
                        self.update_preview()?;
                        break;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Get the currently active column
    fn active_column(&self) -> &DirColumn {
        self.columns.back().expect("At least one column should always exist")
    }
    

    
    /// Update the preview panel
    fn update_preview(&mut self) -> Result<()> {
        self.preview = if let Some(entry) = self.active_column().selected_entry() {
            let path = entry.path();
            
            if path.is_dir() {
                let cached_selection = self.selection_cache.get(&path).copied().unwrap_or(0);
                match DirColumn::new(path, cached_selection, &self.config) {
                    Ok(dir_column) => Some(Preview::Directory(dir_column)),
                    Err(_) => None, // Gracefully handle permission errors
                }
            } else {
                match FileDetails::from_path(&path, &self.config) {
                    Ok(details) => Some(Preview::File(details)),
                    Err(_) => None, // Gracefully handle file read errors
                }
            }
        } else {
            None
        };
        
        Ok(())
    }
    
    /// Open the settings panel
    fn open_settings(&mut self) {
        self.settings = Some(SettingsState::new());
    }
    
    /// Handle settings panel key input
    fn handle_settings_key(&mut self, key: KeyEvent) -> Result<()> {
        // Extract the current state to avoid borrowing conflicts
        let (focus, active_tab, display_selection) = {
            let settings_state = self.settings.as_ref().unwrap();
            (settings_state.focus, settings_state.active_tab, settings_state.display_selection)
        };
        
        match focus {
            SettingsFocus::TabList => match key.code {
                KeyCode::Esc | KeyCode::Char('?') => self.settings = None,
                KeyCode::Up => {
                    let settings_state = self.settings.as_mut().unwrap();
                    settings_state.active_tab = settings_state.active_tab.prev();
                }
                KeyCode::Down => {
                    let settings_state = self.settings.as_mut().unwrap();
                    settings_state.active_tab = settings_state.active_tab.next();
                }
                KeyCode::Right | KeyCode::Enter => {
                    let settings_state = self.settings.as_mut().unwrap();
                    settings_state.focus = SettingsFocus::TabContent;
                }
                _ => {}
            },
            SettingsFocus::TabContent => match active_tab {
                SettingsTab::Display => match key.code {
                    KeyCode::Left | KeyCode::Esc => {
                        let settings_state = self.settings.as_mut().unwrap();
                        settings_state.focus = SettingsFocus::TabList;
                    }
                    KeyCode::Up => {
                        let settings_state = self.settings.as_mut().unwrap();
                        settings_state.display_selection = settings_state.display_selection.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        let settings_state = self.settings.as_mut().unwrap();
                        settings_state.display_selection = (settings_state.display_selection + 1).min(1);
                    }
                    KeyCode::Char(' ') | KeyCode::Enter => {
                        match display_selection {
                            0 => {
                                self.config.show_hidden_files = !self.config.show_hidden_files;
                                // Reload all columns to reflect the change
                                for column in &mut self.columns {
                                    let _ = column.reload(&self.config);
                                }
                                let _ = self.update_preview();
                            }
                            1 => self.config.show_icons = !self.config.show_icons,
                            _ => {}
                        }
                    }
                    _ => {}
                },
                SettingsTab::FileTypes => {
                    let has_add_state = self.settings.as_ref().unwrap().add_file_type_state.is_some();
                    if has_add_state {
                        self.handle_add_file_type_key(key)?;
                    } else {
                        self.handle_file_types_key(key)?;
                    }
                }
                SettingsTab::Keybindings => match key.code {
                    KeyCode::Left | KeyCode::Esc => {
                        let settings_state = self.settings.as_mut().unwrap();
                        settings_state.focus = SettingsFocus::TabList;
                    }
                    _ => {}
                },
            },
            SettingsFocus::AddFileTypePopup => {
                self.handle_add_file_type_key(key)?;
            }
        }
        Ok(())
    }

    /// Handle file types tab key input
    fn handle_file_types_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Left => {
                let settings_state = self.settings.as_mut().unwrap();
                if settings_state.file_type_column_selection > 0 {
                    settings_state.file_type_column_selection -= 1;
                } else {
                    settings_state.focus = SettingsFocus::TabList;
                }
            }
            KeyCode::Right => {
                let settings_state = self.settings.as_mut().unwrap();
                settings_state.file_type_column_selection =
                    (settings_state.file_type_column_selection + 1).min(2);
            }
            KeyCode::Up => {
                let settings_state = self.settings.as_mut().unwrap();
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
                let settings_state = self.settings.as_mut().unwrap();
                settings_state.file_type_selection =
                    (settings_state.file_type_selection + 1).min(max_len);
                settings_state
                    .file_type_table_state
                    .select(Some(settings_state.file_type_selection));
            }
            KeyCode::Char('a') => {
                let settings_state = self.settings.as_mut().unwrap();
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
                self.delete_selected_file_type();
            }
            KeyCode::Char('e') => {
                self.edit_selected_file_type();
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle add/edit file type popup key input
    fn handle_add_file_type_key(&mut self, key: KeyEvent) -> Result<()> {
        let (focused_field, _is_editing) = {
            let settings_state = self.settings.as_ref().unwrap();
            let add_state = settings_state.add_file_type_state.as_ref().unwrap();
            (add_state.focused_field, add_state.is_editing.is_none())
        };

        match key.code {
            KeyCode::Esc => {
                let settings_state = self.settings.as_mut().unwrap();
                settings_state.add_file_type_state = None;
                settings_state.focus = SettingsFocus::TabContent;
            }
            KeyCode::Tab => {
                let settings_state = self.settings.as_mut().unwrap();
                if let Some(add_state) = &mut settings_state.add_file_type_state {
                    add_state.focused_field = (add_state.focused_field + 1) % 3;
                }
            }
            KeyCode::BackTab => {
                let settings_state = self.settings.as_mut().unwrap();
                if let Some(add_state) = &mut settings_state.add_file_type_state {
                    add_state.focused_field = (add_state.focused_field + 2) % 3;
                }
            }
            KeyCode::Char(c) => {
                let settings_state = self.settings.as_mut().unwrap();
                if let Some(add_state) = &mut settings_state.add_file_type_state {
                    match add_state.focused_field {
                        0 => add_state.mime_type.push(c),
                        1 => add_state.icon.push(c),
                        _ => {}
                    }
                }
            }
            KeyCode::Backspace => {
                let settings_state = self.settings.as_mut().unwrap();
                if let Some(add_state) = &mut settings_state.add_file_type_state {
                    match add_state.focused_field {
                        0 if add_state.is_editing.is_none() => {
                            add_state.mime_type.pop();
                        }
                        1 => {
                            add_state.icon.pop();
                        }
                        _ => {}
                    }
                }
            }
            KeyCode::Enter => {
                if focused_field == 2 {
                    let settings_state = self.settings.as_mut().unwrap();
                    if let Some(add_state) = &mut settings_state.add_file_type_state {
                        add_state.preview = !add_state.preview;
                    }
                } else {
                    self.save_file_type_rule()?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Delete the currently selected file type rule
    fn delete_selected_file_type(&mut self) {
        let file_type_selection = self.settings.as_ref().unwrap().file_type_selection;
        
        let mut sorted_exts: Vec<_> = self.config
            .mime_types
            .primary
            .keys()
            .chain(self.config.mime_types.subtypes.keys())
            .cloned()
            .collect();
        sorted_exts.sort();

        if let Some(ext) = sorted_exts.get(file_type_selection) {
            if self.config.mime_types.primary.contains_key(ext) {
                self.config.mime_types.primary.remove(ext);
            } else {
                self.config.mime_types.subtypes.remove(ext);
            }
        }
    }

    /// Edit the currently selected file type rule
    fn edit_selected_file_type(&mut self) {
        let file_type_selection = self.settings.as_ref().unwrap().file_type_selection;
        
        let mut sorted_exts: Vec<_> = self.config
            .mime_types
            .primary
            .keys()
            .chain(self.config.mime_types.subtypes.keys())
            .cloned()
            .collect();
        sorted_exts.sort();

        if let Some(ext) = sorted_exts.get(file_type_selection) {
            let rule = if let Some(rule) = self.config.mime_types.primary.get(ext) {
                rule.clone()
            } else if let Some(rule) = self.config.mime_types.subtypes.get(ext) {
                rule.clone()
            } else {
                return;
            };

            let settings_state = self.settings.as_mut().unwrap();
            settings_state.add_file_type_state = Some(AddFileTypeState {
                mime_type: ext.clone(),
                icon: rule.icon,
                preview: rule.preview,
                focused_field: 0,
                is_editing: Some(ext.clone()),
            });
            settings_state.focus = SettingsFocus::AddFileTypePopup;
        }
    }

    /// Save the file type rule from the add/edit popup
    fn save_file_type_rule(&mut self) -> Result<()> {
        let (mime_type, icon, preview, is_editing) = {
            let settings_state = self.settings.as_ref().unwrap();
            let add_state = settings_state.add_file_type_state.as_ref().unwrap();
            (add_state.mime_type.clone(), add_state.icon.clone(), add_state.preview, add_state.is_editing.clone())
        };

        let rule = crate::config::FileTypeRule {
            icon,
            preview,
        };

        // Remove the old rule if editing
        if let Some(original_key) = &is_editing {
            if original_key.contains('/') {
                self.config.mime_types.subtypes.remove(original_key);
            } else {
                self.config.mime_types.primary.remove(original_key);
            }
        }

        // Add the new rule
        if mime_type.contains('/') {
            self.config.mime_types.subtypes.insert(mime_type, rule);
        } else {
            self.config.mime_types.primary.insert(mime_type, rule);
        }

        // Close the popup
        let settings_state = self.settings.as_mut().unwrap();
        settings_state.add_file_type_state = None;
        settings_state.focus = SettingsFocus::TabContent;

        Ok(())
    }


    
    // Getter methods for UI rendering
    pub fn columns(&self) -> &VecDeque<DirColumn> {
        &self.columns
    }
    
    pub fn preview(&self) -> &Option<Preview> {
        &self.preview
    }
    
    pub fn settings(&self) -> &Option<SettingsState> {
        &self.settings
    }
    
    pub fn search_string(&self) -> &str {
        &self.search_string
    }
} 