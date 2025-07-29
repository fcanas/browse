use crate::app::App;
use crate::config::Settings;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};

use ratatui::{
    prelude::*,
    widgets::*,
};

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

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SettingsFocus {
    TabList,
    TabContent,
    AddFileTypePopup,
}


/// Settings manager handling all settings UI state and interactions
#[derive(Debug)]
pub struct SettingsManager {
    state: Option<SettingsState>,
}

impl SettingsManager {
    /// Create a new settings manager
    pub fn new() -> Self {
        Self { state: None }
    }

    /// Check if settings panel is open
    pub fn is_open(&self) -> bool {
        self.state.is_some()
    }

    /// Get reference to settings state
    pub fn state(&self) -> &Option<SettingsState> {
        &self.state
    }

    /// Open the settings panel
    pub fn open(&mut self) {
        self.state = Some(SettingsState::new());
    }

    /// Close the settings panel
    pub fn close(&mut self) {
        self.state = None;
    }

    /// Handle settings panel key input
    /// Returns true if browser needs to be reloaded due to settings changes
    pub fn handle_key(&mut self, key: KeyEvent, config: &mut Settings) -> Result<bool> {
        if self.state.is_none() {
            return Ok(false);
        }

        let mut needs_browser_reload = false;

        // Extract the current state to avoid borrowing conflicts
        let (focus, active_tab, display_selection) = {
            let settings_state = self.state.as_ref().unwrap();
            (settings_state.focus, settings_state.active_tab, settings_state.display_selection)
        };

        match focus {
            SettingsFocus::TabList => match key.code {
                KeyCode::Esc | KeyCode::Char('?') => self.state = None,
                KeyCode::Up => {
                    if let Some(settings_state) = &mut self.state {
                        settings_state.active_tab = settings_state.active_tab.prev();
                    }
                }
                KeyCode::Down => {
                    if let Some(settings_state) = &mut self.state {
                        settings_state.active_tab = settings_state.active_tab.next();
                    }
                }
                KeyCode::Right | KeyCode::Tab => {
                    if let Some(settings_state) = &mut self.state {
                        settings_state.focus = SettingsFocus::TabContent;
                    }
                }
                _ => {}
            },
            SettingsFocus::TabContent => match active_tab {
                SettingsTab::Display => match key.code {
                    KeyCode::Esc | KeyCode::Char('?') => self.state = None,
                    KeyCode::Left => {
                        if let Some(settings_state) = &mut self.state {
                            settings_state.focus = SettingsFocus::TabList;
                        }
                    }
                    KeyCode::Up => {
                        if let Some(settings_state) = &mut self.state {
                            settings_state.display_selection = settings_state.display_selection.saturating_sub(1);
                        }
                    }
                    KeyCode::Down => {
                        if let Some(settings_state) = &mut self.state {
                            settings_state.display_selection = (settings_state.display_selection + 1).min(1);
                        }
                    }
                    KeyCode::Char(' ') | KeyCode::Enter => {
                        match display_selection {
                            0 => {
                                config.show_hidden_files = !config.show_hidden_files;
                                needs_browser_reload = true;
                            }
                            1 => config.show_icons = !config.show_icons,
                            _ => {}
                        }
                    }
                    _ => {}
                },
                SettingsTab::FileTypes => {
                    self.handle_file_types_key(key, config)?;
                }
                SettingsTab::Keybindings => match key.code {
                    KeyCode::Esc | KeyCode::Char('?') => self.state = None,
                    KeyCode::Left => {
                        if let Some(settings_state) = &mut self.state {
                            settings_state.focus = SettingsFocus::TabList;
                        }
                    }
                    _ => {}
                },
            },
            SettingsFocus::AddFileTypePopup => {
                self.handle_add_file_type_key(key, config)?;
            }
        }
        Ok(needs_browser_reload)
    }

