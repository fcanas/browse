use crate::app::{App, LayoutInfo};
use crate::browser::{render_browser};
use crate::error::render_error_log;
use crate::utils::{truncate_text};

use ratatui::{
    prelude::*,
    widgets::*,
};

/// Main UI rendering function
pub fn render_ui(frame: &mut Frame, app: &mut App) -> LayoutInfo {
    let layout_info = calculate_layout_info(frame.area(), app);
    render_ui_with_layout(frame, app, &layout_info);
    layout_info
}

/// Calculate layout information for mouse interactions
fn calculate_layout_info(area: Rect, app: &App) -> LayoutInfo {
    let mut layout_info = LayoutInfo::default();
    // Create layout with tabs at the top
    let main_layout = if app.error_log().is_visible() {
        Layout::vertical([
            Constraint::Length(1),   // Tab bar
            Constraint::Min(0),      // Browser content
            Constraint::Length(8),   // Error log panel (8 lines)
            Constraint::Length(1),   // Status bar
        ]).split(area)
    } else {
        Layout::vertical([
            Constraint::Length(1),   // Tab bar
            Constraint::Min(0),      // Browser content
            Constraint::Length(1),   // Status bar
        ]).split(area)
    };

    layout_info.tab_area = main_layout[0];
    layout_info.browser_area = main_layout[1];

    // Calculate browser column areas
    let browser = app.browser();
    let num_cols = browser.columns().len() + if browser.preview().is_some() { 1 } else { 0 };
    let constraints = (0..num_cols)
        .map(|_| Constraint::Ratio(1, num_cols as u32))
        .collect::<Vec<_>>();
    layout_info.column_areas = Layout::horizontal(constraints).split(main_layout[1]).to_vec();

    if app.error_log().is_visible() {
        layout_info.status_area = main_layout[3];
    } else {
        layout_info.status_area = main_layout[2];
    }

    layout_info
}

/// Render UI with pre-calculated layout info
fn render_ui_with_layout(frame: &mut Frame, app: &mut App, _layout_info: &LayoutInfo) {
    // Create layout with tabs at the top
    let main_layout = if app.error_log().is_visible() {
        Layout::vertical([
            Constraint::Length(1),   // Tab bar
            Constraint::Min(0),      // Browser content
            Constraint::Length(8),   // Error log panel (8 lines)
            Constraint::Length(1),   // Status bar
        ]).split(frame.area())
    } else {
        Layout::vertical([
            Constraint::Length(1),   // Tab bar
            Constraint::Min(0),      // Browser content
            Constraint::Length(1),   // Status bar
        ]).split(frame.area())
    };

    // Render tab bar at the top
    render_tab_bar(frame, app, main_layout[0]);

    // Render browser in the main area
    render_browser(frame, app, main_layout[1]);

    if app.error_log().is_visible() {
        // Render error log in the middle area
        render_error_log(frame, app.error_log(), main_layout[2]);
        // Render status bar in the bottom area
        render_status_bar(frame, app, main_layout[3]);
    } else {
        // Render status bar in the bottom area
        render_status_bar(frame, app, main_layout[2]);
    }
}

/// Render tab bar showing all open tabs
fn render_tab_bar(frame: &mut Frame, app: &App, area: Rect) {
    let tab_manager = app.tab_manager();
    let tabs = tab_manager.tabs();
    let active_index = tab_manager.active_index();

    if tabs.len() <= 1 {
        // If only one tab, show a simple title bar
        let title = format!(" {} ", tabs[0].display_name());
        let title_paragraph = Paragraph::new(title)
            .style(Style::default().bg(Color::Blue).fg(Color::White))
            .alignment(Alignment::Left);
        frame.render_widget(title_paragraph, area);
        return;
    }

    // Calculate tab widths
    let available_width = area.width as usize;
    let tab_count = tabs.len();
    let min_tab_width = 8; // Minimum width for each tab
    let max_tab_width = 20; // Maximum width for each tab

    let tab_width = if tab_count * min_tab_width <= available_width {
        std::cmp::min(available_width / tab_count, max_tab_width)
    } else {
        min_tab_width
    };

    // Create tab titles
    let mut tab_titles = Vec::new();
    let mut tab_styles = Vec::new();

    for (i, tab) in tabs.iter().enumerate() {
        let is_active = i == active_index;
        let mut title = tab.display_name().to_string();

        // Truncate title if too long
        if title.len() > tab_width - 2 {
            title = format!("{}…", &title[..tab_width - 3]);
        }

        // Add padding
        title = format!(" {} ", title);

        tab_titles.push(title);

        if is_active {
            tab_styles.push(Style::default().bg(Color::Blue).fg(Color::White));
        } else {
            tab_styles.push(Style::default().bg(Color::DarkGray).fg(Color::White));
        }
    }

    // Render tabs
    let mut x = 0;
    for (_i, (title, style)) in tab_titles.iter().zip(tab_styles.iter()).enumerate() {
        if x >= area.width {
            break;
        }

        let tab_width = std::cmp::min(title.len(), (area.width - x) as usize);
        let tab_area = Rect {
            x: area.x + x,
            y: area.y,
            width: tab_width as u16,
            height: 1,
        };

        let tab_title = if title.len() > tab_width {
            &title[..tab_width]
        } else {
            title
        };

        let tab_paragraph = Paragraph::new(tab_title)
            .style(*style)
            .alignment(Alignment::Left);

        frame.render_widget(tab_paragraph, tab_area);
        x += tab_width as u16;
    }

    // Fill remaining space with background
    if x < area.width {
        let remaining_area = Rect {
            x: area.x + x,
            y: area.y,
            width: area.width - x,
            height: 1,
        };
        let background = Paragraph::new("")
            .style(Style::default().bg(Color::DarkGray));
        frame.render_widget(background, remaining_area);
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

    let tab_info = if app.tab_manager().tab_count() > 1 {
        format!(" | Tab {}/{}", app.tab_manager().active_index() + 1, app.tab_manager().tab_count())
    } else {
        String::new()
    };

    let status_text = if !app.browser().search_string().is_empty() {
        format!("Search: '{}' | {} | {} items{}{} | Esc to clear | ? for settings{}{}",
                app.browser().search_string(), current_path, file_count, selected_info, tab_info, error_help, error_indicator)
    } else {
        format!("{} | {} items{}{} | ? for settings{}{}",
                current_path, file_count, selected_info, tab_info, error_help, error_indicator)
    };

    let status_paragraph = Paragraph::new(truncate_text(&status_text, area.width as usize))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    frame.render_widget(status_paragraph, area);
}
