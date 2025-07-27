use crate::app::{App, DirColumn, Preview};
use crate::config::Settings;
use crate::file_operations::{get_icon, FileDetails};
use crate::utils::{format_file_size, truncate_text};
use ratatui::{
    prelude::*,
    widgets::*,
};


/// Settings panel state
#[derive(Debug)]
pub struct SettingsState {
    pub active_tab: SettingsTab,
    pub focus: SettingsFocus,
    pub display_selection: usize,
    pub file_type_selection: usize,
    pub file_type_column_selection: usize,
    pub file_type_table_state: TableState,
    pub add_file_type_state: Option<AddFileTypeState>,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            active_tab: SettingsTab::Display,
            focus: SettingsFocus::TabList,
            display_selection: 0,
            file_type_selection: 0,
            file_type_column_selection: 0,
            file_type_table_state: TableState::default(),
            add_file_type_state: None,
        }
    }
}

/// State for adding/editing file type rules
#[derive(Debug)]
pub struct AddFileTypeState {
    pub mime_type: String,
    pub icon: String,
    pub preview: bool,
    pub focused_field: usize,
    pub is_editing: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsTab {
    Display,
    FileTypes,
    Keybindings,
}

impl SettingsTab {
    pub fn next(self) -> Self {
        match self {
            Self::Display => Self::FileTypes,
            Self::FileTypes => Self::Keybindings,
            Self::Keybindings => Self::Display,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Display => Self::Keybindings,
            Self::FileTypes => Self::Display,
            Self::Keybindings => Self::FileTypes,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SettingsFocus {
    TabList,
    TabContent,
    AddFileTypePopup,
}

/// Main UI rendering function
pub fn render_ui(frame: &mut Frame, app: &mut App) {
    let num_cols = app.columns().len() + if app.preview().is_some() { 1 } else { 0 };
    let constraints = (0..num_cols)
        .map(|_| Constraint::Ratio(1, num_cols as u32))
        .collect::<Vec<_>>();
    let layout = Layout::horizontal(constraints).split(frame.area());

    // Render columns
    let active_column_index = app.columns().len() - 1;
    for (i, column) in app.columns().iter().enumerate() {
        let is_active = i == active_column_index;
        render_dir_column(frame, column, layout[i], is_active, false, app.config());
    }

    // Render preview
    if let Some(preview) = app.preview() {
        let preview_area = layout[app.columns().len()];
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
    let title = column
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    
    // Calculate available width for the title (subtract borders and padding)
    let title_width = area.width.saturating_sub(4) as usize;
    let truncated_title = truncate_text(&title, title_width);

    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    // Calculate available width for filenames (subtract borders, padding, and icon space)
    let content_width = area.width.saturating_sub(4) as usize;
    let filename_width = if config.show_icons {
        content_width.saturating_sub(3) // Reserve space for icon + space
    } else {
        content_width
    };

    let items: Vec<ListItem> = column
        .entries
        .iter()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let truncated_name = truncate_text(&name, filename_width);
            let icon = get_icon(entry, config);
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
                .borders(Borders::ALL)
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
    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Render file preview panel
fn render_file_preview(frame: &mut Frame, details: &FileDetails, area: Rect) {
    let chunks = Layout::vertical([Constraint::Max(8), Constraint::Min(0)]).split(area);
    
    let title = details
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    
    // Truncate title to fit in the preview area
    let title_width = area.width.saturating_sub(4) as usize;
    let truncated_title = truncate_text(&title, title_width);

    // Metadata section
    let mut lines = vec![Line::from(vec![
        Span::styled("Size: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format_file_size(details.size)),
    ])];

    if let Some(created) = details.created {
        lines.push(Line::from(vec![
            Span::styled("Created: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(created.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));
    }

    if let Some(modified) = details.modified {
        lines.push(Line::from(vec![
            Span::styled("Modified: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(modified.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));
    }

    if let Some(target) = &details.symlink_target {
        let target_str = target.to_string_lossy().to_string();
        // Truncate symlink target to fit (reserve space for "Symlink -> " prefix)
        let target_width = area.width.saturating_sub(16) as usize; // "Symlink -> " is ~11 chars + padding
        let truncated_target = truncate_text(&target_str, target_width);
        lines.push(Line::from(vec![
            Span::styled("Symlink -> ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(truncated_target),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("MIME Type: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(details.mime_type.as_deref().unwrap_or("unknown")),
    ]));

    let metadata_widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(truncated_title)
            .padding(Padding::uniform(1)),
    );

    // Content preview section
    let content_widget = Paragraph::new(details.content_preview.clone())
        .block(Block::default().borders(Borders::ALL).title("Preview"));

    frame.render_widget(metadata_widget, chunks[0]);
    frame.render_widget(content_widget, chunks[1]);
}

/// Render settings panel
fn render_settings_panel(frame: &mut Frame, app: &mut App) {
    let area = centered_rect(80, 70, frame.area());
    frame.render_widget(Clear, area);

    let settings_state = app.settings().as_ref().unwrap();
    let config = app.config();
    
    let chunks = Layout::horizontal([Constraint::Length(20), Constraint::Min(0)]).split(area);

    // Left panel - tab list
    let tab_list_style = if settings_state.focus == SettingsFocus::TabList {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let tab_items = vec![
        ListItem::new("Display"),
        ListItem::new("File Types"),
        ListItem::new("Keybindings"),
    ];

    let mut tab_list_state = ListState::default();
    tab_list_state.select(Some(settings_state.active_tab as usize));

    let tab_list = List::new(tab_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Settings")
                .border_style(tab_list_style)
                .padding(Padding::uniform(1)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(tab_list, chunks[0], &mut tab_list_state);

    // Right panel - tab content
    let content_border_style = if settings_state.focus == SettingsFocus::TabContent {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    match settings_state.active_tab {
        SettingsTab::Display => {
            render_display_settings(frame, chunks[1], settings_state, config, content_border_style);
        }
        SettingsTab::FileTypes => {
            render_file_types_settings(frame, chunks[1], settings_state, config, content_border_style);
        }
        SettingsTab::Keybindings => {
            render_keybindings_settings(frame, chunks[1], content_border_style);
        }
    }

    // Render add file type popup if active
    if let Some(add_state) = &settings_state.add_file_type_state {
        render_add_file_type_popup(frame, add_state);
    }
}

/// Render display settings tab
fn render_display_settings(
    frame: &mut Frame,
    area: Rect,
    settings_state: &SettingsState,
    config: &Settings,
    border_style: Style,
) {
    let items = vec![
        ListItem::new(format!(
            "[{}] Show hidden files",
            if config.show_hidden_files { "✓" } else { " " }
        )),
        ListItem::new(format!(
            "[{}] Show icons",
            if config.show_icons { "✓" } else { " " }
        )),
    ];

    let mut list_state = ListState::default();
    list_state.select(Some(settings_state.display_selection));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Display Options")
                .border_style(border_style)
                .padding(Padding::uniform(1)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Render file types settings tab
fn render_file_types_settings(
    frame: &mut Frame,
    area: Rect,
    settings_state: &SettingsState,
    config: &Settings,
    border_style: Style,
) {
    let file_types_chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);

    let mut sorted_exts: Vec<_> = config
        .mime_types
        .primary
        .keys()
        .chain(config.mime_types.subtypes.keys())
        .collect();
    sorted_exts.sort();

    let rows = sorted_exts.iter().enumerate().map(|(row_index, ext)| {
        let rule = if let Some(rule) = config.mime_types.primary.get(*ext) {
            rule
        } else {
            config.mime_types.subtypes.get(*ext).unwrap()
        };
        
        let is_selected_row = row_index == settings_state.file_type_selection
            && settings_state.focus == SettingsFocus::TabContent;

        let row_style = if is_selected_row {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };

        let cells_data = [
            ext.to_string(),
            rule.icon.clone(),
            if rule.preview { "✅".to_string() } else { "❌".to_string() },
        ];

        let cells: Vec<Cell> = cells_data
            .into_iter()
            .enumerate()
            .map(|(col_index, data)| {
                let is_selected_cell = is_selected_row
                    && col_index == settings_state.file_type_column_selection;
                let style = if is_selected_cell {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    row_style
                };
                Cell::from(data).style(style)
            })
            .collect();

        Row::new(cells)
    });

    let table = Table::new(
        rows,
        [Constraint::Length(20), Constraint::Length(4), Constraint::Length(8)],
    )
    .header(
        Row::new(vec!["MIME Type", "Icon", "Preview"])
            .style(Style::new().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .title("File Type Handling")
            .borders(Borders::ALL)
            .padding(Padding::uniform(1))
            .border_style(border_style),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut table_state = settings_state.file_type_table_state.clone();
    frame.render_stateful_widget(table, file_types_chunks[0], &mut table_state);

    let footer = Paragraph::new("[A]dd, [D]elete, [E]dit").alignment(Alignment::Center);
    frame.render_widget(footer, file_types_chunks[1]);
}

/// Render keybindings settings tab
fn render_keybindings_settings(frame: &mut Frame, area: Rect, border_style: Style) {
    let commands = [
        ("Ctrl+Q", "Quit the application"),
        ("?", "Show this help window"),
        ("Up/Down", "Navigate list"),
        ("Left", "Navigate to parent directory"),
        ("Right", "Enter directory / activate preview"),
        (".", "Set selected directory as anchor"),
    ];

    let rows = commands.iter().map(|(key, desc)| {
        Row::new(vec![Cell::from(*key), Cell::from(*desc)])
    });

    let table = Table::new(rows, [Constraint::Percentage(30), Constraint::Percentage(70)])
        .block(
            Block::default()
                .title("Keybindings")
                .borders(Borders::ALL)
                .padding(Padding::uniform(1))
                .border_style(border_style),
        )
        .header(
            Row::new(vec!["Key", "Description"])
                .style(Style::new().add_modifier(Modifier::BOLD)),
        );

    frame.render_widget(table, area);
}

/// Render add/edit file type popup
fn render_add_file_type_popup(frame: &mut Frame, add_state: &AddFileTypeState) {
    let popup_area = centered_rect(50, 30, frame.area());
    frame.render_widget(Clear, popup_area);

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
    ])
    .split(popup_area);

    let title = if add_state.is_editing.is_some() {
        "Edit File Type"
    } else {
        "Add File Type"
    };

    // MIME Type field
    let mime_type_style = if add_state.focused_field == 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    
    let mime_type_widget = Paragraph::new(add_state.mime_type.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("MIME Type")
                .border_style(mime_type_style),
        );

    // Icon field
    let icon_style = if add_state.focused_field == 1 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    
    let icon_widget = Paragraph::new(add_state.icon.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Icon")
                .border_style(icon_style),
        );

    // Preview checkbox
    let preview_style = if add_state.focused_field == 2 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    
    let preview_text = format!("[{}] Preview", if add_state.preview { "✓" } else { " " });
    let preview_widget = Paragraph::new(preview_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Options")
                .border_style(preview_style),
        );

    // Main popup block
    let popup_block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .padding(Padding::uniform(1));

    frame.render_widget(popup_block, popup_area);
    frame.render_widget(mime_type_widget, chunks[0]);
    frame.render_widget(icon_widget, chunks[1]);
    frame.render_widget(preview_widget, chunks[2]);
}

/// Create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
} 