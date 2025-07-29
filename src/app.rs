use crate::browser::{DirColumn, Browser};
use crate::commands::{CommandRegistry, CommandAction};
use crate::config::{Settings, load_settings};
use crate::file_operations::{FileDetails};
use crate::ui::render_ui;
use crate::settings::{SettingsManager, SettingsState};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::Frame;


/// Preview content for the right panel
#[derive(Debug)]
pub enum Preview {
    Directory(DirColumn),
    File(FileDetails),
}

/// Main application state
pub struct App {
    browser: Browser,
    settings_manager: SettingsManager,
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

        let browser = Browser::new(current_dir, &config)?;

        Ok(Self {
            browser,
            settings_manager: SettingsManager::new(),
            config,
            should_quit: false,
            command_registry: CommandRegistry::new(),
        })
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
                let _ = self.browser.reload_all_columns(&self.config);
            }
            return Ok(());
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
            CommandAction::ClearSearch => {
                self.browser.clear_search();
            }
            CommandAction::NavigateUp => {
                self.browser.select_previous();
                self.browser.update_preview(&self.config)?;
            }
            CommandAction::NavigateDown => {
                self.browser.select_next();
                self.browser.update_preview(&self.config)?;
            }
            CommandAction::NavigateLeft => self.browser.navigate_left(&self.config)?,
            CommandAction::NavigateRight => self.browser.navigate_right(&self.config)?,
            CommandAction::SetAnchor => self.browser.set_anchor(&self.config)?,
            CommandAction::JumpToFirst => {
                self.browser.jump_to_first(&self.config)?;
            }
            CommandAction::JumpToLast => {
                self.browser.jump_to_last(&self.config)?;
            }
            CommandAction::JumpUpBy10 => {
                self.browser.jump_up_by_10(&self.config)?;
            }
            CommandAction::JumpDownBy10 => {
                self.browser.jump_down_by_10(&self.config)?;
            }
            CommandAction::SearchChar => {
                if let KeyCode::Char(c) = key.code {
                    self.browser.handle_search_char(c)?;
                }
            }
        }
        Ok(())
    }



    // Getter methods for UI rendering
    pub fn browser(&self) -> &Browser {
        &self.browser
    }

    pub fn settings(&self) -> &Option<SettingsState> {
        self.settings_manager.state()
    }
}
