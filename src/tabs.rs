use crate::browser::Browser;
use crate::config::Settings;
use crate::error::ErrorLog;
use color_eyre::Result;
use std::path::PathBuf;

/// Represents a single tab containing a browser instance
#[derive(Debug)]
pub struct Tab {
    /// The browser instance for this tab
    pub browser: Browser,
    /// Display name for the tab (usually the directory name)
    pub name: String,
    /// The root path this tab was created with
    pub root_path: PathBuf,
}

impl Tab {
    /// Create a new tab with a browser instance
    pub fn new(path: PathBuf, config: &Settings, error_log: Option<&mut ErrorLog>) -> Result<Self> {
        let browser = Browser::new_with_error_log(path.clone(), config, error_log)?;

        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("~")
            .to_string();

        Ok(Self {
            browser,
            name,
            root_path: path,
        })
    }

    /// Get the current directory name for display
    pub fn display_name(&self) -> &str {
        &self.name
    }

    /// Update the tab name based on current directory
    pub fn update_name(&mut self) {
        if let Some(current_col) = self.browser.columns().back() {
            let new_name = current_col.path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("~")
                .to_string();
            self.name = new_name;
        }
    }
}

/// Manages a collection of tabs
pub struct TabManager {
    /// Collection of all tabs
    tabs: Vec<Tab>,
    /// Index of the currently active tab
    active_index: usize,
}

impl TabManager {
    /// Create a new tab manager with an initial tab
    pub fn new(initial_path: PathBuf, config: &Settings, error_log: Option<&mut ErrorLog>) -> Result<Self> {
        let initial_tab = Tab::new(initial_path, config, error_log)?;

        Ok(Self {
            tabs: vec![initial_tab],
            active_index: 0,
        })
    }

    /// Get the currently active tab
    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active_index]
    }

    /// Get mutable reference to the currently active tab
    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_index]
    }

    /// Get all tabs for rendering
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Get the active tab index
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Create a new tab
    pub fn create_tab(&mut self, config: &Settings, error_log: Option<&mut ErrorLog>) -> Result<()> {
        // Use the current directory of the active tab as the starting point for the new tab
        let current_path = if let Some(current_col) = self.active_tab().browser.columns().back() {
            current_col.path.clone()
        } else {
            std::env::current_dir()?
        };

        let new_tab = Tab::new(current_path, config, error_log)?;
        self.tabs.push(new_tab);
        self.active_index = self.tabs.len() - 1;

        Ok(())
    }

    /// Close the current tab
    pub fn close_current_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            // Don't close the last tab
            return false;
        }

        self.tabs.remove(self.active_index);

        // Adjust active index if necessary
        if self.active_index >= self.tabs.len() {
            self.active_index = self.tabs.len() - 1;
        }

        true
    }

    /// Navigate to the next tab (with wrapping)
    pub fn next_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.active_index = (self.active_index + 1) % self.tabs.len();
        }
    }

    /// Navigate to the previous tab (with wrapping)
    pub fn prev_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.active_index = if self.active_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_index - 1
            };
        }
    }

    /// Get the number of tabs
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Reload all columns in all tabs
    pub fn reload_all_tabs(&mut self, config: &Settings) {
        for tab in &mut self.tabs {
            let _ = tab.browser.reload_all_columns(config);
        }
    }

    /// Update the name of the active tab based on current directory
    pub fn update_active_tab_name(&mut self) {
        self.active_tab_mut().update_name();
    }
}
