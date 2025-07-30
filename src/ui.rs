use crate::app::{App};
use crate::browser::{render_browser};
use crate::error::render_error_log;
use crate::utils::{truncate_text};

use ratatui::{
    prelude::*,
    widgets::*,
};

/// Main UI rendering function
pub fn render_ui(frame: &mut Frame, app: &mut App) {
    // Create layout based on whether error log is visible
    let main_layout = if app.error_log().is_visible() {
        Layout::vertical([
            Constraint::Min(0),      // Browser content
            Constraint::Length(8),   // Error log panel (8 lines)
            Constraint::Length(1),   // Status bar
        ]).split(frame.area())
    } else {
        Layout::vertical([
            Constraint::Min(0),      // Browser content
            Constraint::Length(1),   // Status bar
        ]).split(frame.area())
    };

    // Render browser in the top area
    render_browser(frame, app, main_layout[0]);

    if app.error_log().is_visible() {
        // Render error log in the middle area
        render_error_log(frame, app.error_log(), main_layout[1]);
        // Render status bar in the bottom area
        render_status_bar(frame, app, main_layout[2]);
    } else {
        // Render status bar in the bottom area
        render_status_bar(frame, app, main_layout[1]);
    }
}


/// Render status bar with helpful information
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let current_path = app.browser().columns()
        .back()
        .map(|col| col.path.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let file_count = app.browser().columns()
        .back()
        .map(|col| col.entries.len())
        .unwrap_or(0);

    let selected_info = app.browser().columns()
        .back()
        .and_then(|col| col.selected.selected())
        .map(|idx| format!(" ({}/{})", idx + 1, file_count))
        .unwrap_or_default();

    // Create error count display
    let error_count = app.error_log().unread_count();
    let error_indicator = if error_count > 0 {
        if app.error_log().has_errors() {
            format!(" | ❌ {} errors", error_count)
        } else {
            format!(" | ⚠️ {} warnings", error_count)
        }
    } else {
        String::new()
    };

    let error_help = if app.error_log().is_visible() {
        " | Enter to expand, Esc to hide"
    } else {
        " | Ctrl+E for errors"
    };

    let status_text = if !app.browser().search_string().is_empty() {
        format!("Search: '{}' | {} | {} items{} | Esc to clear | ? for settings{}{}",
                app.browser().search_string(), current_path, file_count, selected_info, error_help, error_indicator)
    } else {
        format!("{} | {} items{} | ? for settings{}{}",
                current_path, file_count, selected_info, error_help, error_indicator)
    };

    let status_paragraph = Paragraph::new(truncate_text(&status_text, area.width as usize))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    frame.render_widget(status_paragraph, area);
}