    /// Handle file types tab key input
    fn handle_file_types_key(&mut self, key: KeyEvent, config: &mut Settings) -> Result<()> {
        let has_add_state = self.state.as_ref().unwrap().add_file_type_state.is_some();
        if has_add_state {
            return self.handle_add_file_type_key(key, config);
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('?') => self.close(),
            KeyCode::Left => {
                if let Some(settings_state) = &mut self.state {
                    settings_state.focus = SettingsFocus::TabList;
                }
            }
            KeyCode::Up => {
                if let Some(settings_state) = &mut self.state {
                    settings_state.file_type_selection = settings_state.file_type_selection.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(settings_state) = &mut self.state {
                    let total_items = config.mime_types.primary.len() + config.mime_types.subtypes.len();
                    settings_state.file_type_selection = (settings_state.file_type_selection + 1).min(total_items.saturating_sub(1));
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                if let Some(settings_state) = &mut self.state {
                    settings_state.add_file_type_state = Some(AddFileTypeState {
                        mime_type: String::new(),
                        icon: String::new(),
                        preview: false,
                        focused_field: 0,
                        is_editing: None,
                    });
                    settings_state.focus = SettingsFocus::AddFileTypePopup;
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.delete_selected_file_type(config);
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                self.edit_selected_file_type(config);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle add/edit file type popup key input
    fn handle_add_file_type_key(&mut self, key: KeyEvent, config: &mut Settings) -> Result<()> {
        let focused_field = self.state.as_ref().unwrap().add_file_type_state.as_ref().unwrap().focused_field;

        match key.code {
            KeyCode::Esc => {
                if let Some(settings_state) = &mut self.state {
                    settings_state.add_file_type_state = None;
                    settings_state.focus = SettingsFocus::TabContent;
                }
            }
            KeyCode::Tab => {
                if let Some(settings_state) = &mut self.state {
                    if let Some(add_state) = &mut settings_state.add_file_type_state {
                        add_state.focused_field = (add_state.focused_field + 1) % 3;
                    }
                }
            }
            KeyCode::BackTab => {
                if let Some(settings_state) = &mut self.state {
                    if let Some(add_state) = &mut settings_state.add_file_type_state {
                        add_state.focused_field = if add_state.focused_field == 0 { 2 } else { add_state.focused_field - 1 };
                    }
                }
            }
            KeyCode::Enter => {
                self.save_file_type_rule(config)?;
            }
            KeyCode::Char(' ') if focused_field == 2 => {
                if let Some(settings_state) = &mut self.state {
                    if let Some(add_state) = &mut settings_state.add_file_type_state {
                        add_state.preview = !add_state.preview;
                    }
                }
            }
            KeyCode::Char(c) => {
                if let Some(settings_state) = &mut self.state {
                    if let Some(add_state) = &mut settings_state.add_file_type_state {
                        match focused_field {
                            0 => add_state.mime_type.push(c),
                            1 => add_state.icon.push(c),
                            _ => {}
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(settings_state) = &mut self.state {
                    if let Some(add_state) = &mut settings_state.add_file_type_state {
                        match focused_field {
                            0 => { add_state.mime_type.pop(); }
                            1 => { add_state.icon.pop(); }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Delete the currently selected file type rule
    fn delete_selected_file_type(&mut self, config: &mut Settings) {
        let file_type_selection = self.state.as_ref().unwrap().file_type_selection;

        let mut sorted_exts: Vec<_> = config
            .mime_types
            .primary
            .keys()
            .chain(config.mime_types.subtypes.keys())
            .collect();
        sorted_exts.sort();

        if let Some(ext) = sorted_exts.get(file_type_selection) {
            let ext = ext.to_string();
            config.mime_types.primary.remove(&ext);
            config.mime_types.subtypes.remove(&ext);

            // Adjust selection if needed
            if let Some(settings_state) = &mut self.state {
                let total_items = config.mime_types.primary.len() + config.mime_types.subtypes.len();
                if settings_state.file_type_selection >= total_items && total_items > 0 {
                    settings_state.file_type_selection = total_items - 1;
                }
            }
        }
    }

    /// Edit the currently selected file type rule
    fn edit_selected_file_type(&mut self, config: &Settings) {
        let file_type_selection = self.state.as_ref().unwrap().file_type_selection;

        let mut sorted_exts: Vec<_> = config
            .mime_types
            .primary
            .keys()
            .chain(config.mime_types.subtypes.keys())
            .collect();
        sorted_exts.sort();

        if let Some(ext) = sorted_exts.get(file_type_selection) {
            let rule = if let Some(rule) = config.mime_types.primary.get(*ext) {
                rule
            } else {
                config.mime_types.subtypes.get(*ext).unwrap()
            }.clone();

            if let Some(settings_state) = &mut self.state {
                settings_state.add_file_type_state = Some(AddFileTypeState {
                    mime_type: ext.to_string(),
                    icon: rule.icon,
                    preview: rule.preview,
                    focused_field: 0,
                    is_editing: Some(ext.to_string()),
                });
                settings_state.focus = SettingsFocus::AddFileTypePopup;
            }
        }
    }

    /// Save file type rule from the add/edit popup
    fn save_file_type_rule(&mut self, config: &mut Settings) -> Result<()> {
        if let Some(settings_state) = &mut self.state {
            if let Some(add_state) = &settings_state.add_file_type_state {
                if !add_state.mime_type.is_empty() {
                    let rule = crate::config::FileTypeRule {
                        icon: add_state.icon.clone(),
                        preview: add_state.preview,
                    };

                    // If editing, remove the old entry first
                    if let Some(old_mime_type) = &add_state.is_editing {
                        config.mime_types.primary.remove(old_mime_type);
                        config.mime_types.subtypes.remove(old_mime_type);
                    }

                    // Add the new/updated rule
                    config.mime_types.primary.insert(add_state.mime_type.clone(), rule);
                }
            }

            // Close the popup
            settings_state.add_file_type_state = None;
            settings_state.focus = SettingsFocus::TabContent;
        }
        Ok(())
    }
}

/// Render settings panel
pub fn render_settings_panel(frame: &mut Frame, app: &mut App) {
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
            render_keybindings_settings(frame, chunks[1], content_border_style, app);
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
            if rule.preview { "✓".to_string() } else { "✗".to_string() },
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
fn render_keybindings_settings(frame: &mut Frame, area: Rect, border_style: Style, app: &App) {
    let commands = app.command_registry().get_display_commands();

    let rows = commands.iter().map(|(key, desc)| {
        Row::new(vec![Cell::from(key.clone()), Cell::from(*desc)])
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
