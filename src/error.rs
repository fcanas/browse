use chrono::{DateTime, Local};
use std::collections::VecDeque;
use ratatui::{
    prelude::*,
    widgets::*,
};
use crate::utils::truncate_text;

/// Maximum number of error entries to keep in memory
const MAX_ERROR_ENTRIES: usize = 1000;

/// Represents a single error entry in the log
#[derive(Debug, Clone)]
pub struct ErrorEntry {
    pub timestamp: DateTime<Local>,
    pub message: String,
    pub context: Option<String>,
    pub severity: ErrorSeverity,
}

/// Error severity levels
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
}

impl ErrorSeverity {
    pub fn display_prefix(&self) -> &'static str {
        match self {
            ErrorSeverity::Info => "ℹ️",
            ErrorSeverity::Warning => "⚠️",
            ErrorSeverity::Error => "❌",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ErrorSeverity::Info => "INFO",
            ErrorSeverity::Warning => "WARN",
            ErrorSeverity::Error => "ERROR",
        }
    }
}

impl ErrorEntry {
    pub fn new(message: String, context: Option<String>, severity: ErrorSeverity) -> Self {
        Self {
            timestamp: Local::now(),
            message,
            context,
            severity,
        }
    }

    pub fn error(message: String, context: Option<String>) -> Self {
        Self::new(message, context, ErrorSeverity::Error)
    }

    pub fn warning(message: String, context: Option<String>) -> Self {
        Self::new(message, context, ErrorSeverity::Warning)
    }

    pub fn info(message: String, context: Option<String>) -> Self {
        Self::new(message, context, ErrorSeverity::Info)
    }

    /// Format the error entry for display
    pub fn format_for_display(&self) -> String {
        let timestamp = self.timestamp.format("%H:%M:%S");
        let context_str = self.context
            .as_ref()
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();

        format!("{} {} {}{}: {}",
            self.severity.display_prefix(),
            timestamp,
            self.severity.display_name(),
            context_str,
            self.message
        )
    }
}

/// Error log manager
#[derive(Debug)]
pub struct ErrorLog {
    entries: VecDeque<ErrorEntry>,
    unread_count: usize,
    selected_index: usize,
    is_visible: bool,
    expanded_entries: std::collections::HashSet<usize>,
}

impl ErrorLog {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            unread_count: 0,
            selected_index: 0,
            is_visible: false,
            expanded_entries: std::collections::HashSet::new(),
        }
    }

    /// Add an error entry to the log
    pub fn add_entry(&mut self, entry: ErrorEntry) {
        if self.entries.len() >= MAX_ERROR_ENTRIES {
            self.entries.pop_front();
        }

        self.entries.push_back(entry);
        self.unread_count += 1;
    }

    /// Add an error message
    pub fn error(&mut self, message: String, context: Option<String>) {
        self.add_entry(ErrorEntry::error(message, context));
    }

    /// Add a warning message
    pub fn warning(&mut self, message: String, context: Option<String>) {
        self.add_entry(ErrorEntry::warning(message, context));
    }

    #[allow(dead_code)]
    pub fn info(&mut self, message: String, context: Option<String>) {
        self.add_entry(ErrorEntry::info(message, context));
    }

    /// Get all error entries
    pub fn entries(&self) -> &VecDeque<ErrorEntry> {
        &self.entries
    }

    /// Get the number of unread error entries
    pub fn unread_count(&self) -> usize {
        self.unread_count
    }

    /// Check if the error log panel is visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Toggle the visibility of the error log panel
    pub fn toggle_visibility(&mut self) {
        self.is_visible = !self.is_visible;
        if self.is_visible {
            // Mark all entries as read when panel becomes visible
            self.unread_count = 0;
            // Reset selection to the most recent entry
            if !self.entries.is_empty() {
                self.selected_index = self.entries.len() - 1;
            }
        }
    }

    /// Hide the error log panel
    pub fn hide(&mut self) {
        self.is_visible = false;
    }

    /// Get the currently selected entry index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Move selection up in the error log
    pub fn select_previous(&mut self) {
        if !self.entries.is_empty() && self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down in the error log
    pub fn select_next(&mut self) {
        if !self.entries.is_empty() && self.selected_index < self.entries.len() - 1 {
            self.selected_index += 1;
        }
    }

    /// Jump to the first entry
    pub fn select_first(&mut self) {
        if !self.entries.is_empty() {
            self.selected_index = 0;
        }
    }

    /// Jump to the last entry
    pub fn select_last(&mut self) {
        if !self.entries.is_empty() {
            self.selected_index = self.entries.len() - 1;
        }
    }

    /// Clear all error entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.unread_count = 0;
        self.selected_index = 0;
    }

    /// Check if there are any errors (as opposed to just warnings/info)
    pub fn has_errors(&self) -> bool {
        self.entries.iter().any(|entry| entry.severity == ErrorSeverity::Error)
    }

    /// Toggle line-wrapping for the currently selected entry
    pub fn toggle_selected_wrap(&mut self) {
        if !self.entries.is_empty() {
            if self.expanded_entries.contains(&self.selected_index) {
                self.expanded_entries.remove(&self.selected_index);
            } else {
                self.expanded_entries.insert(self.selected_index);
            }
        }
    }

    /// Check if an entry is expanded (line-wrapped)
    pub fn is_entry_expanded(&self, index: usize) -> bool {
        self.expanded_entries.contains(&index)
    }
}

