use crate::app::{App, Preview};
use crate::config::{Settings, SEARCH_TIMEOUT_SECONDS, MAX_COLUMNS_DISPLAY};
use crate::settings::render_settings_panel;
use crate::utils::{truncate_text};
use crate::file_operations::{get_icon_with_error_log, read_directory_with_error_log, is_safe_path, FileDetails};
use crate::file_preview::render_file_preview;
use crate::error::ErrorLog;
use color_eyre::Result;
use std::collections::{HashMap, VecDeque};
use std::fs::DirEntry;
use std::io;
use std::path::PathBuf;
use std::time::Instant;
use std::cmp;

use ratatui::{
    prelude::*,
    widgets::*,
};


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
        Self::new_with_error_log(path, initial_selection, config, None)
    }

    pub fn scroll(&mut self, direction: ScrollDirection, view_height: usize) {
        // TODO: ListState might not let us scroll such that the selected item becomes invisible.
        // Not sure how that's working, but it leads to some unexpected scrolling behavior.
        match direction {
            ScrollDirection::Backward => *self.selected.offset_mut() = cmp::max(0, self.selected.offset().saturating_sub(1)),
            ScrollDirection::Forward => *self.selected.offset_mut() = cmp::min(self.entries.len() - view_height, self.selected.offset().saturating_add(1)),
        }
    }

    /// Create a new directory column with error logging
    pub fn new_with_error_log(path: PathBuf, initial_selection: usize, config: &Settings, error_log: Option<&mut ErrorLog>) -> io::Result<Self> {
        if !is_safe_path(&path) {
            let error_msg = format!("Path not allowed for security reasons: {}", path.display());
            if let Some(log) = error_log {
                log.error(error_msg.clone(), Some("Security Check".to_string()));
            }
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                error_msg,
            ));
        }

        let entries = read_directory_with_error_log(&path, config, error_log)?;
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
        self.reload_with_error_log(config, None)
    }

    /// Reload the directory contents with error logging
    pub fn reload_with_error_log(&mut self, config: &Settings, error_log: Option<&mut ErrorLog>) -> io::Result<()> {
        self.entries = read_directory_with_error_log(&self.path, config, error_log)?;

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

/// Browser state managing columns, preview, and navigation
#[derive(Debug)]
pub struct Browser {
    columns: VecDeque<DirColumn>,
    preview: Option<Preview>,
    selection_cache: HashMap<PathBuf, usize>,
    search_string: String,
    last_key_time: Instant,
}

impl Browser {

    /// Create a new browser with error logging
    pub fn new_with_error_log(initial_dir: PathBuf, config: &Settings, error_log: Option<&mut ErrorLog>) -> Result<Self> {
        let initial_column = DirColumn::new_with_error_log(initial_dir, 0, config, error_log)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to read initial directory: {}", e))?;

        let mut columns = VecDeque::new();
        columns.push_back(initial_column);

        let mut browser = Self {
            columns,
            preview: None,
            selection_cache: HashMap::new(),
            search_string: String::new(),
            last_key_time: Instant::now(),
        };

        _ = browser.update_preview(config);
        Ok(browser)
    }

    /// Get reference to columns
    pub fn columns(&self) -> &VecDeque<DirColumn> {
        &self.columns
    }

    /// Get mutable reference to columns
    pub fn columns_mut(&mut self) -> &mut VecDeque<DirColumn> {
        &mut self.columns
    }

    /// Get reference to preview
    pub fn preview(&self) -> &Option<Preview> {
        &self.preview
    }

    /// Get the search string
    pub fn search_string(&self) -> &str {
        &self.search_string
    }

    /// Get the currently active column
    pub fn active_column(&self) -> &DirColumn {
        self.columns.back().expect("At least one column should always exist")
    }

    pub fn activate_column(&mut self, index: usize, config: &Settings) -> Result<(),()> {

        if index > self.columns.len() {
            return Err(());
        }

        if index == self.columns.len() {
            return self.navigate_right(config);
        }

        while (index + 1) < self.columns.len() {
            if let Err(x) = self.navigate_left(config) {
                return Err(x);
            }
        }

        Ok(())
    }

    /// Navigate left (parent directory)
    pub fn navigate_left(&mut self, config: &Settings) -> Result<(), ()> {
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
                let initial_selection = if let Some(name) = current_name
                   && let Ok(parent_column) = DirColumn::new(parent_path.clone(), 0, config)
                    {
                    parent_column.entries.iter().position(|entry| entry.file_name() == name).unwrap_or(0)
                } else {
                    0
                };

                let parent_column = DirColumn::new(parent_path, initial_selection, config);
                self.columns.clear();
                match parent_column {
                    Ok(column) => self.columns.push_back(column),
                    Err(_) => {
                        return Err(());
                    }
                }
            }
        }

        return self.update_preview(config);
    }

    /// Navigate right (enter directory)
    pub fn navigate_right(&mut self, config: &Settings) -> Result<(), ()> {
        if let Some(entry) = self.active_column().selected_entry() {
            let path = entry.path();

            if path.is_dir() {
                // Cache current selection
                if let Some(selected_idx) = self.active_column().selected.selected() {
                    self.selection_cache.insert(self.active_column().path.clone(), selected_idx);
                }

                let cached_selection = self.selection_cache.get(&path).copied().unwrap_or(0);

                // Try to create new column, but don't fail the whole operation if it fails
                match DirColumn::new(path, cached_selection, config) {
                    Ok(new_column) => {
                        // Limit the number of columns displayed
                        if self.columns.len() >= MAX_COLUMNS_DISPLAY {
                            self.columns.pop_front();
                        }

                        self.columns.push_back(new_column);
                        return self.update_preview(config);
                    }
                    Err(_) => {
                        // Directory might be inaccessible, just update preview
                        return self.update_preview(config);
                    }
                }
            }
        }
        Ok(())
    }

    /// Set the current directory as anchor (clear all columns to the left)
    pub fn set_anchor(&mut self, config: &Settings) -> Result<()> {
        if let Some(current_column) = self.columns.back() {
            let path = current_column.path.clone();
            let selection = current_column.selected.selected().unwrap_or(0);

            self.columns.clear();
            let new_column = DirColumn::new(path, selection, config)?;
            self.columns.push_back(new_column);
            _ = self.update_preview(config);
        }
        Ok(())
    }

    /// Handle search character input
    pub fn handle_search_char(&mut self, c: char) -> Result<()> {
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
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.starts_with(&self.search_string.to_lowercase()) {
                    column.selected.select(Some(i));
                    break;
                }
            }
        }

        Ok(())
    }

    /// Navigate to previous item in current column
    pub fn select_previous(&mut self) {
        if let Some(column) = self.columns.back_mut() {
            column.select_previous();
        }
    }

    /// Navigate to next item in current column
    pub fn select_next(&mut self) {
        if let Some(column) = self.columns.back_mut() {
            column.select_next();
        }
    }

    /// Update the preview panel
    pub fn update_preview(&mut self, config: &Settings) -> Result<(),()> {
        self.preview = if let Some(entry) = self.active_column().selected_entry() {
            let path = entry.path();

            if path.is_dir() {
                let cached_selection = self.selection_cache.get(&path).copied().unwrap_or(0);
                match DirColumn::new(path, cached_selection, config) {
                    Ok(preview_column) => Some(Preview::Directory(preview_column)),
                    Err(_) => None,
                }
            } else {
                match FileDetails::from_path(&path, config) {
                    Ok(details) => Some(Preview::File(details)),
                    Err(_) => None,
                }
            }
        } else {
            None
        };

        Ok(())
    }

    /// Clear the search string
    pub fn clear_search(&mut self) {
        self.search_string.clear();
    }

    /// Reload all columns
    pub fn reload_all_columns(&mut self, config: &Settings) -> Result<()> {
        for column in &mut self.columns {
            let _ = column.reload(config);
        }
        _ = self.update_preview(config);
        Ok(())
    }

    /// Jump to first item in current column
    pub fn jump_to_first(&mut self, config: &Settings) -> Result<()> {
        if let Some(column) = self.columns.back_mut() {
            if !column.entries.is_empty() {
                column.selected.select(Some(0));
                _ = self.update_preview(config);
            }
        }
        Ok(())
    }

    /// Jump to last item in current column
    pub fn jump_to_last(&mut self, config: &Settings) -> Result<()> {
        if let Some(column) = self.columns.back_mut() {
            if !column.entries.is_empty() {
                column.selected.select(Some(column.entries.len() - 1));
                _ = self.update_preview(config);
            }
        }
        Ok(())
    }

    /// Jump up by 10 items in current column
    pub fn jump_up_by_10(&mut self, config: &Settings) -> Result<()> {
        if let Some(column) = self.columns.back_mut() {
            if let Some(current) = column.selected.selected() {
                let new_index = current.saturating_sub(10);
                column.selected.select(Some(new_index));
                _ = self.update_preview(config);
            }
        }
        Ok(())
    }

    /// Jump down by 10 items in current column
    pub fn jump_down_by_10(&mut self, config: &Settings) -> Result<()> {
        if let Some(column) = self.columns.back_mut() {
            if let Some(current) = column.selected.selected() {
                let new_index = (current + 10).min(column.entries.len().saturating_sub(1));
                column.selected.select(Some(new_index));
                _ = self.update_preview(config);
            }
        }
        Ok(())
    }
}

const BORDER_AND_PADDING_WIDTH: u16 = 4; // 2 for borders + 2 for padding
const ICON_SPACE_WIDTH: usize = 3; // icon + space + buffer

/// Calculate available width for content within a bordered area
pub fn content_width(area: Rect) -> usize {
    area.width.saturating_sub(BORDER_AND_PADDING_WIDTH) as usize
}

/// Calculate available width for filenames, accounting for icons
fn filename_width(area: Rect, show_icons: bool) -> usize {
    let width = content_width(area);
    if show_icons {
        width.saturating_sub(ICON_SPACE_WIDTH)
    } else {
        width
    }
}

/// Render the main content area (columns and preview)
pub fn render_browser(frame: &mut Frame, app: &mut App, area: Rect) {
    let browser = app.browser();
    let num_cols = browser.columns().len() + if browser.preview().is_some() { 1 } else { 0 };
    let constraints = (0..num_cols)
        .map(|_| Constraint::Ratio(1, num_cols as u32))
        .collect::<Vec<_>>();
    let layout = Layout::horizontal(constraints).split(area);

    // Render columns
    let active_column_index = browser.columns().len() - 1;
    for (i, column) in browser.columns().iter().enumerate() {
        let is_active = i == active_column_index;
        render_dir_column(frame, column, layout[i], is_active, false, app.config());
    }

    // Render preview
    if let Some(preview) = browser.preview() {
        let preview_area = layout[browser.columns().len()];
        match preview {
            Preview::Directory(dir_column) => {
                render_dir_column(frame, dir_column, preview_area, false, true, app.config());
            }
            Preview::File(details) => {
                render_file_preview(frame, details, preview_area);
            }
        }
    }

    // Render settings panel if open
    if app.settings().is_some() {
        render_settings_panel(frame, app);
    }
}

