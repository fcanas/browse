use crate::app::{App};
use crate::browser::{render_browser};
use crate::utils::{truncate_text};

use ratatui::{
    prelude::*,
    widgets::*,
};

/// Main UI rendering function
pub fn render_ui(frame: &mut Frame, app: &mut App) {
    let main_layout = Layout::vertical([
        Constraint::Min(0),      // Main content
        Constraint::Length(1),   // Status bar
    ]).split(frame.area());

    render_browser(frame, app, main_layout[0]);
    render_status_bar(frame, app, main_layout[1]);
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

    let status_text = if !app.browser().search_string().is_empty() {
        format!("Search: '{}' | {} | {} items{} | Esc to clear | ? for settings & help",
                app.browser().search_string(), current_path, file_count, selected_info)
    } else {
        format!("{} | {} items{} | ? for settings & help", current_path, file_count, selected_info)
    };

    let status_paragraph = Paragraph::new(truncate_text(&status_text, area.width as usize))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    frame.render_widget(status_paragraph, area);
}
