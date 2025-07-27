use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{prelude::*, widgets::*};
use std::{fs, io, path::PathBuf, time::Duration};
use ratatui::DefaultTerminal;

struct App {
    columns: Vec<DirColumn>,
    should_quit: bool,
}

impl App {
    fn new() -> io::Result<Self> {
        let mut columns = vec![];
        let mut current_path = std::env::current_dir()?;
        let mut paths_to_create = vec![current_path.clone()];

        while let Some(parent) = current_path.parent() {
            paths_to_create.push(parent.to_path_buf());
            current_path = parent.to_path_buf();
        }

        for path in paths_to_create.into_iter().rev() {
            columns.push(DirColumn::new(path)?);
        }

        Ok(Self {
            columns,
            should_quit: false,
        })
    }
    fn on_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if key.kind == KeyEventKind::Press {
            match key.code {
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Up => self.active_column_mut().select_previous(),
                KeyCode::Down => self.active_column_mut().select_next(),
                KeyCode::Right => self.enter_dir()?,
                KeyCode::Left => self.leave_dir()?,
                _ => {}
            }
        }
        Ok(())
    }

    fn active_column(&self) -> &DirColumn {
        self.columns.last().unwrap()
    }

    fn active_column_mut(&mut self) -> &mut DirColumn {
        self.columns.last_mut().unwrap()
    }

    fn enter_dir(&mut self) -> io::Result<()> {
        if let Some(selected_entry) = self.active_column().selected_entry() {
            if selected_entry.path().is_dir() {
                self.columns.push(DirColumn::new(selected_entry.path())?);
            }
        }
        Ok(())
    }

    fn leave_dir(&mut self) -> io::Result<()> {
        if self.columns.len() > 1 {
            self.columns.pop();
        }
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
    let constraints = app.columns.iter().map(|_| Constraint::Ratio(1, app.columns.len() as u32)).collect::<Vec<_>>();
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
} 