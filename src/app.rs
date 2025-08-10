use crate::browser::{DirColumn, Browser};
use crate::commands::{CommandRegistry, CommandAction};
use crate::config::{Settings, load_settings};
use crate::error::ErrorLog;
use crate::file_operations::{FileDetails};
use crate::tabs::TabManager;
use crate::ui::render_ui;
use crate::settings::{SettingsManager, SettingsState};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use ratatui::widgets::ScrollDirection;
use ratatui::{Frame, prelude::Rect};


/// Preview content for the right panel
#[derive(Debug)]
pub enum Preview {
    Directory(DirColumn),
    File(FileDetails),
}

/// UI layout tracking for mouse interactions
#[derive(Debug, Default)]
pub struct LayoutInfo {
    pub column_areas: Vec<Rect>,
    pub browser_area: Rect,
    pub tab_area: Rect,
    pub status_area: Rect,
}

/// Main application state
pub struct App {
    tab_manager: TabManager,
    settings_manager: SettingsManager,
    error_log: ErrorLog,
    config: Settings,
    should_quit: bool,
    command_registry: CommandRegistry,
    layout_info: LayoutInfo,
}

impl App {
    /// Create a new application instance
    pub fn new() -> Result<Self> {
        let current_dir = std::env::current_dir()
            .map_err(|e| color_eyre::eyre::eyre!("Failed to get current directory: {}", e))?;

        let config = load_settings()
            .map_err(|e| color_eyre::eyre::eyre!("Failed to load settings: {}", e))?;

        let mut error_log = ErrorLog::new();
        let tab_manager = TabManager::new(current_dir, &config, Some(&mut error_log))?;

        let app = Self {
            tab_manager,
            settings_manager: SettingsManager::new(),
            error_log,
            config,
            should_quit: false,
            command_registry: CommandRegistry::new(),
            layout_info: LayoutInfo::default(),
        };

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

    /// Get reference to the command registry
    pub fn command_registry(&self) -> &CommandRegistry {
        &self.command_registry
    }

    /// Render the application UI and return layout info for mouse handling
    pub fn render(&mut self, frame: &mut Frame) -> LayoutInfo {
        render_ui(frame, self)
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        // Handle settings panel if open
        if self.settings_manager.is_open() {
            let needs_reload = self.settings_manager.handle_key(key, &mut self.config)?;
            if needs_reload {
                self.tab_manager.reload_all_tabs(&self.config);
            }
            return Ok(());
        }

        // Handle error log navigation if visible
        if self.error_log.is_visible() {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.error_log.hide();
                    return Ok(());
                }
                KeyCode::Up => {
                    self.error_log.select_previous();
                    return Ok(());
                }
                KeyCode::Down => {
                    self.error_log.select_next();
                    return Ok(());
                }
                KeyCode::Home => {
                    self.error_log.select_first();
                    return Ok(());
                }
                KeyCode::End => {
                    self.error_log.select_last();
                    return Ok(());
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.error_log.clear();
                    return Ok(());
                }
                KeyCode::Enter => {
                    self.error_log.toggle_selected_wrap();
                    return Ok(());
                }
                _ => {}
            }
        }

        // Find matching command
        if let Some(command) = self.command_registry.find_command(&key) {
            let action = command.action.clone();
            return self.execute_command(&action, key);
        }