impl Default for ErrorLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the error log panel
pub fn render_error_log(frame: &mut Frame, error_log: &ErrorLog, area: Rect) {
    if !error_log.is_visible() {
        return;
    }

    let title = format!("Error Log ({} entries) - Press Enter to expand/collapse", error_log.entries().len());

    // For expanded entries, we need to use a different approach
    let selected_index = error_log.selected_index();
    let selected_is_expanded = error_log.is_entry_expanded(selected_index);

    if selected_is_expanded && !error_log.entries().is_empty() {
        // Split the area to show the expanded entry separately
        let chunks = Layout::vertical([
            Constraint::Min(3),      // List area
            Constraint::Min(2),      // Expanded entry area
        ]).split(area);

        // Render the list in the top area
        let items: Vec<ListItem> = error_log
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let display_text = entry.format_for_display();
                let text = truncate_text(&display_text, chunks[0].width.saturating_sub(4) as usize);

                // Color code by severity
                let style = match entry.severity {
                    ErrorSeverity::Error => Style::default().fg(Color::Red),
                    ErrorSeverity::Warning => Style::default().fg(Color::Yellow),
                    ErrorSeverity::Info => Style::default().fg(Color::Blue),
                };

                // Add expansion indicator for selected item
                let final_text = if index == selected_index {
                    format!("▼ {}", text)
                } else {
                    format!("  {}", text)
                };

                ListItem::new(final_text).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(Color::Cyan))
                    .padding(Padding::uniform(1)),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut list_state = ListState::default();
        list_state.select(Some(selected_index));
        frame.render_stateful_widget(list, chunks[0], &mut list_state);

        // Render the expanded entry in the bottom area
        if let Some(entry) = error_log.entries().get(selected_index) {
            let display_text = entry.format_for_display();
            let style = match entry.severity {
                ErrorSeverity::Error => Style::default().fg(Color::Red),
                ErrorSeverity::Warning => Style::default().fg(Color::Yellow),
                ErrorSeverity::Info => Style::default().fg(Color::Blue),
            };

            let expanded_widget = Paragraph::new(display_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Expanded Entry")
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .style(style)
                .wrap(ratatui::widgets::Wrap { trim: false });

            frame.render_widget(expanded_widget, chunks[1]);
        }
    } else {
        // Normal list rendering
        let items: Vec<ListItem> = error_log
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let display_text = entry.format_for_display();
                let text = truncate_text(&display_text, area.width.saturating_sub(4) as usize);

                // Color code by severity
                let style = match entry.severity {
                    ErrorSeverity::Error => Style::default().fg(Color::Red),
                    ErrorSeverity::Warning => Style::default().fg(Color::Yellow),
                    ErrorSeverity::Info => Style::default().fg(Color::Blue),
                };

                // Add expansion indicator for selected item
                let final_text = if index == selected_index {
                    format!("▶ {}", text)
                } else {
                    format!("  {}", text)
                };

                ListItem::new(final_text).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(Color::Cyan))
                    .padding(Padding::uniform(1)),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut list_state = ListState::default();
        if !error_log.entries().is_empty() {
            list_state.select(Some(selected_index));
        }

        frame.render_stateful_widget(list, area, &mut list_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_log_basic_functionality() {
        let mut error_log = ErrorLog::new();

        // Test initial state
        assert_eq!(error_log.unread_count(), 0);
        assert_eq!(error_log.entries().len(), 0);
        assert!(!error_log.is_visible());

        // Test adding entries
        error_log.error("Test error".to_string(), Some("Test".to_string()));
        error_log.warning("Test warning".to_string(), None);
        error_log.info("Test info".to_string(), Some("Info".to_string()));

        assert_eq!(error_log.unread_count(), 3);
        assert_eq!(error_log.entries().len(), 3);
        assert!(error_log.has_errors());

        // Test visibility toggle
        error_log.toggle_visibility();
        assert!(error_log.is_visible());
        assert_eq!(error_log.unread_count(), 0); // Should be marked as read

        // Test navigation - should start at last entry (index 2)
        assert_eq!(error_log.selected_index(), 2);

        error_log.select_previous();
        assert_eq!(error_log.selected_index(), 1);

        error_log.select_next();
        assert_eq!(error_log.selected_index(), 2);

        // Test expansion functionality
        error_log.toggle_selected_wrap();
        assert!(error_log.is_entry_expanded(2));

        error_log.toggle_selected_wrap();
        assert!(!error_log.is_entry_expanded(2));

        // Test clear
        error_log.clear();
        assert_eq!(error_log.entries().len(), 0);
        assert_eq!(error_log.unread_count(), 0);
        assert!(!error_log.is_entry_expanded(0)); // Should clear expanded entries too
    }

    #[test]
    fn test_error_entry_formatting() {
        let entry = ErrorEntry::error(
            "Test error message".to_string(),
            Some("Context".to_string())
        );

        let formatted = entry.format_for_display();
        assert!(formatted.contains("ERROR"));
        assert!(formatted.contains("Test error message"));
        assert!(formatted.contains("[Context]"));
        assert!(formatted.contains("❌"));
    }

    #[test]
    fn test_error_severity() {
        assert_eq!(ErrorSeverity::Error.display_prefix(), "❌");
        assert_eq!(ErrorSeverity::Warning.display_prefix(), "⚠️");
        assert_eq!(ErrorSeverity::Info.display_prefix(), "ℹ️");

        assert_eq!(ErrorSeverity::Error.display_name(), "ERROR");
        assert_eq!(ErrorSeverity::Warning.display_name(), "WARN");
        assert_eq!(ErrorSeverity::Info.display_name(), "INFO");
    }

    #[test]
    fn test_error_log_expansion() {
        let mut error_log = ErrorLog::new();

        error_log.error("Test error".to_string(), None);
        error_log.warning("Test warning".to_string(), None);

        // Test expansion toggle
        assert!(!error_log.is_entry_expanded(0));
        assert!(!error_log.is_entry_expanded(1));

        error_log.toggle_selected_wrap(); // Should expand index 0 (selected by default after adding entries)
        error_log.toggle_visibility(); // This sets selected_index to last entry (1)
        assert_eq!(error_log.selected_index(), 1);

        error_log.toggle_selected_wrap(); // Should expand index 1
        assert!(error_log.is_entry_expanded(1));

        error_log.toggle_selected_wrap(); // Should collapse index 1
        assert!(!error_log.is_entry_expanded(1));
    }
}
