use crate::browser::{DirColumn, Browser};
use crate::commands::{CommandRegistry, CommandAction};
use crate::config::{Settings, load_settings};
use crate::error::ErrorLog;
use crate::file_operations::{FileDetails};
use crate::tabs::TabManager;
use crate::ui::render_ui;
use crate::settings::{SettingsManager, SettingsState};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Frame;


/// Preview content for the right panel
#[derive(Debug)]
pub enum Preview {
    Directory(DirColumn),
    File(FileDetails),
}

/// Main application state
pub struct App {
    tab_manager: TabManager,
    settings_manager: SettingsManager,
    error_log: ErrorLog,
    config: Settings,
    should_quit: bool,
    command_registry: CommandRegistry,
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
                active_tab.browser.update_preview(&self.config)?;
            }
            CommandAction::NavigateDown => {
                let active_tab = self.tab_manager.active_tab_mut();
                active_tab.browser.select_next();
                active_tab.browser.update_preview(&self.config)?;
            }
            CommandAction::NavigateLeft => {
                let active_tab = self.tab_manager.active_tab_mut();
                active_tab.browser.navigate_left(&self.config)?;
                self.tab_manager.update_active_tab_name();
            }
            CommandAction::NavigateRight => {
                let active_tab = self.tab_manager.active_tab_mut();
                active_tab.browser.navigate_right(&self.config)?;
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


}