        Ok(())
    }

    /// Execute a command action
    fn execute_command(&mut self, action: &CommandAction, key: KeyEvent) -> Result<()> {
        match action {
            CommandAction::Quit => {
                self.should_quit = true;
            }
            CommandAction::ShowSettings => {
                self.settings_manager.open();
            }
            CommandAction::ShowErrorLog => {
                self.error_log.toggle_visibility();
            }
            CommandAction::NewTab => {
                self.tab_manager.create_tab(&self.config, Some(&mut self.error_log))?;
            }
            CommandAction::CloseTab => {
                if !self.tab_manager.close_current_tab() {
                    // Cannot close the last tab - add a message to error log
                    self.error_log.warning("Cannot close the last tab".to_string(), None);
                }
            }
            CommandAction::NextTab => {
                self.tab_manager.next_tab();
            }
            CommandAction::PrevTab => {
                self.tab_manager.prev_tab();
            }
            CommandAction::ClearSearch => {
                self.tab_manager.active_tab_mut().browser.clear_search();
            }
            CommandAction::NavigateUp => {
                let active_tab = self.tab_manager.active_tab_mut();
                active_tab.browser.select_previous();
                _ = active_tab.browser.update_preview(&self.config);
            }
            CommandAction::NavigateDown => {
                let active_tab = self.tab_manager.active_tab_mut();
                active_tab.browser.select_next();
                _ = active_tab.browser.update_preview(&self.config);
            }
            CommandAction::NavigateLeft => {
                let active_tab = self.tab_manager.active_tab_mut();
                _ = active_tab.browser.navigate_left(&self.config);
                self.tab_manager.update_active_tab_name();
            }
            CommandAction::NavigateRight => {
                let active_tab = self.tab_manager.active_tab_mut();
                _ = active_tab.browser.navigate_right(&self.config);
                self.tab_manager.update_active_tab_name();
            }
            CommandAction::SetAnchor => {
                self.tab_manager.active_tab_mut().browser.set_anchor(&self.config)?;
            }
            CommandAction::JumpToFirst => {
                self.tab_manager.active_tab_mut().browser.jump_to_first(&self.config)?;
            }
            CommandAction::JumpToLast => {
                self.tab_manager.active_tab_mut().browser.jump_to_last(&self.config)?;
            }
            CommandAction::JumpUpBy10 => {
                self.tab_manager.active_tab_mut().browser.jump_up_by_10(&self.config)?;
            }
            CommandAction::JumpDownBy10 => {
                self.tab_manager.active_tab_mut().browser.jump_down_by_10(&self.config)?;
            }
            CommandAction::SearchChar => {
                if let KeyCode::Char(c) = key.code {
                    self.tab_manager.active_tab_mut().browser.handle_search_char(c)?;
                }
            }
        }
        Ok(())
    }

    /// Handle mouse input
    ///
    /// Supports:
    /// - Mouse wheel scrolling: Scrolls the view in whichever column the mouse is over without changing selection
    /// - Left mouse clicks: Selects items and navigates between columns
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        // Debug logging (can be enabled for troubleshooting)
        // Only handle mouse events if settings panel is not open
        if self.settings_manager.is_open() {
            return Ok(());
        }

        // Only handle mouse events if error log is not visible or not focused
            // For now, let mouse events pass through when error log is visible
            // Could add specific mouse handling for error log here

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.handle_mouse_scroll(&mouse)?;
            }
            MouseEventKind::ScrollDown => {
                self.handle_mouse_scroll(&mouse)?;
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Handle left click - this will be used for row selection
                self.handle_mouse_click(mouse.column, mouse.row)?;
            }
            _ => {
                // Ignore other mouse events for now
            }
        }

        Ok(())
    }

    fn handle_mouse_scroll(&mut self, mouse: &MouseEvent) -> Result<()> {
        // Find which column the mouse is over and scroll that specific column
        if let Some(column_index) = self.get_column_under_mouse(mouse.column, mouse.row) {
            let scroll_direction = match mouse.kind {
                MouseEventKind::ScrollDown => ScrollDirection::Forward,
                MouseEventKind::ScrollUp => ScrollDirection::Backward,
                _ => return Ok(())
            };
            let area = self.layout_info.column_areas[column_index];
            let active_tab = self.tab_manager.active_tab_mut();
            let browser_columns_len = active_tab.browser.columns().len();

            // Don't try to scroll preview columns, only actual directory columns
            if column_index < browser_columns_len {
                if let Some(column) = active_tab.browser.columns_mut().get_mut(column_index) {
                    column.scroll(scroll_direction, usize::from(area.height));
                    if std::env::var("BROWSE_DEBUG_MOUSE").is_ok() {
                        let message = format!("Scrolled column {} down {}", column_index, column.selected.offset());
                        self.error_log.info(message, Some("Mouse Event".to_string()));
                    }
                }
            }
        }
        Ok(())
    }

    /// Handle mouse clicks for row selection
    fn handle_mouse_click(&mut self, column: u16, row: u16) -> Result<()> {
        // Check if click is within any column area
        for (col_index, col_area) in self.layout_info.column_areas.iter().enumerate() {
            if column >= col_area.x && column < col_area.x + col_area.width
                && row >= col_area.y && row < col_area.y + col_area.height
            {
                let active_tab = self.tab_manager.active_tab_mut();
                let browser = &mut active_tab.browser;

                let message = format!("Clicked column {}", col_index);
                self.error_log.info(message, Some("Mouse Event".to_string()));

                // Calculate which row was clicked within this column
                // Account for column borders and title
                let content_start_row = col_area.y + 2; // Title + top border + padding
                if row >= content_start_row && row < col_area.y + col_area.height - 3 {
                    let clicked_row_in_view = (row - content_start_row) as usize;

                    if let Ok(_) = browser.activate_column(col_index, &self.config) {

                        // Get the actual item index by adding the scroll offset
                        // This is crucial: clicked_row_in_view is the visual row (0-based from top of visible area)
                        // but we need to account for how far the column has been scrolled down
                        if let Some(target_column) = browser.columns().get(col_index) {
                            let scroll_offset = target_column.selected.offset();
                            let actual_item_index = clicked_row_in_view + scroll_offset;

                            // Now select the clicked row in the target column
                            if let Some(column_to_update) = browser.columns_mut().get_mut(col_index) {
                                if actual_item_index < column_to_update.entries.len() {
                                    column_to_update.selected.select(Some(actual_item_index));
                                }
                            }
                        }
                    } else {
                        self.error_log.error("Failed to activate column".to_string(), Some("Mouse Event".to_string()));
                        return Ok(());
                    }

                    _ = browser.update_preview(&self.config);
                }
                break;
            }
        }

        Ok(())
    }

    /// Determine which column the mouse cursor is over
    ///
    /// This function iterates through all column areas (stored during UI rendering)
    /// and checks if the mouse coordinates fall within any column's bounds.
    /// This enables precise column-aware scrolling - when you scroll over a specific
    /// column, only that column scrolls, not just the active (rightmost) column.
    fn get_column_under_mouse(&mut self, mouse_column: u16, mouse_row: u16) -> Option<usize> {
        // Ensure we have layout info and are within the browser area
        if self.layout_info.column_areas.is_empty() {
            return None;
        }

        // Check if mouse is within the overall browser area first
        let browser_area = &self.layout_info.browser_area;
        if mouse_row < browser_area.y || mouse_row >= browser_area.y + browser_area.height {
            return None;
        }

        // Find which column the mouse is over
        for (index, area) in self.layout_info.column_areas.iter().enumerate() {
            if mouse_column >= area.x && mouse_column < area.x + area.width &&
               mouse_row >= area.y && mouse_row < area.y + area.height {
                // Additional check: make sure this column actually exists in the browser
                if index < self.tab_manager.active_tab().browser.columns().len() ||
                   (index == self.tab_manager.active_tab().browser.columns().len() &&
                    self.tab_manager.active_tab().browser.preview().is_some()) {
                    if std::env::var("BROWSE_DEBUG_MOUSE").is_ok() {
                        let message = format!("Found column {} at mouse position ({}, {})", index, mouse_column, mouse_row);
                        self.error_log.info(message, Some("Mouse Event".to_string()));
                    }
                    return Some(index);
                }
            }
        }

        if std::env::var("BROWSE_DEBUG_MOUSE").is_ok() {
            let message = format!("No column found at mouse position ({}, {})", mouse_column, mouse_row);
            self.error_log.info(message, Some("Mouse Event".to_string()));
        }
        None
    }

    // Getter methods for UI rendering
    pub fn browser(&self) -> &Browser {
        &self.tab_manager.active_tab().browser
    }

    pub fn tab_manager(&self) -> &TabManager {
        &self.tab_manager
    }

    pub fn settings(&self) -> &Option<SettingsState> {
        self.settings_manager.state()
    }

    pub fn error_log(&self) -> &ErrorLog {
        &self.error_log
    }

    /// Update layout info for mouse interaction
    pub fn set_layout_info(&mut self, layout_info: LayoutInfo) {
        self.layout_info = layout_info;
    }

}