/// Render a directory column
fn render_dir_column(
    frame: &mut Frame,
    column: &DirColumn,
    area: Rect,
    is_active: bool,
    _is_preview: bool,
    config: &Settings,
) {
    use crate::utils::get_path_info;
    use ratatui::layout::{Constraint, Layout, Direction};
    use ratatui::widgets::{Paragraph, Wrap};
    use ratatui::style::{Color, Style, Modifier};

    let title = column
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let truncated_title = truncate_text(&title, content_width(area));

    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    // Split the area: main list + info footer (2 lines)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Main list area
            Constraint::Length(2), // Info footer
        ])
        .split(area);

    let max_filename_width = filename_width(chunks[0], config.show_icons);

    let items: Vec<ListItem> = column
        .entries
        .iter()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let truncated_name = truncate_text(&name, max_filename_width);
            let icon = get_icon_with_error_log(entry, config, None);
            let display_text = if icon.is_empty() {
                truncated_name
            } else {
                format!("{} {}", icon, truncated_name)
            };
            ListItem::new(display_text)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .title(truncated_title)
                .border_style(border_style)
                .padding(Padding::uniform(1)),
        )
        .highlight_style(
            Style::default()
                .add_modifier(if is_active { Modifier::REVERSED } else { Modifier::DIM })
        );

    // Create a mutable state for rendering
    let mut list_state = column.selected.clone();
    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    // Render directory info at the bottom
    let entry_count = column.entries.len();
    let info_text = if let Some((permissions, date)) = get_path_info(&column.path) {
        format!("{} {} ({} items)", permissions, date, entry_count)
    } else {
        format!("--------- ???? ({} items)", entry_count)
    };

    let info_paragraph = Paragraph::new(info_text)
        .block(
            Block::default()
                .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
                .border_style(border_style)
                .padding(Padding::horizontal(1)),
        )
        .style(Style::default().fg(Color::DarkGray))
        .wrap(Wrap { trim: true });

    frame.render_widget(info_paragraph, chunks[1]);
}
