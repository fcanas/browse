use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{prelude::*, widgets::*};
use std::{collections::VecDeque, fs, io, path::PathBuf, time::Duration};
use ratatui::DefaultTerminal;
use std::collections::HashMap;

enum Preview {
    Directory(DirColumn),
    File(PathBuf, String),
}

struct App {
    columns: VecDeque<DirColumn>,
    preview: Option<Preview>,
    selection_cache: HashMap<PathBuf, usize>,
    should_quit: bool,
}

impl App {
    fn new() -> io::Result<Self> {
        let path = std::env::current_dir()?;
        let initial_column = DirColumn::new(path, 0)?;
        let mut columns = VecDeque::new();
        columns.push_back(initial_column);
        let mut app = Self {
            columns,
            preview: None,
            selection_cache: HashMap::new(),
            should_quit: false,
        };
        app.update_preview()?;
        Ok(app)
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if key.kind == KeyEventKind::Press {
            match key.code {
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Up => {
                    self.active_column_mut().select_previous();
                    self.update_preview()?;
                }
                KeyCode::Down => {
                    self.active_column_mut().select_next();
                    self.update_preview()?;
                }
                KeyCode::Right => self.on_right()?,
                KeyCode::Left => self.on_left()?,
                KeyCode::Char('.') => self.set_anchor()?,
                _ => {}
            }
        }
        Ok(())
    }

    fn active_column(&self) -> &DirColumn {
        self.columns.back().unwrap()
    }

    fn active_column_mut(&mut self) -> &mut DirColumn {
        self.columns.back_mut().unwrap()
    }

    fn on_right(&mut self) -> io::Result<()> {
        if self
            .active_column()
            .selected_entry()
            .map_or(false, |e| e.path().is_dir())
        {
            self.cache_active_selection();
            if let Some(Preview::Directory(dir_col)) = self.preview.take() {
                self.columns.push_back(dir_col);
                self.update_preview()?;
            }
        }
        // If the selected item is a file, do nothing.
        Ok(())
    }

    fn on_left(&mut self) -> io::Result<()> {
        self.cache_active_selection();
        let child_path = self.active_column().path.clone();
        if self.columns.len() > 1 {
            self.columns.pop_back();
        } else if let Some(parent) = self.active_column().path.parent() {
            let parent_col = DirColumn::new(parent.to_path_buf(), 0)?;
            self.columns.pop_back();
            self.columns.push_back(parent_col);
        } else {
            // at root, do nothing
            return Ok(());
        }

        if let Some(active_col) = self.columns.back_mut() {
            if let Some(idx) = active_col
                .entries
                .iter()
                .position(|e| e.path() == child_path)
            {
                active_col.selected.select(Some(idx));
            }
        }

        self.update_preview()?;
        Ok(())
    }

    fn set_anchor(&mut self) -> io::Result<()> {
        self.cache_active_selection();
        if let Some(selected_entry) = self.active_column().selected_entry() {
            if selected_entry.path().is_dir() {
                let new_anchor_path = selected_entry.path();
                self.columns.clear();
                self.columns.push_back(DirColumn::new(new_anchor_path, 0)?);
                self.update_preview()?;
            }
        }
        Ok(())
    }

    fn cache_active_selection(&mut self) {
        if let Some(active_col) = self.columns.back() {
            if let Some(selection) = active_col.selected.selected() {
                self.selection_cache
                    .insert(active_col.path.clone(), selection);
            }
        }
    }

    fn update_preview(&mut self) -> io::Result<()> {
        self.preview = if let Some(entry) = self.active_column().selected_entry() {
            if entry.path().is_dir() {
                let path = entry.path();
                let selection = self.selection_cache.get(&path).copied().unwrap_or(0);
                Some(Preview::Directory(DirColumn::new(path, selection)?))
            } else {
                let path = entry.path();
                let details = fs::read_to_string(&path)
                    .unwrap_or_else(|_| "Cannot read file".to_string());
                Some(Preview::File(path, details))
            }
        } else {
            None
        };
        Ok(())
    }
}

struct DirColumn {
    path: PathBuf,
    entries: Vec<fs::DirEntry>,
    selected: ListState,
}

impl DirColumn {
    fn new(path: PathBuf, initial_selection: usize) -> io::Result<Self> {
        let entries = Self::read_dir(&path)?;
        let mut selected = ListState::default();
        if !entries.is_empty() {
            selected.select(Some(initial_selection.min(entries.len() - 1)));
        }
        Ok(Self {
            path,
            entries,
            selected,
        })
    }

    fn read_dir(path: &PathBuf) -> io::Result<Vec<fs::DirEntry>> {
        let mut entries: Vec<_> = fs::read_dir(path)?.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.path());
        Ok(entries)
    }

    fn selected_entry(&self) -> Option<&fs::DirEntry> {
        self.selected.selected().and_then(|i| self.entries.get(i))
    }

    fn select_previous(&mut self) {
        let i = match self.selected.selected() {
            Some(i) => {
                if i == 0 {
                    self.entries.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.selected.select(Some(i));
    }

    fn select_next(&mut self) {
        let i = match self.selected.selected() {
            Some(i) => {
                if i >= self.entries.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.selected.select(Some(i));
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let mut app = App::new()?;
    run(&mut terminal, &mut app)?;
    ratatui::restore();
    Ok(())
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|f| ui(f, app))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key)?;
            }
        }
    }
    Ok(())
}

fn ui(frame: &mut Frame, app: &mut App) {
    let num_cols = app.columns.len() + if app.preview.is_some() { 1 } else { 0 };
    let constraints = (0..num_cols)
        .map(|_| Constraint::Ratio(1, num_cols as u32))
        .collect::<Vec<_>>();
    let layout = Layout::horizontal(constraints).split(frame.area());

    let active_column_index = app.columns.len() - 1;
    for (i, column) in app.columns.iter_mut().enumerate() {
        let is_active = i == active_column_index;
        let items: Vec<ListItem> = column
            .entries
            .iter()
            .map(|entry| {
                let path = entry.path();
                let file_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let style = if path.is_dir() {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                };
                ListItem::new(Span::styled(file_name, style))
            })
            .collect();

        let highlight_style = if is_active {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default().bg(Color::DarkGray)
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(if is_active {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    })
                    .title(column.path.to_string_lossy().to_string()),
            )
            .highlight_style(highlight_style)
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, layout[i], &mut column.selected);
    }

    if let Some(preview) = &mut app.preview {
        let preview_area = layout[app.columns.len()];
        match preview {
            Preview::Directory(dir_column) => {
                let items: Vec<ListItem> = dir_column
                    .entries
                    .iter()
                    .map(|entry| {
                        let path = entry.path();
                        let file_name =
                            path.file_name().unwrap_or_default().to_string_lossy();
                        let style = if path.is_dir() {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default()
                        };
                        ListItem::new(Span::styled(file_name.to_string(), style))
                    })
                    .collect();
                let list = List::new(items).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(dir_column.path.to_string_lossy().to_string()),
                );
                frame.render_widget(list, preview_area);
            }
            Preview::File(path, details) => {
                let title = path.file_name().unwrap_or_default().to_string_lossy();
                let paragraph = Paragraph::new(details.clone())
                    .block(Block::default().borders(Borders::ALL).title(title.to_string()));
                frame.render_widget(paragraph, preview_area);
            }
        }
    }
} 