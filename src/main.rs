use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{prelude::*, widgets::*};
use std::{collections::VecDeque, fs, io, path::PathBuf, time::Duration};
use ratatui::DefaultTerminal;

enum Preview {
    Directory(DirColumn),
    File(String),
}

struct App {
    columns: VecDeque<DirColumn>,
    preview: Option<Preview>,
    should_quit: bool,
}

impl App {
    fn new() -> io::Result<Self> {
        let path = std::env::current_dir()?;
        let initial_column = DirColumn::new(path)?;
        let mut columns = VecDeque::new();
        columns.push_back(initial_column);
        let mut app = Self {
            columns,
            preview: None,
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
            if let Some(Preview::Directory(dir_col)) = self.preview.take() {
                self.columns.push_back(dir_col);
                self.update_preview()?;
            }
        }
        // If the selected item is a file, do nothing.
        Ok(())
    }

    fn on_left(&mut self) -> io::Result<()> {
        if self.columns.len() > 1 {
            self.columns.pop_back();
        } else if let Some(first_col) = self.columns.front() {
            if let Some(parent) = first_col.path.parent() {
                let parent_col = DirColumn::new(parent.to_path_buf())?;
                self.columns.push_front(parent_col);
            }
        }
        self.update_preview()?;
        Ok(())
    }

    fn set_anchor(&mut self) -> io::Result<()> {
        if let Some(selected_entry) = self.active_column().selected_entry() {
            if selected_entry.path().is_dir() {
                let new_anchor_path = selected_entry.path();
                self.columns.clear();
                self.columns
                    .push_back(DirColumn::new(new_anchor_path)?);
                self.update_preview()?;
            }
        }
        Ok(())
    }

    fn update_preview(&mut self) -> io::Result<()> {
        self.preview = if let Some(entry) = self.active_column().selected_entry() {
            if entry.path().is_dir() {
                Some(Preview::Directory(DirColumn::new(entry.path())?))
            } else {
                let details = fs::read_to_string(entry.path()).unwrap_or_else(|_| "Cannot read file".to_string());
                Some(Preview::File(details))
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
    fn new(path: PathBuf) -> io::Result<Self> {
        let entries = Self::read_dir(&path)?;
        let mut selected = ListState::default();
        selected.select(Some(0));
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

    for (i, column) in app.columns.iter_mut().enumerate() {
        let items: Vec<ListItem> = column
            .entries
            .iter()
            .map(|entry| {
                let path = entry.path();
                let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let style = if path.is_dir() {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                };
                ListItem::new(Span::styled(file_name, style))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(column.path.to_string_lossy().to_string()),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
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
                let list = List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(dir_column.path.to_string_lossy().to_string()),
                    )
                    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                    .highlight_symbol(">> ");
                frame.render_stateful_widget(list, preview_area, &mut dir_column.selected);
            }
            Preview::File(details) => {
                let paragraph = Paragraph::new(details.clone())
                    .block(Block::default().borders(Borders::ALL).title("File Preview"));
                frame.render_widget(paragraph, preview_area);
            }
        }
    }
} 